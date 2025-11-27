//! SIMD-Optimized Filter Operations
//!
//! Provides high-performance filtering for common predicates.
//! Uses SIMD instructions when available with automatic fallback to scalar code.

use super::{simd_capability, SimdCapability, SimdFilter};

/// SIMD filter for i32 values
pub struct SimdFilterI32 {
    capability: SimdCapability,
}

impl SimdFilterI32 {
    pub fn new() -> Self {
        Self {
            capability: simd_capability(),
        }
    }
}

impl Default for SimdFilterI32 {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdFilter<i32> for SimdFilterI32 {
    fn filter_eq(&self, values: &[i32], target: i32, result: &mut Vec<usize>) {
        match self.capability {
            #[cfg(target_arch = "x86_64")]
            SimdCapability::AVX2 => unsafe { filter_eq_avx2(values, target, result) },
            _ => filter_eq_scalar(values, target, result),
        }
    }

    fn filter_ne(&self, values: &[i32], target: i32, result: &mut Vec<usize>) {
        filter_ne_scalar(values, target, result)
    }

    fn filter_lt(&self, values: &[i32], target: i32, result: &mut Vec<usize>) {
        match self.capability {
            #[cfg(target_arch = "x86_64")]
            SimdCapability::AVX2 => unsafe { filter_lt_avx2(values, target, result) },
            _ => filter_lt_scalar(values, target, result),
        }
    }

    fn filter_le(&self, values: &[i32], target: i32, result: &mut Vec<usize>) {
        filter_le_scalar(values, target, result)
    }

    fn filter_gt(&self, values: &[i32], target: i32, result: &mut Vec<usize>) {
        match self.capability {
            #[cfg(target_arch = "x86_64")]
            SimdCapability::AVX2 => unsafe { filter_gt_avx2(values, target, result) },
            _ => filter_gt_scalar(values, target, result),
        }
    }

    fn filter_ge(&self, values: &[i32], target: i32, result: &mut Vec<usize>) {
        filter_ge_scalar(values, target, result)
    }
}

/// Scalar implementation: equality filter
#[inline]
fn filter_eq_scalar<T: PartialEq>(values: &[T], target: T, result: &mut Vec<usize>) {
    result.clear();
    for (index, value) in values.iter().enumerate() {
        if *value == target {
            result.push(index);
        }
    }
}

/// Scalar implementation: not-equal filter
#[inline]
fn filter_ne_scalar<T: PartialEq>(values: &[T], target: T, result: &mut Vec<usize>) {
    result.clear();
    for (index, value) in values.iter().enumerate() {
        if *value != target {
            result.push(index);
        }
    }
}

/// Scalar implementation: less-than filter
#[inline]
fn filter_lt_scalar<T: PartialOrd>(values: &[T], target: T, result: &mut Vec<usize>) {
    result.clear();
    for (index, value) in values.iter().enumerate() {
        if *value < target {
            result.push(index);
        }
    }
}

/// Scalar implementation: less-than-or-equal filter
#[inline]
fn filter_le_scalar<T: PartialOrd>(values: &[T], target: T, result: &mut Vec<usize>) {
    result.clear();
    for (index, value) in values.iter().enumerate() {
        if *value <= target {
            result.push(index);
        }
    }
}

/// Scalar implementation: greater-than filter
#[inline]
fn filter_gt_scalar<T: PartialOrd>(values: &[T], target: T, result: &mut Vec<usize>) {
    result.clear();
    for (index, value) in values.iter().enumerate() {
        if *value > target {
            result.push(index);
        }
    }
}

/// Scalar implementation: greater-than-or-equal filter
#[inline]
fn filter_ge_scalar<T: PartialOrd>(values: &[T], target: T, result: &mut Vec<usize>) {
    result.clear();
    for (index, value) in values.iter().enumerate() {
        if *value >= target {
            result.push(index);
        }
    }
}

// AVX2 implementations for x86_64
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_eq_avx2(values: &[i32], target: i32, result: &mut Vec<usize>) {
    use std::arch::x86_64::*;

    result.clear();
    let len = values.len();
    let vec_size = 8; // AVX2 processes 8 i32 values at once
    let target_vec = _mm256_set1_epi32(target);

    // Process 8 values at a time with AVX2
    let chunks = len / vec_size;
    for chunk_idx in 0..chunks {
        let offset = chunk_idx * vec_size;
        let ptr = values.as_ptr().add(offset);
        let values_vec = _mm256_loadu_si256(ptr as *const __m256i);

        // Compare for equality
        let cmp_result = _mm256_cmpeq_epi32(values_vec, target_vec);
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(cmp_result));

        // Extract matching indices
        for i in 0..vec_size {
            if (mask & (1 << i)) != 0 {
                result.push(offset + i);
            }
        }
    }

    // Handle remaining values with scalar code
    for index in (chunks * vec_size)..len {
        if values[index] == target {
            result.push(index);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_lt_avx2(values: &[i32], target: i32, result: &mut Vec<usize>) {
    use std::arch::x86_64::*;

    result.clear();
    let len = values.len();
    let vec_size = 8;
    let target_vec = _mm256_set1_epi32(target);

    let chunks = len / vec_size;
    for chunk_idx in 0..chunks {
        let offset = chunk_idx * vec_size;
        let ptr = values.as_ptr().add(offset);
        let values_vec = _mm256_loadu_si256(ptr as *const __m256i);

        // Compare for less-than
        let cmp_result = _mm256_cmpgt_epi32(target_vec, values_vec); // target > values
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(cmp_result));

        for i in 0..vec_size {
            if (mask & (1 << i)) != 0 {
                result.push(offset + i);
            }
        }
    }

    // Handle remainder
    for index in (chunks * vec_size)..len {
        if values[index] < target {
            result.push(index);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_gt_avx2(values: &[i32], target: i32, result: &mut Vec<usize>) {
    use std::arch::x86_64::*;

    result.clear();
    let len = values.len();
    let vec_size = 8;
    let target_vec = _mm256_set1_epi32(target);

    let chunks = len / vec_size;
    for chunk_idx in 0..chunks {
        let offset = chunk_idx * vec_size;
        let ptr = values.as_ptr().add(offset);
        let values_vec = _mm256_loadu_si256(ptr as *const __m256i);

        // Compare for greater-than
        let cmp_result = _mm256_cmpgt_epi32(values_vec, target_vec); // values > target
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(cmp_result));

        for i in 0..vec_size {
            if (mask & (1 << i)) != 0 {
                result.push(offset + i);
            }
        }
    }

    // Handle remainder
    for index in (chunks * vec_size)..len {
        if values[index] > target {
            result.push(index);
        }
    }
}

/// SIMD filter for i64 values
pub struct SimdFilterI64 {
    #[allow(dead_code)]
    capability: SimdCapability,
}

impl SimdFilterI64 {
    pub fn new() -> Self {
        Self {
            capability: simd_capability(),
        }
    }
}

impl Default for SimdFilterI64 {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdFilter<i64> for SimdFilterI64 {
    fn filter_eq(&self, values: &[i64], target: i64, result: &mut Vec<usize>) {
        filter_eq_scalar(values, target, result)
    }

    fn filter_ne(&self, values: &[i64], target: i64, result: &mut Vec<usize>) {
        filter_ne_scalar(values, target, result)
    }

    fn filter_lt(&self, values: &[i64], target: i64, result: &mut Vec<usize>) {
        filter_lt_scalar(values, target, result)
    }

    fn filter_le(&self, values: &[i64], target: i64, result: &mut Vec<usize>) {
        filter_le_scalar(values, target, result)
    }

    fn filter_gt(&self, values: &[i64], target: i64, result: &mut Vec<usize>) {
        filter_gt_scalar(values, target, result)
    }

    fn filter_ge(&self, values: &[i64], target: i64, result: &mut Vec<usize>) {
        filter_ge_scalar(values, target, result)
    }
}

/// SIMD filter for f64 values
pub struct SimdFilterF64 {
    #[allow(dead_code)]
    capability: SimdCapability,
}

impl SimdFilterF64 {
    pub fn new() -> Self {
        Self {
            capability: simd_capability(),
        }
    }
}

impl Default for SimdFilterF64 {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdFilter<f64> for SimdFilterF64 {
    fn filter_eq(&self, values: &[f64], target: f64, result: &mut Vec<usize>) {
        filter_eq_scalar(values, target, result)
    }

    fn filter_ne(&self, values: &[f64], target: f64, result: &mut Vec<usize>) {
        filter_ne_scalar(values, target, result)
    }

    fn filter_lt(&self, values: &[f64], target: f64, result: &mut Vec<usize>) {
        filter_lt_scalar(values, target, result)
    }

    fn filter_le(&self, values: &[f64], target: f64, result: &mut Vec<usize>) {
        filter_le_scalar(values, target, result)
    }

    fn filter_gt(&self, values: &[f64], target: f64, result: &mut Vec<usize>) {
        filter_gt_scalar(values, target, result)
    }

    fn filter_ge(&self, values: &[f64], target: f64, result: &mut Vec<usize>) {
        filter_ge_scalar(values, target, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_filter_eq() {
        let values = vec![1, 5, 3, 5, 2, 5, 7];
        let mut result = Vec::new();
        filter_eq_scalar(&values, 5, &mut result);
        assert_eq!(result, vec![1, 3, 5]);
    }

    #[test]
    fn test_scalar_filter_lt() {
        let values = vec![1, 5, 3, 5, 2, 5, 7];
        let mut result = Vec::new();
        filter_lt_scalar(&values, 4, &mut result);
        assert_eq!(result, vec![0, 2, 4]); // Values 1, 3, 2 at indices 0, 2, 4
    }

    #[test]
    fn test_scalar_filter_gt() {
        let values = vec![1, 5, 3, 5, 2, 5, 7];
        let mut result = Vec::new();
        filter_gt_scalar(&values, 4, &mut result);
        assert_eq!(result, vec![1, 3, 5, 6]); // Values 5, 5, 5, 7
    }

    #[test]
    fn test_simd_filter_i32() {
        let filter = SimdFilterI32::new();
        let values: Vec<i32> = (0..1000).collect();
        let mut result = Vec::new();

        filter.filter_eq(&values, 500, &mut result);
        assert_eq!(result, vec![500]);

        filter.filter_lt(&values, 10, &mut result);
        assert_eq!(result.len(), 10);

        filter.filter_gt(&values, 990, &mut result);
        assert_eq!(result.len(), 9); // 991-999
    }

    #[test]
    fn test_simd_filter_large_dataset() {
        let filter = SimdFilterI32::new();
        let values: Vec<i32> = (0..10000).map(|i| i % 100).collect();
        let mut result = Vec::new();

        filter.filter_eq(&values, 42, &mut result);
        assert_eq!(result.len(), 100); // 42 appears 100 times

        filter.filter_lt(&values, 50, &mut result);
        assert_eq!(result.len(), 5000); // Half the values
    }
}
