//! Durable storage for a Raft node: the hard state, the log, and where the log starts.
//!
//! `save` is atomic and synced: when it returns, everything in it survives a crash. The node calls
//! it before sending any message that depends on what it saves.

use super::core::{Entry, HardState, Index, SnapshotMeta};

/// What a node reads back when it starts.
#[derive(Default, Debug)]
pub struct Loaded {
    pub hard: HardState,
    /// Where the log starts: entries up to `index` are in the state machine, not the log.
    pub base: Option<SnapshotMeta>,
    pub entries: Vec<Entry>,
}

/// One atomic update.
#[derive(Default, Debug)]
pub struct Save<'a> {
    pub hard: Option<&'a HardState>,
    /// Delete log entries with index at or above this before appending.
    pub truncate_from: Option<Index>,
    pub append: &'a [Entry],
    /// Move the start of the log: drop entries up to `base.index` and remember the base.
    pub base: Option<&'a SnapshotMeta>,
}

pub trait RaftStorage: Send {
    fn load(&self) -> Result<Loaded, String>;
    fn save(&mut self, save: Save<'_>) -> Result<(), String>;
}

/// Keeps everything in memory; for tests.
#[derive(Default)]
pub struct MemStorage {
    pub hard: HardState,
    pub base: Option<SnapshotMeta>,
    pub log: Vec<Entry>,
}

impl RaftStorage for MemStorage {
    fn load(&self) -> Result<Loaded, String> {
        Ok(Loaded {
            hard: self.hard.clone(),
            base: self.base.clone(),
            entries: self.log.clone(),
        })
    }

    fn save(&mut self, s: Save<'_>) -> Result<(), String> {
        if let Some(h) = s.hard {
            self.hard = h.clone();
        }
        if let Some(t) = s.truncate_from {
            self.log.retain(|e| e.index < t);
        }
        self.log.extend(s.append.iter().cloned());
        if let Some(b) = s.base {
            self.log.retain(|e| e.index > b.index);
            self.base = Some(b.clone());
        }
        Ok(())
    }
}

#[cfg(feature = "storage-rocksdb")]
pub use rocks::RocksRaftStorage;

#[cfg(feature = "storage-rocksdb")]
mod rocks {
    use super::{Loaded, RaftStorage, Save};
    use crate::raft::core::{Entry, HardState, SnapshotMeta};
    use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch, WriteOptions, DB};
    use std::path::Path;

    const CF_META: &str = "raft_meta";
    const CF_LOG: &str = "raft_log";
    const KEY_HARD: &[u8] = b"hard";
    const KEY_BASE: &[u8] = b"base";

    pub struct RocksRaftStorage {
        db: DB,
    }

    impl RocksRaftStorage {
        pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);
            let cfs = [CF_META, CF_LOG].map(|n| ColumnFamilyDescriptor::new(n, Options::default()));
            let db = DB::open_cf_descriptors(&opts, path, cfs)
                .map_err(|e| format!("open raft store: {e}"))?;
            Ok(Self { db })
        }
    }

    fn key(index: u64) -> [u8; 8] {
        index.to_be_bytes()
    }

    impl RaftStorage for RocksRaftStorage {
        fn load(&self) -> Result<Loaded, String> {
            let meta = self.db.cf_handle(CF_META).ok_or("missing raft meta")?;
            let log = self.db.cf_handle(CF_LOG).ok_or("missing raft log")?;
            let hard: HardState = match self.db.get_cf(meta, KEY_HARD).map_err(|e| e.to_string())? {
                Some(b) => serde_json::from_slice(&b).map_err(|e| format!("hard state: {e}"))?,
                None => HardState::default(),
            };
            let base: Option<SnapshotMeta> =
                match self.db.get_cf(meta, KEY_BASE).map_err(|e| e.to_string())? {
                    Some(b) => {
                        Some(serde_json::from_slice(&b).map_err(|e| format!("log base: {e}"))?)
                    }
                    None => None,
                };
            let mut entries = Vec::new();
            for item in self.db.iterator_cf(log, IteratorMode::Start) {
                let (_, v) = item.map_err(|e| e.to_string())?;
                let e: Entry = bincode::deserialize(&v).map_err(|e| format!("log entry: {e}"))?;
                entries.push(e);
            }
            // the log must be contiguous and start right after the base
            let mut expect = base.as_ref().map_or(1, |b| b.index + 1);
            for e in &entries {
                if e.index != expect {
                    return Err(format!(
                        "raft log is not contiguous: expected index {expect}, found {}",
                        e.index
                    ));
                }
                expect += 1;
            }
            Ok(Loaded {
                hard,
                base,
                entries,
            })
        }

        fn save(&mut self, s: Save<'_>) -> Result<(), String> {
            let meta = self.db.cf_handle(CF_META).ok_or("missing raft meta")?;
            let log = self.db.cf_handle(CF_LOG).ok_or("missing raft log")?;
            let mut batch = WriteBatch::default();
            if let Some(h) = s.hard {
                batch.put_cf(
                    meta,
                    KEY_HARD,
                    serde_json::to_vec(h).map_err(|e| e.to_string())?,
                );
            }
            if let Some(t) = s.truncate_from {
                batch.delete_range_cf(log, key(t), key(u64::MAX));
                batch.delete_cf(log, key(u64::MAX));
            }
            for e in s.append {
                batch.put_cf(
                    log,
                    key(e.index),
                    bincode::serialize(e).map_err(|e| e.to_string())?,
                );
            }
            if let Some(b) = s.base {
                batch.delete_range_cf(log, key(0), key(b.index + 1));
                batch.put_cf(
                    meta,
                    KEY_BASE,
                    serde_json::to_vec(b).map_err(|e| e.to_string())?,
                );
            }
            let mut wo = WriteOptions::default();
            wo.set_sync(true);
            self.db
                .write_opt(batch, &wo)
                .map_err(|e| format!("raft save failed: {e}"))
        }
    }
}
