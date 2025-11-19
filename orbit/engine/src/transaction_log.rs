use crate::error::{EngineError, EngineResult};
use crate::transactions::{TransactionEvent, TransactionId, TransactionLogEntry};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Row};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, Instant};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Synchronous mode for transaction log
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// OFF - No syncing, fastest but least durable (risk of corruption)
    Off,
    /// NORMAL - Sync at critical moments (good balance)
    Normal,
    /// FULL - Sync after every write (slowest but most durable)
    Full,
    /// EXTRA - Even more durable than FULL (sync directory entries too)
    Extra,
}

impl SyncMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncMode::Off => "OFF",
            SyncMode::Normal => "NORMAL",
            SyncMode::Full => "FULL",
            SyncMode::Extra => "EXTRA",
        }
    }
}

/// Configuration for persistent transaction log
#[derive(Debug, Clone)]
pub struct PersistentLogConfig {
    /// Database file path
    pub database_path: PathBuf,
    /// Maximum number of log entries before rotation
    pub max_entries: usize,
    /// Age after which entries can be archived (in seconds)
    pub archive_after_seconds: i64,
    /// Enable automatic log rotation
    pub auto_rotate: bool,
    /// Batch size for bulk operations
    pub batch_size: usize,
    /// Connection pool configuration
    pub max_connections: u32,
    /// Enable Write-Ahead Logging for better performance
    /// When enabled, writes are ~3x faster but requires WAL checkpointing
    pub enable_wal: bool,
    /// Synchronous mode - controls durability vs performance trade-off
    /// - Normal: Good balance (default)
    /// - Full: Maximum durability, slower writes
    /// - Off: Fastest, risk of corruption on crash
    pub sync_mode: SyncMode,
    /// Cache size in pages (negative = KB, positive = pages)
    /// Default: -10000 (~10MB cache)
    pub cache_size: i32,
    /// WAL checkpoint interval (in entries)
    /// Smaller = more frequent checkpoints, less recovery time
    /// Larger = better write performance, longer recovery
    pub wal_checkpoint_interval: usize,
}

impl Default for PersistentLogConfig {
    fn default() -> Self {
        Self {
            database_path: PathBuf::from("orbit_transactions.db"),
            max_entries: 100_000,
            archive_after_seconds: 7 * 24 * 60 * 60, // 7 days
            auto_rotate: true,
            batch_size: 1000,
            max_connections: 5,
            enable_wal: true,
            sync_mode: SyncMode::Normal,
            cache_size: -10000, // ~10MB
            wal_checkpoint_interval: 10000,
        }
    }
}

impl PersistentLogConfig {
    /// Create config optimized for maximum durability (slower performance)
    pub fn durability_optimized() -> Self {
        Self {
            enable_wal: true,
            sync_mode: SyncMode::Full,
            cache_size: -5000, // Smaller cache for more frequent flushes
            wal_checkpoint_interval: 1000, // More frequent checkpoints
            ..Default::default()
        }
    }

    /// Create config optimized for maximum performance (less durable)
    pub fn performance_optimized() -> Self {
        Self {
            enable_wal: true,
            sync_mode: SyncMode::Normal,
            cache_size: -20000, // Larger cache (~20MB)
            wal_checkpoint_interval: 50000, // Less frequent checkpoints
            batch_size: 5000, // Larger batches
            ..Default::default()
        }
    }

    /// Create config for in-memory operation (ephemeral, fastest)
    pub fn in_memory() -> Self {
        Self {
            database_path: PathBuf::from(":memory:"),
            enable_wal: false, // WAL not needed for in-memory
            sync_mode: SyncMode::Off,
            cache_size: -50000, // Large cache (~50MB)
            ..Default::default()
        }
    }
}

/// Persistent transaction log entry with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentLogEntry {
    pub id: Uuid,
    pub timestamp: i64,
    pub transaction_id: TransactionId,
    pub event: TransactionEvent,
    pub details: Option<serde_json::Value>,
    pub node_id: String,
    pub checksum: String,
    pub archived: bool,
}

impl From<TransactionLogEntry> for PersistentLogEntry {
    fn from(entry: TransactionLogEntry) -> Self {
        let serialized = serde_json::to_string(&entry).unwrap_or_default();
        let checksum = format!("{:x}", md5::compute(serialized.as_bytes()));

        Self {
            id: Uuid::new_v4(),
            timestamp: entry.timestamp,
            transaction_id: entry.transaction_id,
            event: entry.event,
            details: entry.details,
            node_id: "unknown".to_string(), // Would be populated from context
            checksum,
            archived: false,
        }
    }
}

/// Statistics for the persistent transaction log
#[derive(Debug, Clone)]
pub struct LogStats {
    pub total_entries: u64,
    pub active_entries: u64,
    pub archived_entries: u64,
    pub database_size_bytes: u64,
    pub oldest_entry_timestamp: Option<i64>,
    pub newest_entry_timestamp: Option<i64>,
}

/// Trait for persistent transaction logging backends
#[async_trait]
pub trait PersistentTransactionLogger: Send + Sync {
    /// Write a single log entry
    async fn write_entry(&self, entry: &TransactionLogEntry) -> EngineResult<()>;

    /// Write multiple log entries in a batch
    async fn write_batch(&self, entries: &[TransactionLogEntry]) -> EngineResult<()>;

    /// Query log entries by transaction ID
    async fn get_transaction_log(
        &self,
        transaction_id: &TransactionId,
    ) -> EngineResult<Vec<PersistentLogEntry>>;

    /// Query log entries within a time range
    async fn get_entries_by_time_range(
        &self,
        start_time: i64,
        end_time: i64,
    ) -> EngineResult<Vec<PersistentLogEntry>>;

    /// Archive old entries
    async fn archive_old_entries(&self, before_timestamp: i64) -> EngineResult<u64>;

    /// Get log statistics
    async fn get_stats(&self) -> EngineResult<LogStats>;

    /// Perform log maintenance (cleanup, optimization, etc.)
    async fn maintenance(&self) -> EngineResult<()>;

    /// Recover transaction state from logs
    async fn recover_transaction_state(
        &self,
        transaction_id: &TransactionId,
    ) -> EngineResult<Option<TransactionEvent>>;
}

/// SQLite-based persistent transaction logger
pub struct SqliteTransactionLogger {
    pool: SqlitePool,
    config: PersistentLogConfig,
    stats_cache: Arc<RwLock<Option<(LogStats, Instant)>>>,
    write_buffer: Arc<Mutex<Vec<TransactionLogEntry>>>,
}

impl SqliteTransactionLogger {
    /// Create a new SQLite transaction logger
    pub async fn new(config: PersistentLogConfig) -> EngineResult<Self> {
        // Ensure database directory exists
        if let Some(parent) = config.database_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                EngineError::internal(format!("Failed to create database directory: {e}"))
            })?;
        }

        // Build connection string
        let database_url = format!("sqlite:{}?mode=rwc", config.database_path.display());

        // Configure connection pool
        let pool = SqlitePool::connect(&database_url)
            .await
            .map_err(|e| EngineError::internal(format!("Failed to connect to database: {e}")))?;

        let logger = Self {
            pool,
            config,
            stats_cache: Arc::new(RwLock::new(None)),
            write_buffer: Arc::new(Mutex::new(Vec::new())),
        };

        logger.initialize_schema().await?;
        logger.configure_database().await?;

        info!(
            "SQLite transaction logger initialized: {}",
            logger.config.database_path.display()
        );
        Ok(logger)
    }

    /// Initialize database schema
    async fn initialize_schema(&self) -> EngineResult<()> {
        let schema_sql = r#"
            CREATE TABLE IF NOT EXISTS transaction_log (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                transaction_id TEXT NOT NULL,
                coordinator_node TEXT NOT NULL,
                event_type TEXT NOT NULL,
                event_data TEXT NOT NULL,
                details TEXT,
                node_id TEXT NOT NULL,
                checksum TEXT NOT NULL,
                archived BOOLEAN NOT NULL DEFAULT FALSE,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_transaction_log_transaction_id 
            ON transaction_log(transaction_id);
            
            CREATE INDEX IF NOT EXISTS idx_transaction_log_timestamp 
            ON transaction_log(timestamp);
            
            CREATE INDEX IF NOT EXISTS idx_transaction_log_node_id 
            ON transaction_log(node_id);
            
            CREATE INDEX IF NOT EXISTS idx_transaction_log_archived 
            ON transaction_log(archived);

            -- Transaction recovery table for coordinator state
            CREATE TABLE IF NOT EXISTS transaction_recovery (
                transaction_id TEXT PRIMARY KEY,
                coordinator_node TEXT NOT NULL,
                state TEXT NOT NULL,
                participants TEXT NOT NULL,
                operations TEXT NOT NULL,
                timeout_seconds INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_recovery_coordinator_node 
            ON transaction_recovery(coordinator_node);
        "#;

        sqlx::query(schema_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| EngineError::internal(format!("Failed to initialize schema: {e}")))?;

        Ok(())
    }

    /// Configure database settings
    async fn configure_database(&self) -> EngineResult<()> {
        // Configure WAL mode if enabled
        if self.config.enable_wal {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&self.pool)
                .await
                .map_err(|e| EngineError::internal(format!("Failed to enable WAL: {e}")))?;

            info!("WAL mode enabled for transaction log");
        } else {
            sqlx::query("PRAGMA journal_mode = DELETE")
                .execute(&self.pool)
                .await
                .map_err(|e| EngineError::internal(format!("Failed to set journal mode: {e}")))?;
        }

        // Configure synchronous mode
        let sync_pragma = format!("PRAGMA synchronous = {}", self.config.sync_mode.as_str());
        sqlx::query(&sync_pragma)
            .execute(&self.pool)
            .await
            .map_err(|e| EngineError::internal(format!("Failed to set synchronous mode: {e}")))?;

        debug!("Synchronous mode set to: {:?}", self.config.sync_mode);

        // Configure cache size
        let cache_pragma = format!("PRAGMA cache_size = {}", self.config.cache_size);
        sqlx::query(&cache_pragma)
            .execute(&self.pool)
            .await
            .map_err(|e| EngineError::internal(format!("Failed to set cache size: {e}")))?;

        // Configure temp store to memory for better performance
        sqlx::query("PRAGMA temp_store = memory")
            .execute(&self.pool)
            .await
            .map_err(|e| EngineError::internal(format!("Failed to set temp store: {e}")))?;

        // Configure WAL autocheckpoint (number of pages before auto-checkpoint)
        if self.config.enable_wal {
            let checkpoint_pages = (self.config.wal_checkpoint_interval / 10).max(1000);
            let checkpoint_pragma = format!("PRAGMA wal_autocheckpoint = {}", checkpoint_pages);
            sqlx::query(&checkpoint_pragma)
                .execute(&self.pool)
                .await
                .map_err(|e| EngineError::internal(format!("Failed to set WAL autocheckpoint: {e}")))?;
        }

        info!(
            "Transaction log configured: WAL={}, sync={:?}, cache={}KB",
            self.config.enable_wal,
            self.config.sync_mode,
            self.config.cache_size.abs() / 1024
        );

        Ok(())
    }

    /// Flush buffered entries to database
    async fn flush_buffer(&self) -> EngineResult<()> {
        let entries = {
            let mut buffer = self.write_buffer.lock().await;
            if buffer.is_empty() {
                return Ok(());
            }
            let entries = buffer.clone();
            buffer.clear();
            entries
        };

        if !entries.is_empty() {
            self.write_batch_internal(&entries).await?;
            debug!("Flushed {} buffered log entries", entries.len());
        }

        Ok(())
    }

    /// Internal batch write implementation
    async fn write_batch_internal(&self, entries: &[TransactionLogEntry]) -> EngineResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| EngineError::internal(format!("Failed to begin transaction: {e}")))?;

        for entry in entries {
            let persistent_entry = PersistentLogEntry::from(entry.clone());

            let event_data = serde_json::to_string(&entry.event)
                .map_err(|e| EngineError::internal(format!("Failed to serialize event: {e}")))?;

            let details_json = entry
                .details
                .as_ref()
                .map(|d| serde_json::to_string(d).unwrap_or_default());

            sqlx::query(
                r#"INSERT INTO transaction_log 
                   (id, timestamp, transaction_id, coordinator_node, event_type, event_data, details, node_id, checksum, archived) 
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
            )
            .bind(persistent_entry.id.to_string())
            .bind(persistent_entry.timestamp)
            .bind(entry.transaction_id.id.clone())
            .bind(entry.transaction_id.coordinator_node.to_string())
            .bind(discriminant_name(&entry.event))
            .bind(event_data)
            .bind(details_json)
            .bind(persistent_entry.node_id)
            .bind(persistent_entry.checksum)
            .bind(persistent_entry.archived)
            .execute(&mut *tx).await
            .map_err(|e| EngineError::internal(format!("Failed to insert log entry: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| EngineError::internal(format!("Failed to commit batch: {e}")))?;

        Ok(())
    }

    /// Start background maintenance tasks
    pub async fn start_background_tasks(&self) -> EngineResult<()> {
        let logger = Arc::new(self.clone());

        // Buffer flush task
        {
            let logger_clone = Arc::clone(&logger);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                loop {
                    interval.tick().await;
                    if let Err(e) = logger_clone.flush_buffer().await {
                        error!("Failed to flush log buffer: {}", e);
                    }
                }
            });
        }

        // Maintenance task
        {
            let logger_clone = Arc::clone(&logger);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
                loop {
                    interval.tick().await;
                    if let Err(e) = logger_clone.maintenance().await {
                        error!("Log maintenance failed: {}", e);
                    }
                }
            });
        }

        info!("Transaction logger background tasks started");
        Ok(())
    }
}

impl Clone for SqliteTransactionLogger {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            config: self.config.clone(),
            stats_cache: Arc::clone(&self.stats_cache),
            write_buffer: Arc::clone(&self.write_buffer),
        }
    }
}

#[async_trait]
impl PersistentTransactionLogger for SqliteTransactionLogger {
    async fn write_entry(&self, entry: &TransactionLogEntry) -> EngineResult<()> {
        // Add to buffer for batch processing
        {
            let mut buffer = self.write_buffer.lock().await;
            buffer.push(entry.clone());

            // Flush if buffer is full
            if buffer.len() >= self.config.batch_size {
                let entries = buffer.clone();
                buffer.clear();
                drop(buffer); // Release lock before async call

                self.write_batch_internal(&entries).await?;
            }
        }

        Ok(())
    }

    async fn write_batch(&self, entries: &[TransactionLogEntry]) -> EngineResult<()> {
        self.write_batch_internal(entries).await
    }

    async fn get_transaction_log(
        &self,
        transaction_id: &TransactionId,
    ) -> EngineResult<Vec<PersistentLogEntry>> {
        let rows = sqlx::query(
            "SELECT * FROM transaction_log WHERE transaction_id = ? ORDER BY timestamp ASC",
        )
        .bind(&transaction_id.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EngineError::internal(format!("Failed to query transaction log: {e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(self.row_to_persistent_entry(row)?);
        }

        Ok(entries)
    }

    async fn get_entries_by_time_range(
        &self,
        start_time: i64,
        end_time: i64,
    ) -> EngineResult<Vec<PersistentLogEntry>> {
        let rows = sqlx::query(
            "SELECT * FROM transaction_log WHERE timestamp BETWEEN ? AND ? ORDER BY timestamp ASC",
        )
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EngineError::internal(format!("Failed to query time range: {e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(self.row_to_persistent_entry(row)?);
        }

        Ok(entries)
    }

    async fn archive_old_entries(&self, before_timestamp: i64) -> EngineResult<u64> {
        let result = sqlx::query(
            "UPDATE transaction_log SET archived = TRUE WHERE timestamp < ? AND archived = FALSE",
        )
        .bind(before_timestamp)
        .execute(&self.pool)
        .await
        .map_err(|e| EngineError::internal(format!("Failed to archive entries: {e}")))?;

        let archived_count = result.rows_affected();
        if archived_count > 0 {
            info!("Archived {} old transaction log entries", archived_count);
        }

        Ok(archived_count)
    }

    async fn get_stats(&self) -> EngineResult<LogStats> {
        // Check cache first
        {
            let cache = self.stats_cache.read().await;
            if let Some((stats, cached_at)) = cache.as_ref() {
                if cached_at.elapsed() < Duration::from_secs(30) {
                    return Ok(stats.clone());
                }
            }
        }

        // Query fresh stats
        let stats_row = sqlx::query(
            r#"SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN archived = FALSE THEN 1 END) as active,
                COUNT(CASE WHEN archived = TRUE THEN 1 END) as archived,
                MIN(timestamp) as oldest,
                MAX(timestamp) as newest
               FROM transaction_log"#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| EngineError::internal(format!("Failed to get stats: {e}")))?;

        let database_size = tokio::fs::metadata(&self.config.database_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let stats = LogStats {
            total_entries: stats_row.get::<i64, _>("total") as u64,
            active_entries: stats_row.get::<i64, _>("active") as u64,
            archived_entries: stats_row.get::<i64, _>("archived") as u64,
            database_size_bytes: database_size,
            oldest_entry_timestamp: stats_row.get::<Option<i64>, _>("oldest"),
            newest_entry_timestamp: stats_row.get::<Option<i64>, _>("newest"),
        };

        // Update cache
        {
            let mut cache = self.stats_cache.write().await;
            *cache = Some((stats.clone(), Instant::now()));
        }

        Ok(stats)
    }

    async fn maintenance(&self) -> EngineResult<()> {
        // Flush any pending writes
        self.flush_buffer().await?;

        // Archive old entries if auto-rotation is enabled
        if self.config.auto_rotate {
            let cutoff_time = chrono::Utc::now().timestamp() - self.config.archive_after_seconds;
            self.archive_old_entries(cutoff_time).await?;
        }

        // Vacuum database to reclaim space
        sqlx::query("VACUUM")
            .execute(&self.pool)
            .await
            .map_err(|e| EngineError::internal(format!("Failed to vacuum database: {e}")))?;

        // Update statistics
        sqlx::query("ANALYZE")
            .execute(&self.pool)
            .await
            .map_err(|e| EngineError::internal(format!("Failed to analyze database: {e}")))?;

        debug!("Transaction log maintenance completed");
        Ok(())
    }

    async fn recover_transaction_state(
        &self,
        transaction_id: &TransactionId,
    ) -> EngineResult<Option<TransactionEvent>> {
        let row = sqlx::query(
            r#"SELECT event_data FROM transaction_log 
               WHERE transaction_id = ? 
               ORDER BY timestamp DESC 
               LIMIT 1"#,
        )
        .bind(&transaction_id.id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EngineError::internal(format!("Failed to recover transaction state: {e}")))?;

        if let Some(row) = row {
            let event_data: String = row.get("event_data");
            let event: TransactionEvent = serde_json::from_str(&event_data)
                .map_err(|e| EngineError::internal(format!("Failed to deserialize event: {e}")))?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }
}

impl SqliteTransactionLogger {
    /// Convert database row to persistent log entry
    fn row_to_persistent_entry(
        &self,
        row: sqlx::sqlite::SqliteRow,
    ) -> EngineResult<PersistentLogEntry> {
        let id_str: String = row.get("id");
        let transaction_id_str: String = row.get("transaction_id");
        let coordinator_node_str: String = row.get("coordinator_node");
        let event_data: String = row.get("event_data");
        let details_str: Option<String> = row.get("details");

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| EngineError::internal(format!("Invalid UUID: {e}")))?;

        let transaction_id = TransactionId {
            id: transaction_id_str,
            coordinator_node: parse_node_id_from_string(&coordinator_node_str),
            created_at: row.get("timestamp"),
        };

        let event: TransactionEvent = serde_json::from_str(&event_data)
            .map_err(|e| EngineError::internal(format!("Failed to deserialize event: {e}")))?;

        let details =
            if let Some(details_str) = details_str {
                Some(serde_json::from_str(&details_str).map_err(|e| {
                    EngineError::internal(format!("Failed to deserialize details: {e}"))
                })?)
            } else {
                None
            };

        Ok(PersistentLogEntry {
            id,
            timestamp: row.get("timestamp"),
            transaction_id,
            event,
            details,
            node_id: row.get("node_id"),
            checksum: row.get("checksum"),
            archived: row.get("archived"),
        })
    }
}

/// Get discriminant name for transaction event (for indexing)
fn discriminant_name(event: &TransactionEvent) -> &'static str {
    match event {
        TransactionEvent::Started => "Started",
        TransactionEvent::PrepareRequested => "PrepareRequested",
        TransactionEvent::VoteReceived { .. } => "VoteReceived",
        TransactionEvent::CommitRequested => "CommitRequested",
        TransactionEvent::AbortRequested { .. } => "AbortRequested",
        TransactionEvent::Committed => "Committed",
        TransactionEvent::Aborted { .. } => "Aborted",
        TransactionEvent::TimedOut => "TimedOut",
        TransactionEvent::Failed { .. } => "Failed",
    }
}

/// Parse NodeId from string, handling the "(namespace:key)" format
fn parse_node_id_from_string(s: &str) -> crate::cluster::NodeId {
    let trimmed = s.trim_start_matches('(').trim_end_matches(')');
    trimmed.to_string() // NodeId is just a String
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::NodeId;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sqlite_logger_initialization() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = PersistentLogConfig {
            database_path: db_path,
            ..Default::default()
        };

        let logger = SqliteTransactionLogger::new(config).await.unwrap();

        // Verify database file was created
        assert!(logger.config.database_path.exists());
    }

    #[tokio::test]
    async fn test_log_entry_persistence() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = PersistentLogConfig {
            database_path: db_path,
            ..Default::default()
        };

        let logger = SqliteTransactionLogger::new(config).await.unwrap();

        let transaction_id =
            TransactionId::new(NodeId::new("test".to_string(), "region".to_string()));
        let entry = TransactionLogEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            transaction_id: transaction_id.clone(),
            event: TransactionEvent::Started,
            details: Some(serde_json::json!({"test": "data"})),
        };

        logger.write_entry(&entry).await.unwrap();
        logger.flush_buffer().await.unwrap();

        let retrieved_entries = logger.get_transaction_log(&transaction_id).await.unwrap();
        assert_eq!(retrieved_entries.len(), 1);
        assert_eq!(retrieved_entries[0].transaction_id.id, transaction_id.id);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = PersistentLogConfig {
            database_path: db_path,
            batch_size: 2,
            ..Default::default()
        };

        let logger = SqliteTransactionLogger::new(config).await.unwrap();

        let transaction_id =
            TransactionId::new(NodeId::new("test".to_string(), "region".to_string()));
        let entries: Vec<_> = (0..5)
            .map(|i| TransactionLogEntry {
                timestamp: chrono::Utc::now().timestamp_millis() + i,
                transaction_id: transaction_id.clone(),
                event: if i % 2 == 0 {
                    TransactionEvent::Started
                } else {
                    TransactionEvent::Committed
                },
                details: Some(serde_json::json!({"batch_index": i})),
            })
            .collect();

        logger.write_batch(&entries).await.unwrap();

        let retrieved_entries = logger.get_transaction_log(&transaction_id).await.unwrap();
        assert_eq!(retrieved_entries.len(), 5);
    }
}
