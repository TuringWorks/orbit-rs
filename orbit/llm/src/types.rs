//! Request and response types shared by every provider.
//!
//! # Modelling honesty
//!
//! Two choices here are deliberate and worth reading before changing them:
//!
//! * [`TokenUsage`] fields are `Option`. Ollama and most local servers do not report token counts.
//!   Defaulting those to `0` would assert "this request used no tokens", which is a claim about the
//!   world that nobody measured. Absent stays absent.
//! * [`Cost`] is only produced when a price is *configured* for the model. Orbit-RS ships no
//!   built-in price table, because a price table baked into a database binary goes stale silently
//!   and then reports confident wrong numbers.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Who authored a message in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Role {
    /// Instructions that frame the whole exchange.
    System,
    /// Input from the caller.
    User,
    /// A prior model response.
    Assistant,
}

impl Role {
    /// Wire name used by OpenAI-shaped and Anthropic APIs alike.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// One turn in a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Author of this turn.
    pub role: Role,
    /// Message body.
    pub content: String,
}

impl Message {
    /// Build a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// Build a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Build an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// Generation parameters that override the profile's defaults for a single request.
///
/// Every field here is applied by every provider that supports it, and a provider that cannot
/// honor a field reports [`crate::LlmError::Unsupported`] rather than dropping it silently. A
/// parameter that can be removed without changing any output is not a parameter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationParams {
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// Upper bound on generated tokens.
    pub max_tokens: Option<u32>,
    /// Nucleus sampling cutoff.
    pub top_p: Option<f32>,
    /// Sequences that end generation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    /// Deterministic seed, where the provider supports one.
    pub seed: Option<u64>,
}

impl GenerationParams {
    /// Overlay `override_with` on top of `self`, field by field.
    ///
    /// Per-request values win; unset per-request fields inherit the profile default. `stop` is
    /// replaced rather than concatenated, because a caller that specifies stop sequences means
    /// *those* sequences, not those plus whatever the profile happened to carry.
    #[must_use]
    pub fn merged_with(&self, override_with: &GenerationParams) -> GenerationParams {
        GenerationParams {
            temperature: override_with.temperature.or(self.temperature),
            max_tokens: override_with.max_tokens.or(self.max_tokens),
            top_p: override_with.top_p.or(self.top_p),
            stop: if override_with.stop.is_empty() {
                self.stop.clone()
            } else {
                override_with.stop.clone()
            },
            seed: override_with.seed.or(self.seed),
        }
    }
}

/// A chat/completion request, provider-independent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Conversation turns, oldest first.
    pub messages: Vec<Message>,
    /// Per-request parameter overrides.
    #[serde(default)]
    pub params: GenerationParams,
}

impl ChatRequest {
    /// Build a single-turn request from a prompt, with an optional system message.
    pub fn prompt(prompt: impl Into<String>, system: Option<String>) -> Self {
        let messages = system
            .map(Message::system)
            .into_iter()
            .chain(std::iter::once(Message::user(prompt)))
            .collect();
        Self {
            messages,
            params: GenerationParams::default(),
        }
    }

    /// Apply parameter overrides, returning the modified request.
    #[must_use]
    pub fn with_params(mut self, params: GenerationParams) -> Self {
        self.params = params;
        self
    }

    /// The system message, if the caller supplied one.
    ///
    /// Anthropic takes the system prompt as a top-level field rather than a message, so providers
    /// need to split it out.
    #[must_use]
    pub fn system_message(&self) -> Option<&str> {
        self.messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.as_str())
    }

    /// The non-system turns, in order.
    pub fn conversation(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(|m| m.role != Role::System)
    }
}

/// Why generation stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FinishReason {
    /// The model finished naturally or hit a stop sequence.
    Stop,
    /// The token budget was exhausted before the model was done.
    Length,
    /// The provider's safety system intervened.
    ContentFilter,
    /// A reason the provider reported that does not map onto the above.
    Other(String),
}

impl FinishReason {
    /// Map a provider's wire value onto a [`FinishReason`].
    ///
    /// Unrecognized values are preserved verbatim in [`FinishReason::Other`] rather than collapsed
    /// into `Stop` — a response truncated for a reason we do not model should not read as complete.
    pub fn from_wire(value: &str) -> Self {
        match value {
            "stop" | "end_turn" | "stop_sequence" | "eos" => FinishReason::Stop,
            "length" | "max_tokens" | "model_length" => FinishReason::Length,
            "content_filter" | "refusal" => FinishReason::ContentFilter,
            other => FinishReason::Other(other.to_owned()),
        }
    }

    /// The normalized wire value for this reason.
    ///
    /// Lives here rather than at each call site because the enum is `#[non_exhaustive]`: an
    /// external crate matching on it needs a catch-all arm, which would silently absorb a new
    /// variant instead of failing to compile. Inside the crate the match stays exhaustive.
    #[must_use]
    pub fn as_wire(&self) -> &str {
        match self {
            FinishReason::Stop => "stop",
            FinishReason::Length => "length",
            FinishReason::ContentFilter => "content_filter",
            FinishReason::Other(other) => other,
        }
    }
}

impl std::fmt::Display for FinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_wire())
    }
}

/// Token counts for one request.
///
/// Every field is optional because not every provider reports them. See the module docs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Tokens consumed by the prompt.
    pub prompt_tokens: Option<u32>,
    /// Tokens produced by the model.
    pub completion_tokens: Option<u32>,
}

impl TokenUsage {
    /// Total tokens, when both halves are known.
    ///
    /// Returns `None` if either half is unreported: a partial sum presented as a total is a wrong
    /// number, and a wrong number in a billing column is worse than a missing one.
    #[must_use]
    pub fn total(&self) -> Option<u32> {
        self.prompt_tokens
            .zip(self.completion_tokens)
            .map(|(p, c)| p.saturating_add(c))
    }

    /// Whether the provider reported anything at all.
    #[must_use]
    pub fn is_reported(&self) -> bool {
        self.prompt_tokens.is_some() || self.completion_tokens.is_some()
    }
}

/// Money spent on one request, in USD.
///
/// Only produced when the model profile carries a configured price. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    /// Cost attributable to input tokens.
    pub prompt_usd: f64,
    /// Cost attributable to output tokens.
    pub completion_usd: f64,
}

impl Cost {
    /// Total spend for the request.
    #[must_use]
    pub fn total_usd(&self) -> f64 {
        self.prompt_usd + self.completion_usd
    }
}

/// A completed generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Generated text.
    pub text: String,
    /// Model that produced it, as reported by the provider where available.
    pub model: String,
    /// Profile name that was ultimately used — may differ from the requested one if a fallback
    /// fired.
    pub profile: String,
    /// Token counts, where reported.
    pub usage: TokenUsage,
    /// Computed spend, where the profile carries prices.
    pub cost: Option<Cost>,
    /// Why generation stopped, where reported.
    pub finish_reason: Option<FinishReason>,
    /// Wall-clock time for the successful attempt.
    #[serde(with = "duration_millis")]
    pub latency: Duration,
    /// Profiles tried and rejected before this one succeeded.
    ///
    /// Empty on the happy path. Non-empty means a failover fired, and a failover nobody can see is
    /// an outage nobody can see.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks_used: Vec<String>,
}

/// An embedding request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Inputs to embed. Batched in one call where the provider supports it.
    pub inputs: Vec<String>,
    /// Requested output dimensionality, where the provider supports truncation.
    pub dimensions: Option<u32>,
}

impl EmbeddingRequest {
    /// Build a request for a batch of inputs.
    pub fn new(inputs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            inputs: inputs.into_iter().map(Into::into).collect(),
            dimensions: None,
        }
    }
}

/// Embedding vectors, one per input, in input order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Vectors, aligned with [`EmbeddingRequest::inputs`].
    pub embeddings: Vec<Vec<f32>>,
    /// Model that produced them.
    pub model: String,
    /// Profile used.
    pub profile: String,
    /// Token counts, where reported.
    pub usage: TokenUsage,
    /// Wall-clock time for the successful attempt.
    #[serde(with = "duration_millis")]
    pub latency: Duration,
}

impl EmbeddingResponse {
    /// Dimensionality of the returned vectors, when at least one was returned.
    #[must_use]
    pub fn dimensions(&self) -> Option<usize> {
        self.embeddings.first().map(Vec::len)
    }
}

/// Serialize `Duration` as whole milliseconds, which is the resolution operators reason in.
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(value: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(value.as_millis() as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        u64::deserialize(d).map(Duration::from_millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_merge_prefers_per_request_values() {
        let profile = GenerationParams {
            temperature: Some(0.2),
            max_tokens: Some(1024),
            top_p: Some(0.9),
            stop: vec!["PROFILE".into()],
            seed: Some(7),
        };
        let request = GenerationParams {
            temperature: Some(0.9),
            max_tokens: None,
            top_p: None,
            stop: vec![],
            seed: None,
        };
        let merged = profile.merged_with(&request);

        assert_eq!(merged.temperature, Some(0.9), "request wins");
        assert_eq!(merged.max_tokens, Some(1024), "profile fills the gap");
        assert_eq!(merged.top_p, Some(0.9));
        assert_eq!(merged.stop, vec!["PROFILE".to_string()]);
        assert_eq!(merged.seed, Some(7));
    }

    #[test]
    fn stop_sequences_are_replaced_not_merged() {
        let profile = GenerationParams {
            stop: vec!["A".into(), "B".into()],
            ..Default::default()
        };
        let request = GenerationParams {
            stop: vec!["C".into()],
            ..Default::default()
        };
        assert_eq!(profile.merged_with(&request).stop, vec!["C".to_string()]);
    }

    #[test]
    fn prompt_helper_places_system_first() {
        let req = ChatRequest::prompt("hello", Some("be terse".into()));
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, Role::System);
        assert_eq!(req.messages[1].role, Role::User);
        assert_eq!(req.system_message(), Some("be terse"));
        assert_eq!(req.conversation().count(), 1);
    }

    #[test]
    fn prompt_helper_omits_absent_system_message() {
        let req = ChatRequest::prompt("hello", None);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.system_message(), None);
    }

    #[test]
    fn unreported_usage_has_no_total() {
        let partial = TokenUsage {
            prompt_tokens: Some(10),
            completion_tokens: None,
        };
        assert_eq!(partial.total(), None, "a partial sum is not a total");
        assert!(partial.is_reported());

        let none = TokenUsage::default();
        assert_eq!(none.total(), None);
        assert!(!none.is_reported());

        let full = TokenUsage {
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
        };
        assert_eq!(full.total(), Some(15));
    }

    #[test]
    fn finish_reason_preserves_unknown_wire_values() {
        assert_eq!(FinishReason::from_wire("stop"), FinishReason::Stop);
        assert_eq!(FinishReason::from_wire("end_turn"), FinishReason::Stop);
        assert_eq!(FinishReason::from_wire("max_tokens"), FinishReason::Length);
        assert_eq!(FinishReason::from_wire("length"), FinishReason::Length);
        assert_eq!(
            FinishReason::from_wire("tool_use"),
            FinishReason::Other("tool_use".into()),
            "an unmodelled stop reason must not read as a clean finish"
        );
    }

    #[test]
    fn role_wire_names_match_both_api_families() {
        assert_eq!(Role::System.as_str(), "system");
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Assistant.as_str(), "assistant");
    }
}
