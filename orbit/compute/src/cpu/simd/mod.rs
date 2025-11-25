//! SIMD-Optimized Operators for Vectorized Execution
//!
//! This module provides high-performance SIMD operations for query execution.
//! It uses trait-based abstraction for portability across different architectures.
//!
//! ## Supported Architectures
//!
//! - **x86_64**: AVX2 and AVX-512 (with runtime detection)
//! - **ARM**: NEON (aarch64)
//! - **Fallback**: Pure Rust scalar implementations
//!
//! ## Design Patterns
//!
//! - **Strategy Pattern**: Pluggable SIMD vs scalar implementations
//! - **Trait Objects**: Runtime selection of optimal implementation
//! - **Zero-Cost Abstractions**: Compile-time monomorphization where possible

pub mod filters;
pub mod aggregates;
pub mod types;

pub use types::NullBitmap;

/// Trait for SIMD-capable filter operations
///
/// Provides a common interface for vectorized filtering across
/// different data types and architectures.
pub trait SimdFilter<T> {
    /// Filter values based on a predicate, returning indices of matching rows
    fn filter_eq(&self, values: &[T], target: T, result: &mut Vec<usize>);

    /// Filter for not-equal comparison
    fn filter_ne(&self, values: &[T], target: T, result: &mut Vec<usize>);

    /// Filter for less-than comparison
    fn filter_lt(&self, values: &[T], target: T, result: &mut Vec<usize>);

    /// Filter for less-than-or-equal comparison
    fn filter_le(&self, values: &[T], target: T, result: &mut Vec<usize>);

    /// Filter for greater-than comparison
    fn filter_gt(&self, values: &[T], target: T, result: &mut Vec<usize>);

    /// Filter for greater-than-or-equal comparison
    fn filter_ge(&self, values: &[T], target: T, result: &mut Vec<usize>);
}

/// Trait for SIMD-capable aggregation operations
pub trait SimdAggregate<T> {
    /// Compute sum of values
    fn sum(&self, values: &[T], null_bitmap: &NullBitmap) -> Option<T>;

    /// Compute minimum value
    fn min(&self, values: &[T], null_bitmap: &NullBitmap) -> Option<T>;

    /// Compute maximum value
    fn max(&self, values: &[T], null_bitmap: &NullBitmap) -> Option<T>;

    /// Count non-null values
    fn count(&self, null_bitmap: &NullBitmap) -> usize;
}

/// Runtime SIMD capability detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdCapability {
    /// No SIMD support (scalar fallback)
    None,
    /// SSE2 (x86_64 baseline)
    SSE2,
    /// AVX2 (256-bit vectors)
    AVX2,
    /// AVX-512 (512-bit vectors)
    AVX512,
    /// ARM NEON
    NEON,
}

impl SimdCapability {
    /// Detect available SIMD capabilities at runtime
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") {
                return SimdCapability::AVX512;
            }
            if is_x86_feature_detected!("avx2") {
                return SimdCapability::AVX2;
            }
            if is_x86_feature_detected!("sse2") {
                return SimdCapability::SSE2;
            }
            return SimdCapability::None;
        }

        #[cfg(target_arch = "aarch64")]
        {
            // NEON is mandatory on aarch64
            SimdCapability::NEON
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            SimdCapability::None
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            SimdCapability::None => "Scalar",
            SimdCapability::SSE2 => "SSE2",
            SimdCapability::AVX2 => "AVX2",
            SimdCapability::AVX512 => "AVX-512",
            SimdCapability::NEON => "NEON",
        }
    }
}

/// Global SIMD capability (detected once at startup)
static SIMD_CAPABILITY: std::sync::OnceLock<SimdCapability> = std::sync::OnceLock::new();

/// Get the detected SIMD capability
pub fn simd_capability() -> SimdCapability {
    *SIMD_CAPABILITY.get_or_init(SimdCapability::detect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_detection() {
        let capability = simd_capability();
        println!("Detected SIMD capability: {}", capability.name());

        // Should always detect at least scalar
        assert_ne!(capability, SimdCapability::AVX512); // Unlikely in test env
    }
}
