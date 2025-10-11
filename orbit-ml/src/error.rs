//! Error types for the Orbit ML engine.

use thiserror::Error;

/// Result type alias for the ML engine
pub type Result<T> = std::result::Result<T, MLError>;

/// Main error type for the ML engine
#[derive(Error, Debug)]
pub enum MLError {
    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config {
        /// Error message describing the configuration issue
        message: String,
    },

    /// Model errors
    #[error("Model error: {message}")]
    Model {
        /// Error message describing the model issue
        message: String,
    },

    /// Training errors
    #[error("Training error: {message}")]
    Training {
        /// Error message describing the training issue
        message: String,
    },

    /// Inference errors
    #[error("Inference error: {message}")]
    Inference {
        /// Error message describing the inference issue
        message: String,
    },

    /// Data processing errors
    #[error("Data error: {message}")]
    Data {
        /// Error message describing the data processing issue
        message: String,
    },

    /// Neural network errors
    #[error("Neural network error: {message}")]
    NeuralNetwork {
        /// Error message describing the neural network issue
        message: String,
    },

    /// Transformer errors
    #[error("Transformer error: {message}")]
    Transformer {
        /// Error message describing the transformer issue
        message: String,
    },

    /// Graph neural network errors
    #[error("Graph neural network error: {message}")]
    GraphNeuralNetwork {
        /// Error message describing the graph neural network issue
        message: String,
    },

    /// Multi-language integration errors
    #[error("Multi-language error ({language}): {message}")]
    MultiLanguage {
        /// Programming language that caused the error
        language: String,
        /// Error message describing the multi-language issue
        message: String,
    },

    /// Python integration errors
    #[cfg(feature = "python")]
    #[error("Python error: {0}")]
    Python(#[from] pyo3::PyErr),

    /// JavaScript integration errors
    #[error("JavaScript error: {message}")]
    JavaScript {
        /// Error message from JavaScript runtime
        message: String,
    },

    /// Lua integration errors
    #[error("Lua error: {message}")]
    Lua {
        /// Error message from Lua runtime
        message: String,
    },

    /// GPU/CUDA errors
    #[cfg(feature = "gpu")]
    #[error("GPU error: {message}")]
    Gpu {
        /// Error message describing the GPU/CUDA issue
        message: String,
    },

    /// Distributed computing errors
    #[cfg(feature = "distributed")]
    #[error("Distributed computing error: {message}")]
    Distributed { message: String },

    /// SQL extension errors
    #[error("SQL extension error: {message}")]
    SqlExtension {
        /// Error message from SQL extension
        message: String,
    },

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid input errors
    #[error("Invalid input: {message}")]
    InvalidInput {
        /// Description of the invalid input
        message: String,
    },

    /// Resource not found errors
    #[error("Resource not found: {resource}")]
    NotFound {
        /// Name or identifier of the resource that was not found
        resource: String,
    },

    /// Permission/access errors
    #[error("Access denied: {message}")]
    AccessDenied {
        /// Description of the access denial reason
        message: String,
    },

    /// Resource allocation errors
    #[error("Resource allocation error: {message}")]
    ResourceAllocation {
        /// Description of the resource allocation failure
        message: String,
    },

    /// Timeout errors
    #[error("Operation timed out: {operation}")]
    Timeout {
        /// Name of the operation that timed out
        operation: String,
    },

    /// Internal errors
    #[error("Internal error: {message}")]
    Internal {
        /// Internal error message
        message: String,
    },

    /// External service errors
    #[error("External service error ({service}): {message}")]
    ExternalService {
        /// Name of the external service
        service: String,
        /// Error message from the external service
        message: String,
    },

    /// Validation errors
    #[error("Validation error: {message}")]
    Validation {
        /// Validation error description
        message: String,
    },

    /// Compatibility errors
    #[error("Compatibility error: {message}")]
    Compatibility {
        /// Compatibility issue description
        message: String,
    },

    /// Memory errors
    #[error("Memory error: {message}")]
    Memory {
        /// Memory error description
        message: String,
    },

    /// Thread safety errors
    #[error("Thread safety error: {message}")]
    ThreadSafety {
        /// Thread safety violation description
        message: String,
    },
}

impl MLError {
    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a model error
    pub fn model(message: impl Into<String>) -> Self {
        Self::Model {
            message: message.into(),
        }
    }

    /// Create a training error
    pub fn training(message: impl Into<String>) -> Self {
        Self::Training {
            message: message.into(),
        }
    }

    /// Create an inference error
    pub fn inference(message: impl Into<String>) -> Self {
        Self::Inference {
            message: message.into(),
        }
    }

    /// Create a data error
    pub fn data(message: impl Into<String>) -> Self {
        Self::Data {
            message: message.into(),
        }
    }

    /// Create a neural network error
    pub fn neural_network(message: impl Into<String>) -> Self {
        Self::NeuralNetwork {
            message: message.into(),
        }
    }

    /// Create a transformer error
    pub fn transformer(message: impl Into<String>) -> Self {
        Self::Transformer {
            message: message.into(),
        }
    }

    /// Create a graph neural network error
    pub fn graph_neural_network(message: impl Into<String>) -> Self {
        Self::GraphNeuralNetwork {
            message: message.into(),
        }
    }

    /// Create a multi-language error
    pub fn multi_language(language: impl Into<String>, message: impl Into<String>) -> Self {
        Self::MultiLanguage {
            language: language.into(),
            message: message.into(),
        }
    }

    /// Create a JavaScript error
    pub fn javascript(message: impl Into<String>) -> Self {
        Self::JavaScript {
            message: message.into(),
        }
    }

    /// Create a Lua error
    pub fn lua(message: impl Into<String>) -> Self {
        Self::Lua {
            message: message.into(),
        }
    }

    /// Create a GPU error
    #[cfg(feature = "gpu")]
    pub fn gpu(message: impl Into<String>) -> Self {
        Self::Gpu {
            message: message.into(),
        }
    }

    /// Create a distributed computing error
    #[cfg(feature = "distributed")]
    pub fn distributed(message: impl Into<String>) -> Self {
        Self::Distributed {
            message: message.into(),
        }
    }

    /// Create a SQL extension error
    pub fn sql_extension(message: impl Into<String>) -> Self {
        Self::SqlExtension {
            message: message.into(),
        }
    }

    /// Create an invalid input error
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    /// Create a not found error
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Create an access denied error
    pub fn access_denied(message: impl Into<String>) -> Self {
        Self::AccessDenied {
            message: message.into(),
        }
    }

    /// Create a resource allocation error
    pub fn resource_allocation(message: impl Into<String>) -> Self {
        Self::ResourceAllocation {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::Timeout {
            operation: operation.into(),
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create an external service error
    pub fn external_service(service: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ExternalService {
            service: service.into(),
            message: message.into(),
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a compatibility error
    pub fn compatibility(message: impl Into<String>) -> Self {
        Self::Compatibility {
            message: message.into(),
        }
    }

    /// Create a memory error
    pub fn memory(message: impl Into<String>) -> Self {
        Self::Memory {
            message: message.into(),
        }
    }

    /// Create a thread safety error
    pub fn thread_safety(message: impl Into<String>) -> Self {
        Self::ThreadSafety {
            message: message.into(),
        }
    }

    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            MLError::Timeout { .. }
                | MLError::ExternalService { .. }
                | MLError::ResourceAllocation { .. }
                | MLError::Memory { .. }
        )
    }

    /// Get error category
    pub fn category(&self) -> &'static str {
        match self {
            MLError::Config { .. } => "configuration",
            MLError::Model { .. } => "model",
            MLError::Training { .. } => "training",
            MLError::Inference { .. } => "inference",
            MLError::Data { .. } => "data",
            MLError::NeuralNetwork { .. } => "neural_network",
            MLError::Transformer { .. } => "transformer",
            MLError::GraphNeuralNetwork { .. } => "graph_neural_network",
            MLError::MultiLanguage { .. } => "multi_language",
            #[cfg(feature = "python")]
            MLError::Python(_) => "python",
            MLError::JavaScript { .. } => "javascript",
            MLError::Lua { .. } => "lua",
            #[cfg(feature = "gpu")]
            MLError::Gpu { .. } => "gpu",
            #[cfg(feature = "distributed")]
            MLError::Distributed { .. } => "distributed",
            MLError::SqlExtension { .. } => "sql_extension",
            MLError::Serialization(_) => "serialization",
            MLError::Io(_) => "io",
            MLError::InvalidInput { .. } => "invalid_input",
            MLError::NotFound { .. } => "not_found",
            MLError::AccessDenied { .. } => "access_denied",
            MLError::ResourceAllocation { .. } => "resource_allocation",
            MLError::Timeout { .. } => "timeout",
            MLError::Internal { .. } => "internal",
            MLError::ExternalService { .. } => "external_service",
            MLError::Validation { .. } => "validation",
            MLError::Compatibility { .. } => "compatibility",
            MLError::Memory { .. } => "memory",
            MLError::ThreadSafety { .. } => "thread_safety",
        }
    }
}

/// Convert from anyhow::Error
impl From<anyhow::Error> for MLError {
    fn from(err: anyhow::Error) -> Self {
        MLError::internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = MLError::config("test config error");
        assert_eq!(err.category(), "configuration");
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_error_display() {
        let err = MLError::model("test model error");
        assert_eq!(err.to_string(), "Model error: test model error");
    }

    #[test]
    fn test_recoverable_errors() {
        assert!(MLError::timeout("test").is_recoverable());
        assert!(MLError::external_service("test", "message").is_recoverable());
        assert!(!MLError::config("test").is_recoverable());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(MLError::training("test").category(), "training");
        assert_eq!(MLError::inference("test").category(), "inference");
        assert_eq!(MLError::neural_network("test").category(), "neural_network");
    }
}
