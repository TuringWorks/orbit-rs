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
pub mod hybrid;
pub mod simd;
pub mod vectorized;

#[cfg(feature = "iceberg-cold")]
pub mod iceberg_cold;

#[cfg(feature = "iceberg-cold")]
pub mod storage_config;

// Parallel execution will be added last
// pub mod parallel;

pub use columnar::{Column, ColumnBatch, ColumnBatchBuilder, NullBitmap, DEFAULT_BATCH_SIZE};
pub use hybrid::{
    AccessPattern, ColumnSchema, FilterPredicate, HybridStorageConfig, HybridStorageManager,
    MigrationStats, QueryResult, Row, RowBasedStore, StorageTier, TimeRange, WorkloadType,
};
pub use simd::{simd_capability, SimdAggregate, SimdCapability, SimdFilter};
pub use vectorized::{
    AggregateFunction, ComparisonOp, PlanNodeType, VectorizedExecutor, VectorizedExecutorConfig,
    VectorizedExecutorConfigBuilder,
};

#[cfg(feature = "iceberg-cold")]
pub use iceberg_cold::{
    arrow_to_column_batch, column_batch_to_arrow, create_file_io_for_storage,
    create_rest_catalog_with_storage, IcebergColdStore,
};

#[cfg(feature = "iceberg-cold")]
pub use storage_config::{AzureConfig, S3Config, StorageBackend};
