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
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::{
    BlockBasedOptions, Cache, DBCompressionType, DBRecoveryMode, IteratorMode, Options,
    WriteOptions, DB,
};
use tokio::sync::RwLock;

use super::storage::{
    UnifiedStorageBackend, UnifiedStorageError, UnifiedStorageMetrics, UnifiedStorageResult,
};

const BYTES_PER_MB: usize = 1024 * 1024;

/// Which codec compresses the on-disk blocks.
///
/// Only the codecs actually linked into the binary appear here; asking for one
/// that is not compiled in makes RocksDB refuse to open the database, so the
/// set is kept to what the build guarantees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Compression {
    /// Store blocks uncompressed.
    None,
    /// LZ4 — fast, modest ratio. The default.
    #[default]
    Lz4,
    /// Zstandard — slower, better ratio.
    Zstd,
}

impl FromStr for Compression {
    type Err = UnifiedStorageError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name.to_lowercase().as_str() {
            "none" | "off" => Ok(Self::None),
            "lz4" => Ok(Self::Lz4),
            "zstd" => Ok(Self::Zstd),
            other => Err(UnifiedStorageError::InvalidOperation(format!(
                "unknown compression algorithm '{other}'; expected one of none, lz4, zstd"
            ))),
        }
    }
}

impl From<Compression> for DBCompressionType {
    fn from(compression: Compression) -> Self {
        match compression {
            Compression::None => Self::None,
            Compression::Lz4 => Self::Lz4,
            Compression::Zstd => Self::Zstd,
        }
    }
}

/// How the backend trades durability, space, and speed.
///
/// The defaults are the durable ones: an acknowledged write is on disk before
/// it is acknowledged. That costs an fsync per write, which is the price of
/// not losing data when the machine loses power.
#[derive(Debug, Clone)]
pub struct RocksDbBackendConfig {
    /// Flush the write-ahead log to the physical disk before a write returns.
    ///
    /// With this off, RocksDB hands the record to the operating system and
    /// returns. The data survives a process crash, because the kernel still
    /// holds the buffer — but a power cut or kernel panic loses every write
    /// made since the last flush, *after* the client was told it was durable.
    /// Turn it off only where losing recent writes is acceptable.
    pub sync_writes: bool,

    /// Write to the write-ahead log at all.
    ///
    /// With this off there is nothing to replay: a crash loses everything back
    /// to the last memtable flush, whether or not `sync_writes` is set.
    pub enable_wal: bool,

    /// Compress on-disk blocks with this codec.
    pub compression: Compression,

    /// Size of the block cache, in megabytes.
    pub block_cache_mb: usize,

    /// Size of each in-memory write buffer, in megabytes.
    pub write_buffer_mb: usize,

    /// How many write buffers may exist before writes stall.
    pub max_write_buffers: u32,

    /// Bits per key for the bloom filter, or `None` for no bloom filter.
    ///
    /// Modelled as an option rather than a flag beside a number, so "filters
    /// off" and "ten bits per key" cannot be stated at the same time.
    pub bloom_bits_per_key: Option<u32>,
}

impl Default for RocksDbBackendConfig {
    fn default() -> Self {
        Self {
            sync_writes: true,
            enable_wal: true,
            compression: Compression::default(),
            block_cache_mb: 256,
            write_buffer_mb: 64,
            max_write_buffers: 3,
            bloom_bits_per_key: Some(10),
        }
    }
}

impl RocksDbBackendConfig {
    /// The settings that trade durability for speed: no fsync, no log.
    ///
    /// Intended for tests and for throwaway data. A crash loses writes that
    /// were reported as successful.
    #[must_use]
    pub fn unsafe_fast() -> Self {
        Self {
            sync_writes: false,
            enable_wal: false,
            ..Self::default()
        }
    }
}

/// Key-value storage backed by a RocksDB database on disk.
pub struct RocksDbBackend {
    db: Arc<DB>,
    /// Built once at open, because the durability of a write must not depend
    /// on which call site made it.
    write_options: WriteOptions,
    metrics: Arc<RwLock<UnifiedStorageMetrics>>,
}

impl RocksDbBackend {
    /// Open (or create) a RocksDB database under `path` with durable defaults.
    ///
    /// # Errors
    /// Returns an error when the database cannot be opened — most often
    /// because another process already holds its lock, or the directory is not
    /// writable.
    pub fn open(path: impl AsRef<Path>) -> UnifiedStorageResult<Self> {
        Self::open_with(path, &RocksDbBackendConfig::default())
    }

    /// Open (or create) a RocksDB database under `path` with explicit settings.
    ///
    /// # Errors
    /// Returns an error when the database cannot be opened, which includes the
    /// case where its files are corrupt beyond what point-in-time recovery can
    /// repair.
    pub fn open_with(
        path: impl AsRef<Path>,
        config: &RocksDbBackendConfig,
    ) -> UnifiedStorageResult<Self> {
        // RocksDB rejects a synchronous write when there is no log to
        // synchronise, one write at a time. Caught here, the contradiction is
        // one startup error naming both settings; left alone, it is every
        // write failing at run time on a server that started cleanly.
        if config.sync_writes && !config.enable_wal {
            return Err(UnifiedStorageError::InvalidOperation(
                "sync_wal is set but enable_wal is not: there is no write-ahead \
                 log to flush. Turn on enable_wal to get durable writes, or turn \
                 off sync_wal to accept losing recent writes on a crash."
                    .to_string(),
            ));
        }

        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        // Corruption handling, stated rather than inherited.
        //
        // `paranoid_checks` makes RocksDB validate as it reads instead of
        // trusting its own files. Point-in-time recovery stops replaying the
        // log at the first damaged record, which keeps every write that was
        // completed and discards only a torn tail — the record that was being
        // written when the power went out, which no client was ever told had
        // succeeded. The stricter mode refuses to open at all after a torn
        // tail, which turns an ordinary power cut into an outage without
        // saving any data that was actually acknowledged.
        options.set_paranoid_checks(true);
        options.set_wal_recovery_mode(DBRecoveryMode::PointInTime);

        options.set_compression_type(config.compression.into());
        options.set_write_buffer_size(config.write_buffer_mb * BYTES_PER_MB);
        options.set_max_write_buffer_number(config.max_write_buffers as i32);

        let mut block_options = BlockBasedOptions::default();
        let cache = Cache::new_lru_cache(config.block_cache_mb * BYTES_PER_MB);
        block_options.set_block_cache(&cache);
        if let Some(bits) = config.bloom_bits_per_key {
            block_options.set_bloom_filter(f64::from(bits), false);
        }
        options.set_block_based_table_factory(&block_options);

        let db = DB::open(&options, path.as_ref()).map_err(|e| {
            UnifiedStorageError::Backend(format!(
                "could not open the unified store at {}: {e}",
                path.as_ref().display()
            ))
        })?;

        let mut write_options = WriteOptions::new();
        write_options.set_sync(config.sync_writes);
        write_options.disable_wal(!config.enable_wal);

        if !config.enable_wal {
            tracing::warn!(
                path = %path.as_ref().display(),
                "unified store opened with the write-ahead log disabled: a crash \
                 will lose every write since the last flush"
            );
        } else if !config.sync_writes {
            tracing::warn!(
                path = %path.as_ref().display(),
                "unified store opened without sync-on-write: acknowledged writes \
                 survive a process crash but not a power loss"
            );
        }

        Ok(Self {
            db: Arc::new(db),
            write_options,
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
        // Push the log to disk first, so a stop that is interrupted between
        // these two calls still has every write recoverable, then flush the
        // memtable so the next start has nothing to replay.
        self.db
            .flush_wal(true)
            .map_err(|e| UnifiedStorageError::Backend(format!("flushing the log failed: {e}")))?;
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
        let result = self.db.put_opt(key.as_bytes(), value, &self.write_options);
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

        let result = self.db.delete_opt(key.as_bytes(), &self.write_options);
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

        let result = self.db.write_opt(batch, &self.write_options);
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

        let result = self.db.write_opt(batch, &self.write_options);
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
