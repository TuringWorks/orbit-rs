//! Error types for the Orbit storage engine

use thiserror::Error;

/// Result type for engine operations
pub type EngineResult<T> = Result<T, EngineError>;

/// Unified error type for all engine operations
#[derive(Error, Debug)]
pub enum EngineError {
    /// Storage layer errors
    #[error("Storage error: {0}")]
    Storage(String),

    /// Transaction errors
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Cluster/consensus errors
    #[error("Cluster error: {0}")]
    Cluster(String),

    /// Query execution errors
    #[error("Query error: {0}")]
    Query(String),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Addressable not found errors
    #[error("Addressable not found: {0}")]
    AddressableNotFound(String),

    /// Already exists errors
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    /// Conflict errors (e.g., write conflicts in transactions)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Timeout errors
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Invalid input errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Feature not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Internal errors (should not happen in normal operation)
    #[error("Internal error: {0}")]
    Internal(String),
}

impl EngineError {
    /// Create a storage error
    pub fn storage(msg: impl Into<String>) -> Self {
        Self::Storage(msg.into())
    }

    /// Create a transaction error
    pub fn transaction(msg: impl Into<String>) -> Self {
        Self::Transaction(msg.into())
    }

    /// Create a cluster error
    pub fn cluster(msg: impl Into<String>) -> Self {
        Self::Cluster(msg.into())
    }

    /// Create a query error
    pub fn query(msg: impl Into<String>) -> Self {
        Self::Query(msg.into())
    }

    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create an already exists error
    pub fn already_exists(msg: impl Into<String>) -> Self {
        Self::AlreadyExists(msg.into())
    }

    /// Create a conflict error
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    /// Create an invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a not implemented error
    pub fn not_implemented(msg: impl Into<String>) -> Self {
        Self::NotImplemented(msg.into())
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Create a timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }
}

// Convert from anyhow::Error
impl From<anyhow::Error> for EngineError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

// Convert from serde_json::Error
impl From<serde_json::Error> for EngineError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}
