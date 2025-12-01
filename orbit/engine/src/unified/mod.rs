//! Unified Cross-Protocol Storage Layer
//!
//! This module provides a unified storage layer that enables true cross-protocol
//! data sharing in Orbit-RS. Data written via one protocol (Redis, PostgreSQL,
//! MySQL, CQL, Cypher, AQL, REST, gRPC) is immediately accessible through all
//! other protocols.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Protocol Layer                              │
//! ├─────────┬─────────┬─────────┬─────────┬─────────┬──────────────┤
//! │  Redis  │PostgreSQL│  MySQL  │   CQL   │ Cypher  │  AQL/REST    │
//! │ Adapter │ Adapter  │ Adapter │ Adapter │ Adapter │  Adapter     │
//! ├─────────┴─────────┴─────────┴─────────┴─────────┴──────────────┤
//! │                    Schema Registry                               │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                 Unified Query Engine                             │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                   Unified Storage                                │
//! │         (Single RocksDB + Tiered Storage Backend)                │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Components
//!
//! - [`UniversalValue`]: A canonical data type that all protocols map to/from
//! - [`UniversalOperation`]: Protocol-agnostic operations (CRUD, queries, etc.)
//! - [`UnifiedStorage`]: Single storage backend shared by all protocols
//! - [`SchemaRegistry`]: Manages namespace schemas and cross-protocol projections
//! - Protocol Adapters: Translate between protocol-specific and universal formats
//!
//! # Example
//!
//! ```rust,ignore
//! use orbit_engine::unified::{UnifiedStorage, UniversalValue};
//!
//! // Create unified storage (shared by all protocols)
//! let storage = UnifiedStorage::new("./data").await?;
//!
//! // Write via Redis adapter
//! let record = UniversalRecord::new(
//!     "users",
//!     "alice",
//!     UniversalValue::Map(/* ... */),
//!     "redis",
//! );
//! storage.put(record).await?;
//!
//! // Read via PostgreSQL adapter - same data!
//! let record = storage.get("users", "alice").await?;
//! ```
//!
//! # Cross-Protocol Data Flow
//!
//! When data is written via Redis:
//! 1. `HSET user:alice name "Alice" email "alice@example.com"`
//! 2. RedisAdapter translates to `UniversalOperation::Put { namespace: "user", key: "alice", ... }`
//! 3. UnifiedStorage stores the record
//!
//! When queried via PostgreSQL:
//! 1. `SELECT * FROM user WHERE id = 'alice'`
//! 2. PostgresAdapter translates to `UniversalOperation::Get { namespace: "user", key: "alice" }`
//! 3. UnifiedStorage returns the same record
//! 4. PostgresAdapter formats as SQL result set

pub mod operations;
pub mod storage;
pub mod types;

// Re-export commonly used types
pub use operations::{
    AggregateOp, FieldDefinition, FieldType, FilterExpression, GraphPattern, IndexDefinition,
    IndexType, IsolationLevel, NamespaceSchema, NodePattern, RelationshipDirection,
    RelationshipPattern, SortOrder, UniversalOperation,
};
pub use storage::{
    MemoryBackend, UnifiedStorage, UnifiedStorageBackend, UnifiedStorageConfig,
    UnifiedStorageError, UnifiedStorageMetrics, UnifiedStorageResult,
};
pub use types::{RecordId, RecordMetadata, UniversalRecord, UniversalResult, UniversalValue};

// Future modules (to be implemented)
// pub mod schema;     // SchemaRegistry
// pub mod backend;    // RocksDB backend
// pub mod index;      // Index management
// pub mod transaction; // Transaction management
// pub mod cache;      // Caching layer
// pub mod adapters;   // Protocol adapters
