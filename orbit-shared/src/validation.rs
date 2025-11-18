//! Unified validation utilities
//!
//! This module provides reusable validation functions to reduce duplication
//! across the codebase.

use crate::error::{OrbitError, OrbitResult};

/// Validation context for better error messages
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub component: String,
    pub field: Option<String>,
}

impl ValidationContext {
    pub fn new<S: Into<String>>(component: S) -> Self {
        Self {
            component: component.into(),
            field: None,
        }
    }

    pub fn with_field<S: Into<String>>(mut self, field: S) -> Self {
        self.field = Some(field.into());
        self
    }

    fn error_msg(&self, message: &str) -> String {
        if let Some(field) = &self.field {
            format!("{} - {}: {}", self.component, field, message)
        } else {
            format!("{}: {}", self.component, message)
        }
    }
}

/// Validate that a string is not empty
pub fn validate_non_empty_string(
    value: &str,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    if value.trim().is_empty() {
        Err(OrbitError::configuration(ctx.error_msg("cannot be empty")))
    } else {
        Ok(())
    }
}

/// Validate that an optional string, if present, is not empty
pub fn validate_optional_non_empty_string(
    value: &Option<String>,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    if let Some(s) = value {
        validate_non_empty_string(s, ctx)?;
    }
    Ok(())
}

/// Validate that a path is not empty (alias for validate_non_empty_string for clarity)
pub fn validate_non_empty_path(
    path: &str,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    validate_non_empty_string(path, ctx)
}

/// Validate that a numeric value is positive
pub fn validate_positive<T>(
    value: T,
    ctx: &ValidationContext,
) -> OrbitResult<()>
where
    T: PartialOrd + From<u8> + std::fmt::Display,
{
    if value <= T::from(0) {
        Err(OrbitError::configuration(ctx.error_msg("must be greater than 0")))
    } else {
        Ok(())
    }
}

/// Validate that a numeric value is non-negative
pub fn validate_non_negative<T>(
    value: T,
    ctx: &ValidationContext,
) -> OrbitResult<()>
where
    T: PartialOrd + From<u8> + std::fmt::Display,
{
    if value < T::from(0) {
        Err(OrbitError::configuration(ctx.error_msg("cannot be negative")))
    } else {
        Ok(())
    }
}

/// Validate that a value is within a range
pub fn validate_range<T>(
    value: T,
    min: T,
    max: T,
    ctx: &ValidationContext,
) -> OrbitResult<()>
where
    T: PartialOrd + std::fmt::Display,
{
    if value < min || value > max {
        Err(OrbitError::configuration(ctx.error_msg(
            &format!("must be between {} and {}", min, max)
        )))
    } else {
        Ok(())
    }
}

/// Validate that a collection is not empty
pub fn validate_non_empty_collection<T>(
    collection: &[T],
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    if collection.is_empty() {
        Err(OrbitError::configuration(ctx.error_msg("cannot be empty")))
    } else {
        Ok(())
    }
}

/// Validate S3-style credentials (access key and secret key)
pub fn validate_s3_credentials(
    access_key: &str,
    secret_key: &str,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    validate_non_empty_string(access_key, &ctx.clone().with_field("access_key"))?;
    validate_non_empty_string(secret_key, &ctx.clone().with_field("secret_key"))?;
    Ok(())
}

/// Validate URL format
pub fn validate_url(
    url: &str,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    validate_non_empty_string(url, ctx)?;

    // Basic URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("tcp://") {
        return Err(OrbitError::configuration(ctx.error_msg(
            "must start with http://, https://, or tcp://"
        )));
    }

    Ok(())
}

/// Validate that TLS configuration is consistent
pub fn validate_tls_config(
    enable_tls: bool,
    ca_cert_path: &Option<String>,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    if enable_tls && ca_cert_path.is_none() {
        Err(OrbitError::configuration(ctx.error_msg(
            "CA certificate path required when TLS is enabled"
        )))
    } else {
        Ok(())
    }
}

/// Validate port number
pub fn validate_port(
    port: u16,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    if port == 0 {
        Err(OrbitError::configuration(ctx.error_msg("port cannot be 0")))
    } else {
        Ok(())
    }
}

/// Validate timeout duration (in milliseconds)
pub fn validate_timeout_ms(
    timeout_ms: u64,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    if timeout_ms == 0 {
        Err(OrbitError::configuration(ctx.error_msg("timeout cannot be 0")))
    } else {
        Ok(())
    }
}

/// Validate percentage value (0.0 to 1.0)
pub fn validate_percentage(
    value: f64,
    ctx: &ValidationContext,
) -> OrbitResult<()> {
    validate_range(value, 0.0, 1.0, ctx)
}

/// Validate that min < max for ordered values
pub fn validate_min_max<T>(
    min: T,
    max: T,
    ctx: &ValidationContext,
) -> OrbitResult<()>
where
    T: PartialOrd + std::fmt::Display,
{
    if min >= max {
        Err(OrbitError::configuration(ctx.error_msg(
            &format!("min ({}) must be less than max ({})", min, max)
        )))
    } else {
        Ok(())
    }
}

/// Builder pattern for validation
pub struct Validator<T> {
    value: T,
    context: ValidationContext,
    result: OrbitResult<()>,
}

impl<T> Validator<T> {
    pub fn new(value: T, context: ValidationContext) -> Self {
        Self {
            value,
            context,
            result: Ok(()),
        }
    }

    pub fn validate<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&T, &ValidationContext) -> OrbitResult<()>,
    {
        if self.result.is_ok() {
            self.result = f(&self.value, &self.context);
        }
        self
    }

    pub fn validate_with_field<F, S: Into<String>>(mut self, field: S, f: F) -> Self
    where
        F: FnOnce(&T, &ValidationContext) -> OrbitResult<()>,
    {
        if self.result.is_ok() {
            let ctx = self.context.clone().with_field(field);
            self.result = f(&self.value, &ctx);
        }
        self
    }

    pub fn finish(self) -> OrbitResult<T> {
        self.result.map(|_| self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_non_empty_string() {
        let ctx = ValidationContext::new("TestComponent");

        assert!(validate_non_empty_string("valid", &ctx).is_ok());
        assert!(validate_non_empty_string("", &ctx).is_err());
        assert!(validate_non_empty_string("   ", &ctx).is_err());
    }

    #[test]
    fn test_validate_positive() {
        let ctx = ValidationContext::new("TestComponent");

        assert!(validate_positive(10u32, &ctx).is_ok());
        assert!(validate_positive(0u32, &ctx).is_err());
    }

    #[test]
    fn test_validate_range() {
        let ctx = ValidationContext::new("TestComponent");

        assert!(validate_range(5, 0, 10, &ctx).is_ok());
        assert!(validate_range(0, 0, 10, &ctx).is_ok());
        assert!(validate_range(10, 0, 10, &ctx).is_ok());
        assert!(validate_range(11, 0, 10, &ctx).is_err());
        assert!(validate_range(-1, 0, 10, &ctx).is_err());
    }

    #[test]
    fn test_validate_percentage() {
        let ctx = ValidationContext::new("TestComponent");

        assert!(validate_percentage(0.5, &ctx).is_ok());
        assert!(validate_percentage(0.0, &ctx).is_ok());
        assert!(validate_percentage(1.0, &ctx).is_ok());
        assert!(validate_percentage(1.1, &ctx).is_err());
        assert!(validate_percentage(-0.1, &ctx).is_err());
    }

    #[test]
    fn test_validate_min_max() {
        let ctx = ValidationContext::new("TestComponent");

        assert!(validate_min_max(5, 10, &ctx).is_ok());
        assert!(validate_min_max(10, 10, &ctx).is_err());
        assert!(validate_min_max(10, 5, &ctx).is_err());
    }

    #[test]
    fn test_validator_builder() {
        let ctx = ValidationContext::new("TestComponent");

        let result = Validator::new(5, ctx)
            .validate(|v, ctx| validate_positive(*v, ctx))
            .validate(|v, ctx| validate_range(*v, 0, 10, ctx))
            .finish();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5);
    }

    #[test]
    fn test_validator_builder_fail() {
        let ctx = ValidationContext::new("TestComponent");

        let result = Validator::new(0, ctx)
            .validate(|v, ctx| validate_positive(*v, ctx))
            .validate(|v, ctx| validate_range(*v, 0, 10, ctx))
            .finish();

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_context_error_messages() {
        let ctx = ValidationContext::new("MyComponent").with_field("my_field");
        let err = validate_non_empty_string("", &ctx).unwrap_err();

        let msg = err.to_string();
        assert!(msg.contains("MyComponent"));
        assert!(msg.contains("my_field"));
    }
}
