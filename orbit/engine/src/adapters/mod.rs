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
//! - **Time Travel**: Query historical data using Iceberg snapshots (all protocols)
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use orbit_engine::adapters::{ProtocolAdapter, AdapterContext};
//! use orbit_engine::storage::{HybridStorageManager, HybridStorageConfig, ColumnSchema};
//! use std::sync::Arc;
//!
//! // Create engine instance with proper configuration
//! let table_name = "adapter_table".to_string();
//! let schema = vec![]; // Define your schema
//! let config = HybridStorageConfig::default();
//! let storage = Arc::new(HybridStorageManager::new(table_name, schema, config));
//!
//! // Create adapter context
//! let context = AdapterContext::new(storage);
//!
//! // Use protocol-specific adapter
//! // let pg_adapter = PostgresAdapter::new(context);
//! ```
//!
//! ## Time Travel Support
//!
//! All protocol adapters support querying historical data through Apache Iceberg's snapshot-based
//! time travel capabilities. This enables auditing, debugging, and historical analysis across all
//! supported protocols.
//!
//! ### Supported Syntax by Protocol
//!
//! #### PostgreSQL (SQL)
//!
//! ```sql
//! -- Time travel by timestamp (ISO 8601)
//! SELECT * FROM table TIMESTAMP AS OF '2023-04-11T18:06:36.289+00:00';
//!
//! -- Time travel by UNIX timestamp
//! SELECT * FROM table TIMESTAMP AS OF 1681236397;
//!
//! -- Time travel by snapshot ID
//! SELECT * FROM table VERSION AS OF 2583872980615177898;
//!
//! -- Time travel by named tag or branch
//! SELECT * FROM table VERSION AS OF 'sample_tag';
//! SELECT * FROM table VERSION AS OF 'main';
//!
//! -- View table history
//! SELECT * FROM table.history;
//!
//! -- View snapshots
//! SELECT * FROM table.snapshots;
//!
//! -- View references (branches and tags)
//! SELECT * FROM table.refs;
//!
//! -- Create tag for current snapshot
//! ALTER TABLE table CREATE TAG sample_tag;
//!
//! -- Create branch
//! ALTER TABLE table CREATE BRANCH feature_branch;
//! ```
//!
//! #### OrbitQL
//!
//! ```orbitql
//! -- Time travel in OrbitQL queries
//! SELECT * FROM users AS OF TIMESTAMP '2023-04-11T18:06:36.289Z'
//! WHERE age > 18;
//!
//! -- Version-based time travel
//! SELECT * FROM users AS OF VERSION 2583872980615177898;
//!
//! -- Named reference time travel
//! SELECT * FROM users AS OF 'production_snapshot';
//! ```
//!
//! #### REST API
//!
//! ```http
//! GET /tables/users/rows?as_of_timestamp=2023-04-11T18:06:36.289Z
//! GET /tables/users/rows?as_of_version=2583872980615177898
//! GET /tables/users/rows?as_of_tag=sample_tag
//!
//! GET /tables/users/history
//! GET /tables/users/snapshots
//! GET /tables/users/refs
//!
//! POST /tables/users/tags
//! {
//!   "tag_name": "sample_tag",
//!   "snapshot_id": null  // null = current snapshot
//! }
//! ```
//!
//! #### Redis (RESP)
//!
//! ```redis
//! # Time travel for Redis keys (metadata stored in Iceberg)
//! HGET users:123@2023-04-11T18:06:36 name
//! GET session:abc@version:2583872980615177898
//! LRANGE events@tag:snapshot_v1 0 -1
//! ```
//!
//! ### Implementation Status
//!
//! **Current Status**: Time travel API is defined and documented across all adapters.
//!
//! **Limitation**: The iceberg-rust crate currently does not expose methods for:
//! - Listing/accessing historical snapshots
//! - Querying snapshot metadata (history, refs tables)
//! - Creating tags and branches programmatically
//!
//! **Fallback Behavior**: Time travel queries currently return the latest snapshot (current data)
//! until the upstream Iceberg API provides snapshot access.
//!
//! **Future Implementation**: When the iceberg-rust API adds:
//! - `TableMetadata::snapshots()` - List all snapshots
//! - `TableMetadata::snapshot_by_timestamp(i64)` - Get snapshot at timestamp
//! - `TableMetadata::snapshot_by_id(i64)` - Get specific snapshot
//! - Tag/branch management methods
//!
//! Then time travel will work as documented above with full historical query support.
//!
//! ### Benefits of Time Travel
//!
//! - **Auditing**: Track data changes over time across all protocols
//! - **Debugging**: Investigate state at specific points in time
//! - **Compliance**: Meet regulatory requirements for data retention
//! - **Recovery**: Recover from accidental data modifications
//! - **Analysis**: Perform historical trend analysis
//! - **Testing**: Compare current vs historical data states
//! - **Reproducibility**: Recreate exact query results from the past

use async_trait::async_trait;
use std::sync::Arc;

use crate::error::{EngineError, EngineResult};
use crate::storage::{Row, SqlValue};
use crate::transaction::{IsolationLevel, TransactionId};

pub mod postgres;
pub mod redis;
pub mod rest;
pub mod orbitql;

// Re-exports
pub use postgres::PostgresAdapter;
pub use redis::RedisAdapter;
pub use rest::RestAdapter;
pub use orbitql::OrbitQLAdapter;

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
