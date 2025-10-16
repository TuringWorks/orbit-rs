//! SIMD-optimized operations for vector processing and analytics
//!
//! This module provides platform-specific SIMD optimizations for:
//! - Vector distance calculations (cosine, euclidean, dot product)
//! - Aggregation functions (SUM, AVG, COUNT)
//! - Bulk data processing operations
//!
//! Implements Phase 9.4 SIMD optimizations with automatic fallback to scalar operations
//! for platforms without SIMD support.

// Note: std::ops traits are available if needed for future generic implementations

/// Configuration for SIMD operations
#[derive(Debug, Clone, Copy)]
pub struct SimdConfig {
    /// Enable AVX2 instructions (x86_64)
    pub enable_avx2: bool,
    /// Enable AVX-512 instructions (x86_64)
    pub enable_avx512: bool,
    /// Enable NEON instructions (ARM)
    pub enable_neon: bool,
    /// Minimum vector size for SIMD (use scalar for smaller)
    pub min_simd_size: usize,
}

impl Default for SimdConfig {
    fn default() -> Self {
        Self {
            enable_avx2: cfg!(target_feature = "avx2"),
            enable_avx512: cfg!(target_feature = "avx512f"),
            enable_neon: cfg!(target_feature = "neon"),
            min_simd_size: 32, // Use SIMD for vectors with at least 32 elements
        }
    }
}

/// SIMD-optimized vector distance calculations
pub struct SimdVectorOps;

impl SimdVectorOps {
    /// Calculate dot product between two f32 vectors using SIMD
    ///
    /// This is the core operation for many vector similarity metrics.
    /// Uses platform-specific SIMD instructions when available.
    pub fn dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let len = a.len();

        // For small vectors, use scalar operations
        if len < SimdConfig::default().min_simd_size {
            return Self::dot_product_scalar(a, b);
        }

        // Platform-specific SIMD implementations
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { Self::dot_product_avx2(a, b) };
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if cfg!(target_feature = "neon") {
                return unsafe { Self::dot_product_neon(a, b) };
            }
        }

        // Fallback to scalar implementation
        Self::dot_product_scalar(a, b)
    }

    /// Calculate euclidean distance (L2 norm) between two f32 vectors using SIMD
    pub fn euclidean_distance_f32(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }

        let len = a.len();

        if len < SimdConfig::default().min_simd_size {
            return Self::euclidean_distance_scalar(a, b);
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { Self::euclidean_distance_avx2(a, b) };
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if cfg!(target_feature = "neon") {
                return unsafe { Self::euclidean_distance_neon(a, b) };
            }
        }

        Self::euclidean_distance_scalar(a, b)
    }

    /// Calculate cosine similarity between two f32 vectors using SIMD
    pub fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let len = a.len();

        if len < SimdConfig::default().min_simd_size {
            return Self::cosine_similarity_scalar(a, b);
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { Self::cosine_similarity_avx2(a, b) };
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if cfg!(target_feature = "neon") {
                return unsafe { Self::cosine_similarity_neon(a, b) };
            }
        }

        Self::cosine_similarity_scalar(a, b)
    }

    /// Calculate Manhattan distance (L1 norm) between two f32 vectors
    pub fn manhattan_distance_f32(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }

        let len = a.len();

        if len < SimdConfig::default().min_simd_size {
            return Self::manhattan_distance_scalar(a, b);
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { Self::manhattan_distance_avx2(a, b) };
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if cfg!(target_feature = "neon") {
                return unsafe { Self::manhattan_distance_neon(a, b) };
            }
        }

        Self::manhattan_distance_scalar(a, b)
    }

    // ==================== Scalar Fallback Implementations ====================

    #[inline]
    fn dot_product_scalar(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    #[inline]
    fn euclidean_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt()
    }

    #[inline]
    fn cosine_similarity_scalar(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }

        dot_product / (magnitude_a * magnitude_b)
    }

    #[inline]
    fn manhattan_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
    }

    // ==================== AVX2 Implementations (x86_64) ====================

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn dot_product_avx2(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let len = a.len();
        let mut sum = _mm256_setzero_ps();
        let mut i = 0;

        // Process 8 elements at a time using AVX2
        while i + 8 <= len {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            let prod = _mm256_mul_ps(va, vb);
            sum = _mm256_add_ps(sum, prod);
            i += 8;
        }

        // Horizontal sum of the vector
        let sum = horizontal_add_avx2(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            scalar_sum += a[i] * b[i];
            i += 1;
        }

        scalar_sum
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn euclidean_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let len = a.len();
        let mut sum = _mm256_setzero_ps();
        let mut i = 0;

        // Process 8 elements at a time
        while i + 8 <= len {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            let diff = _mm256_sub_ps(va, vb);
            let sq = _mm256_mul_ps(diff, diff);
            sum = _mm256_add_ps(sum, sq);
            i += 8;
        }

        let sum = horizontal_add_avx2(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            let diff = a[i] - b[i];
            scalar_sum += diff * diff;
            i += 1;
        }

        scalar_sum.sqrt()
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn cosine_similarity_avx2(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let len = a.len();
        let mut dot = _mm256_setzero_ps();
        let mut mag_a = _mm256_setzero_ps();
        let mut mag_b = _mm256_setzero_ps();
        let mut i = 0;

        // Process 8 elements at a time
        while i + 8 <= len {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));

            dot = _mm256_add_ps(dot, _mm256_mul_ps(va, vb));
            mag_a = _mm256_add_ps(mag_a, _mm256_mul_ps(va, va));
            mag_b = _mm256_add_ps(mag_b, _mm256_mul_ps(vb, vb));

            i += 8;
        }

        let dot_sum = horizontal_add_avx2(dot);
        let mag_a_sum = horizontal_add_avx2(mag_a);
        let mag_b_sum = horizontal_add_avx2(mag_b);

        // Process remaining elements
        let mut scalar_dot = dot_sum;
        let mut scalar_mag_a = mag_a_sum;
        let mut scalar_mag_b = mag_b_sum;

        while i < len {
            scalar_dot += a[i] * b[i];
            scalar_mag_a += a[i] * a[i];
            scalar_mag_b += b[i] * b[i];
            i += 1;
        }

        let mag_a = scalar_mag_a.sqrt();
        let mag_b = scalar_mag_b.sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }

        scalar_dot / (mag_a * mag_b)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn manhattan_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let len = a.len();
        let mut sum = _mm256_setzero_ps();
        let mut i = 0;

        // Create a mask for absolute value (clear sign bit)
        let sign_mask = _mm256_set1_ps(f32::from_bits(0x7FFF_FFFF));

        while i + 8 <= len {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            let diff = _mm256_sub_ps(va, vb);
            let abs_diff = _mm256_and_ps(diff, sign_mask);
            sum = _mm256_add_ps(sum, abs_diff);
            i += 8;
        }

        let sum = horizontal_add_avx2(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            scalar_sum += (a[i] - b[i]).abs();
            i += 1;
        }

        scalar_sum
    }

    // ==================== ARM NEON Implementations ====================

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::aarch64::*;

        let len = a.len();
        let mut sum = vdupq_n_f32(0.0);
        let mut i = 0;

        // Process 4 elements at a time using NEON
        while i + 4 <= len {
            let va = vld1q_f32(a.as_ptr().add(i));
            let vb = vld1q_f32(b.as_ptr().add(i));
            let prod = vmulq_f32(va, vb);
            sum = vaddq_f32(sum, prod);
            i += 4;
        }

        // Horizontal sum
        let sum = horizontal_add_neon(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            scalar_sum += a[i] * b[i];
            i += 1;
        }

        scalar_sum
    }

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn euclidean_distance_neon(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::aarch64::*;

        let len = a.len();
        let mut sum = vdupq_n_f32(0.0);
        let mut i = 0;

        while i + 4 <= len {
            let va = vld1q_f32(a.as_ptr().add(i));
            let vb = vld1q_f32(b.as_ptr().add(i));
            let diff = vsubq_f32(va, vb);
            let sq = vmulq_f32(diff, diff);
            sum = vaddq_f32(sum, sq);
            i += 4;
        }

        let sum = horizontal_add_neon(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            let diff = a[i] - b[i];
            scalar_sum += diff * diff;
            i += 1;
        }

        scalar_sum.sqrt()
    }

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn cosine_similarity_neon(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::aarch64::*;

        let len = a.len();
        let mut dot = vdupq_n_f32(0.0);
        let mut mag_a = vdupq_n_f32(0.0);
        let mut mag_b = vdupq_n_f32(0.0);
        let mut i = 0;

        while i + 4 <= len {
            let va = vld1q_f32(a.as_ptr().add(i));
            let vb = vld1q_f32(b.as_ptr().add(i));

            dot = vaddq_f32(dot, vmulq_f32(va, vb));
            mag_a = vaddq_f32(mag_a, vmulq_f32(va, va));
            mag_b = vaddq_f32(mag_b, vmulq_f32(vb, vb));

            i += 4;
        }

        let dot_sum = horizontal_add_neon(dot);
        let mag_a_sum = horizontal_add_neon(mag_a);
        let mag_b_sum = horizontal_add_neon(mag_b);

        // Process remaining elements
        let mut scalar_dot = dot_sum;
        let mut scalar_mag_a = mag_a_sum;
        let mut scalar_mag_b = mag_b_sum;

        while i < len {
            scalar_dot += a[i] * b[i];
            scalar_mag_a += a[i] * a[i];
            scalar_mag_b += b[i] * b[i];
            i += 1;
        }

        let mag_a = scalar_mag_a.sqrt();
        let mag_b = scalar_mag_b.sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }

        scalar_dot / (mag_a * mag_b)
    }

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn manhattan_distance_neon(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::aarch64::*;

        let len = a.len();
        let mut sum = vdupq_n_f32(0.0);
        let mut i = 0;

        while i + 4 <= len {
            let va = vld1q_f32(a.as_ptr().add(i));
            let vb = vld1q_f32(b.as_ptr().add(i));
            let diff = vsubq_f32(va, vb);
            let abs_diff = vabsq_f32(diff);
            sum = vaddq_f32(sum, abs_diff);
            i += 4;
        }

        let sum = horizontal_add_neon(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            scalar_sum += (a[i] - b[i]).abs();
            i += 1;
        }

        scalar_sum
    }
}

// ==================== SIMD Aggregation Operations ====================

/// SIMD-optimized aggregation operations
pub struct SimdAggregations;

impl SimdAggregations {
    /// Calculate sum of f32 values using SIMD
    pub fn sum_f32(values: &[f32]) -> f32 {
        if values.len() < SimdConfig::default().min_simd_size {
            return values.iter().sum();
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { Self::sum_f32_avx2(values) };
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if cfg!(target_feature = "neon") {
                return unsafe { Self::sum_f32_neon(values) };
            }
        }

        values.iter().sum()
    }

    /// Calculate sum of i64 values using SIMD
    pub fn sum_i64(values: &[i64]) -> i64 {
        if values.len() < SimdConfig::default().min_simd_size {
            return values.iter().sum();
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { Self::sum_i64_avx2(values) };
            }
        }

        values.iter().sum()
    }

    /// Calculate average of f32 values using SIMD
    pub fn avg_f32(values: &[f32]) -> f32 {
        if values.is_empty() {
            return 0.0;
        }
        Self::sum_f32(values) / values.len() as f32
    }

    /// Calculate average of i64 values using SIMD
    pub fn avg_i64(values: &[i64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        Self::sum_i64(values) as f64 / values.len() as f64
    }

    /// Count non-null values (for now, assumes all values are non-null)
    pub fn count(len: usize) -> usize {
        len
    }

    // ==================== AVX2 Aggregation Implementations ====================

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn sum_f32_avx2(values: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let len = values.len();
        let mut sum = _mm256_setzero_ps();
        let mut i = 0;

        while i + 8 <= len {
            let v = _mm256_loadu_ps(values.as_ptr().add(i));
            sum = _mm256_add_ps(sum, v);
            i += 8;
        }

        let sum = horizontal_add_avx2(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            scalar_sum += values[i];
            i += 1;
        }

        scalar_sum
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn sum_i64_avx2(values: &[i64]) -> i64 {
        use std::arch::x86_64::*;

        let len = values.len();
        let mut sum = _mm256_setzero_si256();
        let mut i = 0;

        while i + 4 <= len {
            let v = _mm256_loadu_si256(values.as_ptr().add(i) as *const __m256i);
            sum = _mm256_add_epi64(sum, v);
            i += 4;
        }

        // Extract and sum all elements
        let mut result = [0i64; 4];
        _mm256_storeu_si256(result.as_mut_ptr() as *mut __m256i, sum);
        let mut scalar_sum: i64 = result.iter().sum();

        // Process remaining elements
        while i < len {
            scalar_sum += values[i];
            i += 1;
        }

        scalar_sum
    }

    // ==================== NEON Aggregation Implementations ====================

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    unsafe fn sum_f32_neon(values: &[f32]) -> f32 {
        use std::arch::aarch64::*;

        let len = values.len();
        let mut sum = vdupq_n_f32(0.0);
        let mut i = 0;

        while i + 4 <= len {
            let v = vld1q_f32(values.as_ptr().add(i));
            sum = vaddq_f32(sum, v);
            i += 4;
        }

        let sum = horizontal_add_neon(sum);

        // Process remaining elements
        let mut scalar_sum = sum;
        while i < len {
            scalar_sum += values[i];
            i += 1;
        }

        scalar_sum
    }
}

// ==================== Helper Functions ====================

/// Horizontal add for AVX2 __m256 register (8 f32 values)
#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn horizontal_add_avx2(v: std::arch::x86_64::__m256) -> f32 {
    use std::arch::x86_64::*;

    // Add high and low 128-bit lanes
    let high = _mm256_extractf128_ps(v, 1);
    let low = _mm256_castps256_ps128(v);
    let sum128 = _mm_add_ps(high, low);

    // Horizontal add within 128-bit lane
    let shuf = _mm_movehdup_ps(sum128);
    let sums = _mm_add_ps(sum128, shuf);
    let shuf = _mm_movehl_ps(shuf, sums);
    let result = _mm_add_ss(sums, shuf);

    _mm_cvtss_f32(result)
}

/// Horizontal add for NEON float32x4_t register (4 f32 values)
#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn horizontal_add_neon(v: std::arch::aarch64::float32x4_t) -> f32 {
    use std::arch::aarch64::*;

    let sum = vpaddq_f32(v, v);
    let sum = vpaddq_f32(sum, sum);
    vgetq_lane_f32(sum, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![2.0, 3.0, 4.0, 5.0];
        let result = SimdVectorOps::dot_product_f32(&a, &b);
        let expected = 1.0 * 2.0 + 2.0 * 3.0 + 3.0 * 4.0 + 4.0 * 5.0;
        assert!((result - expected).abs() < 1e-5);
    }

    #[test]
    fn test_dot_product_large() {
        let a: Vec<f32> = (0..100).map(|x| x as f32).collect();
        let b: Vec<f32> = (0..100).map(|x| (x + 1) as f32).collect();
        let result = SimdVectorOps::dot_product_f32(&a, &b);

        // Verify against scalar computation
        let expected: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        assert!((result - expected).abs() < 1e-3);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        let result = SimdVectorOps::euclidean_distance_f32(&a, &b);
        assert!((result - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_euclidean_distance_large() {
        let a: Vec<f32> = (0..100).map(|x| x as f32).collect();
        let b: Vec<f32> = (0..100).map(|x| (x + 1) as f32).collect();
        let result = SimdVectorOps::euclidean_distance_f32(&a, &b);

        let expected: f32 = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt();
        assert!((result - expected).abs() < 1e-3);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let result = SimdVectorOps::cosine_similarity_f32(&a, &b);
        assert!((result - 1.0).abs() < 1e-5);

        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let result = SimdVectorOps::cosine_similarity_f32(&a, &b);
        assert!(result.abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_large() {
        let a: Vec<f32> = (1..=100).map(|x| x as f32).collect();
        let b: Vec<f32> = (1..=100).map(|x| x as f32).collect();
        let result = SimdVectorOps::cosine_similarity_f32(&a, &b);
        // Same vectors should have similarity of 1.0
        assert!((result - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_manhattan_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let result = SimdVectorOps::manhattan_distance_f32(&a, &b);
        assert!((result - 7.0).abs() < 1e-5);
    }

    #[test]
    fn test_manhattan_distance_large() {
        let a: Vec<f32> = (0..100).map(|x| x as f32).collect();
        let b: Vec<f32> = (0..100).map(|x| (x + 1) as f32).collect();
        let result = SimdVectorOps::manhattan_distance_f32(&a, &b);
        
        let expected: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum();
        assert!((result - expected).abs() < 1e-3);
    }

    #[test]
    fn test_sum_f32() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = SimdAggregations::sum_f32(&values);
        assert!((result - 15.0).abs() < 1e-5);
    }

    #[test]
    fn test_sum_f32_large() {
        let values: Vec<f32> = (1..=100).map(|x| x as f32).collect();
        let result = SimdAggregations::sum_f32(&values);
        let expected: f32 = values.iter().sum();
        assert!((result - expected).abs() < 1e-3);
    }

    #[test]
    fn test_sum_i64() {
        let values = vec![1i64, 2, 3, 4, 5];
        let result = SimdAggregations::sum_i64(&values);
        assert_eq!(result, 15);
    }

    #[test]
    fn test_sum_i64_large() {
        let values: Vec<i64> = (1..=100).collect();
        let result = SimdAggregations::sum_i64(&values);
        let expected: i64 = values.iter().sum();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_avg_f32() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = SimdAggregations::avg_f32(&values);
        assert!((result - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_avg_i64() {
        let values = vec![1i64, 2, 3, 4, 5];
        let result = SimdAggregations::avg_i64(&values);
        assert!((result - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_count() {
        let result = SimdAggregations::count(100);
        assert_eq!(result, 100);
    }

    #[test]
    fn test_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(SimdVectorOps::dot_product_f32(&a, &b), 0.0);
        assert_eq!(SimdVectorOps::cosine_similarity_f32(&a, &b), 0.0);
    }

    #[test]
    fn test_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(SimdVectorOps::dot_product_f32(&a, &b), 0.0);
        assert_eq!(SimdVectorOps::euclidean_distance_f32(&a, &b), f32::INFINITY);
    }
}
