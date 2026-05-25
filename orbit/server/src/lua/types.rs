//! Lua type system and conversions
//!
//! This module provides the core type system for Lua integration, including:
//! - `LuaValue`: Enum representing all Lua value types
//! - `LuaError`: Error types for Lua execution
//! - `LuaFunction`: Metadata for registered Lua functions
//! - Conversions between Lua, Rust, and protocol types (RESP, SQL, etc.)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

#[cfg(feature = "lua-mlua")]
use mlua;

use crate::protocols::resp::types::RespValue;
use bytes::Bytes;

/// Result type for Lua operations
pub type LuaResult<T> = Result<T, LuaError>;

/// Errors that can occur during Lua execution
#[derive(Error, Debug, Clone)]
pub enum LuaError {
    #[error("Compilation error: {0}")]
    CompilationError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Execution timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Memory limit exceeded: {used_bytes} bytes used, limit: {limit_bytes} bytes")]
    MemoryLimitExceeded {
        used_bytes: usize,
        limit_bytes: usize,
    },

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("Script too large: {size} bytes, max: {max_size} bytes")]
    ScriptTooLarge { size: usize, max_size: usize },

    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

#[cfg(feature = "lua-mlua")]
impl From<mlua::Error> for LuaError {
    fn from(err: mlua::Error) -> Self {
        match err {
            mlua::Error::SyntaxError { message, .. } => LuaError::CompilationError(message),
            mlua::Error::RuntimeError(msg) => LuaError::RuntimeError(msg),
            mlua::Error::CallbackError { cause, .. } => LuaError::RuntimeError(cause.to_string()),
            mlua::Error::MemoryError(_msg) => LuaError::MemoryLimitExceeded {
                used_bytes: 0,
                limit_bytes: 0,
            },
            mlua::Error::MemoryControlNotAvailable => {
                LuaError::SecurityViolation("Memory limit enforcement not available".to_string())
            }
            _ => LuaError::InternalError(err.to_string()),
        }
    }
}

impl From<std::io::Error> for LuaError {
    fn from(err: std::io::Error) -> Self {
        LuaError::IoError(err.to_string())
    }
}

/// Represents a Lua value with support for all Lua types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum LuaValue {
    /// Lua nil value
    Nil,
    /// Boolean value
    Boolean(bool),
    /// Integer number (i64)
    Integer(i64),
    /// Floating-point number (f64)
    Number(f64),
    /// String value
    String(String),
    /// Table represented as a map (key-value pairs)
    Table(HashMap<String, LuaValue>),
    /// Table represented as an array (indexed)
    Array(Vec<LuaValue>),
    /// Binary data
    Binary(Vec<u8>),
    /// Function reference (not directly serializable, stored as name)
    Function(String),
}

impl LuaValue {
    /// Check if value is nil
    pub fn is_nil(&self) -> bool {
        matches!(self, LuaValue::Nil)
    }

    /// Try to extract as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            LuaValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to extract as integer
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            LuaValue::Integer(i) => Some(*i),
            LuaValue::Number(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// Try to extract as float
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            LuaValue::Number(f) => Some(*f),
            LuaValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to extract as string
    pub fn as_str(&self) -> Option<&str> {
        match self {
            LuaValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to extract as table
    pub fn as_table(&self) -> Option<&HashMap<String, LuaValue>> {
        match self {
            LuaValue::Table(t) => Some(t),
            _ => None,
        }
    }

    /// Try to extract as array
    pub fn as_array(&self) -> Option<&Vec<LuaValue>> {
        match self {
            LuaValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Convert to RESP value (for Redis protocol)
    pub fn to_resp(&self) -> RespValue {
        match self {
            LuaValue::Nil => RespValue::Null,
            LuaValue::Boolean(true) => RespValue::Integer(1),
            LuaValue::Boolean(false) => RespValue::Null,
            LuaValue::Integer(i) => RespValue::Integer(*i),
            LuaValue::Number(f) => RespValue::BulkString(Bytes::from(f.to_string().into_bytes())),
            LuaValue::String(s) => RespValue::BulkString(Bytes::from(s.as_bytes().to_vec())),
            LuaValue::Binary(b) => RespValue::BulkString(Bytes::from(b.clone())),
            LuaValue::Array(arr) => RespValue::Array(arr.iter().map(|v| v.to_resp()).collect()),
            LuaValue::Table(table) => {
                // Convert table to array of key-value pairs
                let mut pairs = Vec::new();
                for (k, v) in table.iter() {
                    pairs.push(RespValue::BulkString(Bytes::from(k.as_bytes().to_vec())));
                    pairs.push(v.to_resp());
                }
                RespValue::Array(pairs)
            }
            LuaValue::Function(name) => {
                RespValue::BulkString(Bytes::from(format!("function:{}", name).into_bytes()))
            }
        }
    }

    /// Create from RESP value
    pub fn from_resp(resp: &RespValue) -> Self {
        match resp {
            RespValue::Null | RespValue::NullBulkString | RespValue::NullArray => LuaValue::Nil,
            RespValue::SimpleString(s) | RespValue::Error(s) => LuaValue::String(s.clone()),
            RespValue::Integer(i) => LuaValue::Integer(*i),
            RespValue::Boolean(b) => LuaValue::Boolean(*b),
            RespValue::Double(f) => LuaValue::Number(*f),
            RespValue::BulkString(b) => {
                // Try to convert to UTF-8 string, otherwise keep as binary
                match String::from_utf8(b.to_vec()) {
                    Ok(s) => LuaValue::String(s),
                    Err(_) => LuaValue::Binary(b.to_vec()),
                }
            }
            RespValue::Array(arr) => LuaValue::Array(arr.iter().map(LuaValue::from_resp).collect()),
            // Handle other variants by converting to string or nil
            RespValue::BigNumber(n) => LuaValue::String(n.clone()),
            RespValue::VerbatimString { data, .. } => match String::from_utf8(data.to_vec()) {
                Ok(s) => LuaValue::String(s),
                Err(_) => LuaValue::Binary(data.to_vec()),
            },
            RespValue::BulkError(b) => LuaValue::Binary(b.to_vec()),
            RespValue::Attribute(_attr) => LuaValue::Nil, // Ignore attributes for now
            RespValue::Map(map) => {
                let mut table = HashMap::new();
                for (k, v) in map.iter() {
                    // Use string representation of key
                    let key_str = match k {
                        RespValue::SimpleString(s) | RespValue::Error(s) => s.clone(),
                        RespValue::BulkString(b) => String::from_utf8_lossy(b).to_string(),
                        _ => format!("{:?}", k),
                    };
                    table.insert(key_str, LuaValue::from_resp(v));
                }
                LuaValue::Table(table)
            }
            RespValue::Set(set) => LuaValue::Array(set.iter().map(LuaValue::from_resp).collect()),
            RespValue::Push(values) => {
                LuaValue::Array(values.iter().map(LuaValue::from_resp).collect())
            }
        }
    }
}

impl fmt::Display for LuaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LuaValue::Nil => write!(f, "nil"),
            LuaValue::Boolean(b) => write!(f, "{}", b),
            LuaValue::Integer(i) => write!(f, "{}", i),
            LuaValue::Number(n) => write!(f, "{}", n),
            LuaValue::String(s) => write!(f, "\"{}\"", s),
            LuaValue::Binary(b) => write!(f, "<binary {} bytes>", b.len()),
            LuaValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            LuaValue::Table(table) => {
                write!(f, "{{")?;
                for (i, (k, v)) in table.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            LuaValue::Function(name) => write!(f, "function:{}", name),
        }
    }
}

/// Metadata for a registered Lua function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaFunction {
    /// Function name
    pub name: String,
    /// Function body (Lua source code)
    pub body: String,
    /// Function parameters with type hints
    pub parameters: Vec<LuaParameter>,
    /// Return type hint (optional)
    pub return_type: Option<String>,
    /// Whether the function has side effects (for Redis)
    pub is_volatile: bool,
    /// Whether the function is deterministic
    pub is_deterministic: bool,
    /// Function description (for documentation)
    pub description: Option<String>,
    /// Function flags (for Redis compatibility)
    pub flags: Vec<String>,
}

impl LuaFunction {
    /// Create a new Lua function metadata
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            body: body.into(),
            parameters: Vec::new(),
            return_type: None,
            is_volatile: true,
            is_deterministic: false,
            description: None,
            flags: Vec::new(),
        }
    }

    /// Add a parameter
    pub fn with_parameter(mut self, name: impl Into<String>, type_hint: Option<String>) -> Self {
        self.parameters.push(LuaParameter {
            name: name.into(),
            type_hint,
            default: None,
        });
        self
    }

    /// Set return type
    pub fn with_return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    /// Set volatility
    pub fn with_volatility(mut self, is_volatile: bool) -> Self {
        self.is_volatile = is_volatile;
        self
    }

    /// Set determinism
    pub fn with_determinism(mut self, is_deterministic: bool) -> Self {
        self.is_deterministic = is_deterministic;
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add flags (for Redis compatibility)
    pub fn with_flags(mut self, flags: Vec<String>) -> Self {
        self.flags = flags;
        self
    }
}

/// Function parameter metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaParameter {
    /// Parameter name
    pub name: String,
    /// Type hint (optional)
    pub type_hint: Option<String>,
    /// Default value (optional)
    pub default: Option<LuaValue>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant)] // Test values like 3.14 are intentional test floats

    use super::*;

    #[test]
    fn test_lua_value_creation() {
        assert!(LuaValue::Nil.is_nil());
        assert_eq!(LuaValue::Boolean(true).as_bool(), Some(true));
        assert_eq!(LuaValue::Integer(42).as_i64(), Some(42));
        assert_eq!(LuaValue::Number(3.14).as_f64(), Some(3.14));
        assert_eq!(
            LuaValue::String("hello".to_string()).as_str(),
            Some("hello")
        );
    }

    #[test]
    fn test_resp_conversion() {
        // Nil
        let nil = LuaValue::Nil;
        assert_eq!(nil.to_resp(), RespValue::Null);

        // Integer
        let int = LuaValue::Integer(42);
        assert_eq!(int.to_resp(), RespValue::Integer(42));

        // String
        let string = LuaValue::String("hello".to_string());
        assert_eq!(
            string.to_resp(),
            RespValue::BulkString(b"hello".to_vec().into())
        );

        // Array
        let array = LuaValue::Array(vec![LuaValue::Integer(1), LuaValue::Integer(2)]);
        assert_eq!(
            array.to_resp(),
            RespValue::Array(vec![RespValue::Integer(1), RespValue::Integer(2)])
        );
    }

    #[test]
    fn test_from_resp() {
        let resp_int = RespValue::Integer(42);
        assert_eq!(LuaValue::from_resp(&resp_int), LuaValue::Integer(42));

        let resp_str = RespValue::BulkString(b"hello".to_vec().into());
        assert_eq!(
            LuaValue::from_resp(&resp_str),
            LuaValue::String("hello".to_string())
        );

        let resp_array = RespValue::Array(vec![RespValue::Integer(1), RespValue::Integer(2)]);
        assert_eq!(
            LuaValue::from_resp(&resp_array),
            LuaValue::Array(vec![LuaValue::Integer(1), LuaValue::Integer(2)])
        );
    }

    #[test]
    fn test_lua_function_builder() {
        let func = LuaFunction::new("add", "return a + b")
            .with_parameter("a", Some("number".to_string()))
            .with_parameter("b", Some("number".to_string()))
            .with_return_type("number")
            .with_determinism(true)
            .with_description("Adds two numbers");

        assert_eq!(func.name, "add");
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.return_type, Some("number".to_string()));
        assert!(func.is_deterministic);
        assert_eq!(func.description, Some("Adds two numbers".to_string()));
    }
}
