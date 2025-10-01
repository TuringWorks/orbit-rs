use thiserror::Error;

/// Orbit-specific error types
#[derive(Debug, Error)]
pub enum OrbitError {
    #[error("Node not found: {node_id}")]
    NodeNotFound { node_id: String },

    #[error("Addressable not found: {reference}")]
    AddressableNotFound { reference: String },

    #[error("Lease expired: {details}")]
    LeaseExpired { details: String },

    #[error("Invocation failed: {method} on {addressable_type} - {reason}")]
    InvocationFailed {
        addressable_type: String,
        method: String,
        reason: String,
    },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Timeout: {operation}")]
    Timeout { operation: String },

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Cluster error: {0}")]
    ClusterError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl OrbitError {
    pub fn network<S: Into<String>>(msg: S) -> Self {
        OrbitError::NetworkError(msg.into())
    }

    pub fn timeout<S: Into<String>>(operation: S) -> Self {
        OrbitError::Timeout {
            operation: operation.into(),
        }
    }

    pub fn configuration<S: Into<String>>(msg: S) -> Self {
        OrbitError::ConfigurationError(msg.into())
    }

    pub fn cluster<S: Into<String>>(msg: S) -> Self {
        OrbitError::ClusterError(msg.into())
    }

    pub fn internal<S: Into<String>>(msg: S) -> Self {
        OrbitError::Internal(msg.into())
    }
}

/// Result type for Orbit operations
pub type OrbitResult<T> = Result<T, OrbitError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = OrbitError::network("Connection failed");
        assert!(matches!(error, OrbitError::NetworkError(_)));
        assert_eq!(error.to_string(), "Network error: Connection failed");
    }

    #[test]
    fn test_invocation_error() {
        let error = OrbitError::InvocationFailed {
            addressable_type: "TestActor".to_string(),
            method: "test_method".to_string(),
            reason: "Actor not found".to_string(),
        };
        assert!(error.to_string().contains("TestActor"));
        assert!(error.to_string().contains("test_method"));
    }
}
