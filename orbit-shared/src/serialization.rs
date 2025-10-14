//! Serialization utilities for Orbit shared data structures
//!
//! This module provides common serialization and deserialization functionality
//! for data structures shared across the Orbit ecosystem.

use serde::{Deserialize, Serialize};

/// Common serialization error type
#[derive(Debug, Clone)]
pub enum SerializationError {
    /// JSON serialization/deserialization error
    Json(String),
    /// Binary serialization/deserialization error
    Binary(String),
    /// Schema validation error
    Schema(String),
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializationError::Json(msg) => write!(f, "JSON serialization error: {}", msg),
            SerializationError::Binary(msg) => write!(f, "Binary serialization error: {}", msg),
            SerializationError::Schema(msg) => write!(f, "Schema validation error: {}", msg),
        }
    }
}

impl std::error::Error for SerializationError {}

/// Result type for serialization operations
pub type Result<T> = std::result::Result<T, SerializationError>;

/// Trait for serializable data structures
pub trait SerializableData {
    /// Serialize to JSON bytes
    fn to_json_bytes(&self) -> Result<Vec<u8>>;

    /// Deserialize from JSON bytes
    fn from_json_bytes(data: &[u8]) -> Result<Self>
    where
        Self: Sized;

    /// Serialize to binary format
    fn to_binary(&self) -> Result<Vec<u8>>;

    /// Deserialize from binary format
    fn from_binary(data: &[u8]) -> Result<Self>
    where
        Self: Sized;
}

/// Default implementation for types that implement Serialize + DeserializeOwned
impl<T> SerializableData for T
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    fn to_json_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| SerializationError::Json(e.to_string()))
    }

    fn from_json_bytes(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(|e| SerializationError::Json(e.to_string()))
    }

    fn to_binary(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| SerializationError::Binary(e.to_string()))
    }

    fn from_binary(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| SerializationError::Binary(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
    }

    #[test]
    fn test_json_serialization() {
        let data = TestData {
            id: 42,
            name: "test".to_string(),
        };

        let bytes = data.to_json_bytes().unwrap();
        let deserialized = TestData::from_json_bytes(&bytes).unwrap();

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_binary_serialization() {
        let data = TestData {
            id: 42,
            name: "test".to_string(),
        };

        let bytes = data.to_binary().unwrap();
        let deserialized = TestData::from_binary(&bytes).unwrap();

        assert_eq!(data, deserialized);
    }
}
