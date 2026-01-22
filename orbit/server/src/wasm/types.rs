//! WASM Type System and Conversions
//!
//! This module provides type conversions between SQL types and WASM types.
//! WASM supports a limited set of primitive types (i32, i64, f32, f64),
//! so we need to serialize/deserialize complex types.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// WASM-compatible value representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WasmValue {
    /// Null value
    Null,
    /// Boolean (encoded as i32: 0=false, 1=true)
    Bool(bool),
    /// 32-bit integer
    I32(i32),
    /// 64-bit integer
    I64(i64),
    /// 32-bit float
    F32(f32),
    /// 64-bit float
    F64(f64),
    /// String (serialized to bytes)
    String(String),
    /// Binary data
    Bytes(Vec<u8>),
    /// Array of values
    Array(Vec<WasmValue>),
    /// Object/map
    Object(HashMap<String, WasmValue>),
}

impl WasmValue {
    /// Convert to SqlValue
    pub fn to_sql_value(&self) -> ProtocolResult<SqlValue> {
        match self {
            WasmValue::Null => Ok(SqlValue::Null),
            WasmValue::Bool(b) => Ok(SqlValue::Boolean(*b)),
            WasmValue::I32(i) => Ok(SqlValue::Integer(*i)),
            WasmValue::I64(i) => Ok(SqlValue::BigInt(*i)),
            WasmValue::F32(f) => Ok(SqlValue::Real(*f)),
            WasmValue::F64(f) => Ok(SqlValue::DoublePrecision(*f)),
            WasmValue::String(s) => Ok(SqlValue::Text(s.clone())),
            WasmValue::Bytes(b) => Ok(SqlValue::Bytea(b.clone())),
            WasmValue::Array(arr) => {
                let sql_values: Result<Vec<SqlValue>, _> =
                    arr.iter().map(|v| v.to_sql_value()).collect();
                Ok(SqlValue::Array(sql_values?))
            }
            WasmValue::Object(map) => {
                let json = serde_json::to_value(map).map_err(|e| {
                    ProtocolError::SerializationError(format!("Failed to convert to JSON: {}", e))
                })?;
                Ok(SqlValue::Json(json))
            }
        }
    }

    /// Convert from SqlValue
    pub fn from_sql_value(value: &SqlValue) -> ProtocolResult<Self> {
        match value {
            SqlValue::Null => Ok(WasmValue::Null),
            SqlValue::Boolean(b) => Ok(WasmValue::Bool(*b)),
            SqlValue::SmallInt(i) => Ok(WasmValue::I32(*i as i32)),
            SqlValue::Integer(i) => Ok(WasmValue::I32(*i)),
            SqlValue::BigInt(i) => Ok(WasmValue::I64(*i)),
            SqlValue::Real(f) => Ok(WasmValue::F32(*f)),
            SqlValue::DoublePrecision(f) => Ok(WasmValue::F64(*f)),
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                Ok(WasmValue::String(s.clone()))
            }
            SqlValue::Bytea(b) => Ok(WasmValue::Bytes(b.clone())),
            SqlValue::Array(arr) => {
                let wasm_values: Result<Vec<WasmValue>, _> =
                    arr.iter().map(WasmValue::from_sql_value).collect();
                Ok(WasmValue::Array(wasm_values?))
            }
            SqlValue::Json(json) | SqlValue::Jsonb(json) => {
                if let serde_json::Value::Object(map) = json {
                    let mut wasm_map = HashMap::new();
                    for (k, v) in map {
                        wasm_map.insert(k.clone(), json_to_wasm_value(v)?);
                    }
                    Ok(WasmValue::Object(wasm_map))
                } else {
                    Ok(WasmValue::String(json.to_string()))
                }
            }
            SqlValue::Decimal(d) => Ok(WasmValue::String(d.to_string())),
            SqlValue::Uuid(u) => Ok(WasmValue::String(u.to_string())),
            _ => Ok(WasmValue::String(value.to_postgres_string())),
        }
    }

    /// Serialize to MessagePack bytes for passing to WASM memory
    pub fn to_msgpack_bytes(&self) -> ProtocolResult<Vec<u8>> {
        rmp_serde::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("MessagePack encode: {}", e)))
    }

    /// Deserialize from MessagePack bytes
    pub fn from_msgpack_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        rmp_serde::from_slice(bytes)
            .map_err(|e| ProtocolError::SerializationError(format!("MessagePack decode: {}", e)))
    }

    /// Serialize to JSON bytes (fallback)
    pub fn to_json_bytes(&self) -> ProtocolResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("JSON encode: {}", e)))
    }

    /// Deserialize from JSON bytes
    pub fn from_json_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| ProtocolError::SerializationError(format!("JSON decode: {}", e)))
    }
}

/// Helper to convert serde_json::Value to WasmValue
fn json_to_wasm_value(json: &serde_json::Value) -> ProtocolResult<WasmValue> {
    match json {
        serde_json::Value::Null => Ok(WasmValue::Null),
        serde_json::Value::Bool(b) => Ok(WasmValue::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Ok(WasmValue::I32(i as i32))
                } else {
                    Ok(WasmValue::I64(i))
                }
            } else if let Some(f) = n.as_f64() {
                Ok(WasmValue::F64(f))
            } else {
                Ok(WasmValue::String(n.to_string()))
            }
        }
        serde_json::Value::String(s) => Ok(WasmValue::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<WasmValue>, _> = arr.iter().map(json_to_wasm_value).collect();
            Ok(WasmValue::Array(values?))
        }
        serde_json::Value::Object(map) => {
            let mut wasm_map = HashMap::new();
            for (k, v) in map {
                wasm_map.insert(k.clone(), json_to_wasm_value(v)?);
            }
            Ok(WasmValue::Object(wasm_map))
        }
    }
}

/// Streaming buffer for large dataset processing
#[derive(Debug, Clone)]
pub struct StreamingBuffer {
    /// Current chunk of data
    pub data: Vec<u8>,
    /// Offset in the overall stream
    pub offset: usize,
    /// Total size of the stream (if known)
    pub total_size: Option<usize>,
    /// Whether this is the last chunk
    pub is_last: bool,
}

impl StreamingBuffer {
    /// Create a new streaming buffer
    pub fn new(data: Vec<u8>, offset: usize, total_size: Option<usize>, is_last: bool) -> Self {
        Self {
            data,
            offset,
            total_size,
            is_last,
        }
    }

    /// Get progress percentage (0.0-1.0) if total size is known
    pub fn progress(&self) -> Option<f64> {
        self.total_size.map(|total| {
            if total == 0 {
                1.0
            } else {
                (self.offset + self.data.len()) as f64 / total as f64
            }
        })
    }

    /// Check if we've reached the end of the stream
    pub fn is_complete(&self) -> bool {
        self.is_last
    }

    /// Get the current chunk size
    pub fn chunk_size(&self) -> usize {
        self.data.len()
    }
}

/// WASM function parameter metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmParameter {
    pub name: String,
    pub sql_type: String,
}

/// WASM function metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmFunctionMetadata {
    pub name: String,
    pub params: Vec<WasmParameter>,
    pub return_type: String,
    pub wasm_module: Vec<u8>, // Compiled WASM binary
    pub export_name: String,  // Name of exported function
    pub schema: Option<String>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant)] // Test values like 3.14159 are intentional test floats

    use super::*;

    #[test]
    fn test_null_conversion() {
        let wasm_val = WasmValue::Null;
        let sql_val = wasm_val.to_sql_value().unwrap();
        assert_eq!(sql_val, SqlValue::Null);

        let wasm_val2 = WasmValue::from_sql_value(&sql_val).unwrap();
        assert_eq!(wasm_val, wasm_val2);
    }

    #[test]
    fn test_bool_conversion() {
        let wasm_val = WasmValue::Bool(true);
        let sql_val = wasm_val.to_sql_value().unwrap();
        assert_eq!(sql_val, SqlValue::Boolean(true));

        let wasm_val2 = WasmValue::from_sql_value(&sql_val).unwrap();
        assert_eq!(wasm_val, wasm_val2);
    }

    #[test]
    fn test_integer_conversion() {
        let wasm_val = WasmValue::I32(42);
        let sql_val = wasm_val.to_sql_value().unwrap();
        assert_eq!(sql_val, SqlValue::Integer(42));

        let wasm_val2 = WasmValue::from_sql_value(&sql_val).unwrap();
        assert_eq!(wasm_val, wasm_val2);
    }

    #[test]
    fn test_float_conversion() {
        let wasm_val = WasmValue::F64(3.14159);
        let sql_val = wasm_val.to_sql_value().unwrap();
        assert_eq!(sql_val, SqlValue::DoublePrecision(3.14159));

        let wasm_val2 = WasmValue::from_sql_value(&sql_val).unwrap();
        assert_eq!(wasm_val, wasm_val2);
    }

    #[test]
    fn test_string_conversion() {
        let wasm_val = WasmValue::String("hello".to_string());
        let sql_val = wasm_val.to_sql_value().unwrap();
        assert_eq!(sql_val, SqlValue::Text("hello".to_string()));

        let wasm_val2 = WasmValue::from_sql_value(&sql_val).unwrap();
        assert_eq!(wasm_val, wasm_val2);
    }

    #[test]
    fn test_array_conversion() {
        let wasm_val = WasmValue::Array(vec![
            WasmValue::I32(1),
            WasmValue::I32(2),
            WasmValue::I32(3),
        ]);
        let sql_val = wasm_val.to_sql_value().unwrap();
        match sql_val {
            SqlValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], SqlValue::Integer(1));
                assert_eq!(arr[1], SqlValue::Integer(2));
                assert_eq!(arr[2], SqlValue::Integer(3));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_msgpack_serialization() {
        let wasm_val = WasmValue::Object(HashMap::from([
            ("key1".to_string(), WasmValue::I32(42)),
            ("key2".to_string(), WasmValue::String("value".to_string())),
        ]));

        let bytes = wasm_val.to_msgpack_bytes().unwrap();
        let decoded = WasmValue::from_msgpack_bytes(&bytes).unwrap();
        assert_eq!(wasm_val, decoded);
    }

    #[test]
    fn test_json_serialization() {
        let wasm_val = WasmValue::Array(vec![
            WasmValue::Bool(true),
            WasmValue::I32(42),
            WasmValue::String("test".to_string()),
        ]);

        let bytes = wasm_val.to_json_bytes().unwrap();
        let decoded = WasmValue::from_json_bytes(&bytes).unwrap();
        assert_eq!(wasm_val, decoded);
    }
}
