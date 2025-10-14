//! Error handling utilities for reducing boilerplate across the codebase
//!
//! This module provides common error handling patterns and utilities to reduce
//! repetitive error handling code throughout the Rust codebase.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Result type alias for common error handling
pub type Result<T> = std::result::Result<T, Error>;

/// Common error types across the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// Configuration errors
    Config {
        message: String,
        key: Option<String>,
    },
    /// Network/IO errors
    Io {
        message: String,
        source: Option<String>,
    },
    /// Parsing/validation errors
    Parse {
        message: String,
        input: Option<String>,
        position: Option<usize>,
    },
    /// Database/storage errors
    Storage {
        message: String,
        operation: Option<String>,
    },
    /// Authentication/authorization errors
    Auth {
        message: String,
        user: Option<String>,
    },
    /// Generic application errors
    Application {
        message: String,
        context: Option<String>,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Config { message, key } => {
                write!(f, "Config error: {message}")?;
                if let Some(key) = key {
                    write!(f, " (key: {key})")?;
                }
                Ok(())
            }
            Error::Io { message, source } => {
                write!(f, "IO error: {message}")?;
                if let Some(source) = source {
                    write!(f, " (source: {source})")?;
                }
                Ok(())
            }
            Error::Parse {
                message,
                input,
                position,
            } => {
                write!(f, "Parse error: {message}")?;
                if let Some(position) = position {
                    write!(f, " at position {position}")?;
                }
                if let Some(input) = input {
                    write!(f, " in input: {input}")?;
                }
                Ok(())
            }
            Error::Storage { message, operation } => {
                write!(f, "Storage error: {message}")?;
                if let Some(operation) = operation {
                    write!(f, " (operation: {operation})")?;
                }
                Ok(())
            }
            Error::Auth { message, user } => {
                write!(f, "Auth error: {message}")?;
                if let Some(user) = user {
                    write!(f, " (user: {user})")?;
                }
                Ok(())
            }
            Error::Application { message, context } => {
                write!(f, "Application error: {message}")?;
                if let Some(context) = context {
                    write!(f, " (context: {context})")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Error handling macros to reduce boilerplate
#[macro_export]
macro_rules! config_error {
    ($msg:expr) => {
        $crate::error_utils::Error::Config {
            message: $msg.to_string(),
            key: None,
        }
    };
    ($msg:expr, $key:expr) => {
        $crate::error_utils::Error::Config {
            message: $msg.to_string(),
            key: Some($key.to_string()),
        }
    };
}

#[macro_export]
macro_rules! io_error {
    ($msg:expr) => {
        $crate::error_utils::Error::Io {
            message: $msg.to_string(),
            source: None,
        }
    };
    ($msg:expr, $source:expr) => {
        $crate::error_utils::Error::Io {
            message: $msg.to_string(),
            source: Some($source.to_string()),
        }
    };
}

#[macro_export]
macro_rules! parse_error {
    ($msg:expr) => {
        $crate::error_utils::Error::Parse {
            message: $msg.to_string(),
            input: None,
            position: None,
        }
    };
    ($msg:expr, $input:expr) => {
        $crate::error_utils::Error::Parse {
            message: $msg.to_string(),
            input: Some($input.to_string()),
            position: None,
        }
    };
    ($msg:expr, $input:expr, $pos:expr) => {
        $crate::error_utils::Error::Parse {
            message: $msg.to_string(),
            input: Some($input.to_string()),
            position: Some($pos),
        }
    };
}

/// Trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// Add context message to an error
    fn context(self, message: &str) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + 'static,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| Error::Application {
            message: f(),
            context: Some(e.to_string()),
        })
    }

    fn context(self, message: &str) -> Result<T> {
        self.with_context(|| message.to_string())
    }
}

/// Utility function to wrap std errors
pub fn wrap_std_error<E: std::error::Error>(error: E) -> Error {
    Error::Application {
        message: error.to_string(),
        context: None,
    }
}

/// Utility to create application errors with context
pub fn app_error(message: &str) -> Error {
    Error::Application {
        message: message.to_string(),
        context: None,
    }
}

/// Utility to create application errors with context
pub fn app_error_with_context(message: &str, context: &str) -> Error {
    Error::Application {
        message: message.to_string(),
        context: Some(context.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_macros() {
        let config_err = config_error!("Missing configuration");
        assert!(matches!(config_err, Error::Config { .. }));

        let io_err = io_error!("File not found", "test.txt");
        assert!(matches!(io_err, Error::Io { .. }));

        let parse_err = parse_error!("Invalid syntax", "SELECT * FROM", 10);
        assert!(matches!(parse_err, Error::Parse { .. }));
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
