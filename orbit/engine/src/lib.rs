//! Orbit Storage Engine
//!
//! Unified storage engine providing tiered storage (hot/warm/cold), MVCC transactions,
//! distributed clustering, and query execution for all Orbit protocols.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Protocol Layer                            │
//! │  (PostgreSQL, RESP, OrbitQL, AQL, Cypher, REST)             │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   Protocol Adapters                          │
//! │  (Convert protocol-specific requests to engine operations)   │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Orbit Engine Core                         │
//! │                                                              │
//! │  ┌────────────┐  ┌──────────────┐  ┌────────────────────┐  │
//! │  │  Storage   │  │ Transaction  │  │  Query Execution   │  │
//! │  │            │  │              │  │                    │  │
//! │  │  - Hot     │  │  - MVCC      │  │  - Vectorized     │  │
//! │  │  - Warm    │  │  - Snapshot  │  │  - SIMD           │  │
//! │  │  - Cold    │  │  - Deadlock  │  │  - Optimization   │  │
//! │  └────────────┘  └──────────────┘  └────────────────────┘  │
//! │                                                              │
//! │  ┌──────────────────────────────────────────────────────┐  │
//! │  │               Clustering & Replication                │  │
//! │  │                                                        │  │
//! │  │  - Raft Consensus    - Replication   - Recovery      │  │
//! │  └──────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   Storage Backends                           │
//! │  (RocksDB, S3, MinIO, Azure Blob, Memory)                   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Abstractions
//!
//! - **StorageEngine**: Base trait for all storage operations
//! - **TableStorage**: Table-level CRUD operations
//! - **TieredStorage**: Hot/warm/cold tier management with automatic migration
//! - **TransactionManager**: MVCC transaction lifecycle management
//! - **ClusterCoordinator**: Distributed system coordination via Raft
//! - **QueryExecutor**: Vectorized query execution with SIMD optimization
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use orbit_engine::{
//!     storage::{HybridStorageManager, StorageConfig, StorageBackend, S3Config},
//!     transaction::TransactionManager,
//! };
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Configure storage backend (S3, Azure, or Memory)
//!     let backend = StorageBackend::S3(S3Config::minio(
//!         "localhost:9000".to_string(),
//!         "minioadmin".to_string(),
//!         "minioadmin".to_string(),
//!         "warehouse".to_string(),
//!         false,
//!     ));
//!
//!     // Create storage engine with tiered storage
//!     let storage = HybridStorageManager::new(
//!         StorageConfig::default(),
//!         Some(backend),
//!     ).await?;
//!
//!     // Use storage engine
//!     // storage.insert(...).await?;
//!     // storage.query(...).await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod addressable;
pub mod cdc;
pub mod cluster;
pub mod error;
pub mod metrics;
pub mod query;
pub mod storage;
pub mod transaction;
pub mod transaction_log;
pub mod transactions;

// Re-export commonly used types
pub use error::{EngineError, EngineResult};
pub use storage::{StorageEngine, TableStorage, TieredStorage};
pub use transaction::TransactionManager;
pub use cluster::ClusterCoordinator;
pub use query::QueryExecutor;
