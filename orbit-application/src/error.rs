use thiserror::Error;

/// Application-level errors
#[derive(Error, Debug)]
pub enum AppError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Service errors
    #[error("Service error: {0}")]
    Service(String),

    /// Plugin errors
    #[error("Plugin error: {0}")]
    Plugin(String),

    /// Health check errors
    #[error("Health check error: {0}")]
    Health(String),

    /// Telemetry errors
    #[error("Telemetry error: {0}")]
    Telemetry(String),

    /// Event system errors
    #[error("Event error: {0}")]
    Event(String),

    /// Lifecycle management errors
    #[error("Lifecycle error: {0}")]
    Lifecycle(String),

    /// Network/HTTP errors
    #[error("Network error: {0}")]
    Network(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML serialization errors
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Configuration parsing errors
    #[error("Config parsing error: {0}")]
    ConfigParsing(#[from] config::ConfigError),

    /// Generic errors
    #[error("Application error: {0}")]
    Other(#[from] anyhow::Error),
}

impl AppError {
    /// Create a configuration error
    pub fn config<T: Into<String>>(msg: T) -> Self {
        Self::Config(msg.into())
    }

    /// Create a service error
    pub fn service<T: Into<String>>(msg: T) -> Self {
        Self::Service(msg.into())
    }

    /// Create a plugin error
    pub fn plugin<T: Into<String>>(msg: T) -> Self {
        Self::Plugin(msg.into())
    }

    /// Create a health check error
    pub fn health<T: Into<String>>(msg: T) -> Self {
        Self::Health(msg.into())
    }

    /// Create a telemetry error
    pub fn telemetry<T: Into<String>>(msg: T) -> Self {
        Self::Telemetry(msg.into())
    }

    /// Create an event error
    pub fn event<T: Into<String>>(msg: T) -> Self {
        Self::Event(msg.into())
    }

    /// Create a lifecycle error
    pub fn lifecycle<T: Into<String>>(msg: T) -> Self {
        Self::Lifecycle(msg.into())
    }

    /// Create a network error
    pub fn network<T: Into<String>>(msg: T) -> Self {
        Self::Network(msg.into())
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        match self {
            AppError::Config(_) => false,
            AppError::Service(_) => true,
            AppError::Plugin(_) => true,
            AppError::Health(_) => true,
            AppError::Telemetry(_) => true,
            AppError::Event(_) => true,
            AppError::Lifecycle(_) => false,
            AppError::Network(_) => true,
            AppError::Io(_) => true,
            AppError::Json(_) => false,
            AppError::Yaml(_) => false,
            AppError::ConfigParsing(_) => false,
            AppError::Other(_) => false,
        }
    }

    /// Get the error category
    pub fn category(&self) -> ErrorCategory {
        match self {
            AppError::Config(_) | AppError::ConfigParsing(_) => ErrorCategory::Configuration,
            AppError::Service(_) => ErrorCategory::Service,
            AppError::Plugin(_) => ErrorCategory::Plugin,
            AppError::Health(_) => ErrorCategory::Health,
            AppError::Telemetry(_) => ErrorCategory::Telemetry,
            AppError::Event(_) => ErrorCategory::Event,
            AppError::Lifecycle(_) => ErrorCategory::Lifecycle,
            AppError::Network(_) => ErrorCategory::Network,
            AppError::Io(_) => ErrorCategory::Io,
            AppError::Json(_) | AppError::Yaml(_) => ErrorCategory::Serialization,
            AppError::Other(_) => ErrorCategory::Other,
        }
    }
}

/// Error categories for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Configuration,
    Service,
    Plugin,
    Health,
    Telemetry,
    Event,
    Lifecycle,
    Network,
    Io,
    Serialization,
    Other,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCategory::Configuration => write!(f, "Configuration"),
            ErrorCategory::Service => write!(f, "Service"),
            ErrorCategory::Plugin => write!(f, "Plugin"),
            ErrorCategory::Health => write!(f, "Health"),
            ErrorCategory::Telemetry => write!(f, "Telemetry"),
            ErrorCategory::Event => write!(f, "Event"),
            ErrorCategory::Lifecycle => write!(f, "Lifecycle"),
            ErrorCategory::Network => write!(f, "Network"),
            ErrorCategory::Io => write!(f, "IO"),
            ErrorCategory::Serialization => write!(f, "Serialization"),
            ErrorCategory::Other => write!(f, "Other"),
        }
    }
}

/// Result type alias for convenience
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let config_error = AppError::config("test config error");
        assert!(matches!(config_error, AppError::Config(_)));
        assert_eq!(config_error.category(), ErrorCategory::Configuration);
        assert!(!config_error.is_recoverable());

        let service_error = AppError::service("test service error");
        assert!(matches!(service_error, AppError::Service(_)));
        assert_eq!(service_error.category(), ErrorCategory::Service);
        assert!(service_error.is_recoverable());
    }

    #[test]
    fn test_error_display() {
        let error = AppError::config("test error");
        assert_eq!(format!("{}", error), "Configuration error: test error");
    }
}
