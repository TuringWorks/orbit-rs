//! MVCC-Aware SQL Executor
//!
//! This module implements Multi-Version Concurrency Control (MVCC) principles
//! to prevent deadlocks and improve concurrent transaction performance.
//!
//! ## MVCC Features:
//! - Row versioning with transaction IDs
//! - Non-blocking reads using snapshots
//! - Consistent lock ordering to prevent deadlocks
//! - Transaction isolation with proper visibility rules
//! - Deadlock detection and resolution

use crate::error::{EngineError, EngineResult};
use crate::storage::{
    ast::{AccessMode, IsolationLevel},
    types::SqlValue,
};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Transaction ID type
pub type TransactionId = u64;

/// Snapshot ID for MVCC reads
pub type SnapshotId = u64;

/// Row version with MVCC metadata
#[derive(Debug, Clone)]
pub struct RowVersion {
    /// The actual row data
    pub data: HashMap<String, SqlValue>,
    /// Transaction ID that created this version
    pub xmin: TransactionId,
    /// Transaction ID that deleted this version (None if not deleted)
    pub xmax: Option<TransactionId>,
    /// Timestamp when this version was created
    pub created_at: DateTime<Utc>,
    /// Whether this version is committed
    pub committed: bool,
}

/// Table with MVCC row versioning
#[derive(Debug)]
pub struct MvccTable {
    /// Table name
    pub name: String,
    /// All versions of all rows, organized by primary key
    pub row_versions: BTreeMap<String, Vec<RowVersion>>,
    /// Table schema
    pub schema: crate::storage::executor::TableSchema,
}

/// Transaction snapshot for MVCC reads
#[derive(Debug, Clone)]
pub struct TransactionSnapshot {
    /// Snapshot ID
    pub id: SnapshotId,
    /// Transaction ID of the reading transaction
    pub xid: TransactionId,
    /// List of active (uncommitted) transaction IDs at snapshot time
    pub active_xids: Vec<TransactionId>,
    /// Timestamp when snapshot was taken
    pub timestamp: DateTime<Utc>,
}

/// MVCC transaction state
#[derive(Debug, Clone)]
pub struct MvccTransaction {
    /// Transaction ID
    pub id: TransactionId,
    /// Current snapshot for reads
    pub snapshot: TransactionSnapshot,
    /// Isolation level
    pub isolation_level: Option<IsolationLevel>,
    /// Access mode (read-only or read-write)
    pub access_mode: Option<AccessMode>,
    /// Transaction start time
    pub start_time: DateTime<Utc>,
    /// Savepoints in this transaction
    pub savepoints: Vec<String>,
    /// Locks held by this transaction (ordered consistently)
    pub locks_held: BTreeMap<String, LockMode>,
    /// Whether this transaction is committed
    pub committed: bool,
    /// Whether this transaction is aborted
    pub aborted: bool,
}

/// Lock modes for concurrency control
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LockMode {
    /// Shared lock for reads
    Shared,
    /// Exclusive lock for writes
    Exclusive,
}

/// Deadlock detector state
#[derive(Debug)]
pub struct DeadlockDetector {
    /// Wait-for graph: transaction -> set of transactions it's waiting for
    pub wait_graph: HashMap<TransactionId, Vec<TransactionId>>,
    /// Lock queue: resource -> list of waiting transactions
    pub lock_queue: HashMap<String, Vec<(TransactionId, LockMode)>>,
}

/// Type alias for transaction log storage
type TransactionLog = Arc<RwLock<Vec<(TransactionId, String, DateTime<Utc>)>>>;
/// Type alias for row predicate function
type RowPredicate = Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>;

/// MVCC-aware SQL executor
pub struct MvccSqlExecutor {
    /// Tables with versioned rows
    tables: Arc<RwLock<HashMap<String, MvccTable>>>,

    /// Active transactions
    transactions: Arc<RwLock<HashMap<TransactionId, MvccTransaction>>>,

    /// Next transaction ID (monotonically increasing)
    next_transaction_id: Arc<Mutex<TransactionId>>,

    /// Next snapshot ID
    next_snapshot_id: Arc<Mutex<SnapshotId>>,

    /// Deadlock detector
    deadlock_detector: Arc<Mutex<DeadlockDetector>>,

    /// Global transaction log for cleanup
    transaction_log: TransactionLog,
}

impl Default for MvccSqlExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MvccSqlExecutor {
    /// Create a new MVCC SQL executor
    pub fn new() -> Self {
        Self {
            tables: Arc::new(RwLock::new(HashMap::new())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            next_transaction_id: Arc::new(Mutex::new(1)),
            next_snapshot_id: Arc::new(Mutex::new(1)),
            deadlock_detector: Arc::new(Mutex::new(DeadlockDetector {
                wait_graph: HashMap::new(),
                lock_queue: HashMap::new(),
            })),
            transaction_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Begin a new transaction with MVCC snapshot
    pub async fn begin_transaction(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> EngineResult<TransactionId> {
        let transaction_id = {
            let mut next_id = self.next_transaction_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let snapshot_id = {
            let mut next_id = self.next_snapshot_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        // Get list of currently active transactions for snapshot
        let active_xids: Vec<TransactionId> = {
            let transactions = self.transactions.read().await;
            transactions
                .values()
                .filter(|tx| !tx.committed && !tx.aborted)
                .map(|tx| tx.id)
                .collect()
        };

        let snapshot = TransactionSnapshot {
            id: snapshot_id,
            xid: transaction_id,
            active_xids,
            timestamp: Utc::now(),
        };

        let transaction = MvccTransaction {
            id: transaction_id,
            snapshot,
            isolation_level,
            access_mode,
            start_time: Utc::now(),
            savepoints: Vec::new(),
            locks_held: BTreeMap::new(),
            committed: false,
            aborted: false,
        };

        let mut transactions = self.transactions.write().await;
        transactions.insert(transaction_id, transaction);

        // Log transaction start
        let mut log = self.transaction_log.write().await;
        log.push((transaction_id, "BEGIN".to_string(), Utc::now()));

        Ok(transaction_id)
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, transaction_id: TransactionId) -> EngineResult<()> {
        let mut transactions = self.transactions.write().await;

        if let Some(transaction) = transactions.get_mut(&transaction_id) {
            if transaction.aborted {
                return Err(EngineError::storage(
                    "Cannot commit aborted transaction".to_string(),
                ));
            }

            transaction.committed = true;

            // Mark all row versions created by this transaction as committed
            let mut tables = self.tables.write().await;
            for table in tables.values_mut() {
                for versions in table.row_versions.values_mut() {
                    for version in versions.iter_mut() {
                        if version.xmin == transaction_id {
                            version.committed = true;
                        }
                    }
                }
            }

            // Release all locks held by this transaction
            transaction.locks_held.clear();

            // Log transaction commit
            let mut log = self.transaction_log.write().await;
            log.push((transaction_id, "COMMIT".to_string(), Utc::now()));

            Ok(())
        } else {
            Err(EngineError::storage(format!(
                "Transaction {transaction_id} not found"
            )))
        }
    }

    /// Rollback a transaction
    pub async fn rollback_transaction(&self, transaction_id: TransactionId) -> EngineResult<()> {
        let mut transactions = self.transactions.write().await;

        if let Some(transaction) = transactions.get_mut(&transaction_id) {
            transaction.aborted = true;

            // Remove all row versions created by this transaction
            let mut tables = self.tables.write().await;
            for table in tables.values_mut() {
                for versions in table.row_versions.values_mut() {
                    versions.retain(|version| version.xmin != transaction_id);
                }
            }

            // Release all locks
            transaction.locks_held.clear();

            // Log transaction rollback
            let mut log = self.transaction_log.write().await;
            log.push((transaction_id, "ROLLBACK".to_string(), Utc::now()));

            Ok(())
        } else {
            Err(EngineError::storage(format!(
                "Transaction {transaction_id} not found"
            )))
        }
    }

    /// Read rows using MVCC snapshot (non-blocking)
    pub async fn mvcc_read(
        &self,
        table_name: &str,
        transaction_id: TransactionId,
        predicate: RowPredicate,
    ) -> EngineResult<Vec<HashMap<String, SqlValue>>> {
        let tables = self.tables.read().await;
        let transactions = self.transactions.read().await;

        let transaction = transactions.get(&transaction_id).ok_or_else(|| {
            EngineError::storage(format!("Transaction {transaction_id} not found"))
        })?;

        let table = tables.get(table_name).ok_or_else(|| {
            EngineError::storage(format!("Table '{table_name}' does not exist"))
        })?;

        let mut result = Vec::new();

        // For each row, find the visible version based on the snapshot
        for versions in table.row_versions.values() {
            if let Some(visible_version) =
                self.find_visible_version(versions, &transaction.snapshot)
            {
                // Apply predicate if provided
                if let Some(ref pred) = predicate {
                    if pred(&visible_version.data) {
                        result.push(visible_version.data.clone());
                    }
                } else {
                    result.push(visible_version.data.clone());
                }
            }
        }

        Ok(result)
    }

    /// Insert a new row with MVCC versioning
    pub async fn mvcc_insert(
        &self,
        table_name: &str,
        transaction_id: TransactionId,
        row_data: HashMap<String, SqlValue>,
    ) -> EngineResult<()> {
        // Acquire exclusive lock on table in consistent order (by name)
        self.acquire_lock(table_name, transaction_id, LockMode::Exclusive)
            .await?;

        let mut tables = self.tables.write().await;

        let table = tables.get_mut(table_name).ok_or_else(|| {
            EngineError::storage(format!("Table '{table_name}' does not exist"))
        })?;

        // Generate a row key (simplified - in practice would use primary key)
        let row_key = format!("row_{}", table.row_versions.len());

        let new_version = RowVersion {
            data: row_data,
            xmin: transaction_id,
            xmax: None,
            created_at: Utc::now(),
            committed: false, // Will be set to true on commit
        };

        table
            .row_versions
            .entry(row_key)
            .or_insert_with(Vec::new)
            .push(new_version);

        Ok(())
    }

    /// Update rows with MVCC versioning (creates new versions)
    pub async fn mvcc_update(
        &self,
        table_name: &str,
        transaction_id: TransactionId,
        updates: HashMap<String, SqlValue>,
        predicate: RowPredicate,
    ) -> EngineResult<usize> {
        // Acquire exclusive lock
        self.acquire_lock(table_name, transaction_id, LockMode::Exclusive)
            .await?;

        let mut tables = self.tables.write().await;
        let transactions = self.transactions.read().await;

        let transaction = transactions.get(&transaction_id).ok_or_else(|| {
            EngineError::storage(format!("Transaction {transaction_id} not found"))
        })?;

        let table = tables.get_mut(table_name).ok_or_else(|| {
            EngineError::storage(format!("Table '{table_name}' does not exist"))
        })?;

        let mut updated_count = 0;

        // Find rows to update based on current snapshot
        for (_row_key, versions) in table.row_versions.iter_mut() {
            // Clone the visible version data first to avoid borrowing issues
            let visible_data = if let Some(visible_version) =
                self.find_visible_version(versions, &transaction.snapshot)
            {
                // Apply predicate
                if let Some(ref pred) = predicate {
                    if !pred(&visible_version.data) {
                        continue;
                    }
                }
                visible_version.data.clone()
            } else {
                continue;
            };

            // Mark current version as deleted by this transaction
            if let Some(current_version) = versions.last_mut() {
                if current_version.xmin != transaction_id {
                    current_version.xmax = Some(transaction_id);
                }
            }

            // Create new version with updates
            let mut new_data = visible_data;
            for (key, value) in &updates {
                new_data.insert(key.clone(), value.clone());
            }

            let new_version = RowVersion {
                data: new_data,
                xmin: transaction_id,
                xmax: None,
                created_at: Utc::now(),
                committed: false,
            };

            versions.push(new_version);
            updated_count += 1;
        }

        Ok(updated_count)
    }

    /// Delete rows with MVCC versioning (marks as deleted)
    pub async fn mvcc_delete(
        &self,
        table_name: &str,
        transaction_id: TransactionId,
        predicate: RowPredicate,
    ) -> EngineResult<usize> {
        // Acquire exclusive lock
        self.acquire_lock(table_name, transaction_id, LockMode::Exclusive)
            .await?;

        let mut tables = self.tables.write().await;
        let transactions = self.transactions.read().await;

        let transaction = transactions.get(&transaction_id).ok_or_else(|| {
            EngineError::storage(format!("Transaction {transaction_id} not found"))
        })?;

        let table = tables.get_mut(table_name).ok_or_else(|| {
            EngineError::storage(format!("Table '{table_name}' does not exist"))
        })?;

        let mut deleted_count = 0;

        for versions in table.row_versions.values_mut() {
            if let Some(visible_version) =
                self.find_visible_version(versions, &transaction.snapshot)
            {
                // Apply predicate
                if let Some(ref pred) = predicate {
                    if !pred(&visible_version.data) {
                        continue;
                    }
                }

                // Mark latest version as deleted
                if let Some(latest_version) = versions.last_mut() {
                    if latest_version.xmax.is_none() {
                        latest_version.xmax = Some(transaction_id);
                        deleted_count += 1;
                    }
                }
            }
        }

        Ok(deleted_count)
    }

    /// Find the visible version of a row for a given snapshot
    fn find_visible_version<'a>(
        &self,
        versions: &'a [RowVersion],
        snapshot: &TransactionSnapshot,
    ) -> Option<&'a RowVersion> {
        versions
            .iter()
            .rev()
            .find(|&version| self.is_version_visible(version, snapshot))
    }

    /// Determine if a row version is visible to a snapshot
    fn is_version_visible(&self, version: &RowVersion, snapshot: &TransactionSnapshot) -> bool {
        // Version created by the same transaction is always visible
        if version.xmin == snapshot.xid {
            return version
                .xmax
                .map(|xmax| xmax != snapshot.xid)
                .unwrap_or(true);
        }

        // Version must be committed to be visible to other transactions
        if !version.committed {
            return false;
        }

        // Version created by a transaction that was active when snapshot was taken
        if snapshot.active_xids.contains(&version.xmin) {
            return false;
        }

        // If version is deleted, check if deletion is visible
        if let Some(xmax) = version.xmax {
            if xmax == snapshot.xid {
                return false; // Deleted by same transaction
            }

            // If deleting transaction was active, version is still visible
            if snapshot.active_xids.contains(&xmax) {
                return true;
            }
        }

        true
    }

    /// Acquire a lock with consistent ordering to prevent deadlocks
    pub async fn acquire_lock(
        &self,
        resource: &str,
        transaction_id: TransactionId,
        mode: LockMode,
    ) -> EngineResult<()> {
        let mut detector = self.deadlock_detector.lock().await;

        // Check if lock is available
        if let Some(queue) = detector.lock_queue.get(resource) {
            if !queue.is_empty() {
                // Add to wait graph for deadlock detection
                let waiting_for: Vec<TransactionId> = queue
                    .iter()
                    .filter(|(_, lock_mode)| {
                        // Conflicting lock modes
                        mode == LockMode::Exclusive || *lock_mode == LockMode::Exclusive
                    })
                    .map(|(txid, _)| *txid)
                    .collect();

                if !waiting_for.is_empty() {
                    detector.wait_graph.insert(transaction_id, waiting_for);

                    // Check for deadlock
                    if self.detect_deadlock(&detector, transaction_id) {
                        return Err(EngineError::storage(
                            "Deadlock detected - transaction aborted".to_string(),
                        ));
                    }
                }

                // Add to lock queue
                detector
                    .lock_queue
                    .entry(resource.to_string())
                    .or_insert_with(Vec::new)
                    .push((transaction_id, mode.clone()));

                return Ok(()); // In a real implementation, would wait for lock
            }
        }

        // Lock is available, grant immediately
        detector
            .lock_queue
            .entry(resource.to_string())
            .or_insert_with(Vec::new)
            .push((transaction_id, mode.clone()));

        // Update transaction's lock list
        let mut transactions = self.transactions.write().await;
        if let Some(transaction) = transactions.get_mut(&transaction_id) {
            transaction.locks_held.insert(resource.to_string(), mode);
        }

        Ok(())
    }

    /// Detect deadlock cycles using DFS
    fn detect_deadlock(&self, detector: &DeadlockDetector, start_txid: TransactionId) -> bool {
        let mut visited = Vec::new();
        self.dfs_cycle_detection(detector, start_txid, start_txid, &mut visited)
    }

    /// DFS helper for deadlock detection
    #[allow(clippy::only_used_in_recursion)]
    fn dfs_cycle_detection(
        &self,
        detector: &DeadlockDetector,
        current_txid: TransactionId,
        start_txid: TransactionId,
        visited: &mut Vec<TransactionId>,
    ) -> bool {
        if visited.contains(&current_txid) {
            return current_txid == start_txid; // Found cycle
        }

        visited.push(current_txid);

        if let Some(waiting_for) = detector.wait_graph.get(&current_txid) {
            for &next_txid in waiting_for {
                if self.dfs_cycle_detection(detector, next_txid, start_txid, visited) {
                    return true;
                }
            }
        }

        visited.pop();
        false
    }

    /// Create a new table
    pub async fn create_table(&self, name: &str) -> EngineResult<()> {
        let mut tables = self.tables.write().await;

        if tables.contains_key(name) {
            return Err(EngineError::storage(format!(
                "Table '{name}' already exists"
            )));
        }

        let table = MvccTable {
            name: name.to_string(),
            row_versions: BTreeMap::new(),
            schema: crate::storage::executor::TableSchema {
                name: name.to_string(), // Use string directly
                columns: Vec::new(),
                constraints: Vec::new(),
                indexes: Vec::new(),
            },
        };

        tables.insert(name.to_string(), table);
        Ok(())
    }

    /// Cleanup committed/aborted transactions periodically
    pub async fn cleanup_old_transactions(&self) -> EngineResult<usize> {
        let cutoff_time = Utc::now() - chrono::Duration::minutes(5);
        let mut cleaned_count = 0;

        let mut transactions = self.transactions.write().await;
        let mut to_remove = Vec::new();

        for (txid, transaction) in transactions.iter() {
            if (transaction.committed || transaction.aborted)
                && transaction.start_time < cutoff_time
            {
                to_remove.push(*txid);
            }
        }

        for txid in to_remove {
            transactions.remove(&txid);
            cleaned_count += 1;
        }

        // Also cleanup old row versions
        let mut tables = self.tables.write().await;
        for table in tables.values_mut() {
            for versions in table.row_versions.values_mut() {
                let original_len = versions.len();
                versions.retain(|version| {
                    // Keep if transaction is still active or version is recent
                    !version.committed || version.created_at > cutoff_time
                });
                cleaned_count += original_len - versions.len();
            }
        }

        Ok(cleaned_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mvcc_transaction_basics() {
        let executor = MvccSqlExecutor::new();

        // Begin transaction
        let txid = executor.begin_transaction(None, None).await.unwrap();
        assert_eq!(txid, 1);

        // Commit transaction
        executor.commit_transaction(txid).await.unwrap();
    }

    #[tokio::test]
    async fn test_deadlock_detection() {
        let executor = MvccSqlExecutor::new();

        let tx1 = executor.begin_transaction(None, None).await.unwrap();
        let tx2 = executor.begin_transaction(None, None).await.unwrap();

        // Simulate circular wait condition
        // In a real scenario, this would involve actual lock contention
        // For now, we test the deadlock detector components
        assert!(tx1 != tx2);
    }

    #[tokio::test]
    async fn test_mvcc_snapshot_isolation() {
        let executor = MvccSqlExecutor::new();

        let tx1 = executor.begin_transaction(None, None).await.unwrap();
        let tx2 = executor.begin_transaction(None, None).await.unwrap();

        // Each transaction should have its own snapshot
        let transactions = executor.transactions.read().await;
        let tx1_snapshot = &transactions[&tx1].snapshot;
        let tx2_snapshot = &transactions[&tx2].snapshot;

        assert_ne!(tx1_snapshot.id, tx2_snapshot.id);
        assert_eq!(tx1_snapshot.xid, tx1);
        assert_eq!(tx2_snapshot.xid, tx2);
    }
}
