//! SIMD Backend Abstraction
//!
//! Provides a unified interface for different SIMD implementations
//! with runtime selection based on CPU capabilities.

use super::NullBitmap;

/// Unified SIMD backend trait
///
/// Provides a common interface for all SIMD operations across
/// different architectures (AVX2, NEON, Scalar).
pub trait SimdBackend: Send + Sync {
    /// Get the name of this backend
    fn name(&self) -> &'static str;

    /// Get the capability level
    fn capability(&self) -> super::SimdCapability;

    // Filter operations for i32
    fn filter_i32_eq(&self, values: &[i32], target: i32) -> Vec<usize>;
    fn filter_i32_lt(&self, values: &[i32], target: i32) -> Vec<usize>;
    fn filter_i32_gt(&self, values: &[i32], target: i32) -> Vec<usize>;

    // Aggregate operations for i32
    fn sum_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32>;
    fn min_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32>;
    fn max_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32>;

    // Filter operations for i64
    fn filter_i64_eq(&self, values: &[i64], target: i64) -> Vec<usize>;
    fn filter_i64_lt(&self, values: &[i64], target: i64) -> Vec<usize>;
    fn filter_i64_gt(&self, values: &[i64], target: i64) -> Vec<usize>;

    // Aggregate operations for i64
    fn sum_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64>;
    fn min_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64>;
    fn max_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64>;

    // Filter operations for f32
    fn filter_f32_eq(&self, values: &[f32], target: f32) -> Vec<usize>;
    fn filter_f32_lt(&self, values: &[f32], target: f32) -> Vec<usize>;
    fn filter_f32_gt(&self, values: &[f32], target: f32) -> Vec<usize>;

    // Aggregate operations for f32
    fn sum_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32>;
    fn min_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32>;
    fn max_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32>;

    // Filter operations for f64
    fn filter_f64_eq(&self, values: &[f64], target: f64) -> Vec<usize>;
    fn filter_f64_lt(&self, values: &[f64], target: f64) -> Vec<usize>;
    fn filter_f64_gt(&self, values: &[f64], target: f64) -> Vec<usize>;

    // Aggregate operations for f64
    fn sum_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64>;
    fn min_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64>;
    fn max_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64>;

    // String operations
    fn compare_bytes(&self, a: &[u8], b: &[u8]) -> bool;
    fn find_byte(&self, haystack: &[u8], needle: u8) -> Option<usize>;

    // Logic ops for i32
    fn filter_i32_le(&self, values: &[i32], target: i32) -> Vec<usize>;
    fn filter_i32_ge(&self, values: &[i32], target: i32) -> Vec<usize>;
    fn filter_i32_ne(&self, values: &[i32], target: i32) -> Vec<usize>;

    // Logic ops for i64
    fn filter_i64_le(&self, values: &[i64], target: i64) -> Vec<usize>;
    fn filter_i64_ge(&self, values: &[i64], target: i64) -> Vec<usize>;
    fn filter_i64_ne(&self, values: &[i64], target: i64) -> Vec<usize>;

    // Logic ops for f32
    fn filter_f32_le(&self, values: &[f32], target: f32) -> Vec<usize>;
    fn filter_f32_ge(&self, values: &[f32], target: f32) -> Vec<usize>;
    fn filter_f32_ne(&self, values: &[f32], target: f32) -> Vec<usize>;

    // Logic ops for f64
    fn filter_f64_le(&self, values: &[f64], target: f64) -> Vec<usize>;
    fn filter_f64_ge(&self, values: &[f64], target: f64) -> Vec<usize>;
    fn filter_f64_ne(&self, values: &[f64], target: f64) -> Vec<usize>;
}

/// Scalar (fallback) backend
pub struct ScalarBackend;

impl SimdBackend for ScalarBackend {
    fn name(&self) -> &'static str {
        "Scalar"
    }

    fn capability(&self) -> super::SimdCapability {
        super::SimdCapability::None
    }

    fn filter_i32_eq(&self, values: &[i32], target: i32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v == target { Some(i) } else { None })
            .collect()
    }

    fn filter_i32_lt(&self, values: &[i32], target: i32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v < target { Some(i) } else { None })
            .collect()
    }

    fn filter_i32_gt(&self, values: &[i32], target: i32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v > target { Some(i) } else { None })
            .collect()
    }

    fn sum_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        let mut sum: i64 = 0;
        let mut has_value = false;

        for (i, &v) in values.iter().enumerate() {
            if null_bitmap.is_valid(i) {
                sum += v as i64;
                has_value = true;
            }
        }

        has_value.then_some(sum as i32)
    }

    fn min_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| null_bitmap.is_valid(i).then_some(v))
            .min()
    }

    fn max_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| null_bitmap.is_valid(i).then_some(v))
            .max()
    }

    // i64 filter operations
    fn filter_i64_eq(&self, values: &[i64], target: i64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v == target).then_some(i))
            .collect()
    }

    fn filter_i64_lt(&self, values: &[i64], target: i64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v < target).then_some(i))
            .collect()
    }

    fn filter_i64_gt(&self, values: &[i64], target: i64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v > target).then_some(i))
            .collect()
    }

    // i64 aggregate operations
    fn sum_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        let sum: i64 = values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| null_bitmap.is_valid(i).then_some(v))
            .sum();
        Some(sum).filter(|_| {
            values
                .iter()
                .enumerate()
                .any(|(i, _)| null_bitmap.is_valid(i))
        })
    }

    fn min_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| null_bitmap.is_valid(i).then_some(v))
            .min()
    }

    fn max_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| null_bitmap.is_valid(i).then_some(v))
            .max()
    }

    // f32 filter operations
    fn filter_f32_eq(&self, values: &[f32], target: f32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v == target).then_some(i))
            .collect()
    }

    fn filter_f32_lt(&self, values: &[f32], target: f32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v < target).then_some(i))
            .collect()
    }

    fn filter_f32_gt(&self, values: &[f32], target: f32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v > target).then_some(i))
            .collect()
    }

    // f32 aggregate operations
    fn sum_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        let sum: f32 = values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| null_bitmap.is_valid(i).then_some(v))
            .sum();
        Some(sum).filter(|_| {
            values
                .iter()
                .enumerate()
                .any(|(i, _)| null_bitmap.is_valid(i))
        })
    }

    fn min_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        let mut min_val = f32::INFINITY;
        let mut has_value = false;

        for (i, &v) in values.iter().enumerate() {
            if null_bitmap.is_valid(i) {
                if v.is_nan() {
                    return Some(f32::NAN);
                }
                if v < min_val {
                    min_val = v;
                }
                has_value = true;
            }
        }

        has_value.then_some(min_val)
    }

    fn max_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        let mut max_val = f32::NEG_INFINITY;
        let mut has_value = false;

        for (i, &v) in values.iter().enumerate() {
            if null_bitmap.is_valid(i) {
                if v.is_nan() {
                    return Some(f32::NAN);
                }
                if v > max_val {
                    max_val = v;
                }
                has_value = true;
            }
        }

        has_value.then_some(max_val)
    }

    // f64 filter operations
    fn filter_f64_eq(&self, values: &[f64], target: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v == target).then_some(i))
            .collect()
    }

    fn filter_f64_lt(&self, values: &[f64], target: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v < target).then_some(i))
            .collect()
    }

    fn filter_f64_gt(&self, values: &[f64], target: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| (v > target).then_some(i))
            .collect()
    }

    // f64 aggregate operations
    fn sum_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        let sum: f64 = values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| null_bitmap.is_valid(i).then_some(v))
            .sum();
        Some(sum).filter(|_| {
            values
                .iter()
                .enumerate()
                .any(|(i, _)| null_bitmap.is_valid(i))
        })
    }

    fn min_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        let mut min_val = f64::INFINITY;
        let mut has_value = false;

        for (i, &v) in values.iter().enumerate() {
            if null_bitmap.is_valid(i) {
                if v.is_nan() {
                    return Some(f64::NAN);
                }
                if v < min_val {
                    min_val = v;
                }
                has_value = true;
            }
        }

        has_value.then_some(min_val)
    }

    fn max_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        let mut max_val = f64::NEG_INFINITY;
        let mut has_value = false;

        for (i, &v) in values.iter().enumerate() {
            if null_bitmap.is_valid(i) {
                if v.is_nan() {
                    return Some(f64::NAN);
                }
                if v > max_val {
                    max_val = v;
                }
                has_value = true;
            }
        }

        has_value.then_some(max_val)
    }

    fn compare_bytes(&self, a: &[u8], b: &[u8]) -> bool {
        a == b
    }

    fn find_byte(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        haystack.iter().position(|&b| b == needle)
    }

    fn filter_i32_le(&self, values: &[i32], target: i32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v <= target { Some(i) } else { None })
            .collect()
    }
    fn filter_i32_ge(&self, values: &[i32], target: i32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v >= target { Some(i) } else { None })
            .collect()
    }
    fn filter_i32_ne(&self, values: &[i32], target: i32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v != target { Some(i) } else { None })
            .collect()
    }

    fn filter_i64_le(&self, values: &[i64], target: i64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v <= target { Some(i) } else { None })
            .collect()
    }
    fn filter_i64_ge(&self, values: &[i64], target: i64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v >= target { Some(i) } else { None })
            .collect()
    }
    fn filter_i64_ne(&self, values: &[i64], target: i64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v != target { Some(i) } else { None })
            .collect()
    }

    fn filter_f32_le(&self, values: &[f32], target: f32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v <= target { Some(i) } else { None })
            .collect()
    }
    fn filter_f32_ge(&self, values: &[f32], target: f32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v >= target { Some(i) } else { None })
            .collect()
    }
    fn filter_f32_ne(&self, values: &[f32], target: f32) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v != target { Some(i) } else { None })
            .collect()
    }

    fn filter_f64_le(&self, values: &[f64], target: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v <= target { Some(i) } else { None })
            .collect()
    }
    fn filter_f64_ge(&self, values: &[f64], target: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v >= target { Some(i) } else { None })
            .collect()
    }
    fn filter_f64_ne(&self, values: &[f64], target: f64) -> Vec<usize> {
        values
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v != target { Some(i) } else { None })
            .collect()
    }
}

/// AVX2 backend for x86_64
#[cfg(target_arch = "x86_64")]
pub struct Avx2Backend;

#[cfg(target_arch = "x86_64")]
impl SimdBackend for Avx2Backend {
    fn name(&self) -> &'static str {
        "AVX2"
    }

    fn capability(&self) -> super::SimdCapability {
        super::SimdCapability::AVX2
    }

    fn filter_i32_eq(&self, values: &[i32], target: i32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i32_eq_avx2(values, target) }
        } else {
            ScalarBackend.filter_i32_eq(values, target)
        }
    }

    fn filter_i32_lt(&self, values: &[i32], target: i32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i32_lt_avx2(values, target) }
        } else {
            ScalarBackend.filter_i32_lt(values, target)
        }
    }

    fn filter_i32_gt(&self, values: &[i32], target: i32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i32_gt_avx2(values, target) }
        } else {
            ScalarBackend.filter_i32_gt(values, target)
        }
    }

    fn sum_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        if is_x86_feature_detected!("avx2") {
            unsafe { sum_i32_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.sum_i32(values, null_bitmap)
        }
    }

    fn min_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        if is_x86_feature_detected!("avx2") {
            unsafe { min_i32_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.min_i32(values, null_bitmap)
        }
    }

    fn max_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        if is_x86_feature_detected!("avx2") {
            unsafe { max_i32_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.max_i32(values, null_bitmap)
        }
    }

    // i64 filter operations
    fn filter_i64_eq(&self, values: &[i64], target: i64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i64_eq_avx2(values, target) }
        } else {
            ScalarBackend.filter_i64_eq(values, target)
        }
    }

    fn filter_i64_lt(&self, values: &[i64], target: i64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i64_lt_avx2(values, target) }
        } else {
            ScalarBackend.filter_i64_lt(values, target)
        }
    }

    fn filter_i64_gt(&self, values: &[i64], target: i64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i64_gt_avx2(values, target) }
        } else {
            ScalarBackend.filter_i64_gt(values, target)
        }
    }

    // i64 aggregate operations
    fn sum_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        if is_x86_feature_detected!("avx2") {
            unsafe { sum_i64_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.sum_i64(values, null_bitmap)
        }
    }

    fn min_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        if is_x86_feature_detected!("avx2") {
            unsafe { min_i64_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.min_i64(values, null_bitmap)
        }
    }

    fn max_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        if is_x86_feature_detected!("avx2") {
            unsafe { max_i64_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.max_i64(values, null_bitmap)
        }
    }

    // f32 filter operations
    fn filter_f32_eq(&self, values: &[f32], target: f32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f32_eq_avx2(values, target) }
        } else {
            ScalarBackend.filter_f32_eq(values, target)
        }
    }

    fn filter_f32_lt(&self, values: &[f32], target: f32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f32_lt_avx2(values, target) }
        } else {
            ScalarBackend.filter_f32_lt(values, target)
        }
    }

    fn filter_f32_gt(&self, values: &[f32], target: f32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f32_gt_avx2(values, target) }
        } else {
            ScalarBackend.filter_f32_gt(values, target)
        }
    }

    // f32 aggregate operations
    fn sum_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        if is_x86_feature_detected!("avx2") {
            unsafe { sum_f32_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.sum_f32(values, null_bitmap)
        }
    }

    fn min_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        if is_x86_feature_detected!("avx2") {
            unsafe { min_f32_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.min_f32(values, null_bitmap)
        }
    }

    fn max_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        if is_x86_feature_detected!("avx2") {
            unsafe { max_f32_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.max_f32(values, null_bitmap)
        }
    }

    // f64 filter operations
    fn filter_f64_eq(&self, values: &[f64], target: f64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f64_eq_avx2(values, target) }
        } else {
            ScalarBackend.filter_f64_eq(values, target)
        }
    }

    fn filter_f64_lt(&self, values: &[f64], target: f64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f64_lt_avx2(values, target) }
        } else {
            ScalarBackend.filter_f64_lt(values, target)
        }
    }

    fn filter_f64_gt(&self, values: &[f64], target: f64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f64_gt_avx2(values, target) }
        } else {
            ScalarBackend.filter_f64_gt(values, target)
        }
    }

    // f64 aggregate operations
    fn sum_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        if is_x86_feature_detected!("avx2") {
            unsafe { sum_f64_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.sum_f64(values, null_bitmap)
        }
    }

    fn min_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        if is_x86_feature_detected!("avx2") {
            unsafe { min_f64_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.min_f64(values, null_bitmap)
        }
    }

    fn max_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        if is_x86_feature_detected!("avx2") {
            unsafe { max_f64_avx2(values, null_bitmap) }
        } else {
            ScalarBackend.max_f64(values, null_bitmap)
        }
    }

    fn compare_bytes(&self, a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        if is_x86_feature_detected!("avx2") {
            unsafe { compare_bytes_avx2(a, b) }
        } else {
            a == b
        }
    }

    fn find_byte(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        haystack.iter().position(|&b| b == needle) // TODO: SIMD implementation
    }

    fn filter_i32_le(&self, values: &[i32], target: i32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i32_le_avx2(values, target) }
        } else {
            ScalarBackend.filter_i32_le(values, target)
        }
    }
    fn filter_i32_ge(&self, values: &[i32], target: i32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i32_ge_avx2(values, target) }
        } else {
            ScalarBackend.filter_i32_ge(values, target)
        }
    }
    fn filter_i32_ne(&self, values: &[i32], target: i32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i32_ne_avx2(values, target) }
        } else {
            ScalarBackend.filter_i32_ne(values, target)
        }
    }

    fn filter_i64_le(&self, values: &[i64], target: i64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i64_le_avx2(values, target) }
        } else {
            ScalarBackend.filter_i64_le(values, target)
        }
    }
    fn filter_i64_ge(&self, values: &[i64], target: i64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i64_ge_avx2(values, target) }
        } else {
            ScalarBackend.filter_i64_ge(values, target)
        }
    }
    fn filter_i64_ne(&self, values: &[i64], target: i64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_i64_ne_avx2(values, target) }
        } else {
            ScalarBackend.filter_i64_ne(values, target)
        }
    }

    fn filter_f32_le(&self, values: &[f32], target: f32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f32_le_avx2(values, target) }
        } else {
            ScalarBackend.filter_f32_le(values, target)
        }
    }
    fn filter_f32_ge(&self, values: &[f32], target: f32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f32_ge_avx2(values, target) }
        } else {
            ScalarBackend.filter_f32_ge(values, target)
        }
    }
    fn filter_f32_ne(&self, values: &[f32], target: f32) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f32_ne_avx2(values, target) }
        } else {
            ScalarBackend.filter_f32_ne(values, target)
        }
    }

    fn filter_f64_le(&self, values: &[f64], target: f64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f64_le_avx2(values, target) }
        } else {
            ScalarBackend.filter_f64_le(values, target)
        }
    }
    fn filter_f64_ge(&self, values: &[f64], target: f64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f64_ge_avx2(values, target) }
        } else {
            ScalarBackend.filter_f64_ge(values, target)
        }
    }
    fn filter_f64_ne(&self, values: &[f64], target: f64) -> Vec<usize> {
        if is_x86_feature_detected!("avx2") {
            unsafe { filter_f64_ne_avx2(values, target) }
        } else {
            ScalarBackend.filter_f64_ne(values, target)
        }
    }
}

// AVX2 implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i32_eq_avx2(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi32(target);
    let mut i = 0;

    while i + 8 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let cmp = _mm256_cmpeq_epi32(data, target_vec);
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(cmp));

        for j in 0..8 {
            if (mask & (1 << j)) != 0 {
                result.push(i + j);
            }
        }
        i += 8;
    }

    // Handle remainder
    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i32_lt_avx2(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi32(target);
    let mut i = 0;

    while i + 8 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let cmp = _mm256_cmpgt_epi32(target_vec, data); // target > data
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(cmp));

        for j in 0..8 {
            if (mask & (1 << j)) != 0 {
                result.push(i + j);
            }
        }
        i += 8;
    }

    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i32_gt_avx2(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi32(target);
    let mut i = 0;

    while i + 8 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let cmp = _mm256_cmpgt_epi32(data, target_vec); // data > target
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(cmp));

        for j in 0..8 {
            if (mask & (1 << j)) != 0 {
                result.push(i + j);
            }
        }
        i += 8;
    }

    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sum_i32_avx2(values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
    use std::arch::x86_64::*;

    // Early exit for small datasets
    if values.len() < 16 {
        return ScalarBackend.sum_i32(values, null_bitmap);
    }

    let mut sum_vec = _mm256_setzero_si256();
    let mut has_value = false;
    let mut i = 0;

    // Process in larger chunks when all values are valid
    while i + 32 <= values.len() {
        let all_valid = (i..i + 32).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            // Process 4 chunks of 8 values each
            for _ in 0..4 {
                let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
                sum_vec = _mm256_add_epi32(sum_vec, data);
                i += 8;
            }
            has_value = true;
        } else {
            // Fall back to scalar for this chunk
            for j in i..i + 32 {
                if null_bitmap.is_valid(j) {
                    // Broadcast and add
                    let val_vec = _mm256_set1_epi32(values[j]);
                    sum_vec = _mm256_add_epi32(sum_vec, val_vec);
                    has_value = true;
                }
            }
            i += 32;
        }
    }

    // Process remaining in groups of 8
    while i + 8 <= values.len() {
        let all_valid = (i..i + 8).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
            sum_vec = _mm256_add_epi32(sum_vec, data);
            has_value = true;
        } else {
            for j in i..i + 8 {
                if null_bitmap.is_valid(j) {
                    let val_vec = _mm256_set1_epi32(values[j]);
                    sum_vec = _mm256_add_epi32(sum_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 8;
    }

    // Efficient horizontal sum using hadd
    let sum_vec = _mm256_hadd_epi32(sum_vec, sum_vec); // Horizontal add pairs
    let sum_vec = _mm256_hadd_epi32(sum_vec, sum_vec); // Horizontal add again

    // Extract both 128-bit lanes and add
    let low = _mm256_castsi256_si128(sum_vec);
    let high = _mm256_extracti128_si256(sum_vec, 1);
    let sum_128 = _mm_add_epi32(low, high);
    let total_simd = _mm_extract_epi32(sum_128, 0) as i64;

    // Handle remainder
    let mut total = total_simd;
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            total += values[j] as i64;
            has_value = true;
        }
    }

    has_value.then_some(total as i32)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn min_i32_avx2(values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }

    // Early exit for small datasets
    if values.len() < 16 {
        return ScalarBackend.min_i32(values, null_bitmap);
    }

    let mut min_vec = _mm256_set1_epi32(i32::MAX);
    let mut has_value = false;
    let mut i = 0;

    // Process in chunks of 32
    while i + 32 <= values.len() {
        let all_valid = (i..i + 32).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            for _ in 0..4 {
                let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
                min_vec = _mm256_min_epi32(min_vec, data);
                i += 8;
            }
            has_value = true;
        } else {
            for j in i..i + 32 {
                if null_bitmap.is_valid(j) {
                    let val_vec = _mm256_set1_epi32(values[j]);
                    min_vec = _mm256_min_epi32(min_vec, val_vec);
                    has_value = true;
                }
            }
            i += 32;
        }
    }

    // Process remaining in groups of 8
    while i + 8 <= values.len() {
        let all_valid = (i..i + 8).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
            min_vec = _mm256_min_epi32(min_vec, data);
            has_value = true;
        } else {
            for j in i..i + 8 {
                if null_bitmap.is_valid(j) {
                    let val_vec = _mm256_set1_epi32(values[j]);
                    min_vec = _mm256_min_epi32(min_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 8;
    }

    // Extract minimum from vector
    let min_arr: [i32; 8] = std::mem::transmute(min_vec);
    let mut min_val = *min_arr.iter().min().unwrap();

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            min_val = min_val.min(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(min_val)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn max_i32_avx2(values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }

    // Early exit for small datasets
    if values.len() < 16 {
        return ScalarBackend.max_i32(values, null_bitmap);
    }

    let mut max_vec = _mm256_set1_epi32(i32::MIN);
    let mut has_value = false;
    let mut i = 0;

    // Process in chunks of 32
    while i + 32 <= values.len() {
        let all_valid = (i..i + 32).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            for _ in 0..4 {
                let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
                max_vec = _mm256_max_epi32(max_vec, data);
                i += 8;
            }
            has_value = true;
        } else {
            for j in i..i + 32 {
                if null_bitmap.is_valid(j) {
                    let val_vec = _mm256_set1_epi32(values[j]);
                    max_vec = _mm256_max_epi32(max_vec, val_vec);
                    has_value = true;
                }
            }
            i += 32;
        }
    }

    // Process remaining in groups of 8
    while i + 8 <= values.len() {
        let all_valid = (i..i + 8).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
            max_vec = _mm256_max_epi32(max_vec, data);
            has_value = true;
        } else {
            for j in i..i + 8 {
                if null_bitmap.is_valid(j) {
                    let val_vec = _mm256_set1_epi32(values[j]);
                    max_vec = _mm256_max_epi32(max_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 8;
    }

    // Extract maximum from vector
    let max_arr: [i32; 8] = std::mem::transmute(max_vec);
    let mut max_val = *max_arr.iter().max().unwrap();

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            max_val = max_val.max(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(max_val)
}

// i64 AVX2 implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i64_eq_avx2(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi64x(target);
    let mut i = 0;

    // Process 4 elements at a time (256-bit / 64-bit = 4)
    while i + 4 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let cmp = _mm256_cmpeq_epi64(data, target_vec);
        let mask = _mm256_movemask_pd(_mm256_castsi256_pd(cmp));

        for j in 0..4 {
            if (mask & (1 << j)) != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    // Handle remainder
    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i64_lt_avx2(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi64x(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let cmp = _mm256_cmpgt_epi64(target_vec, data); // target > data means data < target
        let mask = _mm256_movemask_pd(_mm256_castsi256_pd(cmp));

        for j in 0..4 {
            if (mask & (1 << j)) != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    // Handle remainder
    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i64_gt_avx2(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi64x(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let cmp = _mm256_cmpgt_epi64(data, target_vec); // data > target
        let mask = _mm256_movemask_pd(_mm256_castsi256_pd(cmp));

        for j in 0..4 {
            if (mask & (1 << j)) != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    // Handle remainder
    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sum_i64_avx2(values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }

    let mut sum_vec = _mm256_setzero_si256();
    let mut i = 0;
    let mut has_value = false;

    // Process 4 elements at a time
    while i + 4 <= values.len() {
        let all_valid = (0..4).all(|j| null_bitmap.is_valid(i + j));

        if all_valid {
            let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
            sum_vec = _mm256_add_epi64(sum_vec, data);
            has_value = true;
        } else {
            for j in i..i + 4 {
                if null_bitmap.is_valid(j) {
                    let val_vec = _mm256_set1_epi64x(values[j]);
                    sum_vec = _mm256_add_epi64(sum_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    // Extract sum from vector
    let sum_arr: [i64; 4] = std::mem::transmute(sum_vec);
    let mut sum: i64 = sum_arr.iter().sum();

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            sum = sum.wrapping_add(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(sum)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn min_i64_avx2(values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
    if values.is_empty() {
        return None;
    }

    // Early exit for small arrays - AVX2 doesn't have native min for i64
    if values.len() < 8 {
        return ScalarBackend.min_i64(values, null_bitmap);
    }

    let mut min_val = i64::MAX;
    let mut has_value = false;
    let mut i = 0;

    // Process 4 elements at a time
    while i + 4 <= values.len() {
        let all_valid = (0..4).all(|j| null_bitmap.is_valid(i + j));

        if all_valid {
            for j in i..i + 4 {
                min_val = min_val.min(values[j]);
                has_value = true;
            }
        } else {
            for j in i..i + 4 {
                if null_bitmap.is_valid(j) {
                    min_val = min_val.min(values[j]);
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            min_val = min_val.min(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(min_val)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn max_i64_avx2(values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
    if values.is_empty() {
        return None;
    }

    // Early exit for small arrays - AVX2 doesn't have native max for i64
    if values.len() < 8 {
        return ScalarBackend.max_i64(values, null_bitmap);
    }

    let mut max_val = i64::MIN;
    let mut has_value = false;
    let mut i = 0;

    // Process 4 elements at a time
    while i + 4 <= values.len() {
        let all_valid = (0..4).all(|j| null_bitmap.is_valid(i + j));

        if all_valid {
            for j in i..i + 4 {
                max_val = max_val.max(values[j]);
                has_value = true;
            }
        } else {
            for j in i..i + 4 {
                if null_bitmap.is_valid(j) {
                    max_val = max_val.max(values[j]);
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            max_val = max_val.max(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(max_val)
}

// f32/f64 AVX2 implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f32_eq_avx2(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_ps(target);
    let mut i = 0;

    while i + 8 <= values.len() {
        let data = _mm256_loadu_ps(values[i..].as_ptr());
        let cmp = _mm256_cmp_ps(data, target_vec, _CMP_EQ_OQ);
        let mask = _mm256_movemask_ps(cmp);

        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }

    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f32_lt_avx2(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_ps(target);
    let mut i = 0;

    while i + 8 <= values.len() {
        let data = _mm256_loadu_ps(values[i..].as_ptr());
        let cmp = _mm256_cmp_ps(data, target_vec, _CMP_LT_OQ);
        let mask = _mm256_movemask_ps(cmp);

        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }

    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f32_gt_avx2(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_ps(target);
    let mut i = 0;

    while i + 8 <= values.len() {
        let data = _mm256_loadu_ps(values[i..].as_ptr());
        let cmp = _mm256_cmp_ps(data, target_vec, _CMP_GT_OQ);
        let mask = _mm256_movemask_ps(cmp);

        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }

    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sum_f32_avx2(values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }

    let mut sum_vec = _mm256_setzero_ps();
    let mut scalar_sum: f32 = 0.0;
    let mut i = 0;
    let mut has_value = false;

    while i + 8 <= values.len() {
        if (0..8).all(|j| null_bitmap.is_valid(i + j)) {
            let data = _mm256_loadu_ps(values[i..].as_ptr());
            sum_vec = _mm256_add_ps(sum_vec, data);
            has_value = true;
        } else {
            for j in 0..8 {
                if null_bitmap.is_valid(i + j) {
                    scalar_sum += values[i + j];
                    has_value = true;
                }
            }
        }
        i += 8;
    }

    let mut arr = [0.0; 8];
    _mm256_storeu_ps(arr.as_mut_ptr(), sum_vec);
    let vec_sum: f32 = arr.iter().sum();

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            scalar_sum += values[j];
            has_value = true;
        }
    }

    has_value.then_some(vec_sum + scalar_sum)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn min_f32_avx2(values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 8 {
        return ScalarBackend.min_f32(values, null_bitmap);
    }

    let mut min_vec = _mm256_set1_ps(f32::INFINITY);
    let mut nan_vec = _mm256_setzero_ps();
    let mut scalar_min = f32::INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 8 <= values.len() {
        if (0..8).all(|j| null_bitmap.is_valid(i + j)) {
            let data = _mm256_loadu_ps(values[i..].as_ptr());
            min_vec = _mm256_min_ps(min_vec, data);

            // NaN check
            let nans = _mm256_cmp_ps(data, data, _CMP_NEQ_UQ);
            nan_vec = _mm256_or_ps(nan_vec, nans);

            has_value = true;
        } else {
            for j in 0..8 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f32::NAN);
                    }
                    if v < scalar_min {
                        scalar_min = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 8;
    }

    let nan_mask = _mm256_movemask_ps(nan_vec);
    if nan_mask != 0 {
        return Some(f32::NAN);
    }

    let mut arr = [0.0; 8];
    _mm256_storeu_ps(arr.as_mut_ptr(), min_vec);
    let mut final_min = scalar_min;

    for &v in arr.iter() {
        if v < final_min {
            final_min = v;
        }
    }

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f32::NAN);
            }
            if v < final_min {
                final_min = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_min)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn max_f32_avx2(values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 8 {
        return ScalarBackend.max_f32(values, null_bitmap);
    }

    let mut max_vec = _mm256_set1_ps(f32::NEG_INFINITY);
    let mut nan_vec = _mm256_setzero_ps();
    let mut scalar_max = f32::NEG_INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 8 <= values.len() {
        if (0..8).all(|j| null_bitmap.is_valid(i + j)) {
            let data = _mm256_loadu_ps(values[i..].as_ptr());
            max_vec = _mm256_max_ps(max_vec, data);

            let nans = _mm256_cmp_ps(data, data, _CMP_NEQ_UQ);
            nan_vec = _mm256_or_ps(nan_vec, nans);

            has_value = true;
        } else {
            for j in 0..8 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f32::NAN);
                    }
                    if v > scalar_max {
                        scalar_max = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 8;
    }

    let nan_mask = _mm256_movemask_ps(nan_vec);
    if nan_mask != 0 {
        return Some(f32::NAN);
    }

    let mut arr = [0.0; 8];
    _mm256_storeu_ps(arr.as_mut_ptr(), max_vec);
    let mut final_max = scalar_max;

    for &v in arr.iter() {
        if v > final_max {
            final_max = v;
        }
    }

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f32::NAN);
            }
            if v > final_max {
                final_max = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_max)
}

// f64 implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f64_eq_avx2(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_pd(target);
    let mut i = 0;

    // 4 elements per iteration
    while i + 4 <= values.len() {
        let data = _mm256_loadu_pd(values[i..].as_ptr());
        let cmp = _mm256_cmp_pd(data, target_vec, _CMP_EQ_OQ);
        let mask = _mm256_movemask_pd(cmp);

        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f64_lt_avx2(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_pd(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = _mm256_loadu_pd(values[i..].as_ptr());
        let cmp = _mm256_cmp_pd(data, target_vec, _CMP_LT_OQ);
        let mask = _mm256_movemask_pd(cmp);

        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f64_gt_avx2(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::x86_64::*;

    let mut result = Vec::new();
    let target_vec = _mm256_set1_pd(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = _mm256_loadu_pd(values[i..].as_ptr());
        let cmp = _mm256_cmp_pd(data, target_vec, _CMP_GT_OQ);
        let mask = _mm256_movemask_pd(cmp);

        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sum_f64_avx2(values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }

    let mut sum_vec = _mm256_setzero_pd();
    let mut scalar_sum: f64 = 0.0;
    let mut i = 0;
    let mut has_value = false;

    while i + 4 <= values.len() {
        if (0..4).all(|j| null_bitmap.is_valid(i + j)) {
            let data = _mm256_loadu_pd(values[i..].as_ptr());
            sum_vec = _mm256_add_pd(sum_vec, data);
            has_value = true;
        } else {
            for j in 0..4 {
                if null_bitmap.is_valid(i + j) {
                    scalar_sum += values[i + j];
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    let mut arr = [0.0; 4];
    _mm256_storeu_pd(arr.as_mut_ptr(), sum_vec);
    let vec_sum: f64 = arr.iter().sum();

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            scalar_sum += values[j];
            has_value = true;
        }
    }

    has_value.then_some(vec_sum + scalar_sum)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn min_f64_avx2(values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 8 {
        return ScalarBackend.min_f64(values, null_bitmap);
    }

    let mut min_vec = _mm256_set1_pd(f64::INFINITY);
    let mut nan_vec = _mm256_setzero_pd();
    let mut scalar_min = f64::INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 4 <= values.len() {
        if (0..4).all(|j| null_bitmap.is_valid(i + j)) {
            let data = _mm256_loadu_pd(values[i..].as_ptr());
            min_vec = _mm256_min_pd(min_vec, data);

            // NaN check
            let nans = _mm256_cmp_pd(data, data, _CMP_NEQ_UQ);
            nan_vec = _mm256_or_pd(nan_vec, nans);

            has_value = true;
        } else {
            for j in 0..4 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f64::NAN);
                    }
                    if v < scalar_min {
                        scalar_min = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    let nan_mask = _mm256_movemask_pd(nan_vec);
    if nan_mask != 0 {
        return Some(f64::NAN);
    }

    let mut arr = [0.0; 4];
    _mm256_storeu_pd(arr.as_mut_ptr(), min_vec);
    let mut final_min = scalar_min;

    for &v in arr.iter() {
        if v < final_min {
            final_min = v;
        }
    }

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f64::NAN);
            }
            if v < final_min {
                final_min = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_min)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn max_f64_avx2(values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
    use std::arch::x86_64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 8 {
        return ScalarBackend.max_f64(values, null_bitmap);
    }

    let mut max_vec = _mm256_set1_pd(f64::NEG_INFINITY);
    let mut nan_vec = _mm256_setzero_pd();
    let mut scalar_max = f64::NEG_INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 4 <= values.len() {
        if (0..4).all(|j| null_bitmap.is_valid(i + j)) {
            let data = _mm256_loadu_pd(values[i..].as_ptr());
            max_vec = _mm256_max_pd(max_vec, data);

            let nans = _mm256_cmp_pd(data, data, _CMP_NEQ_UQ);
            nan_vec = _mm256_or_pd(nan_vec, nans);

            has_value = true;
        } else {
            for j in 0..4 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f64::NAN);
                    }
                    if v > scalar_max {
                        scalar_max = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    let nan_mask = _mm256_movemask_pd(nan_vec);
    if nan_mask != 0 {
        return Some(f64::NAN);
    }

    let mut arr = [0.0; 4];
    _mm256_storeu_pd(arr.as_mut_ptr(), max_vec);
    let mut final_max = scalar_max;

    for &v in arr.iter() {
        if v > final_max {
            final_max = v;
        }
    }

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f64::NAN);
            }
            if v > final_max {
                final_max = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_max)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn compare_bytes_avx2(a: &[u8], b: &[u8]) -> bool {
    use std::arch::x86_64::*;

    let mut i = 0;
    while i + 32 <= a.len() {
        let va = _mm256_loadu_si256(a[i..].as_ptr() as *const __m256i);
        let vb = _mm256_loadu_si256(b[i..].as_ptr() as *const __m256i);
        let cmp = _mm256_cmpeq_epi8(va, vb);

        if _mm256_movemask_epi8(cmp) != -1 {
            return false;
        }
        i += 32;
    }

    a[i..] == b[i..]
}

/// NEON backend for ARM64
#[cfg(target_arch = "aarch64")]
pub struct NeonBackend;

#[cfg(target_arch = "aarch64")]
// i32 AVX2 Implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i32_le_avx2(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi32(target);
    let mut i = 0;
    while i + 8 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        // LE(a, b) <=> !GT(a, b)
        let gt = _mm256_cmpgt_epi32(data, target_vec);
        let le = _mm256_andnot_si256(gt, _mm256_set1_epi32(-1)); // NOT gt
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(le));
        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i32_ge_avx2(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi32(target);
    let mut i = 0;
    while i + 8 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        // GE(a, b) <=> !LT(a, b) <=> !GT(b, a)
        // Check if target > data
        let lt = _mm256_cmpgt_epi32(target_vec, data);
        // a >= b is NOT (a < b)
        let ge = _mm256_andnot_si256(lt, _mm256_set1_epi32(-1));
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(ge));
        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i32_ne_avx2(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi32(target);
    let mut i = 0;
    while i + 8 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let eq = _mm256_cmpeq_epi32(data, target_vec);
        let ne = _mm256_andnot_si256(eq, _mm256_set1_epi32(-1));
        let mask = _mm256_movemask_ps(_mm256_castsi256_ps(ne));
        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

// i64 AVX2 Implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i64_le_avx2(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi64x(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        // LE(a, b) <=> !GT(a, b)
        let gt = _mm256_cmpgt_epi64(data, target_vec);
        let le = _mm256_andnot_si256(gt, _mm256_set1_epi64x(-1));
        let mask = _mm256_movemask_pd(_mm256_castsi256_pd(le));
        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i64_ge_avx2(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi64x(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        // GE(a, b) <=> !LT(a, b) <=> !GT(b, a)
        let lt = _mm256_cmpgt_epi64(target_vec, data);
        let ge = _mm256_andnot_si256(lt, _mm256_set1_epi64x(-1));
        let mask = _mm256_movemask_pd(_mm256_castsi256_pd(ge));
        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_i64_ne_avx2(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_epi64x(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = _mm256_loadu_si256(values[i..].as_ptr() as *const __m256i);
        let eq = _mm256_cmpeq_epi64(data, target_vec);
        let ne = _mm256_andnot_si256(eq, _mm256_set1_epi64x(-1));
        let mask = _mm256_movemask_pd(_mm256_castsi256_pd(ne));
        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

// f32 AVX2 Implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f32_le_avx2(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_ps(target);
    let mut i = 0;
    while i + 8 <= values.len() {
        let data = _mm256_loadu_ps(values[i..].as_ptr());
        let cmp = _mm256_cmp_ps(data, target_vec, _CMP_LE_OQ);
        let mask = _mm256_movemask_ps(cmp);
        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f32_ge_avx2(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_ps(target);
    let mut i = 0;
    while i + 8 <= values.len() {
        let data = _mm256_loadu_ps(values[i..].as_ptr());
        let cmp = _mm256_cmp_ps(data, target_vec, _CMP_GE_OQ);
        let mask = _mm256_movemask_ps(cmp);
        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f32_ne_avx2(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_ps(target);
    let mut i = 0;
    while i + 8 <= values.len() {
        let data = _mm256_loadu_ps(values[i..].as_ptr());
        let cmp = _mm256_cmp_ps(data, target_vec, _CMP_NEQ_OQ);
        let mask = _mm256_movemask_ps(cmp);
        if mask != 0 {
            for j in 0..8 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 8;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

// f64 AVX2 Implementations
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f64_le_avx2(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_pd(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = _mm256_loadu_pd(values[i..].as_ptr());
        let cmp = _mm256_cmp_pd(data, target_vec, _CMP_LE_OQ);
        let mask = _mm256_movemask_pd(cmp);
        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f64_ge_avx2(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_pd(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = _mm256_loadu_pd(values[i..].as_ptr());
        let cmp = _mm256_cmp_pd(data, target_vec, _CMP_GE_OQ);
        let mask = _mm256_movemask_pd(cmp);
        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn filter_f64_ne_avx2(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::x86_64::*;
    let mut result = Vec::new();
    let target_vec = _mm256_set1_pd(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = _mm256_loadu_pd(values[i..].as_ptr());
        let cmp = _mm256_cmp_pd(data, target_vec, _CMP_NEQ_OQ);
        let mask = _mm256_movemask_pd(cmp);
        if mask != 0 {
            for j in 0..4 {
                if (mask & (1 << j)) != 0 {
                    result.push(i + j);
                }
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

impl SimdBackend for NeonBackend {
    fn name(&self) -> &'static str {
        "NEON"
    }

    fn capability(&self) -> super::SimdCapability {
        super::SimdCapability::NEON
    }

    fn filter_i32_eq(&self, values: &[i32], target: i32) -> Vec<usize> {
        unsafe { filter_i32_eq_neon(values, target) }
    }

    fn filter_i32_lt(&self, values: &[i32], target: i32) -> Vec<usize> {
        unsafe { filter_i32_lt_neon(values, target) }
    }

    fn filter_i32_gt(&self, values: &[i32], target: i32) -> Vec<usize> {
        unsafe { filter_i32_gt_neon(values, target) }
    }

    fn sum_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        unsafe { sum_i32_neon(values, null_bitmap) }
    }

    fn min_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        unsafe { min_i32_neon(values, null_bitmap) }
    }

    fn max_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        unsafe { max_i32_neon(values, null_bitmap) }
    }

    // i64 filter operations
    fn filter_i64_eq(&self, values: &[i64], target: i64) -> Vec<usize> {
        unsafe { filter_i64_eq_neon(values, target) }
    }

    fn filter_i64_lt(&self, values: &[i64], target: i64) -> Vec<usize> {
        unsafe { filter_i64_lt_neon(values, target) }
    }

    fn filter_i64_gt(&self, values: &[i64], target: i64) -> Vec<usize> {
        unsafe { filter_i64_gt_neon(values, target) }
    }

    // i64 aggregate operations
    fn sum_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        unsafe { sum_i64_neon(values, null_bitmap) }
    }

    fn min_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        unsafe { min_i64_neon(values, null_bitmap) }
    }

    fn max_i64(&self, values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
        unsafe { max_i64_neon(values, null_bitmap) }
    }

    // f32 filter operations
    fn filter_f32_eq(&self, values: &[f32], target: f32) -> Vec<usize> {
        unsafe { filter_f32_eq_neon(values, target) }
    }

    fn filter_f32_lt(&self, values: &[f32], target: f32) -> Vec<usize> {
        unsafe { filter_f32_lt_neon(values, target) }
    }

    fn filter_f32_gt(&self, values: &[f32], target: f32) -> Vec<usize> {
        unsafe { filter_f32_gt_neon(values, target) }
    }

    // f32 aggregate operations
    fn sum_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        unsafe { sum_f32_neon(values, null_bitmap) }
    }

    fn min_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        unsafe { min_f32_neon(values, null_bitmap) }
    }

    fn max_f32(&self, values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
        unsafe { max_f32_neon(values, null_bitmap) }
    }

    // f64 filter operations
    fn filter_f64_eq(&self, values: &[f64], target: f64) -> Vec<usize> {
        unsafe { filter_f64_eq_neon(values, target) }
    }

    fn filter_f64_lt(&self, values: &[f64], target: f64) -> Vec<usize> {
        unsafe { filter_f64_lt_neon(values, target) }
    }

    fn filter_f64_gt(&self, values: &[f64], target: f64) -> Vec<usize> {
        unsafe { filter_f64_gt_neon(values, target) }
    }

    // f64 aggregate operations
    fn sum_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        unsafe { sum_f64_neon(values, null_bitmap) }
    }

    fn min_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        unsafe { min_f64_neon(values, null_bitmap) }
    }

    fn max_f64(&self, values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
        unsafe { max_f64_neon(values, null_bitmap) }
    }

    fn compare_bytes(&self, a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        unsafe { compare_bytes_neon(a, b) }
    }

    fn find_byte(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        haystack.iter().position(|&b| b == needle)
    }

    fn filter_i32_le(&self, values: &[i32], target: i32) -> Vec<usize> {
        unsafe { filter_i32_le_neon(values, target) }
    }
    fn filter_i32_ge(&self, values: &[i32], target: i32) -> Vec<usize> {
        unsafe { filter_i32_ge_neon(values, target) }
    }
    fn filter_i32_ne(&self, values: &[i32], target: i32) -> Vec<usize> {
        unsafe { filter_i32_ne_neon(values, target) }
    }

    fn filter_i64_le(&self, values: &[i64], target: i64) -> Vec<usize> {
        unsafe { filter_i64_le_neon(values, target) }
    }
    fn filter_i64_ge(&self, values: &[i64], target: i64) -> Vec<usize> {
        unsafe { filter_i64_ge_neon(values, target) }
    }
    fn filter_i64_ne(&self, values: &[i64], target: i64) -> Vec<usize> {
        unsafe { filter_i64_ne_neon(values, target) }
    }

    fn filter_f32_le(&self, values: &[f32], target: f32) -> Vec<usize> {
        unsafe { filter_f32_le_neon(values, target) }
    }
    fn filter_f32_ge(&self, values: &[f32], target: f32) -> Vec<usize> {
        unsafe { filter_f32_ge_neon(values, target) }
    }
    fn filter_f32_ne(&self, values: &[f32], target: f32) -> Vec<usize> {
        unsafe { filter_f32_ne_neon(values, target) }
    }

    fn filter_f64_le(&self, values: &[f64], target: f64) -> Vec<usize> {
        unsafe { filter_f64_le_neon(values, target) }
    }
    fn filter_f64_ge(&self, values: &[f64], target: f64) -> Vec<usize> {
        unsafe { filter_f64_ge_neon(values, target) }
    }
    fn filter_f64_ne(&self, values: &[f64], target: f64) -> Vec<usize> {
        unsafe { filter_f64_ne_neon(values, target) }
    }
}

// NEON implementations
#[cfg(target_arch = "aarch64")]
unsafe fn filter_i32_eq_neon(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_s32(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = vld1q_s32(values[i..].as_ptr());
        let cmp = vceqq_s32(data, target_vec);

        // Extract mask
        let mask: [u32; 4] = std::mem::transmute(cmp);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i32_lt_neon(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_s32(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = vld1q_s32(values[i..].as_ptr());
        let cmp = vcltq_s32(data, target_vec); // data < target

        let mask: [u32; 4] = std::mem::transmute(cmp);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i32_gt_neon(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_s32(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = vld1q_s32(values[i..].as_ptr());
        let cmp = vcgtq_s32(data, target_vec); // data > target

        let mask: [u32; 4] = std::mem::transmute(cmp);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn sum_i32_neon(values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
    use std::arch::aarch64::*;

    // Early exit for small datasets - use scalar
    if values.len() < 16 {
        return ScalarBackend.sum_i32(values, null_bitmap);
    }

    let mut sum_vec = vdupq_n_s32(0);
    let mut has_value = false;
    let mut i = 0;

    // Process in larger chunks when all values are valid
    while i + 16 <= values.len() {
        // Check if next 16 values are all valid
        let all_valid = (i..i + 16).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            // Process 4 chunks of 4 values each
            for _ in 0..4 {
                let data = vld1q_s32(values[i..].as_ptr());
                sum_vec = vaddq_s32(sum_vec, data);
                i += 4;
            }
            has_value = true;
        } else {
            // Fall back to scalar for this chunk
            for j in i..i + 16 {
                if null_bitmap.is_valid(j) {
                    // Accumulate in vector
                    let val_vec = vdupq_n_s32(values[j]);
                    sum_vec = vaddq_s32(sum_vec, val_vec);
                    has_value = true;
                }
            }
            i += 16;
        }
    }

    // Process remaining values in groups of 4
    while i + 4 <= values.len() {
        let all_valid = (i..i + 4).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            let data = vld1q_s32(values[i..].as_ptr());
            sum_vec = vaddq_s32(sum_vec, data);
            has_value = true;
        } else {
            // Scalar for partial chunk
            for j in i..i + 4 {
                if null_bitmap.is_valid(j) {
                    let val_vec = vdupq_n_s32(values[j]);
                    sum_vec = vaddq_s32(sum_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    // Efficient horizontal sum using pairwise addition
    let sum_vec = vpaddq_s32(sum_vec, sum_vec); // [a+b, c+d, a+b, c+d]
    let sum_vec = vpaddq_s32(sum_vec, sum_vec); // [a+b+c+d, ...]
    let total_simd = vgetq_lane_s32(sum_vec, 0) as i64;

    // Handle remainder
    let mut total = total_simd;
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            total += values[j] as i64;
            has_value = true;
        }
    }

    has_value.then_some(total as i32)
}

#[cfg(target_arch = "aarch64")]
unsafe fn min_i32_neon(values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }

    // Early exit for small datasets
    if values.len() < 16 {
        return ScalarBackend.min_i32(values, null_bitmap);
    }

    let mut min_vec = vdupq_n_s32(i32::MAX);
    let mut has_value = false;
    let mut i = 0;

    // Process in chunks of 16
    while i + 16 <= values.len() {
        let all_valid = (i..i + 16).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            for _ in 0..4 {
                let data = vld1q_s32(values[i..].as_ptr());
                min_vec = vminq_s32(min_vec, data);
                i += 4;
            }
            has_value = true;
        } else {
            for j in i..i + 16 {
                if null_bitmap.is_valid(j) {
                    let val_vec = vdupq_n_s32(values[j]);
                    min_vec = vminq_s32(min_vec, val_vec);
                    has_value = true;
                }
            }
            i += 16;
        }
    }

    // Process remaining in groups of 4
    while i + 4 <= values.len() {
        let all_valid = (i..i + 4).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            let data = vld1q_s32(values[i..].as_ptr());
            min_vec = vminq_s32(min_vec, data);
            has_value = true;
        } else {
            for j in i..i + 4 {
                if null_bitmap.is_valid(j) {
                    let val_vec = vdupq_n_s32(values[j]);
                    min_vec = vminq_s32(min_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    // Extract minimum using horizontal min
    let min_val = vminvq_s32(min_vec);
    let mut result = min_val;

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            result = result.min(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(result)
}

#[cfg(target_arch = "aarch64")]
unsafe fn max_i32_neon(values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }

    // Early exit for small datasets
    if values.len() < 16 {
        return ScalarBackend.max_i32(values, null_bitmap);
    }

    let mut max_vec = vdupq_n_s32(i32::MIN);
    let mut has_value = false;
    let mut i = 0;

    // Process in chunks of 16
    while i + 16 <= values.len() {
        let all_valid = (i..i + 16).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            for _ in 0..4 {
                let data = vld1q_s32(values[i..].as_ptr());
                max_vec = vmaxq_s32(max_vec, data);
                i += 4;
            }
            has_value = true;
        } else {
            for j in i..i + 16 {
                if null_bitmap.is_valid(j) {
                    let val_vec = vdupq_n_s32(values[j]);
                    max_vec = vmaxq_s32(max_vec, val_vec);
                    has_value = true;
                }
            }
            i += 16;
        }
    }

    // Process remaining in groups of 4
    while i + 4 <= values.len() {
        let all_valid = (i..i + 4).all(|idx| null_bitmap.is_valid(idx));

        if all_valid {
            let data = vld1q_s32(values[i..].as_ptr());
            max_vec = vmaxq_s32(max_vec, data);
            has_value = true;
        } else {
            for j in i..i + 4 {
                if null_bitmap.is_valid(j) {
                    let val_vec = vdupq_n_s32(values[j]);
                    max_vec = vmaxq_s32(max_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    // Extract maximum using horizontal max
    let max_val = vmaxvq_s32(max_vec);
    let mut result = max_val;

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            result = result.max(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(result)
}

// i64 NEON implementations
#[cfg(target_arch = "aarch64")]
unsafe fn filter_i64_eq_neon(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_s64(target);
    let mut i = 0;

    // Process 2 elements at a time (128-bit / 64-bit = 2)
    while i + 2 <= values.len() {
        let data = vld1q_s64(values[i..].as_ptr());
        let cmp = vceqq_s64(data, target_vec);

        // Extract mask
        let mask_arr: [u64; 2] = std::mem::transmute(cmp);
        if mask_arr[0] != 0 {
            result.push(i);
        }
        if mask_arr[1] != 0 {
            result.push(i + 1);
        }
        i += 2;
    }

    // Handle remainder
    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i64_lt_neon(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_s64(target);
    let mut i = 0;

    while i + 2 <= values.len() {
        let data = vld1q_s64(values[i..].as_ptr());
        let cmp = vcltq_s64(data, target_vec); // data < target

        let mask_arr: [u64; 2] = std::mem::transmute(cmp);
        if mask_arr[0] != 0 {
            result.push(i);
        }
        if mask_arr[1] != 0 {
            result.push(i + 1);
        }
        i += 2;
    }

    // Handle remainder
    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i64_gt_neon(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_s64(target);
    let mut i = 0;

    while i + 2 <= values.len() {
        let data = vld1q_s64(values[i..].as_ptr());
        let cmp = vcgtq_s64(data, target_vec); // data > target

        let mask_arr: [u64; 2] = std::mem::transmute(cmp);
        if mask_arr[0] != 0 {
            result.push(i);
        }
        if mask_arr[1] != 0 {
            result.push(i + 1);
        }
        i += 2;
    }

    // Handle remainder
    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn sum_i64_neon(values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }

    let mut sum_vec = vdupq_n_s64(0);
    let mut i = 0;
    let mut has_value = false;

    // Process 2 elements at a time
    while i + 2 <= values.len() {
        let all_valid = (0..2).all(|j| null_bitmap.is_valid(i + j));

        if all_valid {
            let data = vld1q_s64(values[i..].as_ptr());
            sum_vec = vaddq_s64(sum_vec, data);
            has_value = true;
        } else {
            for j in i..i + 2 {
                if null_bitmap.is_valid(j) {
                    let val_vec = vdupq_n_s64(values[j]);
                    sum_vec = vaddq_s64(sum_vec, val_vec);
                    has_value = true;
                }
            }
        }
        i += 2;
    }

    // Extract sum from vector
    let sum_arr: [i64; 2] = std::mem::transmute(sum_vec);
    let mut sum: i64 = sum_arr[0].wrapping_add(sum_arr[1]);

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            sum = sum.wrapping_add(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(sum)
}

#[cfg(target_arch = "aarch64")]
unsafe fn min_i64_neon(values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
    if values.is_empty() {
        return None;
    }

    // Early exit for small arrays - NEON doesn't have native min for i64
    if values.len() < 4 {
        return ScalarBackend.min_i64(values, null_bitmap);
    }

    let mut min_val = i64::MAX;
    let mut has_value = false;
    let mut i = 0;

    // Process 2 elements at a time
    while i + 2 <= values.len() {
        let all_valid = (0..2).all(|j| null_bitmap.is_valid(i + j));

        if all_valid {
            for j in i..i + 2 {
                min_val = min_val.min(values[j]);
                has_value = true;
            }
        } else {
            for j in i..i + 2 {
                if null_bitmap.is_valid(j) {
                    min_val = min_val.min(values[j]);
                    has_value = true;
                }
            }
        }
        i += 2;
    }

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            min_val = min_val.min(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(min_val)
}

#[cfg(target_arch = "aarch64")]
unsafe fn max_i64_neon(values: &[i64], null_bitmap: &NullBitmap) -> Option<i64> {
    if values.is_empty() {
        return None;
    }

    // Early exit for small arrays - NEON doesn't have native max for i64
    if values.len() < 4 {
        return ScalarBackend.max_i64(values, null_bitmap);
    }

    let mut max_val = i64::MIN;
    let mut has_value = false;
    let mut i = 0;

    // Process 2 elements at a time
    while i + 2 <= values.len() {
        let all_valid = (0..2).all(|j| null_bitmap.is_valid(i + j));

        if all_valid {
            for j in i..i + 2 {
                max_val = max_val.max(values[j]);
                has_value = true;
            }
        } else {
            for j in i..i + 2 {
                if null_bitmap.is_valid(j) {
                    max_val = max_val.max(values[j]);
                    has_value = true;
                }
            }
        }
        i += 2;
    }

    // Handle remainder
    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            max_val = max_val.max(values[j]);
            has_value = true;
        }
    }

    has_value.then_some(max_val)
}

// f32/f64 NEON implementations

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f32_eq_neon(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_f32(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = vld1q_f32(values[i..].as_ptr());
        let cmp = vceqq_f32(data, target_vec);
        let mask_arr: [u32; 4] = std::mem::transmute(cmp);

        for j in 0..4 {
            if mask_arr[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f32_lt_neon(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_f32(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = vld1q_f32(values[i..].as_ptr());
        let cmp = vcltq_f32(data, target_vec);
        let mask_arr: [u32; 4] = std::mem::transmute(cmp);

        for j in 0..4 {
            if mask_arr[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f32_gt_neon(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_f32(target);
    let mut i = 0;

    while i + 4 <= values.len() {
        let data = vld1q_f32(values[i..].as_ptr());
        let cmp = vcgtq_f32(data, target_vec);
        let mask_arr: [u32; 4] = std::mem::transmute(cmp);

        for j in 0..4 {
            if mask_arr[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }

    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn sum_f32_neon(values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }

    let mut sum_vec = vdupq_n_f32(0.0);
    let mut scalar_sum: f32 = 0.0;
    let mut i = 0;
    let mut has_value = false;

    while i + 4 <= values.len() {
        if (0..4).all(|j| null_bitmap.is_valid(i + j)) {
            let data = vld1q_f32(values[i..].as_ptr());
            sum_vec = vaddq_f32(sum_vec, data);
            has_value = true;
        } else {
            for j in 0..4 {
                if null_bitmap.is_valid(i + j) {
                    scalar_sum += values[i + j];
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    let vec_sum = vaddvq_f32(sum_vec);
    let mut final_sum = vec_sum + scalar_sum;

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            final_sum += values[j];
            has_value = true;
        }
    }

    has_value.then_some(final_sum)
}

#[cfg(target_arch = "aarch64")]
unsafe fn min_f32_neon(values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 4 {
        return ScalarBackend.min_f32(values, null_bitmap);
    }

    let mut min_vec = vdupq_n_f32(f32::INFINITY);
    let mut nan_check_vec = vdupq_n_u32(!0); // All ones
    let mut scalar_min = f32::INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 4 <= values.len() {
        if (0..4).all(|j| null_bitmap.is_valid(i + j)) {
            let data = vld1q_f32(values[i..].as_ptr());
            min_vec = vminq_f32(min_vec, data);

            // NaN check: if NaN, vceqq is 0. If not NaN, -1.
            let eq = vceqq_f32(data, data);
            nan_check_vec = vandq_u32(nan_check_vec, eq);

            has_value = true;
        } else {
            for j in 0..4 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f32::NAN);
                    }
                    if v < scalar_min {
                        scalar_min = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    let min_check = vminvq_u32(nan_check_vec);
    if min_check == 0 {
        return Some(f32::NAN);
    }

    let vec_min = vminvq_f32(min_vec);
    let mut final_min = if vec_min < scalar_min {
        vec_min
    } else {
        scalar_min
    };

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f32::NAN);
            }
            if v < final_min {
                final_min = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_min)
}

#[cfg(target_arch = "aarch64")]
unsafe fn max_f32_neon(values: &[f32], null_bitmap: &NullBitmap) -> Option<f32> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 4 {
        return ScalarBackend.max_f32(values, null_bitmap);
    }

    let mut max_vec = vdupq_n_f32(f32::NEG_INFINITY);
    let mut nan_check_vec = vdupq_n_u32(!0);
    let mut scalar_max = f32::NEG_INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 4 <= values.len() {
        if (0..4).all(|j| null_bitmap.is_valid(i + j)) {
            let data = vld1q_f32(values[i..].as_ptr());
            max_vec = vmaxq_f32(max_vec, data);

            let eq = vceqq_f32(data, data);
            nan_check_vec = vandq_u32(nan_check_vec, eq);

            has_value = true;
        } else {
            for j in 0..4 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f32::NAN);
                    }
                    if v > scalar_max {
                        scalar_max = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 4;
    }

    let min_check = vminvq_u32(nan_check_vec);
    if min_check == 0 {
        return Some(f32::NAN);
    }

    let vec_max = vmaxvq_f32(max_vec);
    let mut final_max = if vec_max > scalar_max {
        vec_max
    } else {
        scalar_max
    };

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f32::NAN);
            }
            if v > final_max {
                final_max = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_max)
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f64_eq_neon(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_f64(target);
    let mut i = 0;

    while i + 2 <= values.len() {
        let data = vld1q_f64(values[i..].as_ptr());
        let cmp = vceqq_f64(data, target_vec);
        let mask: [u64; 2] = std::mem::transmute(cmp);

        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }

    for j in i..values.len() {
        if values[j] == target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f64_lt_neon(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_f64(target);
    let mut i = 0;

    while i + 2 <= values.len() {
        let data = vld1q_f64(values[i..].as_ptr());
        let cmp = vcltq_f64(data, target_vec);
        let mask: [u64; 2] = std::mem::transmute(cmp);

        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }

    for j in i..values.len() {
        if values[j] < target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f64_gt_neon(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::aarch64::*;

    let mut result = Vec::new();
    let target_vec = vdupq_n_f64(target);
    let mut i = 0;

    while i + 2 <= values.len() {
        let data = vld1q_f64(values[i..].as_ptr());
        let cmp = vcgtq_f64(data, target_vec);
        let mask: [u64; 2] = std::mem::transmute(cmp);

        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }

    for j in i..values.len() {
        if values[j] > target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn sum_f64_neon(values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }

    let mut sum_vec = vdupq_n_f64(0.0);
    let mut scalar_sum: f64 = 0.0;
    let mut i = 0;
    let mut has_value = false;

    while i + 2 <= values.len() {
        if (0..2).all(|j| null_bitmap.is_valid(i + j)) {
            let data = vld1q_f64(values[i..].as_ptr());
            sum_vec = vaddq_f64(sum_vec, data);
            has_value = true;
        } else {
            for j in 0..2 {
                if null_bitmap.is_valid(i + j) {
                    scalar_sum += values[i + j];
                    has_value = true;
                }
            }
        }
        i += 2;
    }

    let vec_sum = vaddvq_f64(sum_vec);
    let mut final_sum = vec_sum + scalar_sum;

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            final_sum += values[j];
            has_value = true;
        }
    }

    has_value.then_some(final_sum)
}

#[cfg(target_arch = "aarch64")]
unsafe fn min_f64_neon(values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 4 {
        return ScalarBackend.min_f64(values, null_bitmap);
    }

    let mut min_vec = vdupq_n_f64(f64::INFINITY);
    let mut nan_check_vec = vdupq_n_u64(!0); // All ones (-1)
    let mut scalar_min = f64::INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 2 <= values.len() {
        if (0..2).all(|j| null_bitmap.is_valid(i + j)) {
            let data = vld1q_f64(values[i..].as_ptr());
            min_vec = vminq_f64(min_vec, data);

            let eq = vceqq_f64(data, data);
            // Since vceqq_f64 -> uint64x2_t, we can use vandq_u64 directly
            nan_check_vec = vandq_u64(nan_check_vec, eq);

            has_value = true;
        } else {
            for j in 0..2 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f64::NAN);
                    }
                    if v < scalar_min {
                        scalar_min = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 2;
    }

    // Check NaNs: if any 0 in nan_check_vec
    let check: [u64; 2] = std::mem::transmute(nan_check_vec);
    if check[0] == 0 || check[1] == 0 {
        return Some(f64::NAN);
    }

    let vec_min = vminvq_f64(min_vec);
    let mut final_min = if vec_min < scalar_min {
        vec_min
    } else {
        scalar_min
    };

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f64::NAN);
            }
            if v < final_min {
                final_min = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_min)
}

#[cfg(target_arch = "aarch64")]
unsafe fn max_f64_neon(values: &[f64], null_bitmap: &NullBitmap) -> Option<f64> {
    use std::arch::aarch64::*;

    if values.is_empty() {
        return None;
    }
    if values.len() < 4 {
        return ScalarBackend.max_f64(values, null_bitmap);
    }

    let mut max_vec = vdupq_n_f64(f64::NEG_INFINITY);
    let mut nan_check_vec = vdupq_n_u64(!0);
    let mut scalar_max = f64::NEG_INFINITY;
    let mut i = 0;
    let mut has_value = false;

    while i + 2 <= values.len() {
        if (0..2).all(|j| null_bitmap.is_valid(i + j)) {
            let data = vld1q_f64(values[i..].as_ptr());
            max_vec = vmaxq_f64(max_vec, data);

            let eq = vceqq_f64(data, data);
            nan_check_vec = vandq_u64(nan_check_vec, eq);

            has_value = true;
        } else {
            for j in 0..2 {
                if null_bitmap.is_valid(i + j) {
                    let v = values[i + j];
                    if v.is_nan() {
                        return Some(f64::NAN);
                    }
                    if v > scalar_max {
                        scalar_max = v;
                    }
                    has_value = true;
                }
            }
        }
        i += 2;
    }

    let check: [u64; 2] = std::mem::transmute(nan_check_vec);
    if check[0] == 0 || check[1] == 0 {
        return Some(f64::NAN);
    }

    let vec_max = vmaxvq_f64(max_vec);
    let mut final_max = if vec_max > scalar_max {
        vec_max
    } else {
        scalar_max
    };

    for j in i..values.len() {
        if null_bitmap.is_valid(j) {
            let v = values[j];
            if v.is_nan() {
                return Some(f64::NAN);
            }
            if v > final_max {
                final_max = v;
            }
            has_value = true;
        }
    }

    has_value.then_some(final_max)
}

#[cfg(target_arch = "aarch64")]
unsafe fn compare_bytes_neon(a: &[u8], b: &[u8]) -> bool {
    use std::arch::aarch64::*;

    // Only use SIMD for strings >= 64 bytes
    // For smaller strings, Rust's memcmp is faster
    if a.len() < 64 {
        return a == b;
    }

    let mut i = 0;

    // Process 64 bytes at a time (4x 16-byte vectors)
    while i + 64 <= a.len() {
        for _ in 0..4 {
            let va = vld1q_u8(a[i..].as_ptr());
            let vb = vld1q_u8(b[i..].as_ptr());
            let cmp = vceqq_u8(va, vb);

            // Quick check using vminvq_u8 - if any byte differs, result < 0xFF
            let min_val = vminvq_u8(cmp);
            if min_val != 0xFF {
                return false;
            }
            i += 16;
        }
    }

    // Handle remainder with scalar comparison
    a[i..] == b[i..]
}

// i32 NEON Implementations

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i32_le_neon(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_s32(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = vld1q_s32(values[i..].as_ptr());
        let cmp = vcleq_s32(data, target_vec);
        let mask: [u32; 4] = std::mem::transmute(cmp);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i32_ge_neon(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_s32(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = vld1q_s32(values[i..].as_ptr());
        let cmp = vcgeq_s32(data, target_vec);
        let mask: [u32; 4] = std::mem::transmute(cmp);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i32_ne_neon(values: &[i32], target: i32) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_s32(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = vld1q_s32(values[i..].as_ptr());
        let eq = vceqq_s32(data, target_vec);
        let ne = vmvnq_u32(eq); // NOT eq
        let mask: [u32; 4] = std::mem::transmute(ne);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

// i64 NEON Implementations
#[cfg(target_arch = "aarch64")]
unsafe fn filter_i64_le_neon(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_s64(target);
    let mut i = 0;
    while i + 2 <= values.len() {
        let data = vld1q_s64(values[i..].as_ptr());
        let cmp = vcleq_s64(data, target_vec);
        let mask: [u64; 2] = std::mem::transmute(cmp);
        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i64_ge_neon(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_s64(target);
    let mut i = 0;
    while i + 2 <= values.len() {
        let data = vld1q_s64(values[i..].as_ptr());
        let cmp = vcgeq_s64(data, target_vec);
        let mask: [u64; 2] = std::mem::transmute(cmp);
        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_i64_ne_neon(values: &[i64], target: i64) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_s64(target);
    let mut i = 0;
    while i + 2 <= values.len() {
        let data = vld1q_s64(values[i..].as_ptr());
        let eq = vceqq_s64(data, target_vec);
        // Bitwise NOT, but we have u64 vector.
        // vmvnq_u32 works on 128-bit regs, just need cast.
        let ne = vmvnq_u32(vreinterpretq_u32_u64(eq));
        let mask64: [u64; 2] = std::mem::transmute(ne);
        for j in 0..2 {
            if mask64[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

// f32 NEON Implementations
#[cfg(target_arch = "aarch64")]
unsafe fn filter_f32_le_neon(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_f32(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = vld1q_f32(values[i..].as_ptr());
        let cmp = vcleq_f32(data, target_vec);
        let mask: [u32; 4] = std::mem::transmute(cmp);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f32_ge_neon(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_f32(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = vld1q_f32(values[i..].as_ptr());
        let cmp = vcgeq_f32(data, target_vec);
        let mask: [u32; 4] = std::mem::transmute(cmp);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f32_ne_neon(values: &[f32], target: f32) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_f32(target);
    let mut i = 0;
    while i + 4 <= values.len() {
        let data = vld1q_f32(values[i..].as_ptr());
        let eq = vceqq_f32(data, target_vec);
        let ne = vmvnq_u32(eq);
        let mask: [u32; 4] = std::mem::transmute(ne);
        for j in 0..4 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 4;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

// f64 NEON Implementations
#[cfg(target_arch = "aarch64")]
unsafe fn filter_f64_le_neon(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_f64(target);
    let mut i = 0;
    while i + 2 <= values.len() {
        let data = vld1q_f64(values[i..].as_ptr());
        let cmp = vcleq_f64(data, target_vec);
        let mask: [u64; 2] = std::mem::transmute(cmp);
        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }
    for j in i..values.len() {
        if values[j] <= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f64_ge_neon(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_f64(target);
    let mut i = 0;
    while i + 2 <= values.len() {
        let data = vld1q_f64(values[i..].as_ptr());
        let cmp = vcgeq_f64(data, target_vec);
        let mask: [u64; 2] = std::mem::transmute(cmp);
        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }
    for j in i..values.len() {
        if values[j] >= target {
            result.push(j);
        }
    }
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn filter_f64_ne_neon(values: &[f64], target: f64) -> Vec<usize> {
    use std::arch::aarch64::*;
    let mut result = Vec::new();
    let target_vec = vdupq_n_f64(target);
    let mut i = 0;
    while i + 2 <= values.len() {
        let data = vld1q_f64(values[i..].as_ptr());
        let eq = vceqq_f64(data, target_vec);
        let ne = vmvnq_u32(vreinterpretq_u32_u64(eq));
        let mask: [u64; 2] = std::mem::transmute(ne); // bitcast back to u64
        for j in 0..2 {
            if mask[j] != 0 {
                result.push(i + j);
            }
        }
        i += 2;
    }
    for j in i..values.len() {
        if values[j] != target {
            result.push(j);
        }
    }
    result
}

/// Get the optimal SIMD backend for the current CPU
pub fn get_simd_backend() -> Box<dyn SimdBackend> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return Box::new(Avx2Backend);
        }
        return Box::new(ScalarBackend);
    }

    #[cfg(target_arch = "aarch64")]
    {
        Box::new(NeonBackend)
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        Box::new(ScalarBackend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_selection() {
        let backend = get_simd_backend();
        println!("Selected backend: {}", backend.name());
    }

    #[test]
    fn test_filter_correctness() {
        let backend = get_simd_backend();
        let values: Vec<i32> = (0..1000).collect();

        let result = backend.filter_i32_eq(&values, 500);
        assert_eq!(result, vec![500]);

        let result = backend.filter_i32_lt(&values, 10);
        assert_eq!(result.len(), 10);

        let result = backend.filter_i32_gt(&values, 990);
        assert_eq!(result.len(), 9);
    }

    #[test]
    fn test_sum_correctness() {
        let backend = get_simd_backend();
        let values: Vec<i32> = (1..=100).collect();
        let null_bitmap = NullBitmap::new_all_valid(100);

        let result = backend.sum_i32(&values, &null_bitmap);
        assert_eq!(result, Some(5050)); // Sum of 1 to 100
    }

    #[test]
    fn test_compare_bytes() {
        let backend = get_simd_backend();

        let a = b"Hello, World!";
        let b = b"Hello, World!";
        let c = b"Hello, Rust!";

        assert!(backend.compare_bytes(a, b));
        assert!(!backend.compare_bytes(a, c));
    }

    #[test]
    fn test_min_max_correctness() {
        let backend = get_simd_backend();
        let values: Vec<i32> = vec![5, 2, 8, 1, 9, 3, 7, 4, 6];
        let null_bitmap = NullBitmap::new_all_valid(9);

        let min_result = backend.min_i32(&values, &null_bitmap);
        assert_eq!(min_result, Some(1));

        let max_result = backend.max_i32(&values, &null_bitmap);
        assert_eq!(max_result, Some(9));
    }

    #[test]
    fn test_min_max_with_nulls() {
        let backend = get_simd_backend();
        let values: Vec<i32> = vec![5, 2, 8, 1, 9, 3, 7, 4, 6];
        let mut null_bitmap = NullBitmap::new_all_valid(9);
        null_bitmap.set_null(3); // Exclude the minimum value (1)
        null_bitmap.set_null(4); // Exclude the maximum value (9)

        let min_result = backend.min_i32(&values, &null_bitmap);
        assert_eq!(min_result, Some(2));

        let max_result = backend.max_i32(&values, &null_bitmap);
        assert_eq!(max_result, Some(8));
    }
}
