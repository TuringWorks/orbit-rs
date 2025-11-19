//! Storage layer abstractions and implementations
//!
//! This module provides the core storage engine with tiered storage (hot/warm/cold),
//! multi-cloud backends (S3, Azure), and columnar format for analytics.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;

use crate::error::EngineResult;
use crate::metrics::StorageMetrics;

// Module declarations
pub mod config;
pub mod columnar;
pub mod hybrid;
pub mod iceberg;
// TODO: memory.rs needs to be decoupled from PostgreSQL protocol types
// pub mod memory;

// Re-exports
pub use config::{AzureConfig, S3Config, StorageBackend};
pub use columnar::{Column, ColumnBatch, ColumnBatchBuilder, NullBitmap, DEFAULT_BATCH_SIZE};
pub use hybrid::{HybridStorageManager, HybridStorageConfig};
pub use iceberg::IcebergColdStore;
// pub use memory::MemoryTableStorage;

/// Storage tier for data lifecycle management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageTier {
    /// Hot tier - last 24-48 hours, row-based storage optimized for OLTP
    Hot,
    /// Warm tier - 2-30 days, hybrid format for mixed workloads
    Warm,
    /// Cold tier - >30 days, columnar storage (Iceberg) for analytics
    Cold,
}

impl fmt::Display for StorageTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageTier::Hot => write!(f, "hot"),
            StorageTier::Warm => write!(f, "warm"),
            StorageTier::Cold => write!(f, "cold"),
        }
    }
}

/// Primary key type for row identification
pub type PrimaryKey = Vec<u8>;

/// SQL value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlValue {
    /// Null value
    Null,
    /// Boolean
    Boolean(bool),
    /// 16-bit integer (SMALLINT in PostgreSQL)
    Int16(i16),
    /// 32-bit integer (INTEGER in PostgreSQL)
    Int32(i32),
    /// 64-bit integer (BIGINT in PostgreSQL)
    Int64(i64),
    /// 32-bit float (REAL in PostgreSQL)
    Float32(f32),
    /// 64-bit float (DOUBLE PRECISION in PostgreSQL)
    Float64(f64),
    /// String/text (TEXT/VARCHAR in PostgreSQL)
    String(String),
    /// Variable-length character string (VARCHAR)
    Varchar(String),
    /// Fixed-length character string (CHAR)
    Char(String),
    /// Decimal/numeric with arbitrary precision
    Decimal(String),
    /// Binary data (BYTEA in PostgreSQL)
    Binary(Vec<u8>),
    /// Timestamp
    Timestamp(SystemTime),
}

impl SqlValue {
    /// PostgreSQL-compatible alias for Int16
    pub fn small_int(val: i16) -> Self {
        Self::Int16(val)
    }

    /// PostgreSQL-compatible alias for Int32
    pub fn integer(val: i32) -> Self {
        Self::Int32(val)
    }

    /// PostgreSQL-compatible alias for Int64
    pub fn big_int(val: i64) -> Self {
        Self::Int64(val)
    }

    /// PostgreSQL-compatible alias for Float32
    pub fn real(val: f32) -> Self {
        Self::Float32(val)
    }

    /// PostgreSQL-compatible alias for Float64
    pub fn double_precision(val: f64) -> Self {
        Self::Float64(val)
    }

    /// PostgreSQL-compatible alias for String
    pub fn text(val: String) -> Self {
        Self::String(val)
    }

    /// PostgreSQL-compatible alias for Binary
    pub fn bytea(val: Vec<u8>) -> Self {
        Self::Binary(val)
    }

    /// PostgreSQL-compatible alias for Varchar
    pub fn varchar(val: String) -> Self {
        Self::Varchar(val)
    }

    /// PostgreSQL-compatible alias for Char
    pub fn char(val: String) -> Self {
        Self::Char(val)
    }

    /// PostgreSQL-compatible alias for Decimal
    pub fn decimal(val: String) -> Self {
        Self::Decimal(val)
    }
}

/// Row data structure
pub type Row = HashMap<String, SqlValue>;

/// Table schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    /// Table name
    pub name: String,
    /// Column definitions
    pub columns: Vec<ColumnDef>,
    /// Primary key column names
    pub primary_key: Vec<String>,
}

/// Column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: DataType,
    /// Whether column is nullable
    pub nullable: bool,
}

/// Data types for schema
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Boolean
    Boolean,
    /// 32-bit integer
    Int32,
    /// 64-bit integer
    Int64,
    /// 32-bit float
    Float32,
    /// 64-bit float
    Float64,
    /// String
    String,
    /// Binary
    Binary,
    /// Timestamp
    Timestamp,
}

/// Time range for temporal queries
#[derive(Debug, Clone)]
pub struct TimeRange {
    /// Start time (inclusive)
    pub start: Option<SystemTime>,
    /// End time (exclusive)
    pub end: Option<SystemTime>,
}

/// Filter predicate for queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterPredicate {
    /// Equality comparison
    Eq(String, SqlValue),
    /// Not equal
    Ne(String, SqlValue),
    /// Less than
    Lt(String, SqlValue),
    /// Less than or equal
    Le(String, SqlValue),
    /// Greater than
    Gt(String, SqlValue),
    /// Greater than or equal
    Ge(String, SqlValue),
    /// Logical AND
    And(Vec<FilterPredicate>),
    /// Logical OR
    Or(Vec<FilterPredicate>),
    /// Logical NOT
    Not(Box<FilterPredicate>),
}

/// Access pattern for query optimization
#[derive(Debug, Clone)]
pub enum AccessPattern {
    /// Point lookup by primary key
    PointLookup {
        /// Primary key value
        key: PrimaryKey,
    },
    /// Scan with optional filter
    Scan {
        /// Time range filter
        time_range: Option<TimeRange>,
        /// Additional filter predicate
        filter: Option<FilterPredicate>,
    },
    /// Aggregation query
    Aggregation {
        /// Aggregation function
        function: AggregateFunction,
        /// Column to aggregate
        column: String,
        /// Optional filter
        filter: Option<FilterPredicate>,
    },
}

/// Aggregate functions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunction {
    /// Count
    Count,
    /// Sum
    Sum,
    /// Average
    Avg,
    /// Minimum
    Min,
    /// Maximum
    Max,
}

/// Query result
#[derive(Debug, Clone)]
pub enum QueryResult {
    /// Rows returned
    Rows(Vec<Row>),
    /// Column batch (for columnar queries)
    ColumnBatch(ColumnBatch),
    /// Aggregation result
    Aggregate(SqlValue),
    /// Empty result
    Empty,
}

/// Migration statistics for tier transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStats {
    /// Number of rows migrated
    pub rows_migrated: usize,
    /// Bytes migrated
    pub bytes_migrated: usize,
    /// Time taken
    pub duration_ms: u64,
    /// Source tier
    pub from_tier: StorageTier,
    /// Destination tier
    pub to_tier: StorageTier,
}

/// Base trait for all storage engines
#[async_trait]
pub trait StorageEngine: Send + Sync {
    /// Initialize the storage engine
    async fn initialize(&self) -> EngineResult<()>;

    /// Shutdown the storage engine gracefully
    async fn shutdown(&self) -> EngineResult<()>;

    /// Get current metrics
    async fn metrics(&self) -> StorageMetrics;

    /// Health check
    async fn health_check(&self) -> EngineResult<bool>;
}

/// Table-level storage operations
#[async_trait]
pub trait TableStorage: StorageEngine {
    /// Create a new table
    async fn create_table(&self, schema: TableSchema) -> EngineResult<()>;

    /// Drop a table
    async fn drop_table(&self, table_name: &str) -> EngineResult<()>;

    /// Check if table exists
    async fn table_exists(&self, table_name: &str) -> EngineResult<bool>;

    /// Get table schema
    async fn get_schema(&self, table_name: &str) -> EngineResult<TableSchema>;

    /// Insert a row
    async fn insert_row(&self, table_name: &str, row: Row) -> EngineResult<()>;

    /// Insert multiple rows (batch operation)
    async fn insert_rows(&self, table_name: &str, rows: Vec<Row>) -> EngineResult<()>;

    /// Query data
    async fn query(
        &self,
        table_name: &str,
        pattern: AccessPattern,
    ) -> EngineResult<QueryResult>;

    /// Update rows matching filter
    async fn update(
        &self,
        table_name: &str,
        filter: FilterPredicate,
        updates: HashMap<String, SqlValue>,
    ) -> EngineResult<usize>;

    /// Delete rows matching filter
    async fn delete(&self, table_name: &str, filter: FilterPredicate) -> EngineResult<usize>;
}

/// Tiered storage operations
#[async_trait]
pub trait TieredStorage: TableStorage {
    /// Manually migrate data between tiers
    async fn migrate_tier(
        &self,
        table_name: &str,
        from: StorageTier,
        to: StorageTier,
        filter: Option<FilterPredicate>,
    ) -> EngineResult<MigrationStats>;

    /// Query specific tier
    async fn query_tier(
        &self,
        table_name: &str,
        tier: StorageTier,
        pattern: AccessPattern,
    ) -> EngineResult<QueryResult>;

    /// Get tier statistics
    async fn tier_stats(&self, table_name: &str, tier: StorageTier) -> EngineResult<TierStats>;
}

/// Statistics for a storage tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierStats {
    /// Number of rows
    pub row_count: usize,
    /// Total size in bytes
    pub size_bytes: usize,
    /// Oldest timestamp
    pub oldest_timestamp: Option<SystemTime>,
    /// Newest timestamp
    pub newest_timestamp: Option<SystemTime>,
}
