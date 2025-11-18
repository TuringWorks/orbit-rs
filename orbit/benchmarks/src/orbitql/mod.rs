//! OrbitQL Benchmarks Module
//!
//! This module contains comprehensive benchmarks for OrbitQL, the unified
//! multi-model query language. It includes:
//! - TPC-H, TPC-C, TPC-DS benchmark implementations
//! - Custom workload generators
//! - Comprehensive system validation benchmarks
//! - Performance measurement and analysis tools

pub mod benchmark;
pub mod comprehensive_benchmark;

// Re-export key types
pub use benchmark::{
    BenchmarkConfig, BenchmarkFramework, BenchmarkResults, CustomWorkloadResults,
    QueryBenchmarkResult, TpcCResults, TpcDsResults, TpcHResults,
};
pub use comprehensive_benchmark::{
    ComprehensiveBenchmark, ComprehensiveBenchmarkConfig, OrbitQLComponents,
};
