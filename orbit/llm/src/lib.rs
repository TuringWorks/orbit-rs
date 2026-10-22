//! Provider-agnostic LLM and embedding layer for Orbit-RS.
//!
//! # What this is
//!
//! A model *gateway* that lives inside the database process. It gives Orbit-RS the capability set
//! an external LLM proxy would provide — a unified API over several providers, fallback chains,
//! retries, circuit breaking, timeouts, and cost accounting — without a separate deployment, and
//! with the retrieval layer, vector index, and generation call sharing one process and one security
//! boundary.
//!
//! # Design
//!
//! ```text
//!   caller ──▶ LlmRegistry ──▶ Router ──▶ provider
//!              (named,          (timeout,   (HTTP shaping
//!               hot-swappable    retry,      only)
//!               profiles)        breaker,
//!                                fallback,
//!                                accounting)
//! ```
//!
//! Providers do HTTP shaping and nothing else, so every resilience behavior is identical across
//! backends instead of reimplemented per backend. Four wire shapes — OpenAI, Anthropic, Ollama, and
//! generic OpenAI-compatible — cover roughly fifteen named services, because Azure OpenAI, vLLM,
//! Groq, Together, OpenRouter, LM Studio, DeepSeek, and Fireworks all speak `/chat/completions`.
//!
//! # Modelling honesty
//!
//! Three rules are enforced by the types rather than by convention, and each one exists because the
//! comfortable alternative reports a number nobody measured:
//!
//! * **Token counts are `Option`.** Most local servers do not report them. `unwrap_or(0)` would
//!   assert "this request used no tokens".
//! * **Cost is `Option`, computed only from configured prices.** No price table is bundled: one
//!   baked into a database binary goes stale silently and then reports confident wrong costs.
//! * **Credentials are [`SecretString`].** `Debug`, `Display`, and `Serialize` all redact.
//!
//! # Example
//!
//! ```no_run
//! use orbit_llm::{ChatRequest, LlmConfig, LlmRegistry, Router};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut config = LlmConfig::from_toml_str(r#"
//! enabled = true
//! default_profile = "local"
//!
//! [profiles.local]
//! provider = "ollama"
//! model = "llama3.2"
//! "#)?;
//! config.apply_env_overrides();
//!
//! let router = Router::new(Arc::new(LlmRegistry::from_config(&config)?));
//! let answer = router.generate(None, ChatRequest::prompt("Why is the sky blue?", None)).await?;
//! println!("{} (via {})", answer.text, answer.profile);
//! # Ok(())
//! # }
//! ```
//!
//! Switching models at runtime is a registry call — [`LlmRegistry::register`] to add one and
//! [`LlmRegistry::set_default`] to switch — and takes effect on the next request with no restart.
//! Over the wire that is `LLM.REGISTER` and `LLM.USE`.

#![deny(missing_docs)]

pub mod breaker;
pub mod compat;
pub mod config;
pub mod error;
pub mod http;
pub mod provider;
pub mod providers;
pub mod registry;
pub mod retry;
pub mod router;
pub mod secret;
pub mod types;
pub mod usage;

#[cfg(test)]
mod testing;

pub use breaker::{BreakerConfig, BreakerState, CircuitBreaker};
pub use compat::{legacy_provider_name, profile_from_legacy};
pub use config::{
    CompatibleFlavor, LlmConfig, ModelPricing, ModelProfile, ProviderConfig, ProviderSettings,
    DEFAULT_TIMEOUT_MS,
};
pub use error::{LlmError, LlmResult};
pub use provider::{
    EmbeddingProvider, LlmProvider, ProviderChatOutput, ProviderEmbeddingOutput, ProviderKind,
};
pub use providers::{build_provider, BuiltProvider};
pub use registry::{LlmRegistry, ModelSummary, RegisteredModel};
pub use retry::RetryPolicy;
pub use router::Router;
pub use secret::{SecretString, REDACTED};
pub use types::{
    ChatRequest, ChatResponse, Cost, EmbeddingRequest, EmbeddingResponse, FinishReason,
    GenerationParams, Message, Role, TokenUsage,
};
pub use usage::{ProfileCounters, UsageSnapshot};
