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

impl From<String> for OrbitError {
    fn from(msg: String) -> Self {
        OrbitError::Internal(msg)
    }
}

impl From<&str> for OrbitError {
    fn from(msg: &str) -> Self {
        OrbitError::Internal(msg.to_string())
    }
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
    use std::error::Error;

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

    #[test]
    fn test_all_error_constructors() {
        let timeout_error = OrbitError::timeout("operation_timeout");
        assert!(matches!(timeout_error, OrbitError::Timeout { .. }));
        assert_eq!(timeout_error.to_string(), "Timeout: operation_timeout");

        let config_error = OrbitError::configuration("Invalid config");
        assert!(matches!(config_error, OrbitError::ConfigurationError(_)));
        assert_eq!(
            config_error.to_string(),
            "Configuration error: Invalid config"
        );

        let cluster_error = OrbitError::cluster("Cluster unreachable");
        assert!(matches!(cluster_error, OrbitError::ClusterError(_)));
        assert_eq!(
            cluster_error.to_string(),
            "Cluster error: Cluster unreachable"
        );

        let internal_error = OrbitError::internal("Internal fault");
        assert!(matches!(internal_error, OrbitError::Internal(_)));
        assert_eq!(internal_error.to_string(), "Internal error: Internal fault");
    }

    #[test]
    fn test_node_not_found_error() {
        let error = OrbitError::NodeNotFound {
            node_id: "node-123".to_string(),
        };
        assert_eq!(error.to_string(), "Node not found: node-123");
    }

    #[test]
    fn test_addressable_not_found_error() {
        let error = OrbitError::AddressableNotFound {
            reference: "TestActor:key-123".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Addressable not found: TestActor:key-123"
        );
    }

    #[test]
    fn test_lease_expired_error() {
        let error = OrbitError::LeaseExpired {
            details: "Lease for TestActor expired at 2023-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Lease expired: Lease for TestActor expired at 2023-01-01T00:00:00Z"
        );
    }

    #[test]
    fn test_serialization_error_conversion() {
        let json_error = serde_json::from_str::<serde_json::Value>("{invalid json").unwrap_err();
        let orbit_error = OrbitError::from(json_error);

        assert!(matches!(orbit_error, OrbitError::SerializationError(_)));
        assert!(orbit_error.to_string().contains("Serialization error:"));
    }

    #[test]
    fn test_error_trait_implementation() {
        let error = OrbitError::network("Test error");

        // Test that it implements std::error::Error
        let error_trait: &dyn Error = &error;
        assert!(error_trait.source().is_none());

        // Test Debug trait
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("NetworkError"));
        assert!(debug_str.contains("Test error"));
    }

    #[test]
    fn test_error_with_source() {
        let json_error = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let orbit_error = OrbitError::from(json_error);

        // Test that the source error is preserved
        assert!(orbit_error.source().is_some());
    }

    #[test]
    fn test_orbit_result_type() {
        // Test successful result
        let success: OrbitResult<String> = Ok("success".to_string());
        assert!(success.is_ok());
        if let Ok(value) = success {
            assert_eq!(value, "success");
        }

        // Test error result
        let error: OrbitResult<String> = Err(OrbitError::network("Failed"));
        assert!(error.is_err());
        match error {
            Err(OrbitError::NetworkError(msg)) => assert_eq!(msg, "Failed"),
            _ => unreachable!("Test setup guarantees NetworkError"),
        }
    }

    #[test]
    fn test_error_matching() {
        let errors = vec![
            OrbitError::NodeNotFound {
                node_id: "test".to_string(),
            },
            OrbitError::AddressableNotFound {
                reference: "test".to_string(),
            },
            OrbitError::LeaseExpired {
                details: "test".to_string(),
            },
            OrbitError::InvocationFailed {
                addressable_type: "Test".to_string(),
                method: "test".to_string(),
                reason: "test".to_string(),
            },
            OrbitError::NetworkError("test".to_string()),
            OrbitError::Timeout {
                operation: "test".to_string(),
            },
            OrbitError::ConfigurationError("test".to_string()),
            OrbitError::ClusterError("test".to_string()),
            OrbitError::Internal("test".to_string()),
        ];

        for error in errors {
            // Test that all errors can be matched and display correctly
            let error_string = error.to_string();
            assert!(!error_string.is_empty());

            match error {
                OrbitError::NodeNotFound { .. } => assert!(error_string.contains("Node not found")),
                OrbitError::AddressableNotFound { .. } => {
                    assert!(error_string.contains("Addressable not found"))
                }
                OrbitError::LeaseExpired { .. } => assert!(error_string.contains("Lease expired")),
                OrbitError::InvocationFailed { .. } => {
                    assert!(error_string.contains("Invocation failed"))
                }
                OrbitError::SerializationError(_) => {
                    assert!(error_string.contains("Serialization error"))
                }
                OrbitError::NetworkError(_) => assert!(error_string.contains("Network error")),
                OrbitError::Timeout { .. } => assert!(error_string.contains("Timeout")),
                OrbitError::ConfigurationError(_) => {
                    assert!(error_string.contains("Configuration error"))
                }
                OrbitError::ClusterError(_) => assert!(error_string.contains("Cluster error")),
                OrbitError::Internal(_) => assert!(error_string.contains("Internal error")),
            }
        }
    }

    #[test]
    fn test_error_builder_methods_with_different_types() {
        // Test with &str
        let error1 = OrbitError::network("string slice");
        assert_eq!(error1.to_string(), "Network error: string slice");

        // Test with String
        let error2 = OrbitError::network(String::from("owned string"));
        assert_eq!(error2.to_string(), "Network error: owned string");

        // Test with format! macro result
        let error3 = OrbitError::timeout(format!("operation_{}", 42));
        assert_eq!(error3.to_string(), "Timeout: operation_42");
    }

    #[test]
    fn test_error_size() {
        // Ensure error type isn't too large
        let size = std::mem::size_of::<OrbitError>();
        // This test ensures the error enum doesn't grow too large
        // Adjust the limit as needed based on the actual size requirements
        assert!(size <= 256, "OrbitError is too large: {} bytes", size);
    }
}
