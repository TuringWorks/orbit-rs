//! Common configuration utilities to reduce code duplication across modules

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Common configuration trait that provides default validation and serialization
pub trait ConfigDefaults {
    /// Validate the configuration settings
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    /// Merge configuration with another instance, giving priority to non-default values
    fn merge(self, other: Self) -> Self
    where
        Self: Sized,
    {
        // Default implementation - can be overridden
        other
    }

    /// Get configuration source description for debugging
    fn config_source(&self) -> &'static str {
        "default"
    }
}

/// Common timeout configuration used across services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub connection_timeout: Duration,
    pub request_timeout: Duration,
    pub health_check_interval: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_millis(5000),
            request_timeout: Duration::from_millis(10000),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

impl ConfigDefaults for TimeoutConfig {
    fn validate(&self) -> Result<(), String> {
        if self.connection_timeout.as_millis() == 0 {
            return Err("Connection timeout must be greater than 0".to_string());
        }
        if self.request_timeout < self.connection_timeout {
            return Err(
                "Request timeout must be greater than or equal to connection timeout".to_string(),
            );
        }
        Ok(())
    }
}

/// Common retry configuration pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

impl ConfigDefaults for RetryConfig {
    fn validate(&self) -> Result<(), String> {
        if self.max_attempts == 0 {
            return Err("Max attempts must be greater than 0".to_string());
        }
        if self.backoff_multiplier < 1.0 {
            return Err("Backoff multiplier must be >= 1.0".to_string());
        }
        if self.max_delay < self.initial_delay {
            return Err("Max delay must be >= initial delay".to_string());
        }
        Ok(())
    }
}

/// Common resource limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub max_memory_mb: Option<u64>,
    pub max_connections: u32,
    pub thread_pool_size: Option<usize>,
    pub buffer_size: usize,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: None,
            max_connections: 100,
            thread_pool_size: None, // Uses system default
            buffer_size: 8192,
        }
    }
}

impl ConfigDefaults for ResourceConfig {
    fn validate(&self) -> Result<(), String> {
        if self.max_connections == 0 {
            return Err("Max connections must be greater than 0".to_string());
        }
        if self.buffer_size == 0 {
            return Err("Buffer size must be greater than 0".to_string());
        }
        if let Some(memory) = self.max_memory_mb {
            if memory == 0 {
                return Err("Max memory must be greater than 0".to_string());
            }
        }
        Ok(())
    }
}

/// Macro to reduce boilerplate in configuration struct creation
#[macro_export]
macro_rules! config_builder {
    ($name:ident { $($field:ident: $type:ty = $default:expr),* $(,)? }) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $name {
            $(pub $field: $type,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $default,)*
                }
            }
        }

        impl $crate::config_utils::ConfigDefaults for $name {}

        impl $name {
            pub fn new() -> Self {
                Self::default()
            }

            $(
                paste::paste! {
                    pub fn [<with_ $field>](mut self, value: $type) -> Self {
                        self.$field = value;
                        self
                    }
                }
            )*
        }
    };
}

/// Common environment configuration helper
pub fn get_env_or_default<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_config_validation() {
        let mut config = TimeoutConfig::default();
        assert!(config.validate().is_ok());

        config.request_timeout = Duration::from_millis(1000);
        config.connection_timeout = Duration::from_millis(2000);
        assert!(config.validate().is_err()); // request < connection
    }

    #[test]
    fn test_retry_config_validation() {
        let mut config = RetryConfig::default();
        assert!(config.validate().is_ok());

        config.max_attempts = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_resource_config_validation() {
        let config = ResourceConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = ResourceConfig {
            max_connections: 0,
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_config_merge_functionality() {
        let config1 = TimeoutConfig::default();
        let config2 = TimeoutConfig {
            connection_timeout: Duration::from_millis(1000),
            ..Default::default()
        };

        let merged = config1.merge(config2.clone());
        assert_eq!(merged.connection_timeout, config2.connection_timeout);
    }

    #[test]
    fn test_environment_variable_parsing() {
        // Test parsing of different types
        let result: i32 = get_env_or_default("NONEXISTENT_INT", 42);
        assert_eq!(result, 42);

        let result: String = get_env_or_default("NONEXISTENT_STRING", "default".to_string());
        assert_eq!(result, "default");
    }
}
