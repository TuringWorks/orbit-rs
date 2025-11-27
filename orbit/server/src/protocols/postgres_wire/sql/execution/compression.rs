//! Columnar Compression Codecs
//!
//! This module provides specialized compression algorithms for columnar storage,
//! optimized for different data types and patterns.

use crate::protocols::error::ProtocolResult;
use std::collections::HashMap;

/// Compression codec for columnar data
#[derive(Debug, Clone, PartialEq)]
pub enum CompressionCodec {
    /// Delta compression for integer sequences
    Delta { base_value: i64, deltas: Vec<i64> },
    /// Double delta compression (for timestamps)
    DoubleDelta {
        base_value: i64,
        base_delta: i64,
        deltas: Vec<i64>,
    },
    /// Run-length encoding for repeated values
    RLE {
        run_lengths: Vec<u32>,
        values: Vec<i64>,
    },
    /// Bit-packing for small integer ranges
    BitPacking { bit_width: u8, packed_data: Vec<u8> },
    /// Gorilla compression for floating-point time series
    Gorilla {
        precision: f64,
        compressed_data: Vec<u8>,
    },
    /// Dictionary encoding for string columns
    Dictionary {
        dictionary: Vec<String>,
        encoded_values: Vec<u32>,
    },
    /// LZ4 compression (general purpose)
    LZ4 { compressed_data: Vec<u8> },
    /// Zstd compression (high compression ratio)
    Zstd {
        level: i32,
        compressed_data: Vec<u8>,
    },
    /// Snappy compression (fast)
    Snappy { compressed_data: Vec<u8> },
    /// Uncompressed (for small columns or when compression doesn't help)
    Uncompressed { data: Vec<u8> },
}

/// Column compression utilities
pub struct ColumnCompression;

impl ColumnCompression {
    /// Compress integer column using delta encoding
    pub fn compress_delta(values: &[i64]) -> ProtocolResult<CompressionCodec> {
        if values.is_empty() {
            return Ok(CompressionCodec::Uncompressed { data: vec![] });
        }

        let base_value = values[0];
        let mut deltas = Vec::with_capacity(values.len().saturating_sub(1));

        for i in 1..values.len() {
            deltas.push(values[i] - values[i - 1]);
        }

        Ok(CompressionCodec::Delta { base_value, deltas })
    }

    /// Decompress delta-encoded column
    pub fn decompress_delta(codec: &CompressionCodec) -> ProtocolResult<Vec<i64>> {
        match codec {
            CompressionCodec::Delta { base_value, deltas } => {
                let mut result = Vec::with_capacity(deltas.len() + 1);
                result.push(*base_value);
                let mut current = *base_value;
                for &delta in deltas {
                    current += delta;
                    result.push(current);
                }
                Ok(result)
            }
            _ => Err(crate::protocols::error::ProtocolError::PostgresError(
                "Not a delta codec".to_string(),
            )),
        }
    }

    /// Compress integer column using double delta encoding
    pub fn compress_double_delta(values: &[i64]) -> ProtocolResult<CompressionCodec> {
        if values.len() < 2 {
            return Self::compress_delta(values);
        }

        let base_value = values[0];
        let base_delta = values[1] - values[0];
        let mut deltas = Vec::with_capacity(values.len().saturating_sub(2));

        for i in 2..values.len() {
            let first_delta = values[i] - values[i - 1];
            let second_delta = values[i - 1] - values[i - 2];
            deltas.push(first_delta - second_delta);
        }

        Ok(CompressionCodec::DoubleDelta {
            base_value,
            base_delta,
            deltas,
        })
    }

    /// Decompress double delta-encoded column
    pub fn decompress_double_delta(codec: &CompressionCodec) -> ProtocolResult<Vec<i64>> {
        match codec {
            CompressionCodec::DoubleDelta {
                base_value,
                base_delta,
                deltas,
            } => {
                let mut result = Vec::with_capacity(deltas.len() + 2);
                result.push(*base_value);
                result.push(*base_value + *base_delta);

                let mut prev_value = *base_value + *base_delta;
                let mut prev_delta = *base_delta;

                for &delta in deltas {
                    let current_delta = prev_delta + delta;
                    prev_value += current_delta;
                    prev_delta = current_delta;
                    result.push(prev_value);
                }

                Ok(result)
            }
            _ => Err(crate::protocols::error::ProtocolError::PostgresError(
                "Not a double delta codec".to_string(),
            )),
        }
    }

    /// Compress integer column using run-length encoding
    pub fn compress_rle(values: &[i64]) -> ProtocolResult<CompressionCodec> {
        if values.is_empty() {
            return Ok(CompressionCodec::Uncompressed { data: vec![] });
        }

        let mut run_lengths = Vec::new();
        let mut unique_values = Vec::new();
        let mut current_value = values[0];
        let mut current_run = 1u32;

        for &value in values.iter().skip(1) {
            if value == current_value {
                current_run += 1;
            } else {
                run_lengths.push(current_run);
                unique_values.push(current_value);
                current_value = value;
                current_run = 1;
            }
        }

        // Add the last run
        run_lengths.push(current_run);
        unique_values.push(current_value);

        Ok(CompressionCodec::RLE {
            run_lengths,
            values: unique_values,
        })
    }

    /// Decompress RLE-encoded column
    pub fn decompress_rle(codec: &CompressionCodec) -> ProtocolResult<Vec<i64>> {
        match codec {
            CompressionCodec::RLE {
                run_lengths,
                values,
            } => {
                let mut result = Vec::new();
                for (value, &run_length) in values.iter().zip(run_lengths.iter()) {
                    for _ in 0..run_length {
                        result.push(*value);
                    }
                }
                Ok(result)
            }
            _ => Err(crate::protocols::error::ProtocolError::PostgresError(
                "Not an RLE codec".to_string(),
            )),
        }
    }

    /// Compress integer column using bit-packing
    pub fn compress_bitpacking(values: &[i64], bit_width: u8) -> ProtocolResult<CompressionCodec> {
        if values.is_empty() {
            return Ok(CompressionCodec::Uncompressed { data: vec![] });
        }

        // Find min and max to determine if bit_width is sufficient
        let min_val = values.iter().min().copied().unwrap_or(0);
        let max_val = values.iter().max().copied().unwrap_or(0);
        let range = max_val - min_val;

        // Calculate required bits
        let required_bits = if range == 0 {
            0
        } else {
            (range as f64).log2().ceil() as u8
        };

        let effective_bit_width = bit_width.max(required_bits);

        // Pack values (simplified - in production would use proper bit packing)
        let packed_data = values
            .iter()
            .flat_map(|&v| (v - min_val).to_le_bytes().to_vec())
            .collect();

        Ok(CompressionCodec::BitPacking {
            bit_width: effective_bit_width,
            packed_data,
        })
    }

    /// Compress floating-point column using Gorilla compression
    pub fn compress_gorilla(values: &[f64], precision: f64) -> ProtocolResult<CompressionCodec> {
        if values.is_empty() {
            return Ok(CompressionCodec::Uncompressed { data: vec![] });
        }

        // Simplified Gorilla compression
        // In production, this would use proper bit-level encoding
        let mut compressed_data = Vec::new();
        let mut prev_value = 0u64;

        for &value in values {
            let current = value.to_bits();
            let xor = current ^ prev_value;

            if xor == 0 {
                // Value unchanged - write single 0 bit
                compressed_data.push(0);
            } else {
                // Write 1 bit followed by XOR delta
                compressed_data.push(1);
                compressed_data.extend_from_slice(&xor.to_le_bytes());
            }

            prev_value = current;
        }

        Ok(CompressionCodec::Gorilla {
            precision,
            compressed_data,
        })
    }

    /// Compress string column using dictionary encoding
    pub fn compress_dictionary(values: &[String]) -> ProtocolResult<CompressionCodec> {
        if values.is_empty() {
            return Ok(CompressionCodec::Uncompressed { data: vec![] });
        }

        // Build dictionary
        let mut dictionary = Vec::new();
        let mut dict_map = HashMap::new();
        let mut encoded_values = Vec::with_capacity(values.len());

        for value in values {
            let index = match dict_map.get(value) {
                Some(&idx) => idx,
                None => {
                    let idx = dictionary.len() as u32;
                    dictionary.push(value.clone());
                    dict_map.insert(value.clone(), idx);
                    idx
                }
            };
            encoded_values.push(index);
        }

        Ok(CompressionCodec::Dictionary {
            dictionary,
            encoded_values,
        })
    }

    /// Decompress dictionary-encoded column
    pub fn decompress_dictionary(codec: &CompressionCodec) -> ProtocolResult<Vec<String>> {
        match codec {
            CompressionCodec::Dictionary {
                dictionary,
                encoded_values,
            } => {
                let result: Vec<String> = encoded_values
                    .iter()
                    .map(|&idx| {
                        dictionary
                            .get(idx as usize)
                            .cloned()
                            .unwrap_or_else(|| String::new())
                    })
                    .collect();
                Ok(result)
            }
            _ => Err(crate::protocols::error::ProtocolError::PostgresError(
                "Not a dictionary codec".to_string(),
            )),
        }
    }

    /// Estimate compression ratio for a codec
    pub fn estimate_compression_ratio(codec: &CompressionCodec, original_size: usize) -> f64 {
        let compressed_size = match codec {
            CompressionCodec::Delta { deltas, .. } => {
                std::mem::size_of::<i64>() + deltas.len() * std::mem::size_of::<i64>()
            }
            CompressionCodec::DoubleDelta { deltas, .. } => {
                2 * std::mem::size_of::<i64>() + deltas.len() * std::mem::size_of::<i64>()
            }
            CompressionCodec::RLE {
                run_lengths,
                values,
            } => {
                run_lengths.len() * std::mem::size_of::<u32>()
                    + values.len() * std::mem::size_of::<i64>()
            }
            CompressionCodec::BitPacking { packed_data, .. } => packed_data.len(),
            CompressionCodec::Gorilla {
                compressed_data, ..
            } => compressed_data.len(),
            CompressionCodec::Dictionary {
                dictionary,
                encoded_values,
            } => {
                dictionary.iter().map(|s| s.len()).sum::<usize>()
                    + encoded_values.len() * std::mem::size_of::<u32>()
            }
            CompressionCodec::LZ4 { compressed_data } => compressed_data.len(),
            CompressionCodec::Zstd {
                compressed_data, ..
            } => compressed_data.len(),
            CompressionCodec::Snappy { compressed_data } => compressed_data.len(),
            CompressionCodec::Uncompressed { data } => data.len(),
        };

        if compressed_size == 0 {
            return 1.0;
        }

        original_size as f64 / compressed_size as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_compression() {
        let values = vec![100, 101, 102, 103, 104];
        let codec = ColumnCompression::compress_delta(&values).unwrap();
        let decompressed = ColumnCompression::decompress_delta(&codec).unwrap();
        assert_eq!(values, decompressed);
    }

    #[test]
    fn test_rle_compression() {
        let values = vec![5, 5, 5, 10, 10, 20];
        let codec = ColumnCompression::compress_rle(&values).unwrap();
        let decompressed = ColumnCompression::decompress_rle(&codec).unwrap();
        assert_eq!(values, decompressed);
    }

    #[test]
    fn test_dictionary_compression() {
        let values = vec![
            "apple".to_string(),
            "banana".to_string(),
            "apple".to_string(),
            "cherry".to_string(),
        ];
        let codec = ColumnCompression::compress_dictionary(&values).unwrap();
        let decompressed = ColumnCompression::decompress_dictionary(&codec).unwrap();
        assert_eq!(values, decompressed);
    }

    #[test]
    fn test_compression_ratio() {
        let values = vec![100, 101, 102, 103, 104];
        let codec = ColumnCompression::compress_delta(&values).unwrap();
        let original_size = values.len() * std::mem::size_of::<i64>();
        let ratio = ColumnCompression::estimate_compression_ratio(&codec, original_size);
        assert!(ratio > 0.0);
    }
}
