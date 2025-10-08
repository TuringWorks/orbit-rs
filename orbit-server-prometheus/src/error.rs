use thiserror::Error;

/// Errors that can occur in Prometheus operations
#[derive(Error, Debug)]
pub enum PrometheusError {
    /// Metrics collection error
    #[error("Metrics collection error: {message}")]
    Collection { message: String },

    /// Exporter initialization error
    #[error("Exporter initialization failed: {message}")]
    ExporterInit { message: String },

    /// Server startup error
    #[error("Server startup failed: {message}")]
    ServerStartup { message: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Metrics registry error
    #[error("Metrics registry error: {message}")]
    Registry { message: String },

    /// Network error
    #[error("Network error: {message}")]
    Network { message: String },

    /// Serialization error
    #[error("Serialization error: {message}")]
    Serialization { message: String },

    /// Dashboard generation error
    #[error("Dashboard generation error: {message}")]
    Dashboard { message: String },

    /// Health check error
    #[error("Health check error: {service} - {message}")]
    HealthCheck { service: String, message: String },

    /// Metric validation error
    #[error("Metric validation error: {metric} - {message}")]
    MetricValidation { metric: String, message: String },

    /// Generic I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),

    /// Hyper error
    #[error("HTTP error: {0}")]
    Http(#[from] hyper::Error),

    /// Serialization error
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl PrometheusError {
    /// Create a new collection error
    pub fn collection(message: impl Into<String>) -> Self {
        Self::Collection {
            message: message.into(),
        }
    }

    /// Create a new exporter initialization error
    pub fn exporter_init(message: impl Into<String>) -> Self {
        Self::ExporterInit {
            message: message.into(),
        }
    }

    /// Create a new server startup error
    pub fn server_startup(message: impl Into<String>) -> Self {
        Self::ServerStartup {
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a new registry error
    pub fn registry(message: impl Into<String>) -> Self {
        Self::Registry {
            message: message.into(),
        }
    }

    /// Create a new network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    /// Create a new serialization error
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    /// Create a new dashboard error
    pub fn dashboard(message: impl Into<String>) -> Self {
        Self::Dashboard {
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

    /// Create a new metric validation error
    pub fn metric_validation(metric: impl Into<String>, message: impl Into<String>) -> Self {
        Self::MetricValidation {
            metric: metric.into(),
            message: message.into(),
        }
    }
}

/// Result type for Prometheus operations
pub type PrometheusResult<T> = Result<T, PrometheusError>;
