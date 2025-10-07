//! Orbit Benchmarks
//!
//! This crate contains performance benchmarks for the Orbit distributed database.
//! It includes benchmarks for:
//! - Actor system performance
//! - Leader election algorithms
//! - Persistence layer performance
//! - Various storage backends

/// Persistence layer benchmarks and utilities
pub mod persistence;

// Re-export commonly used items
pub use persistence::{
    config::*,
    metrics::*,
    workload::*,
};
