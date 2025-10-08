use thiserror::Error;

/// Errors that can occur in etcd operations
#[derive(Error, Debug)]
pub enum EtcdError {
    /// Connection error
    #[error("Connection error: {message}")]
    Connection { message: String },

    /// Service discovery error
    #[error("Service discovery error: {message}")]
    Discovery { message: String },

    /// Service registration error
    #[error("Service registration error: {service} - {message}")]
    Registration { service: String, message: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Leader election error
    #[error("Leader election error: {message}")]
    Election { message: String },

    /// Coordination error
    #[error("Coordination error: {message}")]
    Coordination { message: String },

    /// Health check error
    #[error("Health check error: {service} - {message}")]
    HealthCheck { service: String, message: String },

    /// Watch error
    #[error("Watch error: {key} - {message}")]
    Watch { key: String, message: String },

    /// Key not found error
    #[error("Key not found: {key}")]
    KeyNotFound { key: String },

    /// Lease error
    #[error("Lease error: {message}")]
    Lease { message: String },

    /// Lock error
    #[error("Lock error: {lock_name} - {message}")]
    Lock { lock_name: String, message: String },

    /// Serialization error
    #[error("Serialization error: {message}")]
    Serialization { message: String },

    /// Timeout error
    #[error("Operation timeout: {operation}")]
    Timeout { operation: String },

    /// Network error
    #[error("Network error: {message}")]
    Network { message: String },

    /// Authentication error
    #[error("Authentication error: {message}")]
    Authentication { message: String },

    /// Permission error
    #[error("Permission error: {message}")]
    Permission { message: String },

    /// etcd client error
    #[error("etcd client error: {0}")]
    EtcdClient(#[from] etcd_client::Error),

    /// Serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),
}

impl EtcdError {
    /// Create a new connection error
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }

    /// Create a new discovery error
    pub fn discovery(message: impl Into<String>) -> Self {
        Self::Discovery {
            message: message.into(),
        }
    }

    /// Create a new registration error
    pub fn registration(service: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Registration {
            service: service.into(),
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a new election error
    pub fn election(message: impl Into<String>) -> Self {
        Self::Election {
            message: message.into(),
        }
    }

    /// Create a new coordination error
    pub fn coordination(message: impl Into<String>) -> Self {
        Self::Coordination {
            message: message.into(),
        }
    }

    /// Create a new health check error
    pub fn health_check(service: impl Into<String>, message: impl Into<String>) -> Self {
        Self::HealthCheck {
            service: service.into(),
            message: message.into(),
        }
    }

    /// Create a new watch error
    pub fn watch(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Watch {
            key: key.into(),
            message: message.into(),
        }
    }

    /// Create a new key not found error
    pub fn key_not_found(key: impl Into<String>) -> Self {
        Self::KeyNotFound { key: key.into() }
    }

    /// Create a new lease error
    pub fn lease(message: impl Into<String>) -> Self {
        Self::Lease {
            message: message.into(),
        }
    }

    /// Create a new lock error
    pub fn lock(lock_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Lock {
            lock_name: lock_name.into(),
            message: message.into(),
        }
    }

    /// Create a new serialization error
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    /// Create a new timeout error
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::Timeout {
            operation: operation.into(),
        }
    }

    /// Create a new network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    /// Create a new authentication error
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a new permission error
    pub fn permission(message: impl Into<String>) -> Self {
        Self::Permission {
            message: message.into(),
        }
    }
}

/// Result type for etcd operations
pub type EtcdResult<T> = Result<T, EtcdError>;
