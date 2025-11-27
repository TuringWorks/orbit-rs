//! Common types for SIMD operations
//!
//! This module provides data types used across SIMD implementations.

/// Bitmap for tracking NULL values in columnar data
///
/// Uses a compact bit representation where each bit indicates
/// whether the corresponding value is NULL (1) or not (0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullBitmap {
    /// Bit storage (each byte holds 8 null flags)
    bits: Vec<u8>,
    /// Number of values represented
    len: usize,
}

impl NullBitmap {
    /// Create a new null bitmap with all values non-null
    pub fn new(len: usize) -> Self {
        let bytes = len.div_ceil(8);
        Self {
            bits: vec![0; bytes],
            len,
        }
    }

    /// Create a null bitmap with all values valid (non-null)
    ///
    /// This is an alias for `new()` to match the naming convention in orbit-engine.
    pub fn new_all_valid(len: usize) -> Self {
        Self::new(len)
    }

    /// Create a null bitmap with all values null
    pub fn all_null(len: usize) -> Self {
        let bytes = len.div_ceil(8);
        let mut bitmap = Self {
            bits: vec![0xFF; bytes],
            len,
        };

        // Clear any extra bits in the last byte beyond len
        if !len.is_multiple_of(8) {
            let last_byte_idx = bytes - 1;
            let valid_bits = len % 8;
            let mask = (1u8 << valid_bits) - 1;
            bitmap.bits[last_byte_idx] = mask;
        }

        bitmap
    }

    /// Create a null bitmap with all values null
    ///
    /// This is an alias for `all_null()` to match the naming convention in orbit-engine.
    pub fn new_all_null(len: usize) -> Self {
        Self::all_null(len)
    }

    /// Create from a boolean slice (true = null, false = not null)
    pub fn from_bools(nulls: &[bool]) -> Self {
        let mut bitmap = Self::new(nulls.len());
        for (i, &is_null) in nulls.iter().enumerate() {
            if is_null {
                bitmap.set_null(i);
            }
        }
        bitmap
    }

    /// Check if value at index is null
    #[inline]
    pub fn is_null(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        (self.bits[byte_idx] & (1 << bit_idx)) != 0
    }

    /// Check if value at index is valid (not null)
    #[inline]
    pub fn is_valid(&self, index: usize) -> bool {
        !self.is_null(index)
    }

    /// Set value at index as null
    #[inline]
    pub fn set_null(&mut self, index: usize) {
        if index < self.len {
            let byte_idx = index / 8;
            let bit_idx = index % 8;
            self.bits[byte_idx] |= 1 << bit_idx;
        }
    }

    /// Set value at index as not null
    #[inline]
    pub fn set_not_null(&mut self, index: usize) {
        if index < self.len {
            let byte_idx = index / 8;
            let bit_idx = index % 8;
            self.bits[byte_idx] &= !(1 << bit_idx);
        }
    }

    /// Set value at index as valid (not null)
    ///
    /// This is an alias for `set_not_null()` to match orbit-engine naming convention.
    #[inline]
    pub fn set_valid(&mut self, index: usize) {
        self.set_not_null(index);
    }

    /// Count number of null values
    pub fn null_count(&self) -> usize {
        self.bits
            .iter()
            .map(|&byte| byte.count_ones() as usize)
            .sum()
    }

    /// Count number of non-null values
    pub fn non_null_count(&self) -> usize {
        self.len - self.null_count()
    }

    /// Get the length (number of values)
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if bitmap is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterate over (index, is_null) pairs
    pub fn iter(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        (0..self.len).map(move |i| (i, self.is_null(i)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_bitmap_creation() {
        let bitmap = NullBitmap::new(100);
        assert_eq!(bitmap.len(), 100);
        assert_eq!(bitmap.null_count(), 0);
        assert_eq!(bitmap.non_null_count(), 100);
    }

    #[test]
    fn test_null_bitmap_all_null() {
        let bitmap = NullBitmap::all_null(100);
        assert_eq!(bitmap.len(), 100);
        assert_eq!(bitmap.null_count(), 100);
        assert_eq!(bitmap.non_null_count(), 0);
    }

    #[test]
    fn test_null_bitmap_set_null() {
        let mut bitmap = NullBitmap::new(10);
        bitmap.set_null(5);
        assert!(bitmap.is_null(5));
        assert!(!bitmap.is_null(4));
        assert!(!bitmap.is_null(6));
        assert_eq!(bitmap.null_count(), 1);
    }

    #[test]
    fn test_null_bitmap_from_bools() {
        let nulls = vec![false, true, false, true, true];
        let bitmap = NullBitmap::from_bools(&nulls);
        assert!(!bitmap.is_null(0));
        assert!(bitmap.is_null(1));
        assert!(!bitmap.is_null(2));
        assert!(bitmap.is_null(3));
        assert!(bitmap.is_null(4));
        assert_eq!(bitmap.null_count(), 3);
    }
}
