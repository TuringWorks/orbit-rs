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

    /// AQL query parsing error
    #[error("AQL query error: {0}")]
    AqlError(String),

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
    IoError(String),

    /// CQL protocol error
    #[error("CQL protocol error: {0}")]
    CqlError(String),

    /// Type conversion error
    #[error("Type conversion error: {0}")]
    ConversionError(String),

    /// Unsupported type
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    /// Invalid opcode
    #[error("Invalid opcode: {0}")]
    InvalidOpcode(u8),

    /// Invalid consistency level
    #[error("Invalid consistency level: {0}")]
    InvalidConsistencyLevel(u16),

    /// Incomplete frame
    #[error("Incomplete frame")]
    IncompleteFrame,

    /// Invalid UTF-8
    #[error("Invalid UTF-8: {0}")]
    InvalidUtf8(String),

    /// Invalid statement
    #[error("Invalid statement: {0}")]
    InvalidStatement(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Invalid state
    #[error("Invalid state")]
    InvalidState,

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for ProtocolError {
    fn from(err: std::io::Error) -> Self {
        ProtocolError::IoError(err.to_string())
    }
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

// Helper functions to reduce error formatting duplication
impl ProtocolError {
    /// Create a "Table does not exist" error
    pub fn table_not_found(table_name: &str) -> Self {
        ProtocolError::PostgresError(format!("Table '{table_name}' does not exist"))
    }

    /// Create an "Actor error" from any error
    pub fn actor_error<E: std::fmt::Display>(err: E) -> Self {
        ProtocolError::PostgresError(format!("Actor error: {err}"))
    }

    /// Create a "Transaction not found" error
    pub fn transaction_not_found<T: std::fmt::Display>(tx_id: T) -> Self {
        ProtocolError::PostgresError(format!("Transaction {tx_id} not found"))
    }

    /// Create a "Failed to get actor" error
    pub fn failed_to_get_actor<E: std::fmt::Display>(err: E) -> Self {
        ProtocolError::PostgresError(format!("Failed to get actor: {err}"))
    }

    /// Create a serialization error
    pub fn serialization_error<E: std::fmt::Display>(err: E) -> Self {
        ProtocolError::PostgresError(format!("Serialization error: {err}"))
    }

    /// Create a "Portal not found" error
    pub fn portal_not_found(portal: &str) -> Self {
        ProtocolError::PostgresError(format!("Portal not found: {portal}"))
    }

    /// Create a "Statement not found" error
    pub fn statement_not_found(statement: &str) -> Self {
        ProtocolError::PostgresError(format!("Statement not found: {statement}"))
    }

    /// Helper for "already exists" errors
    pub fn already_exists(resource_type: &str, name: &str) -> Self {
        ProtocolError::PostgresError(format!("{resource_type} '{name}' already exists"))
    }

    /// Helper for "does not exist" errors  
    pub fn does_not_exist(resource_type: &str, name: &str) -> Self {
        ProtocolError::PostgresError(format!("{resource_type} '{name}' does not exist"))
    }

    /// Helper for relation does not exist errors (common in PostgreSQL)
    pub fn relation_not_found(relation_name: &str) -> Self {
        ProtocolError::PostgresError(format!("Relation '{relation_name}' does not exist"))
    }

    /// Helper for column not found errors
    pub fn column_not_found(column_name: &str, table_name: &str) -> Self {
        ProtocolError::PostgresError(format!(
            "Column '{column_name}' does not exist in table '{table_name}'"
        ))
    }

    /// Helper for "not implemented" errors
    pub fn not_implemented(feature_type: &str, feature_name: &str) -> Self {
        ProtocolError::PostgresError(format!("{feature_type} '{feature_name}' not implemented"))
    }
}
