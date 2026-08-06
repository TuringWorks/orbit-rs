//! Conversion from the pre-existing `orbit_shared::graphrag::LLMProvider` into a
//! [`ModelProfile`].
//!
//! `LLMProvider` is public API, re-exported from `orbit_shared`'s crate root and referenced by the
//! RESP, Cypher, AQL, and PostgreSQL GraphRAG engines. Breaking it would ripple across four
//! protocol surfaces for no user-visible benefit, so it stays as the on-the-wire shape and converts
//! into a profile at the boundary.
//!
//! The conversion is where a long-standing defect is fixed. `create_llm_client` used to accept
//! `temperature` and `max_tokens` and immediately bind them to `_`, leaving the caller to re-derive
//! them with a second `match` over the same enum. Here they land in
//! [`crate::GenerationParams`] and are carried all the way to the request body — verified by the
//! provider tests, which assert every configured parameter appears on the wire.

use crate::config::{ModelProfile, ProviderConfig};
use crate::error::{LlmError, LlmResult};
use crate::secret::SecretString;
use crate::types::GenerationParams;
use orbit_shared::graphrag::LLMProvider;

/// Token cap applied to an Anthropic profile that arrives without one.
///
/// Anthropic's Messages API requires `max_tokens` and offers no server-side default, so a request
/// without one cannot be sent at all. The legacy enum makes it `Option`, so a value is needed here.
/// It is a documented constant rather than an invented per-model guess, and any caller that cares
/// sets `max_tokens` explicitly.
pub const ANTHROPIC_REQUIRED_MAX_TOKENS: u32 = 4_096;

/// Build a named [`ModelProfile`] from a legacy provider description.
///
/// # Errors
///
/// Returns [`LlmError::Configuration`] if the resulting profile is not usable — most often a
/// missing API key.
pub fn profile_from_legacy(
    name: impl Into<String>,
    provider: &LLMProvider,
) -> LlmResult<ModelProfile> {
    let name = name.into();

    let profile = match provider {
        LLMProvider::OpenAI {
            api_key,
            model,
            temperature,
            max_tokens,
        } => ModelProfile::new(
            name,
            ProviderConfig::OpenAi {
                api_key: SecretString::new(api_key.clone()),
                base_url: "https://api.openai.com/v1".to_string(),
                organization: None,
                project: None,
            },
            model.clone(),
        )
        .with_params(GenerationParams {
            temperature: *temperature,
            max_tokens: *max_tokens,
            ..Default::default()
        }),

        LLMProvider::Anthropic {
            api_key,
            model,
            temperature,
            max_tokens,
        } => ModelProfile::new(
            name,
            ProviderConfig::Anthropic {
                api_key: SecretString::new(api_key.clone()),
                base_url: "https://api.anthropic.com/v1".to_string(),
                version: "2023-06-01".to_string(),
            },
            model.clone(),
        )
        .with_params(GenerationParams {
            temperature: *temperature,
            max_tokens: Some(max_tokens.unwrap_or(ANTHROPIC_REQUIRED_MAX_TOKENS)),
            ..Default::default()
        }),

        LLMProvider::Ollama { model, temperature } => ModelProfile::new(
            name,
            ProviderConfig::Ollama {
                base_url: "http://localhost:11434".to_string(),
            },
            model.clone(),
        )
        .with_params(GenerationParams {
            temperature: *temperature,
            ..Default::default()
        }),

        LLMProvider::Local {
            endpoint,
            model,
            temperature,
            max_tokens,
        } => ModelProfile::new(
            name,
            ProviderConfig::Compatible {
                flavor: crate::config::CompatibleFlavor::Generic,
                api_key: None,
                base_url: strip_chat_completions(endpoint),
                api_version: None,
            },
            model.clone(),
        )
        .with_params(GenerationParams {
            temperature: *temperature,
            max_tokens: *max_tokens,
            ..Default::default()
        }),
    };

    profile.validate()?;
    Ok(profile)
}

impl TryFrom<&LLMProvider> for ModelProfile {
    type Error = LlmError;

    /// Convert using the provider's own name as the profile name.
    fn try_from(provider: &LLMProvider) -> Result<Self, Self::Error> {
        profile_from_legacy(legacy_provider_name(provider), provider)
    }
}

/// The conventional profile name for a legacy provider variant.
#[must_use]
pub fn legacy_provider_name(provider: &LLMProvider) -> &'static str {
    match provider {
        LLMProvider::OpenAI { .. } => "openai",
        LLMProvider::Anthropic { .. } => "anthropic",
        LLMProvider::Ollama { .. } => "ollama",
        LLMProvider::Local { .. } => "local",
    }
}

/// Normalize a legacy `Local` endpoint into a base URL.
///
/// The old `LocalLLMClient` POSTed to the configured endpoint verbatim, so existing configurations
/// carry a full `.../v1/chat/completions` URL. The compatible provider appends the route itself, so
/// the suffix is trimmed to avoid producing `/v1/chat/completions/chat/completions`.
fn strip_chat_completions(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    trimmed
        .strip_suffix("/chat/completions")
        .unwrap_or(trimmed)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;

    #[test]
    fn openai_parameters_survive_the_conversion() {
        let legacy = LLMProvider::OpenAI {
            api_key: "sk-legacy".into(),
            model: "gpt-4o-mini".into(),
            temperature: Some(0.35),
            max_tokens: Some(1500),
        };
        let profile = profile_from_legacy("openai", &legacy).expect("converts");

        assert_eq!(profile.provider.kind(), ProviderKind::OpenAi);
        assert_eq!(profile.model, "gpt-4o-mini");
        assert_eq!(
            profile.params.temperature,
            Some(0.35),
            "the old factory bound temperature to `_` and dropped it"
        );
        assert_eq!(profile.params.max_tokens, Some(1500));
    }

    #[test]
    fn anthropic_converts_instead_of_erroring() {
        // The pre-existing create_llm_client returned Err("Anthropic client not yet implemented")
        // for this exact input.
        let legacy = LLMProvider::Anthropic {
            api_key: "sk-ant-legacy".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: Some(0.2),
            max_tokens: Some(2048),
        };
        let profile = profile_from_legacy("anthropic", &legacy).expect("converts");

        assert_eq!(profile.provider.kind(), ProviderKind::Anthropic);
        assert_eq!(profile.params.max_tokens, Some(2048));
        assert_eq!(profile.params.temperature, Some(0.2));
    }

    #[test]
    fn anthropic_without_max_tokens_gets_the_documented_minimum() {
        let legacy = LLMProvider::Anthropic {
            api_key: "sk-ant".into(),
            model: "claude-sonnet-4-5".into(),
            temperature: None,
            max_tokens: None,
        };
        let profile = profile_from_legacy("anthropic", &legacy).expect("converts");
        assert_eq!(
            profile.params.max_tokens,
            Some(ANTHROPIC_REQUIRED_MAX_TOKENS),
            "the Messages API rejects a request without one"
        );
    }

    #[test]
    fn ollama_keeps_its_temperature_and_needs_no_credential() {
        let legacy = LLMProvider::Ollama {
            model: "llama3.2".into(),
            temperature: Some(0.7),
        };
        let profile = profile_from_legacy("ollama", &legacy).expect("converts");
        assert_eq!(profile.provider.kind(), ProviderKind::Ollama);
        assert_eq!(profile.params.temperature, Some(0.7));
    }

    #[test]
    fn a_legacy_local_endpoint_is_normalized_to_a_base_url() {
        let legacy = LLMProvider::Local {
            endpoint: "http://localhost:8000/v1/chat/completions".into(),
            model: "Qwen3-8B".into(),
            temperature: Some(0.1),
            max_tokens: Some(256),
        };
        let profile = profile_from_legacy("local", &legacy).expect("converts");
        assert_eq!(
            profile.provider.base_url(),
            "http://localhost:8000/v1",
            "the compatible provider appends the route itself"
        );
        assert_eq!(profile.params.max_tokens, Some(256));
    }

    #[test]
    fn a_local_endpoint_that_is_already_a_base_url_is_left_alone() {
        let legacy = LLMProvider::Local {
            endpoint: "http://localhost:8000/v1".into(),
            model: "m".into(),
            temperature: None,
            max_tokens: None,
        };
        let profile = profile_from_legacy("local", &legacy).expect("converts");
        assert_eq!(profile.provider.base_url(), "http://localhost:8000/v1");
    }

    #[test]
    fn a_credentialless_openai_provider_is_rejected_at_conversion() {
        let legacy = LLMProvider::OpenAI {
            api_key: String::new(),
            model: "gpt-4o".into(),
            temperature: None,
            max_tokens: None,
        };
        let err = profile_from_legacy("openai", &legacy).expect_err("no credential");
        assert!(err.to_string().contains("api_key"));
    }

    #[test]
    fn every_legacy_variant_has_a_conventional_name_and_converts() {
        // Guards the affordance audit: a variant with no arm here would be a configuration the
        // GraphRAG engines accept and the router cannot serve.
        let variants = [
            LLMProvider::OpenAI {
                api_key: "k".into(),
                model: "m".into(),
                temperature: None,
                max_tokens: None,
            },
            LLMProvider::Anthropic {
                api_key: "k".into(),
                model: "m".into(),
                temperature: None,
                max_tokens: None,
            },
            LLMProvider::Ollama {
                model: "m".into(),
                temperature: None,
            },
            LLMProvider::Local {
                endpoint: "http://localhost:8000/v1".into(),
                model: "m".into(),
                temperature: None,
                max_tokens: None,
            },
        ];

        let names: Vec<_> = variants.iter().map(legacy_provider_name).collect();
        assert_eq!(names, vec!["openai", "anthropic", "ollama", "local"]);

        for variant in &variants {
            let profile = ModelProfile::try_from(variant).expect("every variant converts");
            assert!(!profile.name.is_empty());
        }
    }
}
