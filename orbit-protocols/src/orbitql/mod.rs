//! OrbitQL Protocol Adapter
//!
//! This module provides protocol-specific adaptations for OrbitQL, including custom
//! executors and protocol-specific query handling.
//!
//! The core OrbitQL implementation has been unified in orbit-shared.

// Re-export the unified OrbitQL from orbit-shared
pub use orbit_shared::orbitql::*;

// Protocol-specific extensions remain here
pub mod executor;

// Re-export protocol-specific types
pub use executor::{ExecutionContext, ExecutionResult, OrbitQLExecutor};
