//! Deprecated: This module is maintained for backward compatibility only.
//! Please use the `error` module instead.

// Re-export everything from the new unified error module
pub use crate::error::*;

// Legacy type aliases for backward compatibility
pub type Error = crate::error::OrbitError;
pub type Result<T> = crate::error::OrbitResult<T>;

// Legacy helper functions
pub fn app_error(message: &str) -> OrbitError {
    OrbitError::internal(message)
}

pub fn app_error_with_context(message: &str, context: &str) -> OrbitError {
    OrbitError::internal_with_context(message, context)
}

pub fn wrap_std_error<E: std::error::Error>(error: E) -> OrbitError {
    crate::error::wrap_std_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config_error, io_error, parse_error};

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
}
