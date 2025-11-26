//! Time series data compression algorithms
//!
//! Implements efficient compression for time series data:
//! - Delta compression for timestamps
//! - Double-delta compression for regular intervals
//! - Gorilla compression for floating-point values (Facebook's algorithm)
//! - LZ4/Zstd wrappers for general compression

use super::{CompressionType, DataPoint, TimeSeriesValue, Timestamp};
use anyhow::Result;
use std::collections::HashMap;

/// Compression trait for time series data
pub trait TimeSeriesCompressor: Send + Sync {
    /// Compress a batch of data points
    fn compress(&self, data_points: &[DataPoint]) -> Result<Vec<u8>>;

    /// Decompress data back to data points
    fn decompress(&self, compressed_data: &[u8]) -> Result<Vec<DataPoint>>;

    /// Get compression ratio estimate
    fn compression_ratio(&self) -> f64;
}

// ============================================================================
// Variable-length integer encoding (for deltas)
// ============================================================================

/// Encode a signed integer using variable-length encoding (zigzag + varint)
fn encode_varint_signed(value: i64, output: &mut Vec<u8>) {
    // Zigzag encode: map signed to unsigned
    let unsigned = ((value << 1) ^ (value >> 63)) as u64;
    encode_varint(unsigned, output);
}

/// Decode a signed integer from variable-length encoding
fn decode_varint_signed(input: &[u8], pos: &mut usize) -> Result<i64> {
    let unsigned = decode_varint(input, pos)?;
    // Zigzag decode: map unsigned back to signed
    Ok(((unsigned >> 1) as i64) ^ (-((unsigned & 1) as i64)))
}

/// Encode an unsigned integer using variable-length encoding
fn encode_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

/// Decode an unsigned integer from variable-length encoding
fn decode_varint(input: &[u8], pos: &mut usize) -> Result<u64> {
    let mut result: u64 = 0;
    let mut shift = 0;
    loop {
        if *pos >= input.len() {
            return Err(anyhow::anyhow!("Unexpected end of input"));
        }
        let byte = input[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err(anyhow::anyhow!("Varint too long"));
        }
    }
    Ok(result)
}

// ============================================================================
// Delta Compression
// ============================================================================

/// Delta compression for time series data
/// Stores deltas between consecutive timestamps and values
pub struct DeltaCompressor {
    base_timestamp: Timestamp,
}

impl DeltaCompressor {
    pub fn new(base_timestamp: Timestamp) -> Self {
        Self { base_timestamp }
    }
}

impl TimeSeriesCompressor for DeltaCompressor {
    fn compress(&self, data_points: &[DataPoint]) -> Result<Vec<u8>> {
        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        let mut output = Vec::with_capacity(data_points.len() * 16);

        // Write header: number of points
        encode_varint(data_points.len() as u64, &mut output);

        // Write base timestamp
        encode_varint_signed(self.base_timestamp, &mut output);

        let mut prev_timestamp = self.base_timestamp;
        let mut prev_value: f64 = 0.0;

        for point in data_points {
            // Delta encode timestamp
            let ts_delta = point.timestamp - prev_timestamp;
            encode_varint_signed(ts_delta, &mut output);
            prev_timestamp = point.timestamp;

            // Encode value type and value
            match &point.value {
                TimeSeriesValue::Float(v) => {
                    output.push(0); // Type marker
                    let delta = v - prev_value;
                    output.extend_from_slice(&delta.to_le_bytes());
                    prev_value = *v;
                }
                TimeSeriesValue::Integer(v) => {
                    output.push(1); // Type marker
                    encode_varint_signed(*v, &mut output);
                }
                TimeSeriesValue::String(s) => {
                    output.push(2); // Type marker
                    encode_varint(s.len() as u64, &mut output);
                    output.extend_from_slice(s.as_bytes());
                }
                TimeSeriesValue::Boolean(b) => {
                    output.push(3); // Type marker
                    output.push(if *b { 1 } else { 0 });
                }
                TimeSeriesValue::Null => {
                    output.push(4); // Type marker
                }
            }

            // Encode labels count (simplified - no labels for compression efficiency)
            encode_varint(point.labels.len() as u64, &mut output);
            for (key, value) in &point.labels {
                encode_varint(key.len() as u64, &mut output);
                output.extend_from_slice(key.as_bytes());
                encode_varint(value.len() as u64, &mut output);
                output.extend_from_slice(value.as_bytes());
            }
        }

        Ok(output)
    }

    fn decompress(&self, compressed_data: &[u8]) -> Result<Vec<DataPoint>> {
        if compressed_data.is_empty() {
            return Ok(Vec::new());
        }

        let mut pos = 0;

        // Read header: number of points
        let num_points = decode_varint(compressed_data, &mut pos)? as usize;

        // Read base timestamp
        let base_ts = decode_varint_signed(compressed_data, &mut pos)?;

        let mut points = Vec::with_capacity(num_points);
        let mut prev_timestamp = base_ts;
        let mut prev_value: f64 = 0.0;

        for _ in 0..num_points {
            // Decode timestamp delta
            let ts_delta = decode_varint_signed(compressed_data, &mut pos)?;
            let timestamp = prev_timestamp + ts_delta;
            prev_timestamp = timestamp;

            // Decode value
            if pos >= compressed_data.len() {
                return Err(anyhow::anyhow!("Unexpected end of data"));
            }
            let value_type = compressed_data[pos];
            pos += 1;

            let value = match value_type {
                0 => {
                    // Float (delta encoded)
                    if pos + 8 > compressed_data.len() {
                        return Err(anyhow::anyhow!("Unexpected end of data"));
                    }
                    let delta =
                        f64::from_le_bytes(compressed_data[pos..pos + 8].try_into().unwrap());
                    pos += 8;
                    prev_value += delta;
                    TimeSeriesValue::Float(prev_value)
                }
                1 => {
                    // Integer
                    let v = decode_varint_signed(compressed_data, &mut pos)?;
                    TimeSeriesValue::Integer(v)
                }
                2 => {
                    // String
                    let len = decode_varint(compressed_data, &mut pos)? as usize;
                    if pos + len > compressed_data.len() {
                        return Err(anyhow::anyhow!("Unexpected end of data"));
                    }
                    let s = String::from_utf8(compressed_data[pos..pos + len].to_vec())?;
                    pos += len;
                    TimeSeriesValue::String(s)
                }
                3 => {
                    // Boolean
                    if pos >= compressed_data.len() {
                        return Err(anyhow::anyhow!("Unexpected end of data"));
                    }
                    let b = compressed_data[pos] != 0;
                    pos += 1;
                    TimeSeriesValue::Boolean(b)
                }
                4 => TimeSeriesValue::Null,
                _ => return Err(anyhow::anyhow!("Unknown value type: {}", value_type)),
            };

            // Decode labels
            let num_labels = decode_varint(compressed_data, &mut pos)? as usize;
            let mut labels = HashMap::with_capacity(num_labels);
            for _ in 0..num_labels {
                let key_len = decode_varint(compressed_data, &mut pos)? as usize;
                if pos + key_len > compressed_data.len() {
                    return Err(anyhow::anyhow!("Unexpected end of data"));
                }
                let key = String::from_utf8(compressed_data[pos..pos + key_len].to_vec())?;
                pos += key_len;

                let val_len = decode_varint(compressed_data, &mut pos)? as usize;
                if pos + val_len > compressed_data.len() {
                    return Err(anyhow::anyhow!("Unexpected end of data"));
                }
                let val = String::from_utf8(compressed_data[pos..pos + val_len].to_vec())?;
                pos += val_len;

                labels.insert(key, val);
            }

            points.push(DataPoint {
                timestamp,
                value,
                labels,
            });
        }

        Ok(points)
    }

    fn compression_ratio(&self) -> f64 {
        0.3 // Estimated 70% compression
    }
}

// ============================================================================
// Double-Delta Compression
// ============================================================================

/// Double delta compression (Facebook Gorilla-style for timestamps)
/// Stores delta-of-deltas for timestamps with regular intervals
pub struct DoubleDeltaCompressor;

impl TimeSeriesCompressor for DoubleDeltaCompressor {
    fn compress(&self, data_points: &[DataPoint]) -> Result<Vec<u8>> {
        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        let mut output = Vec::with_capacity(data_points.len() * 12);

        // Write header
        encode_varint(data_points.len() as u64, &mut output);

        // First point: write full timestamp
        let first = &data_points[0];
        encode_varint_signed(first.timestamp, &mut output);
        encode_value(&first.value, &mut output)?;
        encode_labels(&first.labels, &mut output);

        if data_points.len() == 1 {
            return Ok(output);
        }

        // Second point: write delta
        let second = &data_points[1];
        let delta1 = second.timestamp - first.timestamp;
        encode_varint_signed(delta1, &mut output);
        encode_value(&second.value, &mut output)?;
        encode_labels(&second.labels, &mut output);

        // Remaining points: write delta-of-delta
        let mut prev_delta = delta1;
        let mut prev_ts = second.timestamp;

        for point in data_points.iter().skip(2) {
            let delta = point.timestamp - prev_ts;
            let delta_of_delta = delta - prev_delta;

            // Use smaller encoding for small delta-of-deltas
            if delta_of_delta == 0 {
                output.push(0); // Single byte for zero delta-of-delta
            } else if delta_of_delta >= -63 && delta_of_delta <= 64 {
                // 1 byte: marker + 7-bit value
                output.push(0x80 | ((delta_of_delta + 63) as u8 & 0x7F));
            } else {
                // Full varint encoding
                output.push(0xFF);
                encode_varint_signed(delta_of_delta, &mut output);
            }

            encode_value(&point.value, &mut output)?;
            encode_labels(&point.labels, &mut output);

            prev_delta = delta;
            prev_ts = point.timestamp;
        }

        Ok(output)
    }

    fn decompress(&self, compressed_data: &[u8]) -> Result<Vec<DataPoint>> {
        if compressed_data.is_empty() {
            return Ok(Vec::new());
        }

        let mut pos = 0;
        let num_points = decode_varint(compressed_data, &mut pos)? as usize;
        let mut points = Vec::with_capacity(num_points);

        if num_points == 0 {
            return Ok(points);
        }

        // First point
        let first_ts = decode_varint_signed(compressed_data, &mut pos)?;
        let first_value = decode_value(compressed_data, &mut pos)?;
        let first_labels = decode_labels(compressed_data, &mut pos)?;
        points.push(DataPoint {
            timestamp: first_ts,
            value: first_value,
            labels: first_labels,
        });

        if num_points == 1 {
            return Ok(points);
        }

        // Second point
        let delta1 = decode_varint_signed(compressed_data, &mut pos)?;
        let second_ts = first_ts + delta1;
        let second_value = decode_value(compressed_data, &mut pos)?;
        let second_labels = decode_labels(compressed_data, &mut pos)?;
        points.push(DataPoint {
            timestamp: second_ts,
            value: second_value,
            labels: second_labels,
        });

        // Remaining points
        let mut prev_delta = delta1;
        let mut prev_ts = second_ts;

        for _ in 2..num_points {
            if pos >= compressed_data.len() {
                return Err(anyhow::anyhow!("Unexpected end of data"));
            }

            let marker = compressed_data[pos];
            pos += 1;

            let delta_of_delta = if marker == 0 {
                0
            } else if marker == 0xFF {
                decode_varint_signed(compressed_data, &mut pos)?
            } else {
                ((marker & 0x7F) as i64) - 63
            };

            let delta = prev_delta + delta_of_delta;
            let timestamp = prev_ts + delta;

            let value = decode_value(compressed_data, &mut pos)?;
            let labels = decode_labels(compressed_data, &mut pos)?;

            points.push(DataPoint {
                timestamp,
                value,
                labels,
            });

            prev_delta = delta;
            prev_ts = timestamp;
        }

        Ok(points)
    }

    fn compression_ratio(&self) -> f64 {
        0.2 // Estimated 80% compression for regular intervals
    }
}

// ============================================================================
// Gorilla Compression (for floating point values)
// ============================================================================

/// Gorilla compression for floating point values
/// Uses XOR of consecutive values and variable-length encoding
pub struct GorillaCompressor;

impl TimeSeriesCompressor for GorillaCompressor {
    fn compress(&self, data_points: &[DataPoint]) -> Result<Vec<u8>> {
        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        let mut output = Vec::with_capacity(data_points.len() * 10);

        // Write header
        encode_varint(data_points.len() as u64, &mut output);

        let mut prev_ts: i64 = 0;
        let mut prev_value_bits: u64 = 0;
        let mut prev_leading_zeros: u32 = 64;
        let mut prev_trailing_zeros: u32 = 64;

        for (i, point) in data_points.iter().enumerate() {
            // Encode timestamp (delta)
            let ts_delta = point.timestamp - prev_ts;
            encode_varint_signed(ts_delta, &mut output);
            prev_ts = point.timestamp;

            // Encode value using Gorilla XOR compression for floats
            match &point.value {
                TimeSeriesValue::Float(v) => {
                    output.push(0); // Float type marker
                    let value_bits = v.to_bits();

                    if i == 0 {
                        // First value: write full 64 bits
                        output.extend_from_slice(&value_bits.to_le_bytes());
                    } else {
                        let xor = value_bits ^ prev_value_bits;
                        if xor == 0 {
                            // Same value: single 0 bit (encoded as 0 byte)
                            output.push(0);
                        } else {
                            let leading = xor.leading_zeros();
                            let trailing = xor.trailing_zeros();

                            if leading >= prev_leading_zeros && trailing >= prev_trailing_zeros {
                                // Fits in previous block: marker + meaningful bits
                                output.push(1);
                                let meaningful_bits = 64 - prev_leading_zeros - prev_trailing_zeros;
                                let meaningful = (xor >> prev_trailing_zeros) as u64;
                                // Write meaningful bits (up to 8 bytes)
                                let bytes_needed = ((meaningful_bits + 7) / 8) as usize;
                                output.extend_from_slice(&meaningful.to_le_bytes()[..bytes_needed]);
                            } else {
                                // New block: marker + leading zeros + meaningful bits length + bits
                                output.push(2);
                                output.push(leading as u8);
                                let meaningful_bits = 64 - leading - trailing;
                                output.push(meaningful_bits as u8);
                                let meaningful = (xor >> trailing) as u64;
                                let bytes_needed = ((meaningful_bits + 7) / 8) as usize;
                                output.extend_from_slice(&meaningful.to_le_bytes()[..bytes_needed]);
                                prev_leading_zeros = leading;
                                prev_trailing_zeros = trailing;
                            }
                        }
                    }
                    prev_value_bits = value_bits;
                }
                TimeSeriesValue::Integer(v) => {
                    output.push(1); // Integer type marker
                    encode_varint_signed(*v, &mut output);
                }
                TimeSeriesValue::String(s) => {
                    output.push(2); // String type marker
                    encode_varint(s.len() as u64, &mut output);
                    output.extend_from_slice(s.as_bytes());
                }
                TimeSeriesValue::Boolean(b) => {
                    output.push(3); // Boolean type marker
                    output.push(if *b { 1 } else { 0 });
                }
                TimeSeriesValue::Null => {
                    output.push(4); // Null type marker
                }
            }

            // Encode labels
            encode_labels(&point.labels, &mut output);
        }

        Ok(output)
    }

    fn decompress(&self, compressed_data: &[u8]) -> Result<Vec<DataPoint>> {
        if compressed_data.is_empty() {
            return Ok(Vec::new());
        }

        let mut pos = 0;
        let num_points = decode_varint(compressed_data, &mut pos)? as usize;
        let mut points = Vec::with_capacity(num_points);

        let mut prev_ts: i64 = 0;
        let mut prev_value_bits: u64 = 0;
        let mut prev_leading_zeros: u32 = 0;
        let mut prev_meaningful_bits: u32 = 64;

        for i in 0..num_points {
            // Decode timestamp
            let ts_delta = decode_varint_signed(compressed_data, &mut pos)?;
            let timestamp = prev_ts + ts_delta;
            prev_ts = timestamp;

            // Decode value
            if pos >= compressed_data.len() {
                return Err(anyhow::anyhow!("Unexpected end of data"));
            }
            let value_type = compressed_data[pos];
            pos += 1;

            let value = match value_type {
                0 => {
                    // Float with Gorilla compression
                    let value_bits = if i == 0 {
                        if pos + 8 > compressed_data.len() {
                            return Err(anyhow::anyhow!("Unexpected end of data"));
                        }
                        let bits =
                            u64::from_le_bytes(compressed_data[pos..pos + 8].try_into().unwrap());
                        pos += 8;
                        bits
                    } else {
                        if pos >= compressed_data.len() {
                            return Err(anyhow::anyhow!("Unexpected end of data"));
                        }
                        let marker = compressed_data[pos];
                        pos += 1;

                        match marker {
                            0 => prev_value_bits, // Same value
                            1 => {
                                // Same block
                                let bytes_needed = ((prev_meaningful_bits + 7) / 8) as usize;
                                if pos + bytes_needed > compressed_data.len() {
                                    return Err(anyhow::anyhow!("Unexpected end of data"));
                                }
                                let mut meaningful_bytes = [0u8; 8];
                                meaningful_bytes[..bytes_needed]
                                    .copy_from_slice(&compressed_data[pos..pos + bytes_needed]);
                                pos += bytes_needed;
                                let meaningful = u64::from_le_bytes(meaningful_bytes);
                                let trailing = 64 - prev_leading_zeros - prev_meaningful_bits;
                                let xor = meaningful << trailing;
                                prev_value_bits ^ xor
                            }
                            2 => {
                                // New block
                                if pos + 2 > compressed_data.len() {
                                    return Err(anyhow::anyhow!("Unexpected end of data"));
                                }
                                let leading = compressed_data[pos] as u32;
                                let meaningful_bits = compressed_data[pos + 1] as u32;
                                pos += 2;
                                let bytes_needed = ((meaningful_bits + 7) / 8) as usize;
                                if pos + bytes_needed > compressed_data.len() {
                                    return Err(anyhow::anyhow!("Unexpected end of data"));
                                }
                                let mut meaningful_bytes = [0u8; 8];
                                meaningful_bytes[..bytes_needed]
                                    .copy_from_slice(&compressed_data[pos..pos + bytes_needed]);
                                pos += bytes_needed;
                                let meaningful = u64::from_le_bytes(meaningful_bytes);
                                let trailing = 64 - leading - meaningful_bits;
                                let xor = meaningful << trailing;
                                prev_leading_zeros = leading;
                                prev_meaningful_bits = meaningful_bits;
                                prev_value_bits ^ xor
                            }
                            _ => return Err(anyhow::anyhow!("Invalid Gorilla marker")),
                        }
                    };
                    prev_value_bits = value_bits;
                    TimeSeriesValue::Float(f64::from_bits(value_bits))
                }
                1 => {
                    let v = decode_varint_signed(compressed_data, &mut pos)?;
                    TimeSeriesValue::Integer(v)
                }
                2 => {
                    let len = decode_varint(compressed_data, &mut pos)? as usize;
                    if pos + len > compressed_data.len() {
                        return Err(anyhow::anyhow!("Unexpected end of data"));
                    }
                    let s = String::from_utf8(compressed_data[pos..pos + len].to_vec())?;
                    pos += len;
                    TimeSeriesValue::String(s)
                }
                3 => {
                    if pos >= compressed_data.len() {
                        return Err(anyhow::anyhow!("Unexpected end of data"));
                    }
                    let b = compressed_data[pos] != 0;
                    pos += 1;
                    TimeSeriesValue::Boolean(b)
                }
                4 => TimeSeriesValue::Null,
                _ => return Err(anyhow::anyhow!("Unknown value type")),
            };

            let labels = decode_labels(compressed_data, &mut pos)?;

            points.push(DataPoint {
                timestamp,
                value,
                labels,
            });
        }

        Ok(points)
    }

    fn compression_ratio(&self) -> f64 {
        0.15 // Estimated 85% compression for float values
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn encode_value(value: &TimeSeriesValue, output: &mut Vec<u8>) -> Result<()> {
    match value {
        TimeSeriesValue::Float(v) => {
            output.push(0);
            output.extend_from_slice(&v.to_le_bytes());
        }
        TimeSeriesValue::Integer(v) => {
            output.push(1);
            encode_varint_signed(*v, output);
        }
        TimeSeriesValue::String(s) => {
            output.push(2);
            encode_varint(s.len() as u64, output);
            output.extend_from_slice(s.as_bytes());
        }
        TimeSeriesValue::Boolean(b) => {
            output.push(3);
            output.push(if *b { 1 } else { 0 });
        }
        TimeSeriesValue::Null => {
            output.push(4);
        }
    }
    Ok(())
}

fn decode_value(data: &[u8], pos: &mut usize) -> Result<TimeSeriesValue> {
    if *pos >= data.len() {
        return Err(anyhow::anyhow!("Unexpected end of data"));
    }
    let value_type = data[*pos];
    *pos += 1;

    match value_type {
        0 => {
            if *pos + 8 > data.len() {
                return Err(anyhow::anyhow!("Unexpected end of data"));
            }
            let v = f64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Ok(TimeSeriesValue::Float(v))
        }
        1 => {
            let v = decode_varint_signed(data, pos)?;
            Ok(TimeSeriesValue::Integer(v))
        }
        2 => {
            let len = decode_varint(data, pos)? as usize;
            if *pos + len > data.len() {
                return Err(anyhow::anyhow!("Unexpected end of data"));
            }
            let s = String::from_utf8(data[*pos..*pos + len].to_vec())?;
            *pos += len;
            Ok(TimeSeriesValue::String(s))
        }
        3 => {
            if *pos >= data.len() {
                return Err(anyhow::anyhow!("Unexpected end of data"));
            }
            let b = data[*pos] != 0;
            *pos += 1;
            Ok(TimeSeriesValue::Boolean(b))
        }
        4 => Ok(TimeSeriesValue::Null),
        _ => Err(anyhow::anyhow!("Unknown value type: {}", value_type)),
    }
}

fn encode_labels(labels: &HashMap<String, String>, output: &mut Vec<u8>) {
    encode_varint(labels.len() as u64, output);
    for (key, value) in labels {
        encode_varint(key.len() as u64, output);
        output.extend_from_slice(key.as_bytes());
        encode_varint(value.len() as u64, output);
        output.extend_from_slice(value.as_bytes());
    }
}

fn decode_labels(data: &[u8], pos: &mut usize) -> Result<HashMap<String, String>> {
    let num_labels = decode_varint(data, pos)? as usize;
    let mut labels = HashMap::with_capacity(num_labels);

    for _ in 0..num_labels {
        let key_len = decode_varint(data, pos)? as usize;
        if *pos + key_len > data.len() {
            return Err(anyhow::anyhow!("Unexpected end of data"));
        }
        let key = String::from_utf8(data[*pos..*pos + key_len].to_vec())?;
        *pos += key_len;

        let val_len = decode_varint(data, pos)? as usize;
        if *pos + val_len > data.len() {
            return Err(anyhow::anyhow!("Unexpected end of data"));
        }
        let val = String::from_utf8(data[*pos..*pos + val_len].to_vec())?;
        *pos += val_len;

        labels.insert(key, val);
    }

    Ok(labels)
}

// ============================================================================
// Factory
// ============================================================================

/// Factory function to create compressors
pub fn create_compressor(compression_type: &CompressionType) -> Box<dyn TimeSeriesCompressor> {
    match compression_type {
        CompressionType::Delta => Box::new(DeltaCompressor::new(0)),
        CompressionType::DoubleDelta => Box::new(DoubleDeltaCompressor),
        CompressionType::Gorilla => Box::new(GorillaCompressor),
        CompressionType::Lz4 => Box::new(DeltaCompressor::new(0)), // Fallback until LZ4 wrapper
        CompressionType::Zstd => Box::new(DeltaCompressor::new(0)), // Fallback until Zstd wrapper
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<DataPoint> {
        vec![
            DataPoint {
                timestamp: 1000,
                value: TimeSeriesValue::Float(100.5),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 1010,
                value: TimeSeriesValue::Float(101.2),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 1020,
                value: TimeSeriesValue::Float(100.8),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 1030,
                value: TimeSeriesValue::Float(102.1),
                labels: HashMap::new(),
            },
        ]
    }

    #[test]
    fn test_delta_compression_roundtrip() {
        let compressor = DeltaCompressor::new(0);
        let data = create_test_data();

        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(data.len(), decompressed.len());
        for (orig, dec) in data.iter().zip(decompressed.iter()) {
            assert_eq!(orig.timestamp, dec.timestamp);
            match (&orig.value, &dec.value) {
                (TimeSeriesValue::Float(a), TimeSeriesValue::Float(b)) => {
                    assert!((a - b).abs() < 0.001);
                }
                _ => panic!("Value type mismatch"),
            }
        }
    }

    #[test]
    fn test_double_delta_compression_roundtrip() {
        let compressor = DoubleDeltaCompressor;
        let data = create_test_data();

        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(data.len(), decompressed.len());
        for (orig, dec) in data.iter().zip(decompressed.iter()) {
            assert_eq!(orig.timestamp, dec.timestamp);
        }
    }

    #[test]
    fn test_gorilla_compression_roundtrip() {
        let compressor = GorillaCompressor;
        let data = create_test_data();

        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(data.len(), decompressed.len());
        for (orig, dec) in data.iter().zip(decompressed.iter()) {
            assert_eq!(orig.timestamp, dec.timestamp);
            match (&orig.value, &dec.value) {
                (TimeSeriesValue::Float(a), TimeSeriesValue::Float(b)) => {
                    assert_eq!(*a, *b); // Gorilla is lossless
                }
                _ => panic!("Value type mismatch"),
            }
        }
    }

    #[test]
    fn test_compression_with_labels() {
        let compressor = DeltaCompressor::new(0);
        let mut labels = HashMap::new();
        labels.insert("host".to_string(), "server1".to_string());
        labels.insert("region".to_string(), "us-east".to_string());

        let data = vec![DataPoint {
            timestamp: 1000,
            value: TimeSeriesValue::Float(42.0),
            labels,
        }];

        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(
            decompressed[0].labels.get("host"),
            Some(&"server1".to_string())
        );
        assert_eq!(
            decompressed[0].labels.get("region"),
            Some(&"us-east".to_string())
        );
    }

    #[test]
    fn test_empty_data() {
        let compressor = DeltaCompressor::new(0);
        let data: Vec<DataPoint> = vec![];

        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_integer_values() {
        let compressor = DeltaCompressor::new(0);
        let data = vec![
            DataPoint {
                timestamp: 1000,
                value: TimeSeriesValue::Integer(100),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 1010,
                value: TimeSeriesValue::Integer(-50),
                labels: HashMap::new(),
            },
        ];

        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(decompressed[0].value, TimeSeriesValue::Integer(100));
        assert_eq!(decompressed[1].value, TimeSeriesValue::Integer(-50));
    }

    #[test]
    fn test_compression_ratio() {
        let compressor = GorillaCompressor;
        let mut data = Vec::new();

        // Create 1000 data points with slowly changing values
        for i in 0..1000 {
            data.push(DataPoint {
                timestamp: 1000 + i * 10,
                value: TimeSeriesValue::Float(100.0 + (i as f64 * 0.01).sin()),
                labels: HashMap::new(),
            });
        }

        let compressed = compressor.compress(&data).unwrap();

        // Original size: 1000 * (8 bytes timestamp + 8 bytes value) = 16000 bytes
        // Compressed should be significantly smaller
        let original_size = 1000 * 16;
        let compression_ratio = compressed.len() as f64 / original_size as f64;

        // Expect at least 30% compression (ratio < 0.7)
        assert!(
            compression_ratio < 0.7,
            "Compression ratio: {}",
            compression_ratio
        );
    }
}
