//! Provider traits and the provider-shape taxonomy.
//!
//! Orbit-RS deliberately implements four *shapes* rather than chasing a provider count. The
//! OpenAI-compatible shape alone covers Azure OpenAI, vLLM, Groq, Together, OpenRouter, LM Studio,
//! DeepSeek, and Fireworks, because all of them speak `/chat/completions`. Adding a genuinely new
//! shape behind [`LlmProvider`] is roughly eighty lines.

use crate::error::LlmResult;
use crate::types::{ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The wire shapes this build knows how to speak.
///
/// This enum is control flow, not decoration: every variant is constructed by
/// [`crate::config::ProviderConfig::kind`] and dispatched by
/// [`crate::providers::build_provider`]. A variant with no construction site would be a
/// configuration option that silently does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProviderKind {
    /// OpenAI's own API.
    #[serde(rename = "openai")]
    OpenAi,
    /// Anthropic's Messages API.
    Anthropic,
    /// Ollama's native API.
    Ollama,
    /// Any server exposing OpenAI-compatible `/chat/completions`.
    Compatible,
}

impl ProviderKind {
    /// Every shape this build supports, for `LLM.PROVIDERS`.
    #[must_use]
    pub const fn all() -> &'static [ProviderKind] {
        &[
            ProviderKind::OpenAi,
            ProviderKind::Anthropic,
            ProviderKind::Ollama,
            ProviderKind::Compatible,
        ]
    }

    /// Stable identifier used in config files and commands.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ProviderKind::OpenAi => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Ollama => "ollama",
            ProviderKind::Compatible => "compatible",
        }
    }

    /// Whether this shape can produce embeddings.
    ///
    /// Anthropic has no embeddings endpoint; a profile pointed at Anthropic will report
    /// [`crate::LlmError::Unsupported`] rather than quietly returning a zero vector.
    #[must_use]
    pub const fn supports_embeddings(self) -> bool {
        match self {
            ProviderKind::OpenAi | ProviderKind::Ollama | ProviderKind::Compatible => true,
            ProviderKind::Anthropic => false,
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ProviderKind {
    type Err = crate::error::LlmError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "openai" | "open_ai" => Ok(ProviderKind::OpenAi),
            "anthropic" | "claude" => Ok(ProviderKind::Anthropic),
            "ollama" => Ok(ProviderKind::Ollama),
            "compatible" | "openai_compatible" | "local" | "azure" | "azure_openai" | "vllm"
            | "groq" | "together" | "openrouter" | "lmstudio" | "deepseek" | "fireworks" => {
                Ok(ProviderKind::Compatible)
            }
            other => Err(crate::error::LlmError::configuration(format!(
                "unknown provider '{other}'; expected one of: openai, anthropic, ollama, compatible"
            ))),
        }
    }
}

/// A backend that can generate text.
///
/// Implementations own only HTTP shaping: no retries, no timeouts, no fallback. Those belong to
/// [`crate::router::Router`], so their behavior is identical across providers instead of being
/// reinvented per backend.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Wire shape this provider speaks.
    fn kind(&self) -> ProviderKind;

    /// Model identifier this provider was configured with.
    fn model(&self) -> &str;

    /// Perform one generation attempt.
    ///
    /// # Errors
    ///
    /// Returns [`crate::LlmError::Transport`] if the request never reached the provider,
    /// [`crate::LlmError::Api`] for a non-success status, and
    /// [`crate::LlmError::MalformedResponse`] if the body did not match the documented shape.
    async fn generate(&self, request: &ChatRequest) -> LlmResult<ProviderChatOutput>;
}

/// A backend that can produce embeddings.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Wire shape this provider speaks.
    fn kind(&self) -> ProviderKind;

    /// Embedding model identifier this provider was configured with.
    fn embedding_model(&self) -> &str;

    /// Perform one embedding attempt.
    ///
    /// # Errors
    ///
    /// As [`LlmProvider::generate`], plus [`crate::LlmError::Unsupported`] where the provider has
    /// no embeddings endpoint.
    async fn embed(&self, request: &EmbeddingRequest) -> LlmResult<ProviderEmbeddingOutput>;
}

/// What a provider returns before the router adds cost, profile identity, and failover history.
///
/// Kept separate from [`ChatResponse`] so providers cannot fabricate the fields only the router
/// can know — a provider has no way to report which fallbacks fired.
#[derive(Debug, Clone)]
pub struct ProviderChatOutput {
    /// Generated text.
    pub text: String,
    /// Model reported by the provider, falling back to the configured name.
    pub model: String,
    /// Token counts, where reported.
    pub usage: crate::types::TokenUsage,
    /// Stop reason, where reported.
    pub finish_reason: Option<crate::types::FinishReason>,
}

/// What an embedding provider returns before the router adds profile identity.
#[derive(Debug, Clone)]
pub struct ProviderEmbeddingOutput {
    /// Vectors, aligned with the request inputs.
    pub embeddings: Vec<Vec<f32>>,
    /// Model reported by the provider, falling back to the configured name.
    pub model: String,
    /// Token counts, where reported.
    pub usage: crate::types::TokenUsage,
}

/// A provider that can do both jobs, which is how every backend here is configured.
pub trait CompleteProvider: LlmProvider + EmbeddingProvider {}

impl<T: LlmProvider + EmbeddingProvider> CompleteProvider for T {}

/// Marker used by the router to attach the produced response to its originating profile.
#[allow(dead_code)]
pub(crate) fn finalize_chat(
    output: ProviderChatOutput,
    profile: String,
    cost: Option<crate::types::Cost>,
    latency: std::time::Duration,
    fallbacks_used: Vec<String>,
) -> ChatResponse {
    ChatResponse {
        text: output.text,
        model: output.model,
        profile,
        usage: output.usage,
        cost,
        finish_reason: output.finish_reason,
        latency,
        fallbacks_used,
    }
}

#[allow(dead_code)]
pub(crate) fn finalize_embedding(
    output: ProviderEmbeddingOutput,
    profile: String,
    latency: std::time::Duration,
) -> EmbeddingResponse {
    EmbeddingResponse {
        embeddings: output.embeddings,
        model: output.model,
        profile,
        usage: output.usage,
        latency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn every_kind_round_trips_through_its_string_form() {
        for kind in ProviderKind::all() {
            let parsed = ProviderKind::from_str(kind.as_str()).expect("kind parses");
            assert_eq!(parsed, *kind, "round trip for {kind}");
        }
    }

    #[test]
    fn named_services_map_onto_the_compatible_shape() {
        for alias in [
            "azure",
            "azure_openai",
            "vllm",
            "groq",
            "together",
            "openrouter",
            "lmstudio",
            "deepseek",
            "fireworks",
            "local",
        ] {
            assert_eq!(
                ProviderKind::from_str(alias).expect("alias parses"),
                ProviderKind::Compatible,
                "alias {alias}"
            );
        }
    }

    #[test]
    fn unknown_provider_is_rejected_at_parse_time() {
        let err = ProviderKind::from_str("cohere").expect_err("unknown provider rejected");
        assert!(err.to_string().contains("unknown provider 'cohere'"));
    }

    #[test]
    fn anthropic_declares_no_embedding_support() {
        assert!(!ProviderKind::Anthropic.supports_embeddings());
        assert!(ProviderKind::OpenAi.supports_embeddings());
        assert!(ProviderKind::Ollama.supports_embeddings());
        assert!(ProviderKind::Compatible.supports_embeddings());
    }

    #[test]
    fn all_lists_every_variant() {
        // Guards against a variant being added without being surfaced by LLM.PROVIDERS.
        assert_eq!(ProviderKind::all().len(), 4);
    }
}
