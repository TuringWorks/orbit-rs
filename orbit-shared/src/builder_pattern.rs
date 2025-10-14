//! Builder pattern utilities for consistent configuration object creation
//!
//! This module provides macros and traits to implement the builder pattern
//! consistently across configuration objects, reducing code duplication.

// Required for the macro expansions

/// Trait for objects that can be built using the builder pattern
pub trait Buildable: Default {
    type Builder: ConfigBuilder<Self>;

    /// Create a new builder for this type
    fn builder() -> Self::Builder {
        Self::Builder::new()
    }
}

/// Trait for configuration builders
pub trait ConfigBuilder<T> {
    /// Create a new builder
    fn new() -> Self;

    /// Build the final configuration object
    fn build(self) -> T;

    /// Validate the configuration before building
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    /// Build with validation
    fn build_validated(self) -> Result<T, String>
    where
        Self: Sized,
    {
        self.validate()?;
        Ok(self.build())
    }
}

/// Macro to generate builder pattern implementations
#[macro_export]
macro_rules! impl_builder {
    (
        $(#[$struct_meta:meta])*
        pub struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                pub $field:ident: $field_type:ty = $default:expr
            ),* $(,)?
        }
    ) => {
        $(#[$struct_meta])*
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $name {
            $(
                $(#[$field_meta])*
                pub $field: $field_type,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $default,)*
                }
            }
        }

        paste::paste! {
            #[derive(Debug, Clone)]
            pub struct [<$name Builder>] {
                $($field: Option<$field_type>,)*
            }

            impl ConfigBuilder<$name> for [<$name Builder>] {
                fn new() -> Self {
                    Self {
                        $($field: None,)*
                    }
                }

                fn build(self) -> $name {
                    $name {
                        $($field: self.$field.unwrap_or_else(|| $default),)*
                    }
                }

                // Default validation (can be overridden)
            }

            impl [<$name Builder>] {
                /// Create a new builder
                pub fn new() -> Self {
                    <Self as ConfigBuilder<$name>>::new()
                }

                $(
                    /// Set the value for the field
                    pub fn $field(mut self, value: $field_type) -> Self {
                        self.$field = Some(value);
                        self
                    }

                    paste::paste! {
                        /// Set the value for the field with validation
                        pub fn [<$field _validated>]<F>(mut self, value: $field_type, validator: F) -> Result<Self, String>
                        where
                            F: FnOnce(&$field_type) -> Result<(), String>,
                        {
                            validator(&value)?;
                            self.$field = Some(value);
                            Ok(self)
                        }
                    }
                )*

                /// Build the final configuration
                pub fn build(self) -> $name {
                    <Self as ConfigBuilder<$name>>::build(self)
                }

                /// Build with validation
                pub fn build_validated(self) -> Result<$name, String> {
                    <Self as ConfigBuilder<$name>>::build_validated(self)
                }
            }

            impl Default for [<$name Builder>] {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl Buildable for $name {
                type Builder = [<$name Builder>];
            }
        }
    };
}

/// Trait for configuration objects with environment variable support
pub trait EnvConfigurable: Sized {
    /// Load configuration from environment variables with prefix
    fn from_env_with_prefix(prefix: &str) -> Result<Self, String>;

    /// Load configuration from environment variables with default prefix
    fn from_env() -> Result<Self, String> {
        Self::from_env_with_prefix("ORBIT")
    }
}

/// Macro to implement environment variable configuration loading
#[macro_export]
macro_rules! impl_env_config {
    (
        $name:ident {
            $(
                $field:ident: $field_type:ty = $env_name:expr => $parser:expr
            ),* $(,)?
        }
    ) => {
        impl EnvConfigurable for $name {
            fn from_env_with_prefix(prefix: &str) -> Result<Self, String> {
                let mut builder = Self::builder();

                $(
                    let env_var_name = if prefix.is_empty() {
                        $env_name.to_string()
                    } else {
                        format!("{}_{}", prefix, $env_name)
                    };

                    if let Ok(value_str) = std::env::var(&env_var_name) {
                        let parsed_value: $field_type = $parser(&value_str)
                            .map_err(|e| format!("Failed to parse {} from env var {}: {}", stringify!($field), env_var_name, e))?;
                        builder = builder.$field(parsed_value);
                    }
                )*

                builder.build_validated()
            }
        }
    };
}

/// Standard parsers for common types
pub mod parsers {
    use std::str::FromStr;
    use std::time::Duration;

    /// Parse string (identity function)
    pub fn parse_string(s: &str) -> Result<String, String> {
        Ok(s.to_string())
    }

    /// Parse boolean from string
    pub fn parse_bool(s: &str) -> Result<bool, String> {
        match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(format!("Invalid boolean value: {}", s)),
        }
    }

    /// Parse integer types
    pub fn parse_u32(s: &str) -> Result<u32, String> {
        s.parse().map_err(|e| format!("Invalid u32: {}", e))
    }

    pub fn parse_u64(s: &str) -> Result<u64, String> {
        s.parse().map_err(|e| format!("Invalid u64: {}", e))
    }

    pub fn parse_usize(s: &str) -> Result<usize, String> {
        s.parse().map_err(|e| format!("Invalid usize: {}", e))
    }

    /// Parse duration from milliseconds
    pub fn parse_duration_ms(s: &str) -> Result<Duration, String> {
        let ms: u64 = s
            .parse()
            .map_err(|e| format!("Invalid duration ms: {}", e))?;
        Ok(Duration::from_millis(ms))
    }

    /// Parse duration from seconds
    pub fn parse_duration_secs(s: &str) -> Result<Duration, String> {
        let secs: u64 = s
            .parse()
            .map_err(|e| format!("Invalid duration secs: {}", e))?;
        Ok(Duration::from_secs(secs))
    }

    /// Parse optional values
    pub fn parse_optional<T, F, E>(s: &str, parser: F) -> Result<Option<T>, String>
    where
        F: FnOnce(&str) -> Result<T, E>,
        E: std::fmt::Display,
    {
        if s.is_empty() || s.to_lowercase() == "none" || s.to_lowercase() == "null" {
            Ok(None)
        } else {
            parser(s).map(Some).map_err(|e| e.to_string())
        }
    }

    /// Generic parser for types implementing FromStr
    pub fn parse_from_str<T>(s: &str) -> Result<T, String>
    where
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        s.parse().map_err(|e: T::Err| e.to_string())
    }
}

/// Example configuration using the builder pattern
#[cfg(test)]
#[allow(dead_code)]
mod example {
    use super::*;
    use std::time::Duration;

    impl_builder! {
        /// Example database configuration
        pub struct DatabaseConfig {
            /// Database host
            pub host: String = "localhost".to_string(),
            /// Database port
            pub port: u16 = 5432,
            /// Maximum connections
            pub max_connections: u32 = 100,
            /// Connection timeout
            pub timeout: Duration = Duration::from_secs(30),
            /// Enable SSL
            pub ssl_enabled: bool = false,
        }
    }

    impl_env_config! {
        DatabaseConfig {
            host: String = "DB_HOST" => parsers::parse_string,
            port: u16 = "DB_PORT" => parsers::parse_from_str,
            max_connections: u32 = "DB_MAX_CONNECTIONS" => parsers::parse_u32,
            timeout: Duration = "DB_TIMEOUT_SECS" => parsers::parse_duration_secs,
            ssl_enabled: bool = "DB_SSL_ENABLED" => parsers::parse_bool,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::example::*;
    use super::*;

    #[test]
    fn test_builder_pattern() {
        let config = DatabaseConfig::builder()
            .host("postgres.example.com".to_string())
            .port(5432)
            .max_connections(200)
            .ssl_enabled(true)
            .build();

        assert_eq!(config.host, "postgres.example.com");
        assert_eq!(config.port, 5432);
        assert_eq!(config.max_connections, 200);
        assert!(config.ssl_enabled);
    }

    #[test]
    fn test_builder_validation() {
        // Test custom validation using validated setters
        let result = DatabaseConfig::builder().host_validated("".to_string(), |host| {
            if host.is_empty() {
                Err("Host cannot be empty".to_string())
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Host cannot be empty"));
    }

    #[test]
    fn test_default_values() {
        let config = DatabaseConfig::builder().build();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.max_connections, 100);
    }
}
