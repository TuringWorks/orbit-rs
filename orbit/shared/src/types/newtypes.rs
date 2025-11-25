//! Newtype patterns for type safety and semantic clarity
//!
//! This module provides newtype wrappers around primitive types to prevent
//! accidental misuse and add semantic meaning to values.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Deref, DerefMut};

/// Macro to create a newtype wrapper with common trait implementations
#[macro_export]
macro_rules! define_newtype {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner_vis:vis $inner:ty);
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        $vis struct $name($inner_vis $inner);

        impl $name {
            /// Create a new instance
            pub fn new(value: $inner) -> Self {
                Self(value)
            }

            /// Get the inner value
            pub fn into_inner(self) -> $inner {
                self.0
            }

            /// Get a reference to the inner value
            pub fn as_inner(&self) -> &$inner {
                &self.0
            }
        }

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for $inner {
            fn from(wrapper: $name) -> Self {
                wrapper.into_inner()
            }
        }

        impl Deref for $name {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<$inner> for $name {
            fn as_ref(&self) -> &$inner {
                &self.0
            }
        }

        impl AsMut<$inner> for $name {
            fn as_mut(&mut self) -> &mut $inner {
                &mut self.0
            }
        }
    };

    // Variant with FromStr implementation
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner_vis:vis $inner:ty) with FromStr;
    ) => {
        define_newtype! {
            $(#[$meta])*
            $vis struct $name($inner_vis $inner);
        }

        impl FromStr for $name {
            type Err = <$inner as FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <$inner>::from_str(s).map(Self::new)
            }
        }
    };

    // Variant with Default implementation
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner_vis:vis $inner:ty) with Default($default:expr);
    ) => {
        define_newtype! {
            $(#[$meta])*
            $vis struct $name($inner_vis $inner);
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new($default)
            }
        }
    };

    // Variant with validation
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($inner_vis:vis $inner:ty) with validation($validator:expr);
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        $vis struct $name($inner_vis $inner);

        impl $name {
            /// Create a new instance with validation
            pub fn new(value: $inner) -> Result<Self, String> {
                let validator = $validator;
                validator(&value)?;
                Ok(Self(value))
            }

            /// Create without validation (use with caution)
            pub fn new_unchecked(value: $inner) -> Self {
                Self(value)
            }

            /// Get the inner value
            pub fn into_inner(self) -> $inner {
                self.0
            }

            /// Get a reference to the inner value
            pub fn as_inner(&self) -> &$inner {
                &self.0
            }
        }

        impl Deref for $name {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<$inner> for $name {
            fn as_ref(&self) -> &$inner {
                &self.0
            }
        }
    };
}

// ===== Common Newtypes =====

define_newtype! {
    /// Lease duration in seconds
    pub struct LeaseDurationSecs(pub u64);
}

define_newtype! {
    /// Connection timeout in milliseconds
    pub struct TimeoutMs(pub u64);
}

define_newtype! {
    /// Maximum number of connections
    pub struct MaxConnections(pub usize) with validation(|v: &usize| {
        if *v == 0 {
            Err("MaxConnections must be greater than 0".to_string())
        } else if *v > 10000 {
            Err("MaxConnections cannot exceed 10000".to_string())
        } else {
            Ok(())
        }
    });
}

define_newtype! {
    /// Port number
    pub struct Port(pub u16) with validation(|v: &u16| {
        if *v == 0 {
            Err("Port cannot be 0".to_string())
        } else {
            Ok(())
        }
    });
}

define_newtype! {
    /// Non-empty string
    pub struct NonEmptyString(pub String) with validation(|s: &String| {
        if s.trim().is_empty() {
            Err("String cannot be empty".to_string())
        } else {
            Ok(())
        }
    });
}

/// Percentage value (0.0 to 100.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Percentage(pub f64);

impl Percentage {
    pub fn new(value: f64) -> Result<Self, String> {
        if !(0.0..=100.0).contains(&value) {
            Err(format!("Percentage must be between 0.0 and 100.0, got {}", value))
        } else {
            Ok(Self(value))
        }
    }

    pub fn new_unchecked(value: f64) -> Self {
        Self(value)
    }

    pub fn into_inner(self) -> f64 {
        self.0
    }

    pub fn as_inner(&self) -> &f64 {
        &self.0
    }
}

impl Deref for Percentage {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Percentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

/// Normalized percentage (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NormalizedPercentage(pub f64);

impl NormalizedPercentage {
    pub fn new(value: f64) -> Result<Self, String> {
        if !(0.0..=1.0).contains(&value) {
            Err(format!("Normalized percentage must be between 0.0 and 1.0, got {}", value))
        } else {
            Ok(Self(value))
        }
    }

    pub fn new_unchecked(value: f64) -> Self {
        Self(value)
    }

    pub fn into_inner(self) -> f64 {
        self.0
    }

    pub fn as_inner(&self) -> &f64 {
        &self.0
    }
}

impl Deref for NormalizedPercentage {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for NormalizedPercentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

// ===== Smart Constructors =====

/// Helper trait for creating validated values
pub trait Validated: Sized {
    type Error: std::fmt::Display;

    /// Create a validated instance
    fn validated(value: Self) -> Result<Self, Self::Error>;
}

/// Helper trait for parsing and validating from strings
pub trait ParseValidated: Sized {
    type Error: std::fmt::Display;

    /// Parse and validate from string
    fn parse_validated(s: &str) -> Result<Self, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    define_newtype! {
        /// Test newtype
        pub struct TestId(pub String);
    }

    define_newtype! {
        /// Test newtype with default
        pub struct TestCount(pub u32) with Default(0);
    }

    #[test]
    fn test_newtype_basic() {
        let id = TestId::new("test-123".to_string());
        assert_eq!(id.as_inner(), "test-123");
        assert_eq!(id.into_inner(), "test-123");
    }

    #[test]
    fn test_newtype_with_default() {
        let count = TestCount::default();
        assert_eq!(*count, 0);
    }

    #[test]
    fn test_newtype_from() {
        let id: TestId = "test-456".to_string().into();
        assert_eq!(*id, "test-456");
    }

    #[test]
    fn test_max_connections_validation() {
        assert!(MaxConnections::new(0).is_err());
        assert!(MaxConnections::new(100).is_ok());
        assert!(MaxConnections::new(10001).is_err());
    }

    #[test]
    fn test_port_validation() {
        assert!(Port::new(0).is_err());
        assert!(Port::new(8080).is_ok());
        assert!(Port::new(65535).is_ok());
    }

    #[test]
    fn test_non_empty_string() {
        assert!(NonEmptyString::new("".to_string()).is_err());
        assert!(NonEmptyString::new("   ".to_string()).is_err());
        assert!(NonEmptyString::new("valid".to_string()).is_ok());
    }

    #[test]
    fn test_percentage() {
        assert!(Percentage::new(-1.0).is_err());
        assert!(Percentage::new(0.0).is_ok());
        assert!(Percentage::new(50.0).is_ok());
        assert!(Percentage::new(100.0).is_ok());
        assert!(Percentage::new(101.0).is_err());
    }

    #[test]
    fn test_normalized_percentage() {
        assert!(NormalizedPercentage::new(-0.1).is_err());
        assert!(NormalizedPercentage::new(0.0).is_ok());
        assert!(NormalizedPercentage::new(0.5).is_ok());
        assert!(NormalizedPercentage::new(1.0).is_ok());
        assert!(NormalizedPercentage::new(1.1).is_err());
    }

    #[test]
    fn test_deref() {
        let count = TestCount::new(42);
        assert_eq!(*count, 42);
        assert_eq!(count.to_string(), "42");
    }
}
