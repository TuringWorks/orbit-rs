//! PostgreSQL Wire Protocol adapter for Orbit
//!
//! This module implements the PostgreSQL wire protocol, allowing psql and other
//! PostgreSQL clients to query Orbit actor state using SQL-like syntax.
//!
//! ## Supported Operations
//!
//! ### Actor Queries
//! ```sql
//! SELECT * FROM actors WHERE actor_id = 'user:123';
//! SELECT state FROM actors WHERE actor_type = 'UserActor';
//! ```
//!
//! ### Actor State Updates
//! ```sql
//! UPDATE actors SET state = '{"balance": 1000}' WHERE actor_id = 'account:456';
//! ```
//!
//! ### Actor Creation
//! ```sql
//! INSERT INTO actors (actor_id, actor_type, state) VALUES ('user:789', 'UserActor', '{}');
//! ```
//!
//! ## Features
//! - ✅ Startup message handling
//! - ✅ Trust authentication (no password)
//! - ✅ MD5 password authentication
//! - ✅ SCRAM-SHA-256 authentication
//! - ✅ Simple query protocol
//! - ✅ Extended query protocol (Parse, Bind, Execute)
//! - ✅ Result set encoding (DataRow, RowDescription)
//! - ✅ Prepared statements
//! - ✅ SQL parsing (SELECT, INSERT, UPDATE, DELETE)

pub mod auth;
pub mod graphrag_engine;
pub mod jsonb;
pub mod messages;
#[cfg(feature = "storage-rocksdb")]
pub mod persistent_storage;
#[cfg(feature = "storage-rocksdb")]
pub mod protocol;
#[cfg(feature = "storage-rocksdb")]
pub mod query_engine;
// pub mod server;  // Moved to orbit_server::protocols
pub mod sql;
#[cfg(feature = "storage-rocksdb")]
pub mod vector_engine;

// Re-export storage from common for backward compatibility
pub use crate::common::storage;

pub use auth::{compute_md5_hash, AuthManager, AuthMethod, ScramAuth, UserCredentials, UserStore};
pub use graphrag_engine::GraphRAGQueryEngine;
pub use messages::{BackendMessage, FieldDescription, FrontendMessage, TransactionStatus};
#[cfg(feature = "storage-rocksdb")]
pub use persistent_storage::{
    ColumnDefinition, ColumnType, PersistentTableStorage, QueryCondition, RocksDbTableStorage,
    TableRow, TableSchema,
};
#[cfg(feature = "storage-rocksdb")]
pub use protocol::PostgresWireProtocol;
#[cfg(feature = "storage-rocksdb")]
pub use query_engine::{QueryEngine, QueryResult};
// pub use server::PostgresServer;  // Moved to orbit_server::protocols
pub use sql::{SqlEngine, SqlExecutor, SqlParser};
#[cfg(feature = "storage-rocksdb")]
pub use vector_engine::VectorQueryEngine;
