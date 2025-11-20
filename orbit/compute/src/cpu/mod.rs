//! CPU compute implementation with SIMD acceleration
//!
//! This module provides CPU-specific compute optimizations using SIMD instructions
//! across different architectures (x86_64, ARM).
//!
//! ## Supported SIMD Instruction Sets
//!
//! - **x86_64**: SSE2, AVX2, AVX-512
//! - **ARM**: NEON, SVE (Scalable Vector Extensions)
//!
//! ## Architecture
//!
//! The CPU engine uses runtime detection to select the best available SIMD
//! instruction set and provides automatic fallback to scalar implementations.

pub mod engine;
pub mod simd;

pub use engine::{CPUEngine, SimdLevel};
pub use simd::{SimdAggregate, SimdCapability, SimdFilter};
