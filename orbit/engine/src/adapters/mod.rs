//! Protocol Adapters
//!
//! This module provides adapters that bridge protocol-specific requests to the unified
//! orbit-engine storage, transaction, and query APIs.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │              Protocol Layer (PostgreSQL, RESP, etc.)        │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Protocol Adapters                         │
//! │  ┌────────────┐  ┌────────────┐  ┌────────────┐            │
//! │  │ PostgreSQL │  │   RESP     │  │  OrbitQL   │  ...       │
//! │  │  Adapter   │  │  Adapter   │  │  Adapter   │            │
//! │  └────────────┘  └────────────┘  └────────────┘            │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Orbit Engine Core                         │
//! │  Storage | Transactions | Clustering | Query Execution      │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Concepts
//!
//! - **Type Mapping**: Convert protocol-specific types to engine types
//! - **Command Translation**: Translate protocol commands to engine operations
//! - **Error Mapping**: Convert engine errors to protocol-specific errors
//! - **Transaction Bridging**: Map protocol transaction semantics to MVCC
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use orbit_engine::adapters::{ProtocolAdapter, AdapterContext};
//! use orbit_engine::storage::HybridStorageManager;
//!
//! // Create engine instance
//! let storage = HybridStorageManager::new(/* ... */);
//!
//! // Create adapter context
//! let context = AdapterContext::new(storage);
//!
//! // Use protocol-specific adapter
//! // let pg_adapter = PostgresAdapter::new(context);
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use crate::error::{EngineError, EngineResult};
use crate::storage::{Row, SqlValue, TableSchema};
use crate::transaction::{IsolationLevel, TransactionId};

pub mod postgres;
pub mod redis;

// Re-exports
pub use postgres::PostgresAdapter;
pub use redis::RedisAdapter;

/// Protocol adapter context containing shared engine components
#[derive(Clone)]
pub struct AdapterContext {
    /// Storage engine reference
    pub storage: Arc<dyn crate::storage::TableStorage>,
    /// Transaction manager reference (optional)
    pub transaction_manager: Option<Arc<dyn crate::transaction::TransactionManager>>,
}

impl AdapterContext {
    /// Create a new adapter context with storage only
    pub fn new(storage: Arc<dyn crate::storage::TableStorage>) -> Self {
        Self {
            storage,
            transaction_manager: None,
        }
    }

    /// Create a new adapter context with storage and transaction manager
    pub fn with_transactions(
        storage: Arc<dyn crate::storage::TableStorage>,
        transaction_manager: Arc<dyn crate::transaction::TransactionManager>,
    ) -> Self {
        Self {
            storage,
            transaction_manager: Some(transaction_manager),
        }
    }
}

/// Base trait for protocol adapters
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// Protocol name (e.g., "PostgreSQL", "RESP", "OrbitQL")
    fn protocol_name(&self) -> &'static str;

    /// Initialize the adapter
    async fn initialize(&mut self) -> EngineResult<()>;

    /// Shutdown the adapter gracefully
    async fn shutdown(&mut self) -> EngineResult<()>;
}

/// Command execution result
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Rows returned from query
    Rows(Vec<Row>),
    /// Number of rows affected by write operation
    RowsAffected(usize),
    /// Simple OK response
    Ok,
    /// Error response
    Error(String),
    /// Custom response data
    Custom(Vec<u8>),
}

/// Type conversion helpers for adapters
pub mod type_conversion {
    use super::*;

    /// Convert a protocol-specific value to SqlValue
    pub trait ToSqlValue {
        /// Convert to SqlValue
        fn to_sql_value(&self) -> EngineResult<SqlValue>;
    }

    /// Convert SqlValue to protocol-specific value
    pub trait FromSqlValue: Sized {
        /// Convert from SqlValue
        fn from_sql_value(value: &SqlValue) -> EngineResult<Self>;
    }
}

/// Transaction adapter for mapping protocol transaction semantics to engine MVCC
pub struct TransactionAdapter {
    context: AdapterContext,
    active_transactions: std::collections::HashMap<String, TransactionId>,
}

impl TransactionAdapter {
    /// Create a new transaction adapter
    pub fn new(context: AdapterContext) -> Self {
        Self {
            context,
            active_transactions: std::collections::HashMap::new(),
        }
    }

    /// Begin a new transaction with protocol-specific ID
    pub async fn begin(
        &mut self,
        protocol_tx_id: String,
        isolation: IsolationLevel,
    ) -> EngineResult<TransactionId> {
        if let Some(tx_manager) = &self.context.transaction_manager {
            let tx_id = tx_manager.begin(isolation).await?;
            self.active_transactions.insert(protocol_tx_id, tx_id);
            Ok(tx_id)
        } else {
            Err(EngineError::not_implemented(
                "Transaction manager not available",
            ))
        }
    }

    /// Commit a transaction
    pub async fn commit(&mut self, protocol_tx_id: &str) -> EngineResult<()> {
        if let Some(tx_id) = self.active_transactions.remove(protocol_tx_id) {
            if let Some(tx_manager) = &self.context.transaction_manager {
                tx_manager.commit(tx_id).await?;
                Ok(())
            } else {
                Err(EngineError::not_implemented(
                    "Transaction manager not available",
                ))
            }
        } else {
            Err(EngineError::not_found(format!(
                "Transaction '{}' not found",
                protocol_tx_id
            )))
        }
    }

    /// Rollback a transaction
    pub async fn rollback(&mut self, protocol_tx_id: &str) -> EngineResult<()> {
        if let Some(tx_id) = self.active_transactions.remove(protocol_tx_id) {
            if let Some(tx_manager) = &self.context.transaction_manager {
                tx_manager.rollback(tx_id).await?;
                Ok(())
            } else {
                Err(EngineError::not_implemented(
                    "Transaction manager not available",
                ))
            }
        } else {
            Err(EngineError::not_found(format!(
                "Transaction '{}' not found",
                protocol_tx_id
            )))
        }
    }

    /// Get engine transaction ID from protocol transaction ID
    pub fn get_transaction_id(&self, protocol_tx_id: &str) -> Option<TransactionId> {
        self.active_transactions.get(protocol_tx_id).copied()
    }
}

/// Error mapping helpers
pub mod error_mapping {
    use super::*;

    /// Map engine error to protocol-specific error code/message
    pub trait ErrorMapper {
        /// Map to protocol error code
        fn to_error_code(error: &EngineError) -> String;

        /// Map to protocol error message
        fn to_error_message(error: &EngineError) -> String {
            error.to_string()
        }
    }
}
