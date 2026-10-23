//! Durable storage for the SQL engine: a committed transaction is one atomic, synced write batch.

use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;

/// One change made by a transaction. A transaction's `Op`s are the unit that is written atomically
/// (and, later, the unit that is replicated).
#[derive(Debug, Clone)]
pub enum Op {
    /// A DDL statement, stored as text and replayed in order at start-up.
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
    SeqHighWater {
        name: String,
        value: i64,
    },
}

/// Everything read back from storage at start-up.
#[derive(Debug, Default)]
pub struct Loaded {
    pub ddl: Vec<String>,
    pub rows: Vec<(u32, u64, Vec<SqlValue>)>,
    pub seqs: HashMap<String, i64>,
}

pub trait Persist: Send + Sync {
    /// Write `ops` atomically and durably. Returns only when the data would survive a crash.
    fn commit(&self, ops: &[Op]) -> Result<(), String>;
    fn load(&self) -> Result<Loaded, String>;
}

/// Keeps nothing; used for in-memory engines and tests.
#[derive(Default)]
pub struct NoPersist;

impl Persist for NoPersist {
    fn commit(&self, _ops: &[Op]) -> Result<(), String> {
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
    use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch, WriteOptions, DB};
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    const CF_DDL: &str = "sql_ddl";
    const CF_ROWS: &str = "sql_rows";
    const CF_SEQ: &str = "sql_seq";

    /// RocksDB-backed storage. DDL is an append-only log; rows are keyed by table id and row id.
    pub struct RocksPersist {
        db: DB,
        next_ddl: AtomicU64,
    }

    impl RocksPersist {
        pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);
            let cfs = [CF_DDL, CF_ROWS, CF_SEQ]
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
    }

    impl Persist for RocksPersist {
        fn commit(&self, ops: &[Op]) -> Result<(), String> {
            if ops.is_empty() {
                return Ok(());
            }
            let ddl = self
                .db
                .cf_handle(CF_DDL)
                .ok_or("missing ddl column family")?;
            let rows = self
                .db
                .cf_handle(CF_ROWS)
                .ok_or("missing rows column family")?;
            let seq = self
                .db
                .cf_handle(CF_SEQ)
                .ok_or("missing sequence column family")?;
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
                        batch.put_cf(seq, name.as_bytes(), value.to_be_bytes())
                    }
                }
            }
            let mut wo = WriteOptions::default();
            wo.set_sync(true);
            self.db
                .write_opt(batch, &wo)
                .map_err(|e| format!("commit failed: {e}"))?;
            self.next_ddl.store(base + ddl_used, Ordering::SeqCst);
            Ok(())
        }

        fn load(&self) -> Result<Loaded, String> {
            let mut out = Loaded::default();
            let ddl = self
                .db
                .cf_handle(CF_DDL)
                .ok_or("missing ddl column family")?;
            for item in self.db.iterator_cf(ddl, IteratorMode::Start) {
                let (_, v) = item.map_err(|e| e.to_string())?;
                out.ddl
                    .push(String::from_utf8(v.to_vec()).map_err(|e| e.to_string())?);
            }
            let rows = self
                .db
                .cf_handle(CF_ROWS)
                .ok_or("missing rows column family")?;
            for item in self.db.iterator_cf(rows, IteratorMode::Start) {
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
            let seq = self
                .db
                .cf_handle(CF_SEQ)
                .ok_or("missing sequence column family")?;
            for item in self.db.iterator_cf(seq, IteratorMode::Start) {
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
            Ok(out)
        }
    }
}
