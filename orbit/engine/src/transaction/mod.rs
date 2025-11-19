//! Transaction management with MVCC support
//!
//! Provides snapshot isolation, deadlock detection, and distributed transaction coordination.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

use crate::error::{EngineError, EngineResult};
use crate::metrics::TransactionMetrics;

// Module declarations
pub mod mvcc;

// Re-exports
pub use mvcc::MvccTransactionManager;

/// Transaction identifier
pub type TransactionId = Uuid;

/// Transaction isolation level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Read uncommitted
    ReadUncommitted,
    /// Read committed
    ReadCommitted,
    /// Repeatable read
    RepeatableRead,
    /// Serializable
    Serializable,
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    /// Active transaction
    Active,
    /// Preparing to commit
    Preparing,
    /// Committed
    Committed,
    /// Aborted/rolled back
    Aborted,
}

/// Transaction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction ID
    pub id: TransactionId,
    /// Start timestamp
    pub start_time: SystemTime,
    /// Isolation level
    pub isolation_level: IsolationLevel,
    /// Current state
    pub state: TransactionState,
}

/// Transaction manager trait
#[async_trait]
pub trait TransactionManager: Send + Sync {
    /// Begin a new transaction
    async fn begin(&self, isolation: IsolationLevel) -> EngineResult<TransactionId>;

    /// Commit a transaction
    async fn commit(&self, txid: TransactionId) -> EngineResult<()>;

    /// Rollback a transaction
    async fn rollback(&self, txid: TransactionId) -> EngineResult<()>;

    /// Get transaction state
    async fn get_state(&self, txid: TransactionId) -> EngineResult<TransactionState>;

    /// Check if transaction is active
    async fn is_active(&self, txid: TransactionId) -> EngineResult<bool>;

    /// Get transaction metrics
    async fn metrics(&self) -> TransactionMetrics;
}
