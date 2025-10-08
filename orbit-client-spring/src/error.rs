use thiserror::Error;

/// Errors that can occur in Spring-like operations
#[derive(Error, Debug)]
pub enum SpringError {
    /// Dependency injection errors
    #[error("Dependency injection error: {message}")]
    DependencyInjection { message: String },

    /// Bean not found error
    #[error("Bean not found: {bean_name}")]
    BeanNotFound { bean_name: String },

    /// Circular dependency detected
    #[error("Circular dependency detected: {dependency_chain}")]
    CircularDependency { dependency_chain: String },

    /// Bean registration error
    #[error("Bean registration failed: {bean_name} - {reason}")]
    BeanRegistration { bean_name: String, reason: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Application context error
    #[error("Application context error: {message}")]
    Context { message: String },

    /// Lifecycle error
    #[error("Lifecycle error in {phase}: {message}")]
    Lifecycle { phase: String, message: String },

    /// Integration error with Orbit components
    #[error("Orbit integration error: {message}")]
    OrbitIntegration { message: String },

    /// Validation error
    #[error("Validation error: {field} - {message}")]
    Validation { field: String, message: String },

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),
}

impl SpringError {
    /// Create a new dependency injection error
    pub fn dependency_injection(message: impl Into<String>) -> Self {
        Self::DependencyInjection {
            message: message.into(),
        }
    }

    /// Create a new bean not found error
    pub fn bean_not_found(bean_name: impl Into<String>) -> Self {
        Self::BeanNotFound {
            bean_name: bean_name.into(),
        }
    }

    /// Create a new circular dependency error
    pub fn circular_dependency(dependency_chain: impl Into<String>) -> Self {
        Self::CircularDependency {
            dependency_chain: dependency_chain.into(),
        }
    }

    /// Create a new bean registration error
    pub fn bean_registration(bean_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::BeanRegistration {
            bean_name: bean_name.into(),
            reason: reason.into(),
        }
    }

    /// Create a new configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a new context error
    pub fn context(message: impl Into<String>) -> Self {
        Self::Context {
            message: message.into(),
        }
    }

    /// Create a new lifecycle error
    pub fn lifecycle(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Lifecycle {
            phase: phase.into(),
            message: message.into(),
        }
    }

    /// Create a new orbit integration error
    pub fn orbit_integration(message: impl Into<String>) -> Self {
        Self::OrbitIntegration {
            message: message.into(),
        }
    }

    /// Create a new validation error
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Result type for Spring operations
pub type SpringResult<T> = Result<T, SpringError>;
