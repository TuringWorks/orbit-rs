//! A durable [`UnifiedStorageBackend`] on RocksDB.
//!
//! Until this existed, `UnifiedStorageIntegration` built a [`MemoryBackend`]
//! whichever way its `use_memory_backend` flag was set — both arms of the
//! branch constructed the same thing, and the flag documented an intention
//! rather than selecting anything. Every table and row served over the SQL
//! protocols lived only in that process: a restart came back empty while the
//! logs said "persistent backend".
//!
//! [`MemoryBackend`]: super::storage::MemoryBackend

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::{IteratorMode, Options, DB};
use tokio::sync::RwLock;

use super::storage::{
    UnifiedStorageBackend, UnifiedStorageError, UnifiedStorageMetrics, UnifiedStorageResult,
};

/// Key-value storage backed by a RocksDB database on disk.
pub struct RocksDbBackend {
    db: Arc<DB>,
    metrics: Arc<RwLock<UnifiedStorageMetrics>>,
}

impl RocksDbBackend {
    /// Open (or create) a RocksDB database under `path`.
    ///
    /// # Errors
    /// Returns an error when the database cannot be opened — most often
    /// because another process already holds its lock, or the directory is not
    /// writable.
    pub fn open(path: impl AsRef<Path>) -> UnifiedStorageResult<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let db = DB::open(&options, path.as_ref()).map_err(|e| {
            UnifiedStorageError::Backend(format!(
                "could not open the unified store at {}: {e}",
                path.as_ref().display()
            ))
        })?;

        Ok(Self {
            db: Arc::new(db),
            metrics: Arc::new(RwLock::new(UnifiedStorageMetrics::default())),
        })
    }

    /// Count an operation, so the metrics report what actually happened rather
    /// than staying at zero.
    async fn record(&self, operation: Operation, failed: bool) {
        let mut metrics = self.metrics.write().await;
        match operation {
            Operation::Read => metrics.read_operations += 1,
            Operation::Write => metrics.write_operations += 1,
            Operation::Delete => metrics.delete_operations += 1,
        }
        if failed {
            metrics.error_count += 1;
        }
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Read,
    Write,
    Delete,
}

#[async_trait]
impl UnifiedStorageBackend for RocksDbBackend {
    async fn initialize(&self) -> UnifiedStorageResult<()> {
        tracing::info!("RocksDB backend initialized");
        Ok(())
    }

    async fn shutdown(&self) -> UnifiedStorageResult<()> {
        // Flushing on the way out makes a clean stop durable without waiting
        // for the write-ahead log to be replayed on the next start.
        self.db
            .flush()
            .map_err(|e| UnifiedStorageError::Backend(format!("flush failed: {e}")))?;
        tracing::info!("RocksDB backend shut down");
        Ok(())
    }

    async fn get(&self, key: &str) -> UnifiedStorageResult<Option<Vec<u8>>> {
        let result = self.db.get(key.as_bytes());
        self.record(Operation::Read, result.is_err()).await;
        result.map_err(|e| UnifiedStorageError::Backend(format!("get '{key}' failed: {e}")))
    }

    async fn put(&self, key: &str, value: &[u8]) -> UnifiedStorageResult<()> {
        let result = self.db.put(key.as_bytes(), value);
        self.record(Operation::Write, result.is_err()).await;
        result.map_err(|e| UnifiedStorageError::Backend(format!("put '{key}' failed: {e}")))
    }

    async fn delete(&self, key: &str) -> UnifiedStorageResult<bool> {
        // RocksDB's delete succeeds whether or not the key was there, so
        // existence is checked first to answer honestly.
        let existed = self
            .db
            .get(key.as_bytes())
            .map_err(|e| UnifiedStorageError::Backend(format!("delete '{key}' failed: {e}")))?
            .is_some();

        let result = self.db.delete(key.as_bytes());
        self.record(Operation::Delete, result.is_err()).await;
        result.map_err(|e| UnifiedStorageError::Backend(format!("delete '{key}' failed: {e}")))?;
        Ok(existed)
    }

    async fn exists(&self, key: &str) -> UnifiedStorageResult<bool> {
        Ok(self.get(key).await?.is_some())
    }

    async fn scan_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> UnifiedStorageResult<Vec<(String, Vec<u8>)>> {
        // Seeking to the prefix and stopping at the first key that no longer
        // matches keeps the scan proportional to what it returns rather than
        // to the size of the database.
        let iterator = self.db.iterator(IteratorMode::From(
            prefix.as_bytes(),
            rocksdb::Direction::Forward,
        ));

        let mut entries = Vec::new();
        for item in iterator {
            let (key, value) = item.map_err(|e| {
                UnifiedStorageError::Backend(format!("scan of '{prefix}' failed: {e}"))
            })?;

            let Ok(key) = std::str::from_utf8(&key) else {
                continue;
            };
            if !key.starts_with(prefix) {
                break;
            }
            entries.push((key.to_string(), value.to_vec()));
            if limit.is_some_and(|limit| entries.len() >= limit) {
                break;
            }
        }

        self.record(Operation::Read, false).await;
        Ok(entries)
    }

    async fn put_batch(&self, entries: Vec<(String, Vec<u8>)>) -> UnifiedStorageResult<()> {
        let mut batch = rocksdb::WriteBatch::default();
        let count = entries.len();
        for (key, value) in entries {
            batch.put(key.as_bytes(), value);
        }

        let result = self.db.write(batch);
        {
            let mut metrics = self.metrics.write().await;
            metrics.write_operations += count as u64;
            if result.is_err() {
                metrics.error_count += 1;
            }
        }
        result.map_err(|e| UnifiedStorageError::Backend(format!("batch write failed: {e}")))
    }

    async fn delete_batch(&self, keys: Vec<String>) -> UnifiedStorageResult<u64> {
        let mut batch = rocksdb::WriteBatch::default();
        let count = keys.len() as u64;
        for key in keys {
            batch.delete(key.as_bytes());
        }

        let result = self.db.write(batch);
        {
            let mut metrics = self.metrics.write().await;
            metrics.delete_operations += count;
            if result.is_err() {
                metrics.error_count += 1;
            }
        }
        result.map_err(|e| UnifiedStorageError::Backend(format!("batch delete failed: {e}")))?;
        Ok(count)
    }

    async fn metrics(&self) -> UnifiedStorageMetrics {
        self.metrics.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(name: &str) -> (RocksDbBackend, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("orbit-rocksdb-backend-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        (RocksDbBackend::open(&path).expect("opens"), path)
    }

    #[tokio::test]
    async fn a_value_survives_reopening_the_database() {
        let (store, path) = backend("reopen");
        store.put("k", b"v").await.expect("put");
        store.shutdown().await.expect("shutdown");
        drop(store);

        // The point of the backend: the value is still there in a new process.
        let reopened = RocksDbBackend::open(&path).expect("reopens");
        assert_eq!(
            reopened.get("k").await.expect("get").as_deref(),
            Some(&b"v"[..])
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn delete_reports_whether_the_key_was_there() {
        let (store, path) = backend("delete");
        store.put("k", b"v").await.expect("put");

        assert!(store.delete("k").await.expect("delete"));
        assert!(!store.delete("k").await.expect("delete"));
        let _ = std::fs::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn a_prefix_scan_stops_at_the_prefix() {
        let (store, path) = backend("scan");
        for key in ["a:1", "a:2", "b:1"] {
            store.put(key, b"v").await.expect("put");
        }

        let found = store.scan_prefix("a:", None).await.expect("scan");
        let keys: Vec<&str> = found.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["a:1", "a:2"]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn a_prefix_scan_honours_its_limit() {
        let (store, path) = backend("limit");
        for key in ["a:1", "a:2", "a:3"] {
            store.put(key, b"v").await.expect("put");
        }

        let found = store.scan_prefix("a:", Some(2)).await.expect("scan");
        assert_eq!(found.len(), 2);
        let _ = std::fs::remove_dir_all(&path);
    }
}
