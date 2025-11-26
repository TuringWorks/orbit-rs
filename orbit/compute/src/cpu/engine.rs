//! CPU Compute Engine with SIMD Acceleration
//!
//! Provides high-level API for CPU-based compute operations with
//! automatic SIMD optimization and runtime feature detection.

use super::simd::aggregates::{SimdAggregateF64, SimdAggregateI32, SimdAggregateI64};
use super::simd::filters::{SimdFilterF64, SimdFilterI32, SimdFilterI64};
use super::simd::{simd_capability, NullBitmap, SimdAggregate, SimdCapability, SimdFilter};

/// SIMD level supported by the CPU
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdLevel {
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
    /// ARM SVE (Scalable Vector Extensions)
    SVE,
}

impl From<SimdCapability> for SimdLevel {
    fn from(cap: SimdCapability) -> Self {
        match cap {
            SimdCapability::None => SimdLevel::None,
            SimdCapability::SSE2 => SimdLevel::SSE2,
            SimdCapability::AVX2 => SimdLevel::AVX2,
            SimdCapability::AVX512 => SimdLevel::AVX512,
            SimdCapability::NEON => SimdLevel::NEON,
        }
    }
}

/// CPU compute engine for SIMD operations
#[derive(Debug)]
pub struct CPUEngine {
    /// Selected SIMD level
    simd_level: SimdLevel,
    /// SIMD capability (from runtime detection)
    simd_capability: SimdCapability,
}

impl CPUEngine {
    /// Create a new CPU engine with automatic capability detection
    pub fn new() -> Self {
        let capability = simd_capability();
        let simd_level = SimdLevel::from(capability);

        Self {
            simd_level,
            simd_capability: capability,
        }
    }

    /// Get the detected SIMD level
    pub fn simd_level(&self) -> SimdLevel {
        self.simd_level
    }

    /// Get the detected SIMD capability
    pub fn simd_capability(&self) -> SimdCapability {
        self.simd_capability
    }

    // ===== Filter Operations =====

    /// Filter i32 values for equality
    pub fn filter_eq_i32(&self, data: &[i32], value: i32) -> Vec<usize> {
        let filter = SimdFilterI32::new();
        let mut result = Vec::new();
        filter.filter_eq(data, value, &mut result);
        result
    }

    /// Filter i64 values for equality
    pub fn filter_eq_i64(&self, data: &[i64], value: i64) -> Vec<usize> {
        let filter = SimdFilterI64::new();
        let mut result = Vec::new();
        filter.filter_eq(data, value, &mut result);
        result
    }

    /// Filter f64 values for equality (with epsilon for floating point)
    pub fn filter_eq_f64(&self, data: &[f64], value: f64) -> Vec<usize> {
        let filter = SimdFilterF64::new();
        let mut result = Vec::new();
        filter.filter_eq(data, value, &mut result);
        result
    }

    /// Filter i32 values for less-than
    pub fn filter_lt_i32(&self, data: &[i32], value: i32) -> Vec<usize> {
        let filter = SimdFilterI32::new();
        let mut result = Vec::new();
        filter.filter_lt(data, value, &mut result);
        result
    }

    /// Filter i64 values for less-than
    pub fn filter_lt_i64(&self, data: &[i64], value: i64) -> Vec<usize> {
        let filter = SimdFilterI64::new();
        let mut result = Vec::new();
        filter.filter_lt(data, value, &mut result);
        result
    }

    /// Filter f64 values for less-than
    pub fn filter_lt_f64(&self, data: &[f64], value: f64) -> Vec<usize> {
        let filter = SimdFilterF64::new();
        let mut result = Vec::new();
        filter.filter_lt(data, value, &mut result);
        result
    }

    /// Filter i64 values for range (between min and max)
    pub fn filter_between_i64(&self, data: &[i64], min: i64, max: i64) -> Vec<usize> {
        // Combine filter_ge(min) AND filter_le(max)
        let filter = SimdFilterI64::new();
        let mut ge_result = Vec::new();
        let mut le_result = Vec::new();
        filter.filter_ge(data, min, &mut ge_result);
        filter.filter_le(data, max, &mut le_result);

        // Intersect the two result sets
        ge_result
            .into_iter()
            .filter(|idx| le_result.contains(idx))
            .collect()
    }

    // ===== Aggregate Operations =====

    /// Compute sum of i32 values
    pub fn sum_i32(&self, data: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        let agg = SimdAggregateI32::new();
        agg.sum(data, null_bitmap)
    }

    /// Compute sum of i64 values
    pub fn sum_i64(&self, data: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        let agg = SimdAggregateI64::new();
        agg.sum(data, null_bitmap)
    }

    /// Compute sum of f64 values
    pub fn sum_f64(&self, data: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        let agg = SimdAggregateF64::new();
        agg.sum(data, null_bitmap)
    }

    /// Compute average of i32 values
    pub fn avg_i32(&self, data: &[i32], null_bitmap: &NullBitmap) -> Option<f64> {
        let sum = self.sum_i32(data, null_bitmap)?;
        let count = null_bitmap.non_null_count();
        if count > 0 {
            Some(sum as f64 / count as f64)
        } else {
            None
        }
    }

    /// Compute average of i64 values
    pub fn avg_i64(&self, data: &[i64], null_bitmap: &NullBitmap) -> Option<f64> {
        let sum = self.sum_i64(data, null_bitmap)?;
        let count = null_bitmap.non_null_count();
        if count > 0 {
            Some(sum as f64 / count as f64)
        } else {
            None
        }
    }

    /// Compute average of f64 values
    pub fn avg_f64(&self, data: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        let sum = self.sum_f64(data, null_bitmap)?;
        let count = null_bitmap.non_null_count();
        if count > 0 {
            Some(sum / count as f64)
        } else {
            None
        }
    }

    /// Compute minimum of i32 values
    pub fn min_i32(&self, data: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        let agg = SimdAggregateI32::new();
        agg.min(data, null_bitmap)
    }

    /// Compute minimum of i64 values
    pub fn min_i64(&self, data: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        let agg = SimdAggregateI64::new();
        agg.min(data, null_bitmap)
    }

    /// Compute minimum of f64 values
    pub fn min_f64(&self, data: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        let agg = SimdAggregateF64::new();
        agg.min(data, null_bitmap)
    }

    /// Compute maximum of i32 values
    pub fn max_i32(&self, data: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        let agg = SimdAggregateI32::new();
        agg.max(data, null_bitmap)
    }

    /// Compute maximum of i64 values
    pub fn max_i64(&self, data: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        let agg = SimdAggregateI64::new();
        agg.max(data, null_bitmap)
    }

    /// Compute maximum of f64 values
    pub fn max_f64(&self, data: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        let agg = SimdAggregateF64::new();
        agg.max(data, null_bitmap)
    }

    /// Count non-null values
    pub fn count(&self, null_bitmap: &NullBitmap) -> usize {
        null_bitmap.non_null_count()
    }
}

impl Default for CPUEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_engine_creation() {
        let engine = CPUEngine::new();
        println!("SIMD level: {:?}", engine.simd_level());

        // Should always be able to create an engine
        assert!(matches!(
            engine.simd_level(),
            SimdLevel::None
                | SimdLevel::SSE2
                | SimdLevel::AVX2
                | SimdLevel::AVX512
                | SimdLevel::NEON
                | SimdLevel::SVE
        ));
    }

    #[test]
    fn test_filter_eq_i64() {
        let engine = CPUEngine::new();
        let data = vec![1i64, 2, 3, 4, 5, 2, 7, 2];
        let result = engine.filter_eq_i64(&data, 2);
        assert_eq!(result, vec![1, 5, 7]); // Indices where value == 2
    }

    #[test]
    fn test_filter_lt_i64() {
        let engine = CPUEngine::new();
        let data = vec![1i64, 2, 3, 4, 5];
        let result = engine.filter_lt_i64(&data, 3);
        assert_eq!(result, vec![0, 1]); // Indices where value < 3
    }

    #[test]
    fn test_sum_i64() {
        let engine = CPUEngine::new();
        let data = vec![1i64, 2, 3, 4, 5];
        let null_bitmap = NullBitmap::new(5);
        let sum = engine.sum_i64(&data, &null_bitmap);
        assert_eq!(sum, Some(15));
    }

    #[test]
    fn test_sum_i64_with_nulls() {
        let engine = CPUEngine::new();
        let data = vec![1i64, 2, 3, 4, 5];
        let mut null_bitmap = NullBitmap::new(5);
        null_bitmap.set_null(2); // Exclude index 2 (value 3)
        let sum = engine.sum_i64(&data, &null_bitmap);
        assert_eq!(sum, Some(12)); // 1 + 2 + 4 + 5 = 12
    }

    #[test]
    fn test_avg_i64() {
        let engine = CPUEngine::new();
        let data = vec![1i64, 2, 3, 4, 5];
        let null_bitmap = NullBitmap::new(5);
        let avg = engine.avg_i64(&data, &null_bitmap);
        assert_eq!(avg, Some(3.0));
    }

    #[test]
    fn test_min_max_i64() {
        let engine = CPUEngine::new();
        let data = vec![5i64, 2, 8, 1, 9];
        let null_bitmap = NullBitmap::new(5);

        let min = engine.min_i64(&data, &null_bitmap);
        let max = engine.max_i64(&data, &null_bitmap);

        assert_eq!(min, Some(1));
        assert_eq!(max, Some(9));
    }

    #[test]
    fn test_count() {
        let engine = CPUEngine::new();
        let mut null_bitmap = NullBitmap::new(10);
        null_bitmap.set_null(3);
        null_bitmap.set_null(7);

        let count = engine.count(&null_bitmap);
        assert_eq!(count, 8); // 10 - 2 nulls = 8
    }

    #[test]
    fn test_filter_between_i64() {
        let engine = CPUEngine::new();
        let data = vec![1i64, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let result = engine.filter_between_i64(&data, 3, 7);
        assert_eq!(result, vec![2, 3, 4, 5, 6]); // Indices for values 3-7
    }
}
