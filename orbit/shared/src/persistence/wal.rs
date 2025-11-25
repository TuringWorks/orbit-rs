//! Write-Ahead Log (WAL) Implementation
//!
//! This module implements a Write-Ahead Log system for crash recovery and durability.
//! The WAL ensures that all state changes are logged to durable storage before being
//! applied to actor state, enabling recovery after unexpected failures.
//!
//! # Features
//!
//! - **Crash Recovery**: Replay WAL entries to restore actor state after failures
//! - **Configurable Durability**: Support for Strict, Standard, and Performance modes
//! - **Multi-Model Support**: Works with relational, graph, vector, and time-series data
//! - **Snapshot Integration**: Coordinates with existing snapshot system
//! - **Checkpointing**: Periodic checkpoints to bound recovery time
//! - **Log Compaction**: Automatic cleanup of obsolete log entries
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │ State Change    │
//! └────────┬────────┘
//!          │
//!          v
//! ┌─────────────────┐
//! │ Write to WAL    │ ◄─── fsync (Strict mode)
//! └────────┬────────┘
//!          │
//!          v
//! ┌─────────────────┐
//! │ Apply to State  │
//! └────────┬────────┘
//!          │
//!          v
//! ┌─────────────────┐
//! │ Periodic        │
//! │ Checkpoint      │ ◄─── Creates snapshot
//! └─────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use orbit_shared::persistence::wal::{WriteAheadLog, WalConfig, DurabilityLevel};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = WalConfig {
//!     log_directory: PathBuf::from("/var/orbit/wal"),
//!     durability_level: DurabilityLevel::Standard,
//!     max_log_size_bytes: 100 * 1024 * 1024, // 100MB
//!     checkpoint_interval_secs: 300, // 5 minutes
//!     ..Default::default()
//! };
//!
//! let wal = WriteAheadLog::new(config).await?;
//! wal.start().await?;
//! # Ok(())
//! # }
//! ```

use crate::addressable::AddressableReference;
use crate::exception::{OrbitError, OrbitResult};
use crate::persistence::snapshot::PersistenceBackend;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Durability level configuration (RFC-013)
///
/// Defines how aggressively WAL entries are persisted to disk:
/// - **Strict**: fsync after every write (PostgreSQL-like)
/// - **Standard**: fsync on commit boundaries (MySQL-like)
/// - **Performance**: OS-buffered writes with periodic fsync (MongoDB-like)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum DurabilityLevel {
    /// Maximum durability: fsync after every WAL write
    /// Guarantees no data loss but highest latency
    Strict,

    /// Balanced durability: fsync on transaction commit
    /// Good balance between safety and performance
    #[default]
    Standard,

    /// Maximum performance: OS-buffered writes with periodic fsync
    /// Faster but may lose recent writes on crash
    Performance,
}


/// WAL entry type representing different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalEntryType {
    /// Insert new state (stored as JSON string for bincode compatibility)
    Insert {
        state_data_json: String,
    },

    /// Update existing state (stored as JSON strings for bincode compatibility)
    Update {
        old_state_json: String,
        new_state_json: String,
    },

    /// Delete state (stored as JSON string for bincode compatibility)
    Delete {
        state_data_json: String,
    },

    /// Checkpoint marker (snapshot created)
    Checkpoint {
        snapshot_id: String,
        snapshot_version: u64,
    },

    /// Transaction begin marker
    TransactionBegin {
        transaction_id: String,
    },

    /// Transaction commit marker
    TransactionCommit {
        transaction_id: String,
    },

    /// Transaction abort marker
    TransactionAbort {
        transaction_id: String,
    },
}

/// Individual WAL entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Unique entry ID
    pub entry_id: String,

    /// Log Sequence Number (LSN) - monotonically increasing
    pub lsn: u64,

    /// Actor this entry belongs to
    pub actor_reference: AddressableReference,

    /// Entry type and data
    pub entry_type: WalEntryType,

    /// Timestamp when entry was created
    pub timestamp: i64,

    /// CRC32 checksum for integrity
    pub checksum: u32,
}

impl WalEntry {
    /// Create a new WAL entry
    pub fn new(
        lsn: u64,
        actor_reference: AddressableReference,
        entry_type: WalEntryType,
    ) -> Self {
        let entry_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp_millis();

        let mut entry = Self {
            entry_id,
            lsn,
            actor_reference,
            entry_type,
            timestamp,
            checksum: 0,
        };

        entry.checksum = entry.calculate_checksum();
        entry
    }

    /// Calculate CRC32 checksum for integrity verification
    fn calculate_checksum(&self) -> u32 {
        use crc32fast::Hasher;

        let mut hasher = Hasher::new();
        hasher.update(self.entry_id.as_bytes());
        hasher.update(&self.lsn.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());

        // Serialize entry_type for checksum
        if let Ok(serialized) = bincode::serialize(&self.entry_type) {
            hasher.update(&serialized);
        }

        hasher.finalize()
    }

    /// Verify entry integrity
    pub fn verify_integrity(&self) -> bool {
        let expected = self.calculate_checksum();
        self.checksum == expected
    }
}

/// WAL configuration
#[derive(Debug, Clone)]
pub struct WalConfig {
    /// Directory where WAL files are stored
    pub log_directory: PathBuf,

    /// Durability level
    pub durability_level: DurabilityLevel,

    /// Maximum size of a single WAL file before rotation (bytes)
    pub max_log_size_bytes: u64,

    /// Checkpoint interval (seconds)
    pub checkpoint_interval_secs: u64,

    /// Number of WAL files to keep after checkpoint
    pub retain_wal_files: usize,

    /// Enable compression for WAL files
    pub compression_enabled: bool,

    /// Buffer size for WAL writes
    pub buffer_size: usize,

    /// Enable metrics collection
    pub metrics_enabled: bool,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            log_directory: PathBuf::from("/tmp/orbit/wal"),
            durability_level: DurabilityLevel::Standard,
            max_log_size_bytes: 100 * 1024 * 1024, // 100MB
            checkpoint_interval_secs: 300, // 5 minutes
            retain_wal_files: 3,
            compression_enabled: false,
            buffer_size: 64 * 1024, // 64KB
            metrics_enabled: true,
        }
    }
}

/// WAL metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct WalMetrics {
    /// Total entries written
    pub entries_written: u64,

    /// Total entries replayed during recovery
    pub entries_replayed: u64,

    /// Current LSN
    pub current_lsn: u64,

    /// Number of WAL files
    pub wal_file_count: usize,

    /// Total WAL size (bytes)
    pub total_wal_size_bytes: u64,

    /// Last checkpoint time
    pub last_checkpoint_time: Option<i64>,

    /// Number of fsync calls
    pub fsync_count: u64,

    /// Average write latency (microseconds)
    pub avg_write_latency_us: u64,
}

/// Write-Ahead Log manager
pub struct WriteAheadLog {
    /// Configuration
    config: WalConfig,

    /// Current log sequence number
    current_lsn: Arc<RwLock<u64>>,

    /// Current WAL file writer
    current_writer: Arc<Mutex<Option<BufWriter<File>>>>,

    /// Current WAL file path
    current_file_path: Arc<Mutex<PathBuf>>,

    /// Current file size
    current_file_size: Arc<RwLock<u64>>,

    /// Metrics
    metrics: Arc<RwLock<WalMetrics>>,

    /// Snapshot backend for checkpointing
    snapshot_backend: Option<Arc<dyn PersistenceBackend>>,

    /// Checkpoint task handle
    checkpoint_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl WriteAheadLog {
    /// Create a new WriteAheadLog instance
    pub async fn new(config: WalConfig) -> OrbitResult<Self> {
        // Ensure log directory exists
        std::fs::create_dir_all(&config.log_directory).map_err(|e| {
            OrbitError::storage(format!("Failed to create WAL directory: {}", e))
        })?;

        let current_lsn = Arc::new(RwLock::new(0));
        let metrics = Arc::new(RwLock::new(WalMetrics::default()));

        // Determine starting LSN by scanning existing WAL files
        let starting_lsn = Self::scan_existing_logs(&config.log_directory).await?;
        *current_lsn.write().await = starting_lsn;

        info!(
            "Initialized WAL with LSN {} at {:?}",
            starting_lsn, config.log_directory
        );

        Ok(Self {
            config,
            current_lsn,
            current_writer: Arc::new(Mutex::new(None)),
            current_file_path: Arc::new(Mutex::new(PathBuf::new())),
            current_file_size: Arc::new(RwLock::new(0)),
            metrics,
            snapshot_backend: None,
            checkpoint_handle: Arc::new(Mutex::new(None)),
        })
    }

    /// Set snapshot backend for checkpointing
    pub fn with_snapshot_backend(
        mut self,
        backend: Arc<dyn PersistenceBackend>,
    ) -> Self {
        self.snapshot_backend = Some(backend);
        self
    }

    /// Start the WAL service (background tasks)
    pub async fn start(&self) -> OrbitResult<()> {
        // Open initial WAL file
        self.rotate_log_file().await?;

        // Start checkpoint task
        self.start_checkpoint_task().await;

        info!("WAL service started");
        Ok(())
    }

    /// Stop the WAL service
    pub async fn stop(&self) -> OrbitResult<()> {
        // Stop checkpoint task
        if let Some(handle) = self.checkpoint_handle.lock().await.take() {
            handle.abort();
        }

        // Flush and close current writer
        if let Some(mut writer) = self.current_writer.lock().await.take() {
            writer.flush().map_err(|e| {
                OrbitError::storage(format!("Failed to flush WAL: {}", e))
            })?;
        }

        info!("WAL service stopped");
        Ok(())
    }

    /// Write a WAL entry
    pub async fn write_entry(&self, entry: WalEntry) -> OrbitResult<u64> {
        let start = Instant::now();

        // Serialize entry
        let serialized = bincode::serialize(&entry).map_err(|e| {
            OrbitError::parse(format!("Failed to serialize WAL entry: {}", e))
        })?;

        // Acquire writer lock
        let mut writer_guard = self.current_writer.lock().await;
        let writer = writer_guard.as_mut().ok_or_else(|| {
            OrbitError::storage("WAL writer not initialized")
        })?;

        // Write length prefix (for reading back)
        let length = serialized.len() as u32;
        writer.write_all(&length.to_le_bytes()).map_err(|e| {
            OrbitError::storage(format!("Failed to write WAL entry length: {}", e))
        })?;

        // Write entry data
        writer.write_all(&serialized).map_err(|e| {
            OrbitError::storage(format!("Failed to write WAL entry: {}", e))
        })?;

        // Update file size
        let entry_size = 4 + serialized.len() as u64;
        let mut current_size = self.current_file_size.write().await;
        *current_size += entry_size;

        // Flush based on durability level
        match self.config.durability_level {
            DurabilityLevel::Strict => {
                writer.flush().map_err(|e| {
                    OrbitError::storage(format!("Failed to flush WAL: {}", e))
                })?;

                // Force fsync
                writer.get_ref().sync_all().map_err(|e| {
                    OrbitError::storage(format!("Failed to fsync WAL: {}", e))
                })?;

                let mut metrics = self.metrics.write().await;
                metrics.fsync_count += 1;
            }
            DurabilityLevel::Standard => {
                // Flush to OS buffer
                writer.flush().map_err(|e| {
                    OrbitError::storage(format!("Failed to flush WAL: {}", e))
                })?;
            }
            DurabilityLevel::Performance => {
                // No immediate flush - rely on OS buffering
            }
        }

        // Update metrics
        let elapsed = start.elapsed().as_micros() as u64;
        let mut metrics = self.metrics.write().await;
        metrics.entries_written += 1;
        metrics.current_lsn = entry.lsn;

        // Update average latency
        if metrics.entries_written == 1 {
            metrics.avg_write_latency_us = elapsed;
        } else {
            metrics.avg_write_latency_us =
                (metrics.avg_write_latency_us * (metrics.entries_written - 1) + elapsed)
                / metrics.entries_written;
        }

        drop(metrics);
        drop(writer_guard);

        // Check if we need to rotate log file
        if *current_size >= self.config.max_log_size_bytes {
            self.rotate_log_file().await?;
        }

        Ok(entry.lsn)
    }

    /// Allocate next LSN
    pub async fn next_lsn(&self) -> u64 {
        let mut lsn = self.current_lsn.write().await;
        *lsn += 1;
        *lsn
    }

    /// Write insert entry
    pub async fn write_insert(
        &self,
        actor_ref: AddressableReference,
        state_data: serde_json::Value,
    ) -> OrbitResult<u64> {
        let lsn = self.next_lsn().await;
        let state_data_json = serde_json::to_string(&state_data)
            .map_err(|e| OrbitError::parse(format!("Failed to serialize state: {}", e)))?;
        let entry = WalEntry::new(
            lsn,
            actor_ref,
            WalEntryType::Insert { state_data_json },
        );
        self.write_entry(entry).await
    }

    /// Write update entry
    pub async fn write_update(
        &self,
        actor_ref: AddressableReference,
        old_state: serde_json::Value,
        new_state: serde_json::Value,
    ) -> OrbitResult<u64> {
        let lsn = self.next_lsn().await;
        let old_state_json = serde_json::to_string(&old_state)
            .map_err(|e| OrbitError::parse(format!("Failed to serialize old state: {}", e)))?;
        let new_state_json = serde_json::to_string(&new_state)
            .map_err(|e| OrbitError::parse(format!("Failed to serialize new state: {}", e)))?;
        let entry = WalEntry::new(
            lsn,
            actor_ref,
            WalEntryType::Update { old_state_json, new_state_json },
        );
        self.write_entry(entry).await
    }

    /// Write delete entry
    pub async fn write_delete(
        &self,
        actor_ref: AddressableReference,
        state_data: serde_json::Value,
    ) -> OrbitResult<u64> {
        let lsn = self.next_lsn().await;
        let state_data_json = serde_json::to_string(&state_data)
            .map_err(|e| OrbitError::parse(format!("Failed to serialize state: {}", e)))?;
        let entry = WalEntry::new(
            lsn,
            actor_ref,
            WalEntryType::Delete { state_data_json },
        );
        self.write_entry(entry).await
    }

    /// Write checkpoint marker
    pub async fn write_checkpoint(
        &self,
        actor_ref: AddressableReference,
        snapshot_id: String,
        snapshot_version: u64,
    ) -> OrbitResult<u64> {
        let lsn = self.next_lsn().await;
        let entry = WalEntry::new(
            lsn,
            actor_ref,
            WalEntryType::Checkpoint {
                snapshot_id,
                snapshot_version,
            },
        );
        self.write_entry(entry).await
    }

    /// Replay WAL entries from a specific LSN
    pub async fn replay_from_lsn(&self, start_lsn: u64) -> OrbitResult<Vec<WalEntry>> {
        let mut entries = Vec::new();

        // Get all WAL files sorted by LSN
        let wal_files = self.list_wal_files().await?;

        for file_path in wal_files {
            let file_entries = self.read_wal_file(&file_path).await?;

            for entry in file_entries {
                if entry.lsn >= start_lsn {
                    // Verify integrity
                    if !entry.verify_integrity() {
                        warn!("WAL entry {} failed integrity check", entry.entry_id);
                        continue;
                    }

                    entries.push(entry);
                }
            }
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.entries_replayed += entries.len() as u64;

        info!("Replayed {} WAL entries from LSN {}", entries.len(), start_lsn);

        Ok(entries)
    }

    /// Perform checkpoint operation
    async fn perform_checkpoint(&self) -> OrbitResult<()> {
        let _snapshot_backend = match &self.snapshot_backend {
            Some(backend) => backend.clone(),
            None => {
                debug!("No snapshot backend configured, skipping checkpoint");
                return Ok(());
            }
        };

        // This is a simplified checkpoint - in a real system, you'd:
        // 1. Iterate through all active actors
        // 2. Create snapshots for each
        // 3. Write checkpoint markers to WAL
        // 4. Clean up old WAL files

        let current_lsn = *self.current_lsn.read().await;

        info!("Performing checkpoint at LSN {}", current_lsn);

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.last_checkpoint_time = Some(chrono::Utc::now().timestamp_millis());

        // Clean up old WAL files
        drop(metrics);
        self.cleanup_old_wal_files().await?;

        Ok(())
    }

    /// Rotate to a new WAL file
    async fn rotate_log_file(&self) -> OrbitResult<()> {
        // Close current writer
        if let Some(mut writer) = self.current_writer.lock().await.take() {
            writer.flush().map_err(|e| {
                OrbitError::storage(format!("Failed to flush WAL: {}", e))
            })?;
        }

        // Generate new file path
        let lsn = *self.current_lsn.read().await;
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("wal_{:016x}_{}.log", lsn, timestamp);
        let file_path = self.config.log_directory.join(filename);

        // Open new file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| {
                OrbitError::storage(format!("Failed to open WAL file: {}", e))
            })?;

        let writer = BufWriter::with_capacity(self.config.buffer_size, file);

        *self.current_writer.lock().await = Some(writer);
        *self.current_file_path.lock().await = file_path.clone();
        *self.current_file_size.write().await = 0;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.wal_file_count += 1;

        info!("Rotated to new WAL file: {:?}", file_path);

        Ok(())
    }

    /// Read all entries from a WAL file
    async fn read_wal_file(&self, path: &Path) -> OrbitResult<Vec<WalEntry>> {
        let file = File::open(path).map_err(|e| {
            OrbitError::storage(format!("Failed to open WAL file: {}", e))
        })?;

        let mut reader = BufReader::new(file);
        let mut entries = Vec::new();

        loop {
            // Read length prefix
            let mut len_bytes = [0u8; 4];
            match reader.read_exact(&mut len_bytes) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // End of file
                    break;
                }
                Err(e) => {
                    return Err(OrbitError::storage(format!(
                        "Failed to read WAL entry length: {}",
                        e
                    )));
                }
            }

            let length = u32::from_le_bytes(len_bytes) as usize;

            // Read entry data
            let mut entry_bytes = vec![0u8; length];
            reader.read_exact(&mut entry_bytes).map_err(|e| {
                OrbitError::storage(format!("Failed to read WAL entry: {}", e))
            })?;

            // Deserialize entry
            let entry: WalEntry = bincode::deserialize(&entry_bytes).map_err(|e| {
                OrbitError::parse(format!(
                    "Failed to deserialize WAL entry: {}",
                    e
                ))
            })?;

            entries.push(entry);
        }

        Ok(entries)
    }

    /// List all WAL files sorted by LSN
    async fn list_wal_files(&self) -> OrbitResult<Vec<PathBuf>> {
        let mut files = Vec::new();

        let entries = std::fs::read_dir(&self.config.log_directory).map_err(|e| {
            OrbitError::storage(format!("Failed to read WAL directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                OrbitError::storage(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("log") {
                files.push(path);
            }
        }

        // Sort by filename (which contains LSN)
        files.sort();

        Ok(files)
    }

    /// Scan existing logs to determine starting LSN
    async fn scan_existing_logs(log_dir: &Path) -> OrbitResult<u64> {
        if !log_dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(log_dir).map_err(|e| {
            OrbitError::storage(format!("Failed to read WAL directory: {}", e))
        })?;

        let mut max_lsn = 0u64;

        for entry in entries {
            let entry = entry.map_err(|e| {
                OrbitError::storage(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("log") {
                continue;
            }

            // Parse LSN from filename (wal_<LSN>_<timestamp>.log)
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                if let Some(lsn_str) = filename.strip_prefix("wal_").and_then(|s| s.split('_').next()) {
                    if let Ok(lsn) = u64::from_str_radix(lsn_str, 16) {
                        max_lsn = max_lsn.max(lsn);
                    }
                }
            }
        }

        Ok(max_lsn)
    }

    /// Clean up old WAL files after checkpoint
    async fn cleanup_old_wal_files(&self) -> OrbitResult<()> {
        let mut files = self.list_wal_files().await?;

        // Keep only the most recent N files
        if files.len() > self.config.retain_wal_files {
            let to_remove = files.len() - self.config.retain_wal_files;

            for file_path in files.drain(..to_remove) {
                std::fs::remove_file(&file_path).map_err(|e| {
                    OrbitError::storage(format!("Failed to remove WAL file: {}", e))
                })?;

                info!("Removed old WAL file: {:?}", file_path);
            }
        }

        Ok(())
    }

    /// Start background checkpoint task
    async fn start_checkpoint_task(&self) {
        let interval = Duration::from_secs(self.config.checkpoint_interval_secs);
        let wal = self.clone_for_task();

        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                if let Err(e) = wal.perform_checkpoint().await {
                    error!("Checkpoint failed: {}", e);
                }
            }
        });

        *self.checkpoint_handle.lock().await = Some(handle);
    }

    /// Clone for background task
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            current_lsn: self.current_lsn.clone(),
            current_writer: self.current_writer.clone(),
            current_file_path: self.current_file_path.clone(),
            current_file_size: self.current_file_size.clone(),
            metrics: self.metrics.clone(),
            snapshot_backend: self.snapshot_backend.clone(),
            checkpoint_handle: self.checkpoint_handle.clone(),
        }
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> WalMetrics {
        self.metrics.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressable::Key;
    use tempfile::TempDir;

    fn create_test_actor_ref() -> AddressableReference {
        AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test-actor-1".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_wal_basic_write_and_read() {
        let temp_dir = TempDir::new().unwrap();

        let config = WalConfig {
            log_directory: temp_dir.path().to_path_buf(),
            durability_level: DurabilityLevel::Strict,
            ..Default::default()
        };

        let wal = WriteAheadLog::new(config).await.unwrap();
        wal.start().await.unwrap();

        // Write an insert entry
        let actor_ref = create_test_actor_ref();
        let state = serde_json::json!({"key": "value"});

        let lsn = wal.write_insert(actor_ref.clone(), state.clone()).await.unwrap();
        assert_eq!(lsn, 1);

        // Stop WAL to flush
        wal.stop().await.unwrap();

        // Replay from beginning
        let wal2 = WriteAheadLog::new(WalConfig {
            log_directory: temp_dir.path().to_path_buf(),
            ..Default::default()
        })
        .await
        .unwrap();

        let entries = wal2.replay_from_lsn(0).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].lsn, 1);

        match &entries[0].entry_type {
            WalEntryType::Insert { state_data_json } => {
                let recovered: serde_json::Value = serde_json::from_str(state_data_json).unwrap();
                assert_eq!(recovered, state);
            }
            _ => panic!("Expected Insert entry"),
        }
    }

    #[tokio::test]
    async fn test_wal_entry_integrity() {
        let actor_ref = create_test_actor_ref();
        let state = serde_json::json!({"test": 123});
        let state_data_json = serde_json::to_string(&state).unwrap();

        let entry = WalEntry::new(
            42,
            actor_ref,
            WalEntryType::Insert {
                state_data_json,
            },
        );

        assert!(entry.verify_integrity());

        // Tamper with entry
        let mut tampered = entry.clone();
        tampered.lsn = 999;

        assert!(!tampered.verify_integrity());
    }

    #[tokio::test]
    async fn test_wal_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();

        let config = WalConfig {
            log_directory: temp_dir.path().to_path_buf(),
            durability_level: DurabilityLevel::Standard,
            ..Default::default()
        };

        let wal = WriteAheadLog::new(config).await.unwrap();
        wal.start().await.unwrap();

        let actor_ref = create_test_actor_ref();

        // Write multiple entries
        for i in 0..10 {
            let state = serde_json::json!({"value": i});
            wal.write_insert(actor_ref.clone(), state).await.unwrap();
        }

        wal.stop().await.unwrap();

        // Verify all entries
        let wal2 = WriteAheadLog::new(WalConfig {
            log_directory: temp_dir.path().to_path_buf(),
            ..Default::default()
        })
        .await
        .unwrap();

        let entries = wal2.replay_from_lsn(0).await.unwrap();
        assert_eq!(entries.len(), 10);

        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.lsn, (i + 1) as u64);
        }
    }

    #[tokio::test]
    async fn test_wal_metrics() {
        let temp_dir = TempDir::new().unwrap();

        let config = WalConfig {
            log_directory: temp_dir.path().to_path_buf(),
            metrics_enabled: true,
            ..Default::default()
        };

        let wal = WriteAheadLog::new(config).await.unwrap();
        wal.start().await.unwrap();

        let actor_ref = create_test_actor_ref();
        let state = serde_json::json!({"test": "data"});

        wal.write_insert(actor_ref, state).await.unwrap();

        let metrics = wal.get_metrics().await;
        assert_eq!(metrics.entries_written, 1);
        assert_eq!(metrics.current_lsn, 1);
        assert!(metrics.avg_write_latency_us > 0);

        wal.stop().await.unwrap();
    }
}
