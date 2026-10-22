//! Errors for the LLM layer.
//!
//! The important property here is [`LlmError::is_retryable`]: the router must not retry a 401, and
//! must retry a 429. Encoding that classification in the error type — rather than in the router's
//! `match` — keeps it in one place as providers are added.

use orbit_shared::OrbitError;
use std::time::Duration;

/// Result alias for LLM operations.
pub type LlmResult<T> = Result<T, LlmError>;

/// Failure modes of the LLM layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LlmError {
    /// The named model profile is not registered.
    #[error("model profile '{name}' is not registered")]
    UnknownProfile {
        /// Profile name that was requested.
        name: String,
    },

    /// No profile was named and no default is configured.
    #[error("no model profile requested and no default is configured")]
    NoDefaultProfile,

    /// Configuration is invalid or incomplete.
    #[error("LLM configuration error: {message}")]
    Configuration {
        /// What is wrong with the configuration.
        message: String,
    },

    /// The transport failed before an HTTP status was seen.
    #[error("transport error calling {provider}: {message}")]
    Transport {
        /// Provider being called.
        provider: String,
        /// Underlying transport failure.
        message: String,
    },

    /// The request exceeded its deadline.
    #[error("request to {provider} timed out after {}ms", .elapsed.as_millis())]
    Timeout {
        /// Provider being called.
        provider: String,
        /// How long was spent before giving up.
        elapsed: Duration,
    },

    /// The provider returned a non-success HTTP status.
    #[error("{provider} returned HTTP {status}: {body}")]
    Api {
        /// Provider being called.
        provider: String,
        /// HTTP status code.
        status: u16,
        /// Response body, truncated for logging.
        body: String,
        /// Server-suggested wait before retrying, when supplied via `Retry-After`.
        retry_after: Option<Duration>,
    },

    /// The provider's response did not match the shape its API documents.
    #[error("{provider} returned an unexpected response shape: {detail}")]
    MalformedResponse {
        /// Provider being called.
        provider: String,
        /// What was expected and not found.
        detail: String,
    },

    /// The circuit breaker for this provider is open.
    #[error("circuit breaker open for '{profile}'; {failures} consecutive failures")]
    CircuitOpen {
        /// Profile whose breaker is open.
        profile: String,
        /// Consecutive failures that tripped it.
        failures: u32,
    },

    /// Every profile in the fallback chain failed.
    ///
    /// Carries the chain that was attempted so operators can see the failover actually ran, rather
    /// than only the last error.
    #[error("all {} profiles failed: {}", .attempted.len(), .attempted.join(" -> "))]
    AllProvidersFailed {
        /// Profiles attempted, in order.
        attempted: Vec<String>,
        /// The final error encountered.
        last: Box<LlmError>,
    },

    /// A capability was requested that this provider does not offer.
    #[error("{provider} does not support {capability}")]
    Unsupported {
        /// Provider being called.
        provider: String,
        /// Capability requested.
        capability: &'static str,
    },
}

impl LlmError {
    /// Whether retrying this request could plausibly succeed.
    ///
    /// Retrying a `401` burns latency and quota to arrive at the same answer; retrying a `429` or a
    /// `503` is the whole point of having a retry policy. `408`/`409` are included because both are
    /// used by LLM gateways for transient contention.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::Transport { .. } | LlmError::Timeout { .. } => true,
            LlmError::Api { status, .. } => {
                matches!(status, 408 | 409 | 425 | 429 | 500 | 502 | 503 | 504)
            }
            LlmError::AllProvidersFailed { last, .. } => last.is_retryable(),
            LlmError::UnknownProfile { .. }
            | LlmError::NoDefaultProfile
            | LlmError::Configuration { .. }
            | LlmError::MalformedResponse { .. }
            | LlmError::CircuitOpen { .. }
            | LlmError::Unsupported { .. } => false,
        }
    }

    /// Whether this failure should count against the circuit breaker.
    ///
    /// A malformed *request* (400) says the caller is wrong, not that the provider is unhealthy;
    /// tripping a breaker on it would take a working provider out of service because one caller
    /// sent a bad prompt.
    #[must_use]
    pub fn indicates_provider_unhealthy(&self) -> bool {
        match self {
            LlmError::Transport { .. } | LlmError::Timeout { .. } => true,
            LlmError::Api { status, .. } => *status == 429 || *status >= 500,
            LlmError::MalformedResponse { .. } => true,
            LlmError::UnknownProfile { .. }
            | LlmError::NoDefaultProfile
            | LlmError::Configuration { .. }
            | LlmError::CircuitOpen { .. }
            | LlmError::AllProvidersFailed { .. }
            | LlmError::Unsupported { .. } => false,
        }
    }

    /// Server-suggested delay before the next attempt, when the provider supplied one.
    ///
    /// Absent means the provider said nothing — the caller should fall back to its own backoff
    /// rather than assume zero.
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            LlmError::Api { retry_after, .. } => *retry_after,
            LlmError::AllProvidersFailed { last, .. } => last.retry_after(),
            _ => None,
        }
    }

    /// Construct a configuration error.
    pub fn configuration(message: impl Into<String>) -> Self {
        LlmError::Configuration {
            message: message.into(),
        }
    }
}

impl From<LlmError> for OrbitError {
    fn from(err: LlmError) -> Self {
        match err {
            LlmError::Configuration { ref message } => OrbitError::ConfigurationError {
                message: message.clone(),
                key: Some("llm".to_string()),
            },
            LlmError::Timeout { ref provider, .. } => OrbitError::Timeout {
                operation: format!("llm::{provider}"),
            },
            LlmError::Transport { .. } => OrbitError::NetworkError(err.to_string()),
            other => OrbitError::internal_with_context(other.to_string(), "llm"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(status: u16) -> LlmError {
        LlmError::Api {
            provider: "test".into(),
            status,
            body: String::new(),
            retry_after: None,
        }
    }

    #[test]
    fn retryable_classification_is_table_driven() {
        let cases = [
            (400, false),
            (401, false),
            (403, false),
            (404, false),
            (408, true),
            (422, false),
            (429, true),
            (500, true),
            (502, true),
            (503, true),
            (504, true),
        ];
        for (status, expected) in cases {
            assert_eq!(
                api(status).is_retryable(),
                expected,
                "status {status} retryable classification"
            );
        }
    }

    #[test]
    fn client_errors_do_not_trip_the_breaker() {
        assert!(!api(400).indicates_provider_unhealthy());
        assert!(!api(401).indicates_provider_unhealthy());
        assert!(api(429).indicates_provider_unhealthy());
        assert!(api(503).indicates_provider_unhealthy());
    }

    #[test]
    fn transport_and_timeout_are_retryable_and_unhealthy() {
        let transport = LlmError::Transport {
            provider: "test".into(),
            message: "connection reset".into(),
        };
        assert!(transport.is_retryable());
        assert!(transport.indicates_provider_unhealthy());

        let timeout = LlmError::Timeout {
            provider: "test".into(),
            elapsed: Duration::from_secs(30),
        };
        assert!(timeout.is_retryable());
        assert!(timeout.indicates_provider_unhealthy());
    }

    #[test]
    fn circuit_open_is_not_retryable_at_this_layer() {
        let open = LlmError::CircuitOpen {
            profile: "p".into(),
            failures: 5,
        };
        assert!(!open.is_retryable());
        assert!(!open.indicates_provider_unhealthy());
    }

    #[test]
    fn all_providers_failed_delegates_to_last() {
        let err = LlmError::AllProvidersFailed {
            attempted: vec!["a".into(), "b".into()],
            last: Box::new(api(429)),
        };
        assert!(err.is_retryable());
        assert!(err.to_string().contains("a -> b"));
    }

    #[test]
    fn retry_after_is_absent_when_unreported() {
        assert_eq!(api(429).retry_after(), None);
        let with_hint = LlmError::Api {
            provider: "test".into(),
            status: 429,
            body: String::new(),
            retry_after: Some(Duration::from_secs(7)),
        };
        assert_eq!(with_hint.retry_after(), Some(Duration::from_secs(7)));
    }
}
