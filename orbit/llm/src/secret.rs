//! Redacting wrapper for credentials.
//!
//! The pre-existing `orbit_shared::graphrag::LLMProvider` stored API keys in a plain `String` on a
//! `#[derive(Serialize)]` type, which means any config dump, `{:?}` log line, or error context
//! could print a live credential. [`SecretString`] closes that path: the value is only reachable
//! through [`SecretString::expose`], which is deliberately awkward to type and easy to grep for.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Marker written in place of a secret when serializing.
pub const REDACTED: &str = "***REDACTED***";

/// A string that will not print itself.
///
/// `Debug` and `Display` render [`REDACTED`]. `Serialize` also renders [`REDACTED`], so a config
/// round-trip through TOML/JSON cannot exfiltrate the value — a deserialized-then-reserialized
/// config is safe to log but is *not* a usable config, which is the intended trade.
///
/// # Examples
///
/// ```
/// use orbit_llm::SecretString;
///
/// let key = SecretString::new("sk-live-abc123");
/// assert_eq!(format!("{key:?}"), "***REDACTED***");
/// assert_eq!(key.expose(), "sk-live-abc123");
/// ```
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// Wrap a credential.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Read the underlying credential.
    ///
    /// Every call site is a place a secret can escape; keep them few and short-lived.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether the credential is empty.
    ///
    /// An empty credential is distinct from an absent one: absent is `Option::None`, empty is a
    /// configured-but-blank value, which is almost always a misconfiguration worth reporting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Serialize for SecretString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(REDACTED)
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_redact() {
        let secret = SecretString::new("sk-should-never-appear");
        assert_eq!(format!("{secret:?}"), REDACTED);
        assert_eq!(format!("{secret}"), REDACTED);
        assert!(!format!("{secret:?} {secret}").contains("should-never-appear"));
    }

    #[test]
    fn serialize_redacts_but_expose_does_not() {
        let secret = SecretString::new("sk-live");
        let json = serde_json::to_string(&secret).expect("secret serializes");
        assert_eq!(json, format!("\"{REDACTED}\""));
        assert_eq!(secret.expose(), "sk-live");
    }

    #[test]
    fn deserialize_accepts_plain_string() {
        let secret: SecretString = serde_json::from_str("\"sk-from-config\"").expect("parses");
        assert_eq!(secret.expose(), "sk-from-config");
    }

    #[test]
    fn nested_struct_debug_does_not_leak() {
        #[derive(Debug)]
        struct Config {
            api_key: SecretString,
        }
        let cfg = Config {
            api_key: SecretString::new("sk-nested"),
        };
        assert!(!format!("{cfg:?}").contains("sk-nested"));
    }
}
