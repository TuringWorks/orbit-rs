//! Provider implementations and the dispatch that builds them from configuration.

pub mod anthropic;
pub mod compatible;
pub mod ollama;
pub mod openai;
pub mod openai_shape;

use crate::config::{ModelProfile, ProviderConfig};
use crate::error::LlmResult;
use crate::provider::{EmbeddingProvider, LlmProvider};
use std::sync::Arc;

/// A provider that can both generate and embed, type-erased for storage in the registry.
pub struct BuiltProvider {
    /// Text generation.
    pub llm: Arc<dyn LlmProvider>,
    /// Embeddings. Present for every kind; Anthropic's reports `Unsupported` when called.
    pub embedding: Arc<dyn EmbeddingProvider>,
}

impl std::fmt::Debug for BuiltProvider {
    /// Reports the wire shape only. The concrete providers hold credentials, and a derived `Debug`
    /// would be one `{:?}` away from printing them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltProvider")
            .field("kind", &self.llm.kind())
            .field("model", &self.llm.model())
            .finish()
    }
}

/// Construct the provider a profile describes.
///
/// Every [`crate::ProviderKind`] variant is reachable from here. A variant with no arm would be a
/// configuration option that parses and then does nothing.
///
/// # Errors
///
/// Returns [`crate::LlmError::Configuration`] when the profile does not validate.
pub fn build_provider(profile: &ModelProfile) -> LlmResult<BuiltProvider> {
    profile.validate()?;

    let built = match &profile.provider {
        ProviderConfig::OpenAi {
            api_key,
            base_url,
            organization,
            project,
        } => {
            let provider = Arc::new(openai::OpenAiProvider::new(
                api_key.clone(),
                base_url.clone(),
                organization.clone(),
                project.clone(),
                profile.model.clone(),
                profile.embedding_model.clone(),
            ));
            BuiltProvider {
                llm: provider.clone(),
                embedding: provider,
            }
        }

        ProviderConfig::Anthropic {
            api_key,
            base_url,
            version,
        } => {
            let provider = Arc::new(anthropic::AnthropicProvider::new(
                api_key.clone(),
                base_url.clone(),
                version.clone(),
                profile.model.clone(),
            ));
            BuiltProvider {
                llm: provider.clone(),
                embedding: provider,
            }
        }

        ProviderConfig::Ollama { base_url } => {
            let provider = Arc::new(ollama::OllamaProvider::new(
                base_url.clone(),
                profile.model.clone(),
                profile.embedding_model.clone(),
            ));
            BuiltProvider {
                llm: provider.clone(),
                embedding: provider,
            }
        }

        ProviderConfig::Compatible {
            flavor,
            api_key,
            base_url,
            api_version,
        } => {
            let provider = Arc::new(compatible::CompatibleProvider::new(
                *flavor,
                api_key.clone(),
                base_url.clone(),
                api_version.clone(),
                profile.model.clone(),
                profile.embedding_model.clone(),
            ));
            BuiltProvider {
                llm: provider.clone(),
                embedding: provider,
            }
        }
    };

    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CompatibleFlavor;
    use crate::provider::ProviderKind;
    use crate::secret::SecretString;
    use crate::types::GenerationParams;

    fn profile_for(provider: ProviderConfig, model: &str) -> ModelProfile {
        ModelProfile::new("p", provider, model).with_params(GenerationParams {
            // Anthropic requires this; harmless for the rest.
            max_tokens: Some(256),
            ..Default::default()
        })
    }

    #[test]
    fn every_provider_kind_is_constructible() {
        let cases = vec![
            (
                ProviderConfig::OpenAi {
                    api_key: SecretString::new("k"),
                    base_url: "https://api.openai.com/v1".into(),
                    organization: None,
                    project: None,
                },
                ProviderKind::OpenAi,
            ),
            (
                ProviderConfig::Anthropic {
                    api_key: SecretString::new("k"),
                    base_url: "https://api.anthropic.com/v1".into(),
                    version: "2023-06-01".into(),
                },
                ProviderKind::Anthropic,
            ),
            (
                ProviderConfig::Ollama {
                    base_url: "http://localhost:11434".into(),
                },
                ProviderKind::Ollama,
            ),
            (
                ProviderConfig::Compatible {
                    flavor: CompatibleFlavor::Generic,
                    api_key: None,
                    base_url: "http://localhost:8000/v1".into(),
                    api_version: None,
                },
                ProviderKind::Compatible,
            ),
        ];

        assert_eq!(
            cases.len(),
            ProviderKind::all().len(),
            "a kind with no construction site is a config option that does nothing"
        );

        for (config, expected) in cases {
            let built = build_provider(&profile_for(config, "m")).expect("provider builds");
            assert_eq!(built.llm.kind(), expected);
            assert_eq!(built.embedding.kind(), expected);
        }
    }

    #[test]
    fn an_invalid_profile_is_rejected_before_a_provider_is_built() {
        let profile = profile_for(
            ProviderConfig::OpenAi {
                api_key: SecretString::default(),
                base_url: "https://api.openai.com/v1".into(),
                organization: None,
                project: None,
            },
            "gpt-4o",
        );
        let err = build_provider(&profile).expect_err("missing credential rejected");
        assert!(err.to_string().contains("api_key"));
    }

    #[test]
    fn the_profiles_model_reaches_the_provider() {
        let profile = profile_for(
            ProviderConfig::Ollama {
                base_url: "http://localhost:11434".into(),
            },
            "qwen3:8b",
        );
        let built = build_provider(&profile).expect("builds");
        assert_eq!(built.llm.model(), "qwen3:8b");
    }
}
