//! Orbit Benchmarks
//!
//! This crate contains performance benchmarks for the Orbit distributed database.
//! It includes benchmarks for:
//! - Actor system performance
//! - Leader election algorithms
//! - Persistence layer performance
//! - Heterogeneous compute acceleration
//! - Transaction and batch processing performance
//! - Various storage backends
//!
//! This crate has been excluded from the main workspace to prevent
//! accidental execution during regular builds. All benchmarks should
//! be run manually when performance analysis is needed.

/// Persistence layer benchmarks and utilities
pub mod persistence;

/// Heterogeneous compute benchmarks (moved from orbit-compute)
pub mod compute;

/// Performance benchmarks for transactions and batch processing (moved from orbit-shared)
pub mod performance;

// Re-export commonly used items
pub use persistence::{config::*, metrics::*, workload::*};
pub use compute::{
    quick_compute_benchmark, ComputeBenchmarkReport, ComputeBenchmarkResult, ComputeBenchmarkSuite,
};
pub use performance::{
    quick_performance_benchmark, PerformanceBenchmarkResult, PerformanceBenchmarkSuite,
};
