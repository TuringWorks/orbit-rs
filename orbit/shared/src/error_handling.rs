//! Deprecated: This module is maintained for backward compatibility only.
//! Please use the `error` module instead.

// Re-export everything from the new unified error module
pub use crate::error::*;

#[cfg(test)]
mod tests {
    use super::*;

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
