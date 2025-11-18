//! Query Execution Framework for Phase 9
//!
//! This module provides vectorized and parallel query execution capabilities:
//! - Columnar data structures for cache-friendly processing
//! - SIMD-optimized operators for high performance
//! - Parallel query execution with work-stealing
//!
//! ## Module Structure
//!
//! - `columnar`: Columnar storage and batching
//! - `simd`: SIMD-optimized operators (filters, aggregates, joins)
//! - `vectorized`: Vectorized execution engine
//! - `parallel`: Parallel execution framework

pub mod columnar;
pub mod simd;
pub mod vectorized;
pub mod hybrid;

#[cfg(feature = "iceberg-cold")]
pub mod iceberg_cold;

// Parallel execution will be added last
// pub mod parallel;

pub use columnar::{Column, ColumnBatch, ColumnBatchBuilder, NullBitmap, DEFAULT_BATCH_SIZE};
pub use simd::{SimdCapability, SimdFilter, SimdAggregate, simd_capability};
pub use vectorized::{
    VectorizedExecutor, VectorizedExecutorConfig, VectorizedExecutorConfigBuilder,
    PlanNodeType, ComparisonOp, AggregateFunction,
};
pub use hybrid::{
    HybridStorageManager, HybridStorageConfig, StorageTier, WorkloadType,
    AccessPattern, TimeRange, FilterPredicate, QueryResult, MigrationStats,
    RowBasedStore, ColumnSchema, Row,
};

#[cfg(feature = "iceberg-cold")]
pub use iceberg_cold::{IcebergColdStore, column_batch_to_arrow, arrow_to_column_batch};
