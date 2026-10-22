//! Shared HTTP client and response handling.
//!
//! The pre-existing GraphRAG clients called `reqwest::Client::new()` *inside* each `generate()`.
//! That builds a fresh connection pool and TLS session per LLM call — the connection is established
//! and thrown away every time. One process-wide client, cloned by handle, fixes it: `reqwest::Client`
//! is already `Arc`-backed, so cloning shares the pool.

use crate::error::{LlmError, LlmResult};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use std::sync::OnceLock;
use std::time::Duration;

/// How much of an error body to keep. Enough to identify the failure, bounded so a provider
/// returning an HTML error page does not put a megabyte into a log line.
const MAX_ERROR_BODY: usize = 2_048;

/// Idle connections kept per host. LLM traffic is bursty and long-lived; keeping a few warm
/// removes a TLS handshake from the critical path without pinning many sockets.
const POOL_IDLE_PER_HOST: usize = 8;

/// How long an idle connection is retained.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Ceiling on connection establishment, distinct from the per-request deadline.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

static SHARED: OnceLock<Client> = OnceLock::new();

/// The process-wide HTTP client.
///
/// Per-request deadlines are applied by the router with `tokio::time::timeout` rather than
/// `reqwest`'s own timeout, so the deadline covers the whole attempt — including body read — and is
/// configurable per profile rather than baked into the client.
///
/// # Panics
///
/// Never in practice: the builder only fails on TLS backend initialization, and a process that
/// cannot build a TLS client cannot serve any provider. If it does fail, a default client is used
/// so the failure surfaces as a connection error naming the host rather than as a panic at startup.
#[must_use]
pub fn shared_client() -> &'static Client {
    SHARED.get_or_init(|| {
        Client::builder()
            .pool_max_idle_per_host(POOL_IDLE_PER_HOST)
            .pool_idle_timeout(POOL_IDLE_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent(concat!("orbit-rs/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!(
                    error = %e,
                    "failed to build the shared HTTP client; falling back to defaults"
                );
                Client::new()
            })
    })
}

/// Convert a `reqwest` failure into a transport error naming the provider.
pub fn transport_error(provider: &str, err: reqwest::Error) -> LlmError {
    LlmError::Transport {
        provider: provider.to_owned(),
        message: err.to_string(),
    }
}

/// Turn a non-success response into an [`LlmError::Api`], preserving `Retry-After`.
///
/// `Retry-After` is read as seconds; the HTTP-date form is not parsed, and an unparseable value
/// yields `None` rather than a guessed delay — the router then falls back to its own backoff.
async fn api_error(provider: &str, response: Response) -> LlmError {
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs);

    let body = response
        .text()
        .await
        .unwrap_or_else(|e| format!("<error body unreadable: {e}>"));
    let body = truncate(&body, MAX_ERROR_BODY);

    LlmError::Api {
        provider: provider.to_owned(),
        status,
        body,
        retry_after,
    }
}

fn truncate(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    // Cut on a character boundary so the truncated body is still valid UTF-8.
    let boundary = (0..=limit)
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    format!(
        "{}… <{} bytes truncated>",
        &text[..boundary],
        text.len() - boundary
    )
}

/// Check the status and deserialize the body.
///
/// # Errors
///
/// Returns [`LlmError::Api`] for a non-success status and [`LlmError::MalformedResponse`] if the
/// body does not deserialize into `T`.
pub async fn parse_json<T: DeserializeOwned>(provider: &str, response: Response) -> LlmResult<T> {
    if !response.status().is_success() {
        return Err(api_error(provider, response).await);
    }
    // Read as text first: a provider that returns HTML on success (a captive portal, a
    // misconfigured proxy) produces a clear message instead of an opaque decode error.
    let body = response
        .text()
        .await
        .map_err(|e| transport_error(provider, e))?;

    serde_json::from_str(&body).map_err(|e| LlmError::MalformedResponse {
        provider: provider.to_owned(),
        detail: format!("{e}; body was: {}", truncate(&body, 512)),
    })
}

/// Report a field the provider's documented response shape should have contained.
pub fn missing_field(provider: &str, field: &str) -> LlmError {
    LlmError::MalformedResponse {
        provider: provider.to_owned(),
        detail: format!("response did not contain '{field}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_client_is_actually_shared() {
        let a = shared_client();
        let b = shared_client();
        assert!(
            std::ptr::eq(a, b),
            "a new client per call would rebuild the connection pool every request"
        );
    }

    #[test]
    fn truncate_leaves_short_text_alone() {
        assert_eq!(truncate("short", 100), "short");
    }

    #[test]
    fn truncate_bounds_long_text() {
        let long = "x".repeat(5_000);
        let out = truncate(&long, MAX_ERROR_BODY);
        assert!(out.len() < long.len());
        assert!(out.contains("bytes truncated"));
    }

    #[test]
    fn truncate_respects_utf8_boundaries() {
        // Multi-byte characters straddling the limit must not produce invalid UTF-8.
        let text = "é".repeat(2_000);
        let out = truncate(&text, MAX_ERROR_BODY);
        assert!(out.contains("bytes truncated"));
        assert!(out.is_char_boundary(0));
    }

    #[test]
    fn missing_field_names_the_field_and_provider() {
        let err = missing_field("anthropic", "content[0].text");
        assert!(err.to_string().contains("anthropic"));
        assert!(err.to_string().contains("content[0].text"));
        assert!(!err.is_retryable(), "a shape mismatch will not fix itself");
    }
}
