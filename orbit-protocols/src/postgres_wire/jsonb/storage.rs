//! Binary Storage and Serialization for JSONB
//!
//! This module implements efficient binary storage for JSONB values with:
//! - Compact binary representation
//! - Fast serialization/deserialization
//! - Zero-copy access patterns where possible
//! - Compression for large JSON documents

use crate::postgres_wire::jsonb::{JsonbError, JsonbResult, JsonbValue};
use std::io::{Cursor, Read};

/// Binary JSONB format version
const JSONB_VERSION: u8 = 1;

/// JSONB binary format magic bytes
const JSONB_MAGIC: &[u8] = b"JSONB";

/// Binary storage representation of a JSONB value
#[derive(Debug, Clone, PartialEq)]
pub struct JsonbBinary {
    /// Binary data
    data: Vec<u8>,
    /// Version of the binary format
    version: u8,
}

impl JsonbBinary {
    /// Create a new binary JSONB from raw data
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            version: JSONB_VERSION,
        }
    }

    /// Serialize a JsonbValue to binary format
    pub fn from_jsonb(value: &JsonbValue) -> JsonbResult<Self> {
        let mut buffer = Vec::new();

        // Write magic bytes
        buffer.extend_from_slice(JSONB_MAGIC);

        // Write version
        buffer.push(JSONB_VERSION);

        // Serialize the value
        Self::serialize_value(value, &mut buffer)?;

        Ok(Self::new(buffer))
    }

    /// Deserialize binary data back to JsonbValue
    pub fn to_jsonb(&self) -> JsonbResult<JsonbValue> {
        let mut cursor = Cursor::new(&self.data);

        // Verify magic bytes
        let mut magic = [0u8; 5];
        cursor.read_exact(&mut magic).map_err(|e| {
            JsonbError::SerializationError(format!("Failed to read magic bytes: {}", e))
        })?;

        if magic != JSONB_MAGIC {
            return Err(JsonbError::SerializationError(
                "Invalid JSONB magic bytes".to_string(),
            ));
        }

        // Read version
        let mut version = [0u8; 1];
        cursor.read_exact(&mut version).map_err(|e| {
            JsonbError::SerializationError(format!("Failed to read version: {}", e))
        })?;

        if version[0] != JSONB_VERSION {
            return Err(JsonbError::SerializationError(format!(
                "Unsupported JSONB version: {}",
                version[0]
            )));
        }

        // Deserialize the value
        Self::deserialize_value(&mut cursor)
    }

    /// Get the size of the binary data
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the binary data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the format version
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Compress the binary data (placeholder implementation)
    pub fn compress(&mut self) -> JsonbResult<()> {
        // In a real implementation, this would use a compression algorithm
        // like LZ4 or zstd for better performance
        Ok(())
    }

    /// Decompress the binary data (placeholder implementation)  
    pub fn decompress(&mut self) -> JsonbResult<()> {
        // In a real implementation, this would decompress the data
        Ok(())
    }

    /// Serialize a JsonbValue to the buffer
    fn serialize_value(value: &JsonbValue, buffer: &mut Vec<u8>) -> JsonbResult<()> {
        match value {
            JsonbValue::Null => {
                buffer.push(0x00); // Type tag for null
            }
            JsonbValue::Bool(b) => {
                buffer.push(if *b { 0x01 } else { 0x02 }); // Type tags for true/false
            }
            JsonbValue::Number(n) => {
                buffer.push(0x03); // Type tag for number
                buffer.extend_from_slice(&n.to_le_bytes());
            }
            JsonbValue::String(s) => {
                buffer.push(0x04); // Type tag for string
                let bytes = s.as_bytes();
                Self::write_length(bytes.len(), buffer);
                buffer.extend_from_slice(bytes);
            }
            JsonbValue::Array(arr) => {
                buffer.push(0x05); // Type tag for array
                Self::write_length(arr.len(), buffer);
                for item in arr {
                    Self::serialize_value(item, buffer)?;
                }
            }
            JsonbValue::Object(obj) => {
                buffer.push(0x06); // Type tag for object
                Self::write_length(obj.len(), buffer);
                for (key, value) in obj {
                    // Write key
                    let key_bytes = key.as_bytes();
                    Self::write_length(key_bytes.len(), buffer);
                    buffer.extend_from_slice(key_bytes);
                    // Write value
                    Self::serialize_value(value, buffer)?;
                }
            }
        }
        Ok(())
    }

    /// Deserialize a JsonbValue from the cursor
    fn deserialize_value(cursor: &mut Cursor<&Vec<u8>>) -> JsonbResult<JsonbValue> {
        let mut type_tag = [0u8; 1];
        cursor.read_exact(&mut type_tag).map_err(|e| {
            JsonbError::SerializationError(format!("Failed to read type tag: {}", e))
        })?;

        match type_tag[0] {
            0x00 => Ok(JsonbValue::Null),
            0x01 => Ok(JsonbValue::Bool(true)),
            0x02 => Ok(JsonbValue::Bool(false)),
            0x03 => {
                let mut bytes = [0u8; 8];
                cursor.read_exact(&mut bytes).map_err(|e| {
                    JsonbError::SerializationError(format!("Failed to read number: {}", e))
                })?;
                Ok(JsonbValue::Number(f64::from_le_bytes(bytes)))
            }
            0x04 => {
                let length = Self::read_length(cursor)?;
                let mut bytes = vec![0u8; length];
                cursor.read_exact(&mut bytes).map_err(|e| {
                    JsonbError::SerializationError(format!("Failed to read string: {}", e))
                })?;
                let s = String::from_utf8(bytes).map_err(|e| {
                    JsonbError::SerializationError(format!("Invalid UTF-8 string: {}", e))
                })?;
                Ok(JsonbValue::String(s))
            }
            0x05 => {
                let length = Self::read_length(cursor)?;
                let mut array = Vec::with_capacity(length);
                for _ in 0..length {
                    array.push(Self::deserialize_value(cursor)?);
                }
                Ok(JsonbValue::Array(array))
            }
            0x06 => {
                let length = Self::read_length(cursor)?;
                let mut object = std::collections::HashMap::new();
                for _ in 0..length {
                    // Read key
                    let key_length = Self::read_length(cursor)?;
                    let mut key_bytes = vec![0u8; key_length];
                    cursor.read_exact(&mut key_bytes).map_err(|e| {
                        JsonbError::SerializationError(format!("Failed to read object key: {}", e))
                    })?;
                    let key = String::from_utf8(key_bytes).map_err(|e| {
                        JsonbError::SerializationError(format!("Invalid UTF-8 key: {}", e))
                    })?;
                    // Read value
                    let value = Self::deserialize_value(cursor)?;
                    object.insert(key, value);
                }
                Ok(JsonbValue::Object(object))
            }
            _ => Err(JsonbError::SerializationError(format!(
                "Unknown type tag: {}",
                type_tag[0]
            ))),
        }
    }

    /// Write a length value with variable-length encoding
    fn write_length(length: usize, buffer: &mut Vec<u8>) {
        if length < 0x80 {
            buffer.push(length as u8);
        } else if length < 0x4000 {
            buffer.push(((length >> 8) | 0x80) as u8);
            buffer.push(length as u8);
        } else {
            // For very large lengths, use 4 bytes
            buffer.push(0xC0);
            buffer.extend_from_slice(&(length as u32).to_le_bytes());
        }
    }

    /// Read a length value with variable-length encoding
    fn read_length(cursor: &mut Cursor<&Vec<u8>>) -> JsonbResult<usize> {
        let mut first_byte = [0u8; 1];
        cursor
            .read_exact(&mut first_byte)
            .map_err(|e| JsonbError::SerializationError(format!("Failed to read length: {}", e)))?;

        let first = first_byte[0];
        if first < 0x80 {
            Ok(first as usize)
        } else if first < 0xC0 {
            let mut second_byte = [0u8; 1];
            cursor.read_exact(&mut second_byte).map_err(|e| {
                JsonbError::SerializationError(format!("Failed to read length byte 2: {}", e))
            })?;
            Ok((((first & 0x3F) as usize) << 8) | (second_byte[0] as usize))
        } else {
            let mut bytes = [0u8; 4];
            cursor.read_exact(&mut bytes).map_err(|e| {
                JsonbError::SerializationError(format!("Failed to read length bytes: {}", e))
            })?;
            Ok(u32::from_le_bytes(bytes) as usize)
        }
    }
}

/// Trait for efficient JSONB storage operations
pub trait JsonbStorage {
    /// Store a JSONB value
    fn store(&mut self, value: &JsonbValue) -> JsonbResult<Vec<u8>>;

    /// Load a JSONB value from binary data
    fn load(&self, data: &[u8]) -> JsonbResult<JsonbValue>;

    /// Get the storage size of a JSONB value
    fn size_of(&self, value: &JsonbValue) -> usize;

    /// Optimize storage (compress, deduplicate, etc.)
    fn optimize(&mut self) -> JsonbResult<()>;
}

/// In-memory JSONB storage implementation
pub struct MemoryJsonbStorage {
    compression_enabled: bool,
}

impl MemoryJsonbStorage {
    pub fn new() -> Self {
        Self {
            compression_enabled: false,
        }
    }

    pub fn with_compression() -> Self {
        Self {
            compression_enabled: true,
        }
    }
}

impl Default for MemoryJsonbStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonbStorage for MemoryJsonbStorage {
    fn store(&mut self, value: &JsonbValue) -> JsonbResult<Vec<u8>> {
        let mut binary = JsonbBinary::from_jsonb(value)?;

        if self.compression_enabled {
            binary.compress()?;
        }

        Ok(binary.data)
    }

    fn load(&self, data: &[u8]) -> JsonbResult<JsonbValue> {
        let mut binary = JsonbBinary::new(data.to_vec());

        if self.compression_enabled {
            binary.decompress()?;
        }

        binary.to_jsonb()
    }

    fn size_of(&self, value: &JsonbValue) -> usize {
        JsonbBinary::from_jsonb(value)
            .map(|b| b.size())
            .unwrap_or(0)
    }

    fn optimize(&mut self) -> JsonbResult<()> {
        // Enable compression for optimization
        self.compression_enabled = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonb_binary_roundtrip() {
        let original = JsonbValue::from_json_str(
            r#"{"name": "Alice", "age": 30, "tags": ["dev", "rust"], "active": true}"#,
        )
        .unwrap();

        let binary = JsonbBinary::from_jsonb(&original).unwrap();
        let roundtrip = binary.to_jsonb().unwrap();

        assert_eq!(original, roundtrip);
    }

    #[test]
    fn test_binary_storage_size() {
        let value = JsonbValue::from_json_str(r#"{"test": "value"}"#).unwrap();
        let binary = JsonbBinary::from_jsonb(&value).unwrap();

        // Should have some reasonable size (magic + version + data)
        assert!(binary.size() > 10);
        assert!(binary.size() < 100); // Should be compact
    }

    #[test]
    fn test_memory_storage() {
        let mut storage = MemoryJsonbStorage::new();
        let value = JsonbValue::from_json_str(r#"[1, 2, 3, {"key": "value"}]"#).unwrap();

        let stored_data = storage.store(&value).unwrap();
        let loaded_value = storage.load(&stored_data).unwrap();

        assert_eq!(value, loaded_value);
    }

    #[test]
    fn test_storage_with_compression() {
        let mut storage = MemoryJsonbStorage::with_compression();
        let value = JsonbValue::from_json_str(
            r#"{"large_text": "This is a large text field that could benefit from compression when stored in binary format"}"#
        ).unwrap();

        let stored_data = storage.store(&value).unwrap();
        let loaded_value = storage.load(&stored_data).unwrap();

        assert_eq!(value, loaded_value);
    }

    #[test]
    fn test_variable_length_encoding() {
        let small_value = JsonbValue::String("a".repeat(50));
        let medium_value = JsonbValue::String("b".repeat(500));
        let large_value = JsonbValue::String("c".repeat(5000));

        let small_binary = JsonbBinary::from_jsonb(&small_value).unwrap();
        let medium_binary = JsonbBinary::from_jsonb(&medium_value).unwrap();
        let large_binary = JsonbBinary::from_jsonb(&large_value).unwrap();

        // Verify that all can be deserialized correctly
        assert_eq!(small_binary.to_jsonb().unwrap(), small_value);
        assert_eq!(medium_binary.to_jsonb().unwrap(), medium_value);
        assert_eq!(large_binary.to_jsonb().unwrap(), large_value);

        // Verify size scaling
        assert!(medium_binary.size() > small_binary.size());
        assert!(large_binary.size() > medium_binary.size());
    }

    #[test]
    fn test_all_json_types() {
        let complex_value = JsonbValue::Object(
            [
                ("null".to_string(), JsonbValue::Null),
                ("bool_true".to_string(), JsonbValue::Bool(true)),
                ("bool_false".to_string(), JsonbValue::Bool(false)),
                (
                    "number".to_string(),
                    JsonbValue::Number(std::f64::consts::PI),
                ),
                (
                    "string".to_string(),
                    JsonbValue::String("hello world".to_string()),
                ),
                (
                    "array".to_string(),
                    JsonbValue::Array(vec![
                        JsonbValue::Number(1.0),
                        JsonbValue::Number(2.0),
                        JsonbValue::Number(3.0),
                    ]),
                ),
                (
                    "nested".to_string(),
                    JsonbValue::Object(
                        [("inner".to_string(), JsonbValue::String("value".to_string()))]
                            .iter()
                            .cloned()
                            .collect(),
                    ),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        );

        let binary = JsonbBinary::from_jsonb(&complex_value).unwrap();
        let roundtrip = binary.to_jsonb().unwrap();

        assert_eq!(complex_value, roundtrip);
    }
}
