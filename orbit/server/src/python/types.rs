//! Python type system and conversions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Python execution error
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum PythonError {
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    
    #[error("Timeout error: execution exceeded {0}s")]
    TimeoutError(u64),
    
    #[error("Type conversion error: {0}")]
    TypeConversionError(String),
    
    #[error("Function not found: {0}")]
    FunctionNotFound(String),
    
    #[error("Worker error: {0}")]
    WorkerError(String),
    
    #[error("Communication error: {0}")]
    CommunicationError(String),
    
    #[error("Security violation: {0}")]
    SecurityViolation(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type PythonResult<T> = Result<T, PythonError>;

/// Python value representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PythonValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<PythonValue>),
    Dict(HashMap<String, PythonValue>),
}

impl PythonValue {
    /// Convert to SQL-compatible value
    pub fn to_sql_value(&self) -> crate::protocols::postgres_wire::sql::types::SqlValue {
        use crate::protocols::postgres_wire::sql::types::SqlValue;
        
        match self {
            PythonValue::Null => SqlValue::Null,
            PythonValue::Bool(b) => SqlValue::Boolean(*b),
            PythonValue::Int(i) => {
                if *i >= i32::MIN as i64 && *i <= i32::MAX as i64 {
                    SqlValue::Integer(*i as i32)
                } else {
                    SqlValue::BigInt(*i)
                }
            }
            PythonValue::Float(f) => SqlValue::DoublePrecision(*f),
            PythonValue::String(s) => SqlValue::Text(s.clone()),
            PythonValue::Bytes(b) => SqlValue::Bytea(b.clone()),
            PythonValue::List(items) => {
                SqlValue::Array(items.iter().map(|v| v.to_sql_value()).collect())
            }
            PythonValue::Dict(map) => {
                // Convert dict to JSON Value
                let json = serde_json::to_value(map).unwrap_or(serde_json::json!({}));
                SqlValue::Json(json)
            }
        }
    }
    
    /// Create from SQL value
    pub fn from_sql_value(sql: &crate::protocols::postgres_wire::sql::types::SqlValue) -> Self {
        use crate::protocols::postgres_wire::sql::types::SqlValue;
        
        match sql {
            SqlValue::Null => PythonValue::Null,
            SqlValue::Boolean(b) => PythonValue::Bool(*b),
            SqlValue::SmallInt(i) => PythonValue::Int(*i as i64),
            SqlValue::Integer(i) => PythonValue::Int(*i as i64),
            SqlValue::BigInt(i) => PythonValue::Int(*i),
            SqlValue::Real(f) => PythonValue::Float(*f as f64),
            SqlValue::DoublePrecision(f) => PythonValue::Float(*f),
            SqlValue::Decimal(d) => {
                // Try to parse as float via string representation
                PythonValue::Float(d.to_string().parse::<f64>().unwrap_or(0.0))
            }
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                PythonValue::String(s.clone())
            }
            SqlValue::Bytea(b) => PythonValue::Bytes(b.clone()),
            SqlValue::Array(items) => {
                PythonValue::List(items.iter().map(Self::from_sql_value).collect())
            }
            SqlValue::Json(j) | SqlValue::Jsonb(j) => {
                // Convert JSON Value to Python value
                if let serde_json::Value::Object(map) = j {
                    let python_map: HashMap<String, PythonValue> = map
                        .iter()
                        .map(|(k, v)| (k.clone(), Self::from_json_value(v)))
                        .collect();
                    PythonValue::Dict(python_map)
                } else {
                    Self::from_json_value(j)
                }
            }
            SqlValue::Timestamp(ts) => {
                // Return as ISO string
                PythonValue::String(ts.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
            }
            SqlValue::Date(d) => {
                PythonValue::String(d.format("%Y-%m-%d").to_string())
            }
            SqlValue::Time(t) => {
                PythonValue::String(t.format("%H:%M:%S%.f").to_string())
            }
            SqlValue::Uuid(u) => PythonValue::String(u.to_string()),
            _ => PythonValue::String(format!("{:?}", sql)),
        }
    }
    
    fn from_json_value(json: &serde_json::Value) -> Self {
        match json {
            serde_json::Value::Null => PythonValue::Null,
            serde_json::Value::Bool(b) => PythonValue::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    PythonValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    PythonValue::Float(f)
                } else {
                    PythonValue::Null
                }
            }
            serde_json::Value::String(s) => PythonValue::String(s.clone()),
            serde_json::Value::Array(arr) => {
                PythonValue::List(arr.iter().map(Self::from_json_value).collect())
            }
            serde_json::Value::Object(obj) => {
                let map: HashMap<String, PythonValue> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::from_json_value(v)))
                    .collect();
                PythonValue::Dict(map)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_python_value_types() {
        assert_eq!(
            PythonValue::Null,
            PythonValue::Null
        );
        
        assert_eq!(
            PythonValue::Int(42),
            PythonValue::Int(42)
        );
        
        assert_eq!(
            PythonValue::String("test".to_string()),
            PythonValue::String("test".to_string())
        );
    }
    
    #[test]
    fn test_list_and_dict() {
        let list = PythonValue::List(vec![
            PythonValue::Int(1),
            PythonValue::Int(2),
            PythonValue::Int(3),
        ]);
        
        if let PythonValue::List(items) = list {
            assert_eq!(items.len(), 3);
        } else {
            panic!("Expected list");
        }
        
        let mut map = HashMap::new();
        map.insert("key".to_string(), PythonValue::String("value".to_string()));
        let dict = PythonValue::Dict(map);
        
        if let PythonValue::Dict(m) = dict {
            assert_eq!(m.get("key"), Some(&PythonValue::String("value".to_string())));
        } else {
            panic!("Expected dict");
        }
    }
}
