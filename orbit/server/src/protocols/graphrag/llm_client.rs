//! GraphRAG's adapter onto the shared LLM runtime.
//!
//! This module used to carry three hand-rolled HTTP clients (OpenAI, Ollama, and a generic local
//! endpoint), an Anthropic branch that returned `Err("Anthropic client not yet implemented")`, and
//! a factory that accepted `temperature`/`max_tokens` and discarded them. All of that now lives in
//! [`orbit_llm`], which additionally provides the timeouts, retries, circuit breaking, fallback,
//! connection pooling, and cost accounting the hand-rolled clients had none of.
//!
//! What remains here is the translation between GraphRAG's request vocabulary and the router's.

use crate::llm::runtime;
use orbit_llm::{ChatRequest, GenerationParams, Message, ModelProfile, Router};
use orbit_shared::graphrag::LLMProvider;
use orbit_shared::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};

/// A GraphRAG generation request.
#[derive(Debug, Clone, Default)]
pub struct LLMGenerationRequest {
    /// Prompt text.
    pub prompt: String,
    /// Upper bound on generated tokens; falls back to the profile's setting.
    pub max_tokens: Option<u32>,
    /// Sampling temperature; falls back to the profile's setting.
    pub temperature: Option<f32>,
    /// System message, when the caller frames the exchange.
    pub system_message: Option<String>,
}

/// A GraphRAG generation response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMGenerationResponse {
    /// Generated text.
    pub text: String,
    /// Tokens used, when the provider reported them.
    ///
    /// Stays `None` for providers that do not report counts. A zero here would assert the request
    /// was free.
    pub tokens_used: Option<u32>,
    /// Why generation stopped, when reported.
    pub finish_reason: Option<String>,
    /// Model that answered, as reported by the provider.
    pub model: String,
    /// Profile that served the request.
    ///
    /// Differs from the requested profile when a fallback fired, so a caller can tell that its
    /// answer came from the backup model.
    pub profile: String,
}

impl From<LLMGenerationRequest> for ChatRequest {
    fn from(request: LLMGenerationRequest) -> Self {
        let messages = request
            .system_message
            .map(Message::system)
            .into_iter()
            .chain(std::iter::once(Message::user(request.prompt)))
            .collect();

        ChatRequest {
            messages,
            params: GenerationParams {
                temperature: request.temperature,
                max_tokens: request.max_tokens,
                ..Default::default()
            },
        }
    }
}

/// Generate through a named profile in the shared runtime.
///
/// `profile` names a registered model; `None` uses the runtime default. The router applies the
/// profile's timeout, retry policy, circuit breaker, and fallback chain.
///
/// # Errors
///
/// Returns [`OrbitError`] when no model is configured, the named profile is unknown, or every
/// profile in the fallback chain failed.
pub async fn generate(
    profile: Option<&str>,
    request: LLMGenerationRequest,
) -> OrbitResult<LLMGenerationResponse> {
    generate_with_router(runtime().router(), profile, request).await
}

/// Generate through an explicitly supplied router.
///
/// The seam that lets GraphRAG be exercised against a test router instead of the process-wide one.
///
/// # Errors
///
/// As [`generate`].
pub async fn generate_with_router(
    router: &Router,
    profile: Option<&str>,
    request: LLMGenerationRequest,
) -> OrbitResult<LLMGenerationResponse> {
    let response = router
        .generate(profile, request.into())
        .await
        .map_err(OrbitError::from)?;

    Ok(LLMGenerationResponse {
        text: response.text,
        tokens_used: response.usage.total(),
        finish_reason: response
            .finish_reason
            .map(|reason| reason.as_wire().to_string()),
        model: response.model,
        profile: response.profile,
    })
}

/// Register a legacy provider description into the shared runtime, returning its profile name.
///
/// GraphRAG actors carry `LLMProvider` values in their serialized state. Registering one makes it
/// routable without changing that representation.
///
/// # Errors
///
/// Returns [`OrbitError`] if the provider is not usable — most often a missing credential.
pub fn register_legacy_provider(name: &str, provider: &LLMProvider) -> OrbitResult<String> {
    let profile: ModelProfile =
        orbit_llm::profile_from_legacy(name, provider).map_err(OrbitError::from)?;
    let profile_name = profile.name.clone();
    runtime()
        .registry()
        .register(profile)
        .map_err(OrbitError::from)?;
    Ok(profile_name)
}

/// Whether a profile is registered in the shared runtime.
///
/// Lets a caller degrade cleanly — skipping an optional LLM step — instead of issuing a request
/// that is certain to fail.
#[must_use]
pub fn profile_is_available(name: &str) -> bool {
    runtime().registry().contains(name)
}

/// Resolve which profile a GraphRAG call should use.
///
/// Prefers, in order: the profile the query names, a legacy provider the actor carries under that
/// name (registered on demand), and finally the runtime default.
///
/// # Errors
///
/// Returns [`OrbitError`] when a named provider cannot be registered.
pub fn resolve_profile(
    requested: Option<&str>,
    legacy_providers: &std::collections::HashMap<String, LLMProvider>,
) -> OrbitResult<Option<String>> {
    let Some(name) = requested else {
        return Ok(None);
    };

    if runtime().registry().contains(name) {
        return Ok(Some(name.to_string()));
    }

    match legacy_providers.get(name) {
        Some(provider) => register_legacy_provider(name, provider).map(Some),
        // Not registered and not carried by the actor: hand the name to the router so the error
        // names the profile the caller actually asked for.
        None => Ok(Some(name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_llm::{ChatRequest as _ChatRequest, Role};

    fn request() -> LLMGenerationRequest {
        LLMGenerationRequest {
            prompt: "what is orbit?".into(),
            max_tokens: Some(256),
            temperature: Some(0.3),
            system_message: Some("answer from the graph".into()),
        }
    }

    #[test]
    fn a_graphrag_request_becomes_a_two_turn_chat() {
        let chat: _ChatRequest = request().into();

        assert_eq!(chat.messages.len(), 2);
        assert_eq!(chat.messages[0].role, Role::System);
        assert_eq!(chat.messages[0].content, "answer from the graph");
        assert_eq!(chat.messages[1].role, Role::User);
        assert_eq!(chat.messages[1].content, "what is orbit?");
    }

    #[test]
    fn generation_parameters_reach_the_request_instead_of_being_dropped() {
        let chat: _ChatRequest = request().into();
        assert_eq!(chat.params.temperature, Some(0.3));
        assert_eq!(chat.params.max_tokens, Some(256));
    }

    #[test]
    fn an_absent_system_message_produces_a_single_turn() {
        let chat: _ChatRequest = LLMGenerationRequest {
            prompt: "hi".into(),
            system_message: None,
            ..Default::default()
        }
        .into();

        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].role, Role::User);
    }

    #[test]
    fn unset_parameters_stay_unset_so_the_profile_can_supply_them() {
        let chat: _ChatRequest = LLMGenerationRequest {
            prompt: "hi".into(),
            ..Default::default()
        }
        .into();

        assert_eq!(chat.params.temperature, None);
        assert_eq!(chat.params.max_tokens, None);
    }
}
