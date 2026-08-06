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

/// Encode an unsigned integer using variable-length encoding
fn encode_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

// ============================================================================
// Decoding cursor
// ============================================================================

/// Smallest number of bytes any encoder can spend on one data point: a timestamp
/// varint, a value-type marker, and a label count, one byte each.
const MIN_ENCODED_POINT_BYTES: usize = 3;

/// Smallest number of bytes one label can occupy: a zero-length key and value.
const MIN_ENCODED_LABEL_BYTES: usize = 2;

/// A bounds-checked read cursor over a compressed buffer.
///
/// Compressed input is untrusted — it arrives from disk, the network, or a
/// partially written segment. Every primitive here is therefore total: truncated
/// or corrupt input yields an `Err`, never a slice panic. Keeping the check and
/// the read in one place also removes the `pos + len > data.len()` idiom, which
/// silently wraps for a hostile length; [`Cursor::remaining`] subtracts instead.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Bytes not yet consumed.
    const fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Take the next `len` bytes, advancing the cursor.
    ///
    /// # Errors
    /// Returns an error if fewer than `len` bytes remain.
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if len > self.remaining() {
            return Err(anyhow::anyhow!(
                "Unexpected end of data: need {len} bytes, {} remain",
                self.remaining()
            ));
        }
        let bytes = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(bytes)
    }

    /// Take the next `N` bytes as a fixed-size array.
    ///
    /// # Errors
    /// Returns an error if fewer than `N` bytes remain.
    fn take_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.take(N)?;
        <[u8; N]>::try_from(bytes)
            .map_err(|_| anyhow::anyhow!("Expected {N} bytes, got {}", bytes.len()))
    }

    /// Take one byte.
    ///
    /// # Errors
    /// Returns an error at end of input.
    fn take_u8(&mut self) -> Result<u8> {
        self.take_array::<1>().map(|[byte]| byte)
    }

    /// Take a little-endian `u64`.
    ///
    /// # Errors
    /// Returns an error if fewer than 8 bytes remain.
    fn take_u64_le(&mut self) -> Result<u64> {
        self.take_array::<8>().map(u64::from_le_bytes)
    }

    /// Take a little-endian `f64`.
    ///
    /// # Errors
    /// Returns an error if fewer than 8 bytes remain.
    fn take_f64_le(&mut self) -> Result<f64> {
        self.take_array::<8>().map(f64::from_le_bytes)
    }

    /// Take `len` bytes (at most 8) as the low bytes of a little-endian `u64`.
    ///
    /// # Errors
    /// Returns an error if `len` exceeds 8 — a `u64` cannot hold more — or if
    /// fewer than `len` bytes remain.
    fn take_padded_u64(&mut self, len: usize) -> Result<u64> {
        if len > 8 {
            return Err(anyhow::anyhow!(
                "Invalid meaningful-bit width: {len} bytes exceeds 8"
            ));
        }
        let mut buf = [0u8; 8];
        buf[..len].copy_from_slice(self.take(len)?);
        Ok(u64::from_le_bytes(buf))
    }

    /// Decode an unsigned varint.
    ///
    /// # Errors
    /// Returns an error at end of input or if the varint exceeds 64 bits.
    fn take_varint(&mut self) -> Result<u64> {
        // A byte-at-a-time LEB128 decode is inherently sequential; a local
        // accumulator states that more plainly than an iterator adapter would.
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.take_u8()?;
            value |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(anyhow::anyhow!("Varint too long"))
    }

    /// Decode a zigzag-encoded signed varint.
    ///
    /// # Errors
    /// Returns an error at end of input or if the varint exceeds 64 bits.
    fn take_varint_signed(&mut self) -> Result<i64> {
        self.take_varint()
            .map(|unsigned| ((unsigned >> 1) as i64) ^ (-((unsigned & 1) as i64)))
    }

    /// Decode a varint-prefixed UTF-8 string.
    ///
    /// # Errors
    /// Returns an error if the length runs past the end of input or the bytes are
    /// not valid UTF-8.
    fn take_string(&mut self) -> Result<String> {
        let len = usize::try_from(self.take_varint()?)
            .map_err(|_| anyhow::anyhow!("String length exceeds addressable memory"))?;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(Into::into)
    }

    /// Decode an element count.
    ///
    /// # Errors
    /// Returns an error at end of input or if the count exceeds `usize`.
    fn take_count(&mut self) -> Result<usize> {
        usize::try_from(self.take_varint()?)
            .map_err(|_| anyhow::anyhow!("Element count exceeds addressable memory"))
    }

    /// Capacity to preallocate for `count` elements of at least `min_bytes` each.
    ///
    /// A decoded count is untrusted: a ten-byte payload can claim to hold billions
    /// of points. Bounding the hint by what the remaining bytes could physically
    /// contain keeps a corrupt header from requesting a huge allocation before the
    /// first truncated read is even attempted; the decode loop then fails normally.
    const fn capacity_for(&self, count: usize, min_bytes: usize) -> usize {
        let affordable = self.remaining() / min_bytes;
        if count < affordable {
            count
        } else {
            affordable
        }
    }
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

            encode_labels(&point.labels, &mut output);
        }

        Ok(output)
    }

    fn decompress(&self, compressed_data: &[u8]) -> Result<Vec<DataPoint>> {
        if compressed_data.is_empty() {
            return Ok(Vec::new());
        }

        let mut cursor = Cursor::new(compressed_data);

        // Header: number of points, then the base timestamp deltas are relative to.
        let num_points = cursor.take_count()?;
        let base_ts = cursor.take_varint_signed()?;

        let mut points =
            Vec::with_capacity(cursor.capacity_for(num_points, MIN_ENCODED_POINT_BYTES));
        let mut prev_timestamp = base_ts;
        let mut prev_value: f64 = 0.0;

        for _ in 0..num_points {
            let timestamp = prev_timestamp + cursor.take_varint_signed()?;
            prev_timestamp = timestamp;

            let value_type = cursor.take_u8()?;
            let value = match value_type {
                // Float, delta encoded against the running previous value.
                0 => {
                    prev_value += cursor.take_f64_le()?;
                    TimeSeriesValue::Float(prev_value)
                }
                1 => TimeSeriesValue::Integer(cursor.take_varint_signed()?),
                2 => TimeSeriesValue::String(cursor.take_string()?),
                3 => TimeSeriesValue::Boolean(cursor.take_u8()? != 0),
                4 => TimeSeriesValue::Null,
                other => return Err(anyhow::anyhow!("Unknown value type: {other}")),
            };

            points.push(DataPoint {
                timestamp,
                value,
                labels: decode_labels(&mut cursor)?,
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
        encode_value(&first.value, &mut output);
        encode_labels(&first.labels, &mut output);

        if data_points.len() == 1 {
            return Ok(output);
        }

        // Second point: write delta
        let second = &data_points[1];
        let delta1 = second.timestamp - first.timestamp;
        encode_varint_signed(delta1, &mut output);
        encode_value(&second.value, &mut output);
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
            } else if (-63..=64).contains(&delta_of_delta) {
                // 1 byte: marker + 7-bit value
                output.push(0x80 | ((delta_of_delta + 63) as u8 & 0x7F));
            } else {
                // Full varint encoding
                output.push(0xFF);
                encode_varint_signed(delta_of_delta, &mut output);
            }

            encode_value(&point.value, &mut output);
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

        let mut cursor = Cursor::new(compressed_data);
        let num_points = cursor.take_count()?;
        let mut points =
            Vec::with_capacity(cursor.capacity_for(num_points, MIN_ENCODED_POINT_BYTES));

        if num_points == 0 {
            return Ok(points);
        }

        // First point carries a full timestamp; the second carries a delta.
        let first_ts = cursor.take_varint_signed()?;
        points.push(DataPoint {
            timestamp: first_ts,
            value: decode_value(&mut cursor)?,
            labels: decode_labels(&mut cursor)?,
        });

        if num_points == 1 {
            return Ok(points);
        }

        let delta1 = cursor.take_varint_signed()?;
        let second_ts = first_ts + delta1;
        points.push(DataPoint {
            timestamp: second_ts,
            value: decode_value(&mut cursor)?,
            labels: decode_labels(&mut cursor)?,
        });

        // Remaining points carry a delta-of-delta under one of three markers.
        let mut prev_delta = delta1;
        let mut prev_ts = second_ts;

        for _ in 2..num_points {
            let delta_of_delta = match cursor.take_u8()? {
                0 => 0,
                0xFF => cursor.take_varint_signed()?,
                packed => i64::from(packed & 0x7F) - 63,
            };

            let delta = prev_delta + delta_of_delta;
            let timestamp = prev_ts + delta;

            points.push(DataPoint {
                timestamp,
                value: decode_value(&mut cursor)?,
                labels: decode_labels(&mut cursor)?,
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

/// The window of significant bits shared by a run of Gorilla-encoded XOR values.
///
/// Both widths come from the compressed stream, so the pair may describe a block
/// that cannot exist (wider than 64 bits, or empty). [`XorBlock::read_xor`] is the
/// single place that validates them, which keeps the shift below out of reach of
/// malformed input — an unchecked `64 - leading - meaningful` underflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct XorBlock {
    leading_zeros: u32,
    meaningful_bits: u32,
}

impl Default for XorBlock {
    /// The widest possible block: no leading zeros, all 64 bits meaningful.
    fn default() -> Self {
        Self {
            leading_zeros: 0,
            meaningful_bits: 64,
        }
    }
}

impl XorBlock {
    /// Read this block's meaningful bytes and shift them back into position.
    ///
    /// # Errors
    /// Returns an error if the widths do not describe a non-empty 64-bit block, or
    /// if the payload runs past the end of input.
    fn read_xor(self, cursor: &mut Cursor<'_>) -> Result<u64> {
        let trailing_zeros = 64_u32
            .checked_sub(self.leading_zeros)
            .and_then(|rest| rest.checked_sub(self.meaningful_bits))
            .filter(|_| self.meaningful_bits > 0)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid Gorilla block: {} leading + {} meaningful bits",
                    self.leading_zeros,
                    self.meaningful_bits
                )
            })?;

        let meaningful = cursor.take_padded_u64(self.meaningful_bits.div_ceil(8) as usize)?;
        Ok(meaningful << trailing_zeros)
    }
}

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
                                let meaningful = xor >> prev_trailing_zeros;
                                // Write meaningful bits (up to 8 bytes)
                                let bytes_needed = meaningful_bits.div_ceil(8) as usize;
                                output.extend_from_slice(&meaningful.to_le_bytes()[..bytes_needed]);
                            } else {
                                // New block: marker + leading zeros + meaningful bits length + bits
                                output.push(2);
                                output.push(leading as u8);
                                let meaningful_bits = 64 - leading - trailing;
                                output.push(meaningful_bits as u8);
                                let meaningful = xor >> trailing;
                                let bytes_needed = meaningful_bits.div_ceil(8) as usize;
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

        let mut cursor = Cursor::new(compressed_data);
        let num_points = cursor.take_count()?;
        let mut points =
            Vec::with_capacity(cursor.capacity_for(num_points, MIN_ENCODED_POINT_BYTES));

        let mut prev_ts: i64 = 0;
        let mut prev_value_bits: u64 = 0;
        let mut block = XorBlock::default();

        for i in 0..num_points {
            let timestamp = prev_ts + cursor.take_varint_signed()?;
            prev_ts = timestamp;

            let value_type = cursor.take_u8()?;
            let value = match value_type {
                0 => {
                    // Float: the first value is stored whole, the rest as an XOR
                    // against the previous value under one of three markers.
                    let value_bits = if i == 0 {
                        cursor.take_u64_le()?
                    } else {
                        match cursor.take_u8()? {
                            // Unchanged value.
                            0 => prev_value_bits,
                            // Reuse the previous block's leading/meaningful widths.
                            1 => prev_value_bits ^ block.read_xor(&mut cursor)?,
                            // New block: widths are re-stated before the payload.
                            2 => {
                                block = XorBlock {
                                    leading_zeros: u32::from(cursor.take_u8()?),
                                    meaningful_bits: u32::from(cursor.take_u8()?),
                                };
                                prev_value_bits ^ block.read_xor(&mut cursor)?
                            }
                            other => {
                                return Err(anyhow::anyhow!("Invalid Gorilla marker: {other}"))
                            }
                        }
                    };
                    prev_value_bits = value_bits;
                    TimeSeriesValue::Float(f64::from_bits(value_bits))
                }
                1 => TimeSeriesValue::Integer(cursor.take_varint_signed()?),
                2 => TimeSeriesValue::String(cursor.take_string()?),
                3 => TimeSeriesValue::Boolean(cursor.take_u8()? != 0),
                4 => TimeSeriesValue::Null,
                other => return Err(anyhow::anyhow!("Unknown value type: {other}")),
            };

            points.push(DataPoint {
                timestamp,
                value,
                labels: decode_labels(&mut cursor)?,
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

/// Encode a value with its one-byte type marker, absolute (not delta encoded).
fn encode_value(value: &TimeSeriesValue, output: &mut Vec<u8>) {
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
            encode_string(s, output);
        }
        TimeSeriesValue::Boolean(b) => {
            output.push(3);
            output.push(u8::from(*b));
        }
        TimeSeriesValue::Null => {
            output.push(4);
        }
    }
}

/// Decode a value written by [`encode_value`].
///
/// # Errors
/// Returns an error on a truncated payload or an unknown type marker.
fn decode_value(cursor: &mut Cursor<'_>) -> Result<TimeSeriesValue> {
    match cursor.take_u8()? {
        0 => cursor.take_f64_le().map(TimeSeriesValue::Float),
        1 => cursor.take_varint_signed().map(TimeSeriesValue::Integer),
        2 => cursor.take_string().map(TimeSeriesValue::String),
        3 => cursor.take_u8().map(|b| TimeSeriesValue::Boolean(b != 0)),
        4 => Ok(TimeSeriesValue::Null),
        other => Err(anyhow::anyhow!("Unknown value type: {other}")),
    }
}

/// Write a varint-prefixed UTF-8 string.
fn encode_string(value: &str, output: &mut Vec<u8>) {
    encode_varint(value.len() as u64, output);
    output.extend_from_slice(value.as_bytes());
}

fn encode_labels(labels: &HashMap<String, String>, output: &mut Vec<u8>) {
    encode_varint(labels.len() as u64, output);
    labels.iter().for_each(|(key, value)| {
        encode_string(key, output);
        encode_string(value, output);
    });
}

/// Decode a label map written by [`encode_labels`].
///
/// # Errors
/// Returns an error on a truncated payload or non-UTF-8 key or value.
fn decode_labels(cursor: &mut Cursor<'_>) -> Result<HashMap<String, String>> {
    let num_labels = cursor.take_count()?;
    let mut labels =
        HashMap::with_capacity(cursor.capacity_for(num_labels, MIN_ENCODED_LABEL_BYTES));

    for _ in 0..num_labels {
        let key = cursor.take_string()?;
        let value = cursor.take_string()?;
        labels.insert(key, value);
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

    /// One instance of each codec, for tests that must hold across all of them.
    fn all_compressors() -> Vec<Box<dyn TimeSeriesCompressor>> {
        vec![
            Box::new(DeltaCompressor::new(0)),
            Box::new(DoubleDeltaCompressor),
            Box::new(GorillaCompressor),
        ]
    }

    fn data_with_labels() -> Vec<DataPoint> {
        create_test_data()
            .into_iter()
            .map(|point| DataPoint {
                labels: HashMap::from([
                    ("host".to_string(), "server1".to_string()),
                    ("region".to_string(), "us-east".to_string()),
                ]),
                ..point
            })
            .collect()
    }

    /// Every prefix of a valid stream must decode or error — never panic.
    #[test]
    fn test_decompress_survives_truncation_at_every_offset() {
        for compressor in all_compressors() {
            let compressed = compressor.compress(&data_with_labels()).unwrap();

            for len in 0..compressed.len() {
                // A panic here fails the test; the contract is "no panic", not "Err".
                let _ = compressor.decompress(&compressed[..len]);
            }
        }
    }

    /// Single-byte corruption must not panic either: lengths, counts, type markers
    /// and Gorilla bit widths are all attacker-reachable through this path.
    #[test]
    fn test_decompress_survives_single_byte_corruption() {
        for compressor in all_compressors() {
            let compressed = compressor.compress(&data_with_labels()).unwrap();

            for index in 0..compressed.len() {
                for replacement in [0x00, 0x01, 0x02, 0x7F, 0x80, 0xFE, 0xFF] {
                    let corrupted: Vec<u8> = compressed
                        .iter()
                        .enumerate()
                        .map(|(i, &byte)| if i == index { replacement } else { byte })
                        .collect();
                    let _ = compressor.decompress(&corrupted);
                }
            }
        }
    }

    /// A tiny payload claiming a huge point count must fail, not try to allocate it.
    #[test]
    fn test_decompress_rejects_impossible_counts() {
        let mut header = Vec::new();
        encode_varint(u64::MAX, &mut header);

        for compressor in all_compressors() {
            assert!(
                compressor.decompress(&header).is_err(),
                "an impossible point count must be rejected"
            );
        }

        // The count is only ever a capacity *hint*, bounded by the bytes present.
        let cursor = Cursor::new(&[0u8; 12]);
        assert_eq!(cursor.capacity_for(usize::MAX, MIN_ENCODED_POINT_BYTES), 4);
        assert_eq!(cursor.capacity_for(2, MIN_ENCODED_POINT_BYTES), 2);
    }

    /// Gorilla bit widths come from the stream; an impossible pair must be rejected
    /// rather than underflowing `64 - leading - meaningful`.
    #[test]
    fn test_gorilla_block_rejects_impossible_widths() {
        let cases = [
            XorBlock {
                leading_zeros: 200,
                meaningful_bits: 200,
            },
            XorBlock {
                leading_zeros: 60,
                meaningful_bits: 60,
            },
            // Empty block: would shift by 64 and overflow the shift.
            XorBlock {
                leading_zeros: 0,
                meaningful_bits: 0,
            },
        ];

        for block in cases {
            let payload = [0xFFu8; 8];
            assert!(
                block.read_xor(&mut Cursor::new(&payload)).is_err(),
                "expected {block:?} to be rejected"
            );
        }

        // A well-formed block reads back the bits it describes.
        let block = XorBlock {
            leading_zeros: 56,
            meaningful_bits: 8,
        };
        assert_eq!(block.read_xor(&mut Cursor::new(&[0xAB])).unwrap(), 0xAB);
    }

    #[test]
    fn test_varint_roundtrip_and_limits() {
        for value in [0u64, 1, 127, 128, 300, u64::MAX / 2, u64::MAX] {
            let mut buf = Vec::new();
            encode_varint(value, &mut buf);
            assert_eq!(Cursor::new(&buf).take_varint().unwrap(), value);
        }

        for value in [0i64, -1, 1, i64::MIN, i64::MAX] {
            let mut buf = Vec::new();
            encode_varint_signed(value, &mut buf);
            assert_eq!(Cursor::new(&buf).take_varint_signed().unwrap(), value);
        }

        // A continuation bit that never terminates must be rejected, not looped on.
        assert!(Cursor::new(&[0xFFu8; 16]).take_varint().is_err());
        assert!(Cursor::new(&[]).take_varint().is_err());
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
