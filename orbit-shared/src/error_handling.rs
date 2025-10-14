//! Enhanced error handling patterns for consistent error management across the codebase
//!
//! This module provides traits and utilities to reduce error handling boilerplate
//! and ensure consistent error conversion patterns.

use crate::exception::OrbitError;

/// Trait for consistent error conversion patterns
pub trait ErrorConverter<T> {
    /// Convert result to OrbitError with context
    fn to_orbit_error(self, context: &str) -> Result<T, OrbitError>;

    /// Convert result to OrbitError with formatted context
    fn to_orbit_error_with<F>(self, context_fn: F) -> Result<T, OrbitError>
    where
        F: FnOnce() -> String;

    /// Map error with orbit error chain
    fn map_orbit_error<F>(self, mapper: F) -> Result<T, OrbitError>
    where
        F: FnOnce(OrbitError) -> OrbitError;
}

impl<T, E> ErrorConverter<T> for Result<T, E>
where
    E: Into<OrbitError>,
{
    fn to_orbit_error(self, context: &str) -> Result<T, OrbitError> {
        self.map_err(|e| {
            let orbit_error: OrbitError = e.into();
            OrbitError::internal(format!("{context}: {orbit_error}"))
        })
    }

    fn to_orbit_error_with<F>(self, context_fn: F) -> Result<T, OrbitError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let orbit_error: OrbitError = e.into();
            OrbitError::internal(format!("{}: {}", context_fn(), orbit_error))
        })
    }

    fn map_orbit_error<F>(self, mapper: F) -> Result<T, OrbitError>
    where
        F: FnOnce(OrbitError) -> OrbitError,
    {
        self.map_err(|e| {
            let orbit_error: OrbitError = e.into();
            mapper(orbit_error)
        })
    }
}

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
    fn log_error(self, message: &str) -> Result<T, OrbitError>;

    /// Log error at error level and return it
    fn log_critical_error(self, message: &str) -> Result<T, OrbitError>;

    /// Log error with custom level
    fn log_error_with_level(self, level: tracing::Level, message: &str) -> Result<T, OrbitError>;
}

impl<T> ErrorLog<T> for Result<T, OrbitError> {
    fn log_error(self, message: &str) -> Result<T, OrbitError> {
        self.map_err(|e| {
            tracing::warn!("{}: {}", message, e);
            e
        })
    }

    fn log_critical_error(self, message: &str) -> Result<T, OrbitError> {
        self.map_err(|e| {
            tracing::error!("{}: {}", message, e);
            e
        })
    }

    fn log_error_with_level(self, level: tracing::Level, message: &str) -> Result<T, OrbitError> {
        self.inspect_err(|e| {
            log_at_level!(level, message, e);
        })
    }
}

/// Trait for error handling with retry logic
pub trait ErrorRetry<T> {
    /// Retry operation with exponential backoff
    fn retry_with_backoff(
        self,
        max_retries: u32,
        base_delay_ms: u64,
    ) -> impl std::future::Future<Output = Result<T, OrbitError>> + Send
    where
        Self: Clone + Send + Sync,
        T: Send + Sync;
}

/// Security-focused input validation traits
pub trait SecurityValidator {
    /// Validate input for SQL injection patterns
    fn validate_sql_safe(&self) -> Result<(), OrbitError>;

    /// Validate input for XSS patterns
    fn validate_xss_safe(&self) -> Result<(), OrbitError>;

    /// Validate input length constraints
    fn validate_length(&self, max_len: usize) -> Result<(), OrbitError>;

    /// Validate input against allowed characters
    fn validate_allowed_chars(&self, allowed_pattern: &str) -> Result<(), OrbitError>;
}

impl SecurityValidator for String {
    fn validate_sql_safe(&self) -> Result<(), OrbitError> {
        let dangerous_patterns = [
            "DROP", "DELETE", "INSERT", "UPDATE", "UNION", "SELECT", "--", "/*", "*/", ";", "xp_",
            "sp_",
        ];

        let upper_self = self.to_uppercase();
        for pattern in &dangerous_patterns {
            if upper_self.contains(pattern) {
                return Err(OrbitError::internal(format!(
                    "Input contains potentially dangerous SQL pattern: {pattern}"
                )));
            }
        }
        Ok(())
    }

    fn validate_xss_safe(&self) -> Result<(), OrbitError> {
        let dangerous_patterns = [
            "<script",
            "</script>",
            "javascript:",
            "vbscript:",
            "onload=",
            "onerror=",
            "onclick=",
            "onmouseover=",
        ];

        let lower_self = self.to_lowercase();
        for pattern in &dangerous_patterns {
            if lower_self.contains(pattern) {
                return Err(OrbitError::internal(format!(
                    "Input contains potentially dangerous XSS pattern: {pattern}"
                )));
            }
        }
        Ok(())
    }

    fn validate_length(&self, max_len: usize) -> Result<(), OrbitError> {
        if self.len() > max_len {
            return Err(OrbitError::internal(format!(
                "Input length {} exceeds maximum allowed length {}",
                self.len(),
                max_len
            )));
        }
        Ok(())
    }

    fn validate_allowed_chars(&self, allowed_pattern: &str) -> Result<(), OrbitError> {
        use regex::Regex;

        let regex = Regex::new(allowed_pattern)
            .map_err(|e| OrbitError::internal(format!("Invalid regex pattern: {e}")))?;

        if !regex.is_match(self) {
            return Err(OrbitError::internal(format!(
                "Input contains invalid characters. Allowed pattern: {allowed_pattern}"
            )));
        }
        Ok(())
    }
}

impl SecurityValidator for &str {
    fn validate_sql_safe(&self) -> Result<(), OrbitError> {
        self.to_string().validate_sql_safe()
    }

    fn validate_xss_safe(&self) -> Result<(), OrbitError> {
        self.to_string().validate_xss_safe()
    }

    fn validate_length(&self, max_len: usize) -> Result<(), OrbitError> {
        self.to_string().validate_length(max_len)
    }

    fn validate_allowed_chars(&self, allowed_pattern: &str) -> Result<(), OrbitError> {
        self.to_string().validate_allowed_chars(allowed_pattern)
    }
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
    fn test_error_converter() {
        let result: Result<i32, String> = Err("test error".to_string());
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
}
