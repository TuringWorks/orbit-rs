//! Error types for protocol adapters

use thiserror::Error;

/// Result type for protocol operations
pub type ProtocolResult<T> = Result<T, ProtocolError>;

/// Errors that can occur in protocol adapters
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// RESP protocol parsing error
    #[error("RESP protocol error: {0}")]
    RespError(String),

    /// PostgreSQL wire protocol error
    #[error("PostgreSQL protocol error: {0}")]
    PostgresError(String),

    /// Cypher query parsing error
    #[error("Cypher query error: {0}")]
    CypherError(String),

    /// SQL parsing error
    #[error("SQL parse error: {0}")]
    ParseError(String),

    /// REST API error
    #[error("REST API error: {0}")]
    RestError(String),

    /// Actor operation error
    #[error("Actor operation error: {0}")]
    ActorError(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Authentication error
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Authorization error
    #[error("Authorization failed: {0}")]
    AuthorizationError(String),

    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<orbit_shared::exception::OrbitError> for ProtocolError {
    fn from(err: orbit_shared::exception::OrbitError) -> Self {
        ProtocolError::ActorError(err.to_string())
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> Self {
        ProtocolError::SerializationError(err.to_string())
    }
}
