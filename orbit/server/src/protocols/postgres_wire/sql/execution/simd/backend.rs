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

    // String operations
    fn compare_bytes(&self, a: &[u8], b: &[u8]) -> bool;
    fn find_byte(&self, haystack: &[u8], needle: u8) -> Option<usize>;
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

    fn compare_bytes(&self, a: &[u8], b: &[u8]) -> bool {
        a == b
    }

    fn find_byte(&self, haystack: &[u8], needle: u8) -> Option<usize> {
        haystack.iter().position(|&b| b == needle)
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
        ScalarBackend.min_i32(values, null_bitmap) // TODO: SIMD implementation
    }

    fn max_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        ScalarBackend.max_i32(values, null_bitmap) // TODO: SIMD implementation
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
        ScalarBackend.min_i32(values, null_bitmap)
    }

    fn max_i32(&self, values: &[i32], null_bitmap: &NullBitmap) -> Option<i32> {
        ScalarBackend.max_i32(values, null_bitmap)
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
}
