//! Unified error handling module for Orbit-RS
//!
//! This module consolidates all error handling patterns, providing:
//! - Core OrbitError enum with comprehensive error types
//! - Error conversion traits and utilities
//! - Security validation traits
//! - Error logging and context helpers
//! - Convenience macros for error creation

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

    #[error("Configuration error: {message}{}", .key.as_ref().map(|k| format!(" (key: {})", k)).unwrap_or_default())]
    ConfigurationError {
        message: String,
        key: Option<String>,
    },

    #[error("Cluster error: {0}")]
    ClusterError(String),

    #[error("IO error: {message}{}", .source_info.as_ref().map(|s| format!(" (source: {})", s)).unwrap_or_default())]
    IoError {
        message: String,
        source_info: Option<String>,
    },

    #[error("Parse error: {message}{}{}",
        .position.as_ref().map(|p| format!(" at position {}", p)).unwrap_or_default(),
        .input.as_ref().map(|i| format!(" in input: {}", i)).unwrap_or_default()
    )]
    ParseError {
        message: String,
        input: Option<String>,
        position: Option<usize>,
    },

    #[error("Storage error: {message}{}", .operation.as_ref().map(|o| format!(" (operation: {})", o)).unwrap_or_default())]
    StorageError {
        message: String,
        operation: Option<String>,
    },

    #[error("Authentication error: {message}{}", .user.as_ref().map(|u| format!(" (user: {})", u)).unwrap_or_default())]
    AuthError {
        message: String,
        user: Option<String>,
    },

    #[error("Internal error: {message}{}", .context.as_ref().map(|c| format!(" (context: {})", c)).unwrap_or_default())]
    Internal {
        message: String,
        context: Option<String>,
    },
}

impl From<String> for OrbitError {
    fn from(msg: String) -> Self {
        OrbitError::Internal {
            message: msg,
            context: None,
        }
    }
}

impl From<&str> for OrbitError {
    fn from(msg: &str) -> Self {
        OrbitError::Internal {
            message: msg.to_string(),
            context: None,
        }
    }
}

impl From<std::io::Error> for OrbitError {
    fn from(err: std::io::Error) -> Self {
        OrbitError::IoError {
            message: err.to_string(),
            source_info: None,
        }
    }
}

impl OrbitError {
    /// Create a network error
    pub fn network<S: Into<String>>(msg: S) -> Self {
        OrbitError::NetworkError(msg.into())
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(operation: S) -> Self {
        OrbitError::Timeout {
            operation: operation.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration<S: Into<String>>(msg: S) -> Self {
        OrbitError::ConfigurationError {
            message: msg.into(),
            key: None,
        }
    }

    /// Create a configuration error with key
    pub fn configuration_with_key<M: Into<String>, K: Into<String>>(msg: M, key: K) -> Self {
        OrbitError::ConfigurationError {
            message: msg.into(),
            key: Some(key.into()),
        }
    }

    /// Create a cluster error
    pub fn cluster<S: Into<String>>(msg: S) -> Self {
        OrbitError::ClusterError(msg.into())
    }

    /// Create an internal error
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        OrbitError::Internal {
            message: msg.into(),
            context: None,
        }
    }

    /// Create an internal error with context
    pub fn internal_with_context<M: Into<String>, C: Into<String>>(msg: M, context: C) -> Self {
        OrbitError::Internal {
            message: msg.into(),
            context: Some(context.into()),
        }
    }

    /// Create an IO error
    pub fn io<S: Into<String>>(msg: S) -> Self {
        OrbitError::IoError {
            message: msg.into(),
            source_info: None,
        }
    }

    /// Create an IO error with source
    pub fn io_with_source<M: Into<String>, S: Into<String>>(msg: M, source: S) -> Self {
        OrbitError::IoError {
            message: msg.into(),
            source_info: Some(source.into()),
        }
    }

    /// Create a parse error
    pub fn parse<S: Into<String>>(msg: S) -> Self {
        OrbitError::ParseError {
            message: msg.into(),
            input: None,
            position: None,
        }
    }

    /// Create a parse error with input
    pub fn parse_with_input<M: Into<String>, I: Into<String>>(msg: M, input: I) -> Self {
        OrbitError::ParseError {
            message: msg.into(),
            input: Some(input.into()),
            position: None,
        }
    }

    /// Create a parse error with input and position
    pub fn parse_with_position<M: Into<String>, I: Into<String>>(
        msg: M,
        input: I,
        position: usize,
    ) -> Self {
        OrbitError::ParseError {
            message: msg.into(),
            input: Some(input.into()),
            position: Some(position),
        }
    }

    /// Create a storage error
    pub fn storage<S: Into<String>>(msg: S) -> Self {
        OrbitError::StorageError {
            message: msg.into(),
            operation: None,
        }
    }

    /// Create a storage error with operation
    pub fn storage_with_operation<M: Into<String>, O: Into<String>>(msg: M, operation: O) -> Self {
        OrbitError::StorageError {
            message: msg.into(),
            operation: Some(operation.into()),
        }
    }

    /// Create an authentication error
    pub fn auth<S: Into<String>>(msg: S) -> Self {
        OrbitError::AuthError {
            message: msg.into(),
            user: None,
        }
    }

    /// Create an authentication error with user
    pub fn auth_with_user<M: Into<String>, U: Into<String>>(msg: M, user: U) -> Self {
        OrbitError::AuthError {
            message: msg.into(),
            user: Some(user.into()),
        }
    }

    /// Create an execution error (alias for internal)
    pub fn execution<S: Into<String>>(msg: S) -> Self {
        OrbitError::Internal {
            message: msg.into(),
            context: Some("execution".into()),
        }
    }
}

/// Result type for Orbit operations
pub type OrbitResult<T> = std::result::Result<T, OrbitError>;

/// Alias for backward compatibility
pub type Result<T> = std::result::Result<T, OrbitError>;

// ===== Error Conversion Traits =====

/// Trait for consistent error conversion patterns
pub trait ErrorConverter<T> {
    /// Convert result to OrbitError with context
    fn to_orbit_error(self, context: &str) -> OrbitResult<T>;

    /// Convert result to OrbitError with formatted context
    fn to_orbit_error_with<F>(self, context_fn: F) -> OrbitResult<T>
    where
        F: FnOnce() -> String;

    /// Map error with orbit error chain
    fn map_orbit_error<F>(self, mapper: F) -> OrbitResult<T>
    where
        F: FnOnce(OrbitError) -> OrbitError;
}

impl<T, E> ErrorConverter<T> for std::result::Result<T, E>
where
    E: Into<OrbitError>,
{
    fn to_orbit_error(self, context: &str) -> OrbitResult<T> {
        self.map_err(|e| {
            let orbit_error: OrbitError = e.into();
            OrbitError::internal_with_context(orbit_error.to_string(), context.to_string())
        })
    }

    fn to_orbit_error_with<F>(self, context_fn: F) -> OrbitResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let orbit_error: OrbitError = e.into();
            OrbitError::internal_with_context(orbit_error.to_string(), context_fn())
        })
    }

    fn map_orbit_error<F>(self, mapper: F) -> OrbitResult<T>
    where
        F: FnOnce(OrbitError) -> OrbitError,
    {
        self.map_err(|e| {
            let orbit_error: OrbitError = e.into();
            mapper(orbit_error)
        })
    }
}

/// Trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn with_context<F>(self, f: F) -> OrbitResult<T>
    where
        F: FnOnce() -> String;

    /// Add context message to an error
    fn context(self, message: &str) -> OrbitResult<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + 'static,
{
    fn with_context<F>(self, f: F) -> OrbitResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| OrbitError::internal_with_context(f(), e.to_string()))
    }

    fn context(self, message: &str) -> OrbitResult<T> {
        self.with_context(|| message.to_string())
    }
}

// ===== Error Logging Traits =====

/// Helper macro to log at specific level
macro_rules! log_at_level {
    ($level:expr, $message:expr, $error:expr) => {
        match $level {
            tracing::Level::ERROR => tracing::error!("{}: {}", $message, $error),
            tracing::Level::WARN => tracing::warn!("{}: {}", $message, $error),
            tracing::Level::INFO => tracing::info!("{}: {}", $message, $error),
            tracing::Level::DEBUG => tracing::debug!("{}: {}", $message, $error),
            tracing::Level::TRACE => tracing::trace!("{}: {}", $message, $error),
        }
    };
}

/// Trait for error handling with automatic logging
pub trait ErrorLog<T> {
    /// Log error at warn level and return it
    fn log_error(self, message: &str) -> OrbitResult<T>;

    /// Log error at error level and return it
    fn log_critical_error(self, message: &str) -> OrbitResult<T>;

    /// Log error with custom level
    fn log_error_with_level(self, level: tracing::Level, message: &str) -> OrbitResult<T>;
}

impl<T> ErrorLog<T> for OrbitResult<T> {
    fn log_error(self, message: &str) -> OrbitResult<T> {
        self.map_err(|e| {
            tracing::warn!("{}: {}", message, e);
            e
        })
    }

    fn log_critical_error(self, message: &str) -> OrbitResult<T> {
        self.map_err(|e| {
            tracing::error!("{}: {}", message, e);
            e
        })
    }

    fn log_error_with_level(self, level: tracing::Level, message: &str) -> OrbitResult<T> {
        self.inspect_err(|e| {
            log_at_level!(level, message, e);
        })
    }
}

// ===== Security Validation Traits =====

/// SQL keywords and comment/terminator sequences treated as potentially dangerous
/// when they appear in untrusted input. Compared case-insensitively.
const DANGEROUS_SQL_PATTERNS: [&str; 12] = [
    "DROP", "DELETE", "INSERT", "UPDATE", "UNION", "SELECT", "--", "/*", "*/", ";", "xp_", "sp_",
];

/// Markup and script sequences treated as potentially dangerous (XSS) when they
/// appear in untrusted input. Compared case-insensitively.
const DANGEROUS_XSS_PATTERNS: [&str; 8] = [
    "<script",
    "</script>",
    "javascript:",
    "vbscript:",
    "onload=",
    "onerror=",
    "onclick=",
    "onmouseover=",
];

/// Reject `haystack` if it contains any of `patterns`, naming the first match in the
/// error. `haystack` must already be normalised to the case the patterns are written
/// in. Pure helper shared by every [`SecurityValidator`] implementation.
fn reject_if_contains(haystack: &str, patterns: &[&str], kind: &str) -> OrbitResult<()> {
    patterns
        .iter()
        .find(|pattern| haystack.contains(**pattern))
        .map_or(Ok(()), |pattern| {
            Err(OrbitError::internal(format!(
                "Input contains potentially dangerous {kind} pattern: {pattern}"
            )))
        })
}

fn validate_sql_safe_str(input: &str) -> OrbitResult<()> {
    reject_if_contains(&input.to_uppercase(), &DANGEROUS_SQL_PATTERNS, "SQL")
}

fn validate_xss_safe_str(input: &str) -> OrbitResult<()> {
    reject_if_contains(&input.to_lowercase(), &DANGEROUS_XSS_PATTERNS, "XSS")
}

fn validate_length_str(input: &str, max_len: usize) -> OrbitResult<()> {
    if input.len() <= max_len {
        Ok(())
    } else {
        Err(OrbitError::internal(format!(
            "Input length {} exceeds maximum allowed length {}",
            input.len(),
            max_len
        )))
    }
}

fn validate_allowed_chars_str(input: &str, allowed_pattern: &str) -> OrbitResult<()> {
    let regex = regex::Regex::new(allowed_pattern)
        .map_err(|e| OrbitError::internal(format!("Invalid regex pattern: {e}")))?;
    if regex.is_match(input) {
        Ok(())
    } else {
        Err(OrbitError::internal(format!(
            "Input contains invalid characters. Allowed pattern: {allowed_pattern}"
        )))
    }
}

/// Security-focused input validation, implemented for the common owned and borrowed
/// string types so a check reads the same whether the caller holds a `String` or a
/// `&str`. All implementations delegate to the same pure functions above.
pub trait SecurityValidator {
    /// Reject input containing SQL keywords or comment/terminator sequences.
    fn validate_sql_safe(&self) -> OrbitResult<()>;

    /// Reject input containing markup/script (XSS) sequences.
    fn validate_xss_safe(&self) -> OrbitResult<()>;

    /// Reject input longer than `max_len` bytes.
    fn validate_length(&self, max_len: usize) -> OrbitResult<()>;

    /// Reject input that does not fully match `allowed_pattern` (a regex).
    fn validate_allowed_chars(&self, allowed_pattern: &str) -> OrbitResult<()>;
}

impl SecurityValidator for String {
    fn validate_sql_safe(&self) -> OrbitResult<()> {
        validate_sql_safe_str(self)
    }

    fn validate_xss_safe(&self) -> OrbitResult<()> {
        validate_xss_safe_str(self)
    }

    fn validate_length(&self, max_len: usize) -> OrbitResult<()> {
        validate_length_str(self, max_len)
    }

    fn validate_allowed_chars(&self, allowed_pattern: &str) -> OrbitResult<()> {
        validate_allowed_chars_str(self, allowed_pattern)
    }
}

impl SecurityValidator for &str {
    fn validate_sql_safe(&self) -> OrbitResult<()> {
        validate_sql_safe_str(self)
    }

    fn validate_xss_safe(&self) -> OrbitResult<()> {
        validate_xss_safe_str(self)
    }

    fn validate_length(&self, max_len: usize) -> OrbitResult<()> {
        validate_length_str(self, max_len)
    }

    fn validate_allowed_chars(&self, allowed_pattern: &str) -> OrbitResult<()> {
        validate_allowed_chars_str(self, allowed_pattern)
    }
}

// ===== Utility Functions =====

/// Utility function to wrap std errors
pub fn wrap_std_error<E: std::error::Error>(error: E) -> OrbitError {
    OrbitError::internal(error.to_string())
}

// ===== Convenience Macros =====

/// Macro for creating configuration errors
#[macro_export]
macro_rules! config_error {
    ($msg:expr) => {
        $crate::error::OrbitError::configuration($msg)
    };
    ($msg:expr, $key:expr) => {
        $crate::error::OrbitError::configuration_with_key($msg, $key)
    };
}

/// Macro for creating IO errors
#[macro_export]
macro_rules! io_error {
    ($msg:expr) => {
        $crate::error::OrbitError::io($msg)
    };
    ($msg:expr, $source:expr) => {
        $crate::error::OrbitError::io_with_source($msg, $source)
    };
}

/// Macro for creating parse errors
#[macro_export]
macro_rules! parse_error {
    ($msg:expr) => {
        $crate::error::OrbitError::parse($msg)
    };
    ($msg:expr, $input:expr) => {
        $crate::error::OrbitError::parse_with_input($msg, $input)
    };
    ($msg:expr, $input:expr, $pos:expr) => {
        $crate::error::OrbitError::parse_with_position($msg, $input, $pos)
    };
}

/// Macro for quick error conversion with context
#[macro_export]
macro_rules! orbit_error {
    ($e:expr, $context:expr) => {
        $e.to_orbit_error($context)
    };
    ($e:expr, $context:expr, $($args:tt)*) => {
        $e.to_orbit_error_with(|| format!($context, $($args)*))
    };
}

/// Macro for error logging with conversion
#[macro_export]
macro_rules! orbit_log_error {
    ($e:expr, $level:expr, $message:expr) => {
        $e.log_error_with_level($level, $message)
    };
    ($e:expr, $message:expr) => {
        $e.log_error($message)
    };
}

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

    #[test]
    fn test_all_error_constructors() {
        let timeout_error = OrbitError::timeout("operation_timeout");
        assert!(matches!(timeout_error, OrbitError::Timeout { .. }));

        let config_error = OrbitError::configuration("Invalid config");
        assert!(matches!(
            config_error,
            OrbitError::ConfigurationError { .. }
        ));

        let cluster_error = OrbitError::cluster("Cluster unreachable");
        assert!(matches!(cluster_error, OrbitError::ClusterError(_)));

        let internal_error = OrbitError::internal("Internal fault");
        assert!(matches!(internal_error, OrbitError::Internal { .. }));
    }

    #[test]
    fn test_error_macros() {
        let config_err = config_error!("Missing configuration");
        assert!(matches!(config_err, OrbitError::ConfigurationError { .. }));

        let io_err = io_error!("File not found", "test.txt");
        assert!(matches!(io_err, OrbitError::IoError { .. }));

        let parse_err = parse_error!("Invalid syntax", "SELECT * FROM", 10);
        assert!(matches!(parse_err, OrbitError::ParseError { .. }));
    }

    #[test]
    fn test_error_context() {
        fn might_fail() -> std::result::Result<(), std::io::Error> {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "file not found",
            ))
        }

        let result = might_fail().context("Failed to read configuration file");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read configuration file"));
    }

    #[test]
    fn test_error_converter() {
        let result: std::result::Result<i32, String> = Err("test error".to_string());
        let converted = result.to_orbit_error("test context");

        assert!(converted.is_err());
        assert!(converted.unwrap_err().to_string().contains("test context"));
    }

    #[test]
    fn test_security_validator_sql() {
        let safe_input = "user_name".to_string();
        let dangerous_input = "'; DROP TABLE users; --".to_string();

        assert!(safe_input.validate_sql_safe().is_ok());
        assert!(dangerous_input.validate_sql_safe().is_err());
    }

    #[test]
    fn test_security_validator_xss() {
        let safe_input = "normal text".to_string();
        let dangerous_input = "<script>alert('xss')</script>".to_string();

        assert!(safe_input.validate_xss_safe().is_ok());
        assert!(dangerous_input.validate_xss_safe().is_err());
    }

    #[test]
    fn test_security_validator_length() {
        let input = "test".to_string();

        assert!(input.validate_length(10).is_ok());
        assert!(input.validate_length(2).is_err());
    }

    #[test]
    fn test_security_validator_allowed_chars() {
        let alphanumeric = "test123".to_string();
        let with_special = "test@123!".to_string();

        assert!(alphanumeric
            .validate_allowed_chars(r"^[a-zA-Z0-9]+$")
            .is_ok());
        assert!(with_special
            .validate_allowed_chars(r"^[a-zA-Z0-9]+$")
            .is_err());
    }

    #[test]
    fn test_orbit_result_type() {
        let success: OrbitResult<String> = Ok("success".to_string());
        assert!(success.is_ok());

        let error: OrbitResult<String> = Err(OrbitError::network("Failed"));
        assert!(error.is_err());
    }

    #[test]
    fn test_error_size() {
        let size = std::mem::size_of::<OrbitError>();
        assert!(size <= 256, "OrbitError is too large: {} bytes", size);
    }

    #[test]
    fn test_paired_constructors_accept_mixed_string_types() {
        // Independent generic params: the message and the second argument need not
        // be the same string type, so callers can freely mix &str and String.
        let a = OrbitError::configuration_with_key("missing", String::from("db.url"));
        assert!(matches!(
            a,
            OrbitError::ConfigurationError { key: Some(_), .. }
        ));

        let b = OrbitError::io_with_source(String::from("read failed"), "disk");
        assert!(matches!(
            b,
            OrbitError::IoError {
                source_info: Some(_),
                ..
            }
        ));

        let c = OrbitError::parse_with_position("bad token", String::from("SELECT"), 3);
        assert!(matches!(
            c,
            OrbitError::ParseError {
                position: Some(3),
                ..
            }
        ));

        let d = OrbitError::auth_with_user("denied", String::from("alice"));
        assert!(d.to_string().contains("alice"));
    }

    #[test]
    fn test_security_validator_works_on_str_slices() {
        // The &str impl now shares the String logic without an intermediate alloc.
        assert!("user_name".validate_sql_safe().is_ok());
        assert!("'; DROP TABLE users; --".validate_sql_safe().is_err());
        assert!("<script>x</script>".validate_xss_safe().is_err());
        assert!("safe".validate_length(10).is_ok());
    }

    #[test]
    fn test_validator_error_names_the_matched_pattern() {
        let err = "value UNION select".validate_sql_safe().unwrap_err();
        assert!(err.to_string().contains("UNION"));
    }

    #[test]
    fn test_validator_edge_cases() {
        // Empty input is trivially safe; length check is inclusive at the boundary.
        assert!("".validate_sql_safe().is_ok());
        assert!("".validate_xss_safe().is_ok());
        assert!("abcd".validate_length(4).is_ok());
        assert!("abcde".validate_length(4).is_err());
    }

    #[test]
    fn test_str_and_string_validators_agree() {
        for case in ["clean", "DROP", "onload=", ""] {
            assert_eq!(
                case.validate_sql_safe().is_ok(),
                case.to_string().validate_sql_safe().is_ok(),
                "SQL verdict differs for {case:?}"
            );
            assert_eq!(
                case.validate_xss_safe().is_ok(),
                case.to_string().validate_xss_safe().is_ok(),
                "XSS verdict differs for {case:?}"
            );
        }
    }
}
