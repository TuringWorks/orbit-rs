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
//! │                      Protocol Layer                             │
//! ├─────────┬────--────┬─────────┬─────────┬─────────┬──────────────┤
//! │  Redis  │PostgreSQL│  MySQL  │   CQL   │ Cypher  │  AQL/REST    │
//! │ Adapter │ Adapter  │ Adapter │ Adapter │ Adapter │  Adapter     │
//! ├─────────┴──────-───┴─────────┴─────────┴─────────┴──────────────┤
//! │                    Schema Registry                              │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                 Unified Query Engine                            │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                   Unified Storage                               │
//! │         (Single RocksDB + Tiered Storage Backend)               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Components
//!
//! - [`types::UniversalValue`]: A canonical data type that all protocols map to/from
//! - [`operations::UniversalOperation`]: Protocol-agnostic operations (CRUD, queries, etc.)
//! - [`storage::UnifiedStorage`]: Single storage backend shared by all protocols
//! - [`schema::SchemaRegistry`]: Manages namespace schemas and cross-protocol projections
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

pub mod actor_tier_placement;
pub mod adapters;
pub mod index;
pub mod operations;
pub mod s3_backend;
pub mod schema;
pub mod storage;
pub mod tiered;
pub mod types;

// Re-export commonly used types
pub use actor_tier_placement::{
    ActorTierPlacement, ActorTierPlacementBuilder, ActorTierPlacementConfig, ActorTierStats,
    ActorType, TierRecommendation,
};
pub use adapters::{
    AdapterFactory, BaseAdapter, CqlAdapter, GraphAdapter, ProtocolAdapter, RedisAdapter,
    RestAdapter, SqlAdapter,
};
pub use operations::{
    AggregateOp, FieldDefinition, FieldType, FilterExpression, GraphPattern, IndexDefinition,
    IndexType, IsolationLevel, NamespaceSchema, NodePattern, RelationshipDirection,
    RelationshipPattern, SortOrder, UniversalOperation,
};
pub use schema::{Protocol, ProtocolProjection, SchemaRegistry, SchemaVersion};
pub use storage::{
    MemoryBackend, UnifiedStorage, UnifiedStorageBackend, UnifiedStorageConfig,
    UnifiedStorageError, UnifiedStorageMetrics, UnifiedStorageResult,
};
pub use tiered::{
    ColdBackendType, ColdDataFormat, ColdTierConfig, EvictionPolicy, HotTierConfig, StorageTier,
    TierMigrationConfig, TieredStorageBackend, TieredStorageConfig, TieredStorageMetrics,
    WarmTierConfig, WritePolicy,
};
pub use types::{RecordId, RecordMetadata, UniversalRecord, UniversalResult, UniversalValue};

// Re-export index types
pub use index::{IndexEntry, IndexStats, SecondaryIndexManager};

// Re-export S3 backend
pub use s3_backend::{S3Backend, S3BackendConfig};

// Future modules (to be implemented)
// pub mod backend;     // RocksDB persistent backend
// pub mod transaction; // Distributed transaction coordination
// pub mod cache;       // Query result caching
