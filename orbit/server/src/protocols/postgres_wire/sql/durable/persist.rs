//! Durable storage for the SQL engine: a committed transaction is one atomic, synced write batch.

use crate::protocols::postgres_wire::sql::types::SqlValue;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One change made by a transaction. A transaction's `Op`s are the unit that is written atomically
/// (and, later, the unit that is replicated).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Op {
    /// A DDL statement, stored as text and replayed in order at start-up and on replicas. It comes
    /// before the row changes it causes, so a replica applying the ops in order ends in the same state.
    Ddl(String),
    PutRow {
        table: u32,
        rowid: u64,
        row: Vec<SqlValue>,
    },
    DelRow {
        table: u32,
        rowid: u64,
    },
    /// Remove every row of a table (`DROP TABLE`, `TRUNCATE`).
    DropRows {
        table: u32,
    },
    /// Raise a sequence's persisted high-water mark; values below it may have been handed out.
    /// A value of 0 forgets the sequence (its table was dropped).
    SeqHighWater {
        name: String,
        value: i64,
    },
    /// Where a cluster node listens, replicated so that any node that becomes leader can reach it.
    /// Not part of the database; the replicated state machine hands it to the network layer.
    NodeAddr {
        id: u64,
        addr: String,
    },
}

/// Everything read back from storage at start-up.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Loaded {
    pub ddl: Vec<String>,
    pub rows: Vec<(u32, u64, Vec<SqlValue>)>,
    pub seqs: HashMap<String, i64>,
    /// Cluster node addresses.
    #[serde(default)]
    pub peers: HashMap<u64, String>,
}

#[async_trait]
pub trait Persist: Send + Sync {
    /// Write `ops` atomically and durably. Returns only when the data would survive a crash (and,
    /// for a replicated store, when a majority of nodes has it).
    async fn commit(&self, ops: &[Op]) -> Result<(), String>;
    fn load(&self) -> Result<Loaded, String>;
    /// `true` when committed ops reach the engine through [`DurableEngine::apply_replicated`]
    /// (on every node, including the one that proposed them) instead of being applied by the
    /// engine that ran the transaction.
    fn replicated(&self) -> bool {
        false
    }
    /// Whether this node may take writes now. The error is `"[SQLSTATE] message"`.
    fn check_writable(&self) -> Result<(), String> {
        Ok(())
    }
    /// Whether this node may answer reads now. The error is `"[SQLSTATE] message"`.
    fn check_readable(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Keeps nothing; used for in-memory engines and tests.
#[derive(Default)]
pub struct NoPersist;

#[async_trait]
impl Persist for NoPersist {
    async fn commit(&self, _ops: &[Op]) -> Result<(), String> {
        Ok(())
    }
    fn load(&self) -> Result<Loaded, String> {
        Ok(Loaded::default())
    }
}

#[cfg(feature = "storage-rocksdb")]
pub use rocks::RocksPersist;

#[cfg(feature = "storage-rocksdb")]
mod rocks {
    use super::{Loaded, Op, Persist};
    use async_trait::async_trait;
    use rocksdb::{
        ColumnFamilyDescriptor, IteratorMode, Options, ReadOptions, WriteBatch, WriteOptions, DB,
    };
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    const CF_DDL: &str = "sql_ddl";
    const CF_ROWS: &str = "sql_rows";
    const CF_SEQ: &str = "sql_seq";
    const CF_META: &str = "sql_meta";
    const KEY_APPLIED: &[u8] = b"applied";

    /// RocksDB-backed storage. DDL is an append-only log; rows are keyed by table id and row id.
    /// A replicated node also records, in the same atomic write as the ops, the index of the
    /// replicated log entry they came from.
    pub struct RocksPersist {
        db: DB,
        next_ddl: AtomicU64,
    }

    impl RocksPersist {
        pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);
            let cfs = [CF_DDL, CF_ROWS, CF_SEQ, CF_META]
                .map(|n| ColumnFamilyDescriptor::new(n, Options::default()));
            let db = DB::open_cf_descriptors(&opts, path, cfs)
                .map_err(|e| format!("open sql store: {e}"))?;
            let next = {
                let cf = db.cf_handle(CF_DDL).ok_or("missing ddl column family")?;
                match db.iterator_cf(cf, IteratorMode::End).next() {
                    Some(Ok((k, _))) => {
                        u64::from_be_bytes(
                            k.as_ref()
                                .try_into()
                                .map_err(|_| "bad ddl key".to_string())?,
                        ) + 1
                    }
                    _ => 0,
                }
            };
            Ok(Self {
                db,
                next_ddl: AtomicU64::new(next),
            })
        }

        fn row_key(table: u32, rowid: u64) -> [u8; 12] {
            let mut k = [0u8; 12];
            k[..4].copy_from_slice(&table.to_be_bytes());
            k[4..].copy_from_slice(&rowid.to_be_bytes());
            k
        }

        fn cf(&self, name: &str) -> Result<&rocksdb::ColumnFamily, String> {
            self.db
                .cf_handle(name)
                .ok_or_else(|| format!("missing column family {name}"))
        }

        /// Write `ops` atomically and synced, with the replicated log index they belong to, if any.
        fn write(&self, ops: &[Op], applied: Option<u64>) -> Result<(), String> {
            if ops.is_empty() && applied.is_none() {
                return Ok(());
            }
            let (ddl, rows, seq, meta) = (
                self.cf(CF_DDL)?,
                self.cf(CF_ROWS)?,
                self.cf(CF_SEQ)?,
                self.cf(CF_META)?,
            );
            let mut batch = WriteBatch::default();
            let mut ddl_used = 0u64;
            let base = self.next_ddl.load(Ordering::SeqCst);
            for op in ops {
                match op {
                    Op::Ddl(text) => {
                        batch.put_cf(ddl, (base + ddl_used).to_be_bytes(), text.as_bytes());
                        ddl_used += 1;
                    }
                    Op::PutRow { table, rowid, row } => {
                        let bytes =
                            serde_json::to_vec(row).map_err(|e| format!("encode row: {e}"))?;
                        batch.put_cf(rows, Self::row_key(*table, *rowid), bytes);
                    }
                    Op::DelRow { table, rowid } => {
                        batch.delete_cf(rows, Self::row_key(*table, *rowid))
                    }
                    Op::DropRows { table } => {
                        batch.delete_range_cf(
                            rows,
                            Self::row_key(*table, 0),
                            Self::row_key(*table, u64::MAX),
                        );
                        batch.delete_cf(rows, Self::row_key(*table, u64::MAX));
                    }
                    Op::SeqHighWater { name, value } => {
                        if *value == 0 {
                            batch.delete_cf(seq, name.as_bytes());
                        } else {
                            batch.put_cf(seq, name.as_bytes(), value.to_be_bytes());
                        }
                    }
                    Op::NodeAddr { id, addr } => {
                        batch.put_cf(meta, format!("peer:{id}").as_bytes(), addr.as_bytes())
                    }
                }
            }
            if let Some(i) = applied {
                batch.put_cf(meta, KEY_APPLIED, i.to_be_bytes());
            }
            let mut wo = WriteOptions::default();
            wo.set_sync(true);
            self.db
                .write_opt(batch, &wo)
                .map_err(|e| format!("commit failed: {e}"))?;
            self.next_ddl.store(base + ddl_used, Ordering::SeqCst);
            Ok(())
        }

        /// Apply the ops of replicated log entry `index`, recording the index in the same write.
        pub fn apply(&self, ops: &[Op], index: u64) -> Result<(), String> {
            self.write(ops, Some(index))
        }

        /// The last replicated log index applied, or 0.
        pub fn applied_index(&self) -> u64 {
            let read = || -> Result<u64, String> {
                let meta = self.cf(CF_META)?;
                match self
                    .db
                    .get_cf(meta, KEY_APPLIED)
                    .map_err(|e| e.to_string())?
                {
                    Some(b) => Ok(u64::from_be_bytes(
                        b.as_slice()
                            .try_into()
                            .map_err(|_| "bad applied index".to_string())?,
                    )),
                    None => Ok(0),
                }
            };
            read().unwrap_or(0)
        }

        /// Everything in the store as of one instant, with the log index it corresponds to.
        pub fn snapshot(&self) -> Result<(u64, Loaded), String> {
            let snap = self.db.snapshot();
            let ro = || {
                let mut r = ReadOptions::default();
                r.set_snapshot(&snap);
                r
            };
            let meta = self.cf(CF_META)?;
            let applied = match self
                .db
                .get_cf_opt(meta, KEY_APPLIED, &ro())
                .map_err(|e| e.to_string())?
            {
                Some(b) => u64::from_be_bytes(
                    b.as_slice()
                        .try_into()
                        .map_err(|_| "bad applied index".to_string())?,
                ),
                None => 0,
            };
            Ok((applied, self.read_all(&ro)?))
        }

        fn read_all(&self, ro: &dyn Fn() -> ReadOptions) -> Result<Loaded, String> {
            let mut out = Loaded::default();
            let ddl = self.cf(CF_DDL)?;
            for item in self.db.iterator_cf_opt(ddl, ro(), IteratorMode::Start) {
                let (_, v) = item.map_err(|e| e.to_string())?;
                out.ddl
                    .push(String::from_utf8(v.to_vec()).map_err(|e| e.to_string())?);
            }
            let rows = self.cf(CF_ROWS)?;
            for item in self.db.iterator_cf_opt(rows, ro(), IteratorMode::Start) {
                let (k, v) = item.map_err(|e| e.to_string())?;
                let k: [u8; 12] = k
                    .as_ref()
                    .try_into()
                    .map_err(|_| "bad row key".to_string())?;
                let table = u32::from_be_bytes(k[..4].try_into().unwrap());
                let rowid = u64::from_be_bytes(k[4..].try_into().unwrap());
                out.rows.push((
                    table,
                    rowid,
                    serde_json::from_slice(&v).map_err(|e| format!("decode row: {e}"))?,
                ));
            }
            let seq = self.cf(CF_SEQ)?;
            for item in self.db.iterator_cf_opt(seq, ro(), IteratorMode::Start) {
                let (k, v) = item.map_err(|e| e.to_string())?;
                let v: [u8; 8] = v
                    .as_ref()
                    .try_into()
                    .map_err(|_| "bad sequence value".to_string())?;
                out.seqs.insert(
                    String::from_utf8(k.to_vec()).map_err(|e| e.to_string())?,
                    i64::from_be_bytes(v),
                );
            }
            let meta = self.cf(CF_META)?;
            for item in self.db.iterator_cf_opt(meta, ro(), IteratorMode::Start) {
                let (k, v) = item.map_err(|e| e.to_string())?;
                if let Some(id) = k.strip_prefix(b"peer:") {
                    let id: u64 = String::from_utf8_lossy(id)
                        .parse()
                        .map_err(|_| "bad peer key".to_string())?;
                    out.peers.insert(
                        id,
                        String::from_utf8(v.to_vec()).map_err(|e| e.to_string())?,
                    );
                }
            }
            Ok(out)
        }

        /// Replace everything with a snapshot taken at replicated log index `index`.
        pub fn restore(&self, index: u64, loaded: &Loaded) -> Result<(), String> {
            let (ddl, rows, seq, meta) = (
                self.cf(CF_DDL)?,
                self.cf(CF_ROWS)?,
                self.cf(CF_SEQ)?,
                self.cf(CF_META)?,
            );
            let mut batch = WriteBatch::default();
            batch.delete_range_cf(ddl, 0u64.to_be_bytes(), u64::MAX.to_be_bytes());
            batch.delete_cf(ddl, u64::MAX.to_be_bytes());
            batch.delete_range_cf(rows, [0u8; 12], [0xFFu8; 12]);
            batch.delete_cf(rows, [0xFFu8; 12]);
            for item in self.db.iterator_cf(seq, IteratorMode::Start) {
                let (k, _) = item.map_err(|e| e.to_string())?;
                batch.delete_cf(seq, k);
            }
            for (i, text) in loaded.ddl.iter().enumerate() {
                batch.put_cf(ddl, (i as u64).to_be_bytes(), text.as_bytes());
            }
            for (table, rowid, row) in &loaded.rows {
                batch.put_cf(
                    rows,
                    Self::row_key(*table, *rowid),
                    serde_json::to_vec(row).map_err(|e| format!("encode row: {e}"))?,
                );
            }
            for (name, v) in &loaded.seqs {
                batch.put_cf(seq, name.as_bytes(), v.to_be_bytes());
            }
            for item in self.db.iterator_cf(meta, IteratorMode::Start) {
                let (k, _) = item.map_err(|e| e.to_string())?;
                if k.starts_with(b"peer:") {
                    batch.delete_cf(meta, k);
                }
            }
            for (id, addr) in &loaded.peers {
                batch.put_cf(meta, format!("peer:{id}").as_bytes(), addr.as_bytes());
            }
            batch.put_cf(meta, KEY_APPLIED, index.to_be_bytes());
            let mut wo = WriteOptions::default();
            wo.set_sync(true);
            self.db
                .write_opt(batch, &wo)
                .map_err(|e| format!("restore failed: {e}"))?;
            self.next_ddl
                .store(loaded.ddl.len() as u64, Ordering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl Persist for RocksPersist {
        async fn commit(&self, ops: &[Op]) -> Result<(), String> {
            self.write(ops, None)
        }

        fn load(&self) -> Result<Loaded, String> {
            self.read_all(&ReadOptions::default)
        }
    }
}
