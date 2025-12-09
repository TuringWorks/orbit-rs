//! JavaScript Type System
//!
//! Provides type conversions between Rust, SQL, and JavaScript values.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Result type for JavaScript operations
pub type JsResult<T> = Result<T, JsError>;

/// JavaScript execution errors
#[derive(Debug, Error, Clone)]
pub enum JsError {
    /// Script compilation error
    #[error("Compilation error: {0}")]
    CompilationError(String),

    /// Runtime execution error
    #[error("Runtime error: {0}")]
    RuntimeError(String),

    /// Type conversion error
    #[error("Type conversion error: {0}")]
    TypeError(String),

    /// Execution timeout
    #[error("Execution timeout after {0}ms")]
    Timeout(u64),

    /// Memory limit exceeded
    #[error("Memory limit exceeded: {used} bytes > {limit} bytes")]
    MemoryLimitExceeded { used: usize, limit: usize },

    /// Script too large
    #[error("Script too large: {size} bytes > {max} bytes")]
    ScriptTooLarge { size: usize, max: usize },

    /// Security violation
    #[error("Security violation: {0}")]
    SecurityViolation(String),

    /// Engine not available
    #[error("JavaScript engine not available: {0}")]
    EngineNotAvailable(String),

    /// Internal engine error
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// JavaScript value that can cross the Rust/JS boundary
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsValue {
    /// Null value
    Null,
    /// Undefined value
    Undefined,
    /// Boolean value
    Bool(bool),
    /// Integer value (fits in i64)
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<JsValue>),
    /// Object (key-value pairs)
    Object(HashMap<String, JsValue>),
    /// Binary data (Uint8Array in JS)
    Binary(Vec<u8>),
    /// Date value (ISO 8601 string internally)
    Date(String),
    /// BigInt value (as string)
    BigInt(String),
}

impl JsValue {
    /// Create a null value
    pub fn null() -> Self {
        JsValue::Null
    }

    /// Create an undefined value
    pub fn undefined() -> Self {
        JsValue::Undefined
    }

    /// Check if value is null or undefined
    pub fn is_nullish(&self) -> bool {
        matches!(self, JsValue::Null | JsValue::Undefined)
    }

    /// Check if value is truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            JsValue::Null | JsValue::Undefined => false,
            JsValue::Bool(b) => *b,
            JsValue::Integer(n) => *n != 0,
            JsValue::Float(f) => *f != 0.0 && !f.is_nan(),
            JsValue::String(s) => !s.is_empty(),
            JsValue::Array(_) | JsValue::Object(_) | JsValue::Binary(_) => true,
            JsValue::Date(_) | JsValue::BigInt(_) => true,
        }
    }

    /// Try to convert to boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to convert to i64
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            JsValue::Integer(n) => Some(*n),
            JsValue::Float(f) if f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }

    /// Try to convert to f64
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsValue::Integer(n) => Some(*n as f64),
            JsValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Try to convert to string
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to convert to array
    pub fn as_array(&self) -> Option<&Vec<JsValue>> {
        match self {
            JsValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Try to convert to object
    pub fn as_object(&self) -> Option<&HashMap<String, JsValue>> {
        match self {
            JsValue::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, JsError> {
        serde_json::to_string(self)
            .map_err(|e| JsError::TypeError(format!("Failed to serialize to JSON: {}", e)))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, JsError> {
        serde_json::from_str(json)
            .map_err(|e| JsError::TypeError(format!("Failed to parse JSON: {}", e)))
    }
}

impl Default for JsValue {
    fn default() -> Self {
        JsValue::Undefined
    }
}

impl From<bool> for JsValue {
    fn from(b: bool) -> Self {
        JsValue::Bool(b)
    }
}

impl From<i32> for JsValue {
    fn from(n: i32) -> Self {
        JsValue::Integer(n as i64)
    }
}

impl From<i64> for JsValue {
    fn from(n: i64) -> Self {
        JsValue::Integer(n)
    }
}

impl From<f64> for JsValue {
    fn from(f: f64) -> Self {
        JsValue::Float(f)
    }
}

impl From<&str> for JsValue {
    fn from(s: &str) -> Self {
        JsValue::String(s.to_string())
    }
}

impl From<String> for JsValue {
    fn from(s: String) -> Self {
        JsValue::String(s)
    }
}

impl From<Vec<JsValue>> for JsValue {
    fn from(arr: Vec<JsValue>) -> Self {
        JsValue::Array(arr)
    }
}

impl From<HashMap<String, JsValue>> for JsValue {
    fn from(obj: HashMap<String, JsValue>) -> Self {
        JsValue::Object(obj)
    }
}

impl From<serde_json::Value> for JsValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => JsValue::Null,
            serde_json::Value::Bool(b) => JsValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    JsValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    JsValue::Float(f)
                } else {
                    JsValue::Null
                }
            }
            serde_json::Value::String(s) => JsValue::String(s),
            serde_json::Value::Array(arr) => {
                JsValue::Array(arr.into_iter().map(JsValue::from).collect())
            }
            serde_json::Value::Object(obj) => JsValue::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, JsValue::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<JsValue> for serde_json::Value {
    fn from(v: JsValue) -> Self {
        match v {
            JsValue::Null | JsValue::Undefined => serde_json::Value::Null,
            JsValue::Bool(b) => serde_json::Value::Bool(b),
            JsValue::Integer(n) => serde_json::Value::Number(n.into()),
            JsValue::Float(f) => serde_json::Number::from_f64(f)
                .map_or(serde_json::Value::Null, |n| serde_json::Value::Number(n)),
            JsValue::String(s) => serde_json::Value::String(s),
            JsValue::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(serde_json::Value::from).collect())
            }
            JsValue::Object(obj) => serde_json::Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
            JsValue::Binary(data) => serde_json::Value::String(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &data,
            )),
            JsValue::Date(s) | JsValue::BigInt(s) => serde_json::Value::String(s),
        }
    }
}

/// Function parameter definition for stored procedures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type hint
    pub type_hint: Option<String>,
    /// Default value
    pub default: Option<JsValue>,
}

/// Stored function/procedure definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsFunction {
    /// Function name
    pub name: String,
    /// Function body (JavaScript code)
    pub body: String,
    /// Function parameters
    pub parameters: Vec<JsParameter>,
    /// Return type hint
    pub return_type: Option<String>,
    /// Is this a volatile function (can have side effects)?
    pub is_volatile: bool,
    /// Is this function deterministic (same inputs always produce same outputs)?
    pub is_deterministic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_value_conversions() {
        // Boolean
        assert_eq!(JsValue::from(true), JsValue::Bool(true));

        // Numbers
        assert_eq!(JsValue::from(42i32), JsValue::Integer(42));
        assert_eq!(JsValue::from(3.14f64), JsValue::Float(3.14));

        // Strings
        assert_eq!(JsValue::from("hello"), JsValue::String("hello".to_string()));
    }

    #[test]
    fn test_js_value_truthy() {
        assert!(!JsValue::Null.is_truthy());
        assert!(!JsValue::Undefined.is_truthy());
        assert!(!JsValue::Bool(false).is_truthy());
        assert!(JsValue::Bool(true).is_truthy());
        assert!(!JsValue::Integer(0).is_truthy());
        assert!(JsValue::Integer(1).is_truthy());
        assert!(!JsValue::String("".to_string()).is_truthy());
        assert!(JsValue::String("hello".to_string()).is_truthy());
    }

    #[test]
    fn test_json_roundtrip() {
        let value = JsValue::Object(
            [
                ("name".to_string(), JsValue::String("test".to_string())),
                ("count".to_string(), JsValue::Integer(42)),
                ("active".to_string(), JsValue::Bool(true)),
            ]
            .into_iter()
            .collect(),
        );

        let json = value.to_json().unwrap();
        let parsed = JsValue::from_json(&json).unwrap();
        assert_eq!(value, parsed);
    }
}
