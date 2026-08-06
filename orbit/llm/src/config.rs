//! Configuration: provider settings, model profiles, and env layering.
//!
//! Per 12-factor III, config comes from the environment layered over `config/orbit-server.toml`.
//! Credentials in particular should arrive through env, never through a file in the repo — which is
//! why [`ProviderConfig`] holds [`SecretString`] and why [`LlmConfig::apply_env_overrides`] can
//! fill a profile's key from env without the file naming it at all.

use crate::breaker::BreakerConfig;
use crate::error::{LlmError, LlmResult};
use crate::provider::ProviderKind;
use crate::retry::RetryPolicy;
use crate::secret::SecretString;
use crate::types::{Cost, GenerationParams, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// Default request timeout. Unbounded was the pre-existing behavior, and it let a hung provider
/// pin a database query open indefinitely.
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Named variants of the OpenAI-compatible shape.
///
/// These differ only in authentication header and URL construction; the request and response bodies
/// are identical. Keeping them as a flavor rather than separate providers is what makes "support
/// another OpenAI-compatible service" a one-line change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CompatibleFlavor {
    /// `Authorization: Bearer`, `{base_url}/chat/completions`. Covers vLLM, Groq, Together,
    /// OpenRouter, LM Studio, DeepSeek, Fireworks, and any local OpenAI-compatible server.
    #[default]
    Generic,
    /// Azure OpenAI: `api-key` header, deployment in the path, `api-version` in the query string.
    AzureOpenAi,
}

impl CompatibleFlavor {
    /// Stable identifier for config and commands.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            CompatibleFlavor::Generic => "generic",
            CompatibleFlavor::AzureOpenAi => "azure_openai",
        }
    }

    /// Parse a flavor, including the service aliases that map onto it.
    pub fn parse(value: &str) -> LlmResult<Self> {
        match value.to_ascii_lowercase().as_str() {
            "azure" | "azure_openai" | "azureopenai" => Ok(CompatibleFlavor::AzureOpenAi),
            "generic" | "compatible" | "local" | "vllm" | "groq" | "together" | "openrouter"
            | "lmstudio" | "lm_studio" | "deepseek" | "fireworks" => Ok(CompatibleFlavor::Generic),
            other => Err(LlmError::configuration(format!(
                "unknown compatible flavor '{other}'; expected 'generic' or 'azure_openai'"
            ))),
        }
    }
}

/// How to reach a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProviderConfig {
    /// OpenAI's own API.
    OpenAi {
        /// API credential.
        #[serde(default)]
        api_key: SecretString,
        /// API root; override for a proxy or a pinned region.
        #[serde(default = "default_openai_base")]
        base_url: String,
        /// `OpenAI-Organization` header, when the account requires one.
        #[serde(default)]
        organization: Option<String>,
        /// `OpenAI-Project` header, when the account requires one.
        #[serde(default)]
        project: Option<String>,
    },

    /// Anthropic's Messages API.
    Anthropic {
        /// API credential.
        #[serde(default)]
        api_key: SecretString,
        /// API root.
        #[serde(default = "default_anthropic_base")]
        base_url: String,
        /// `anthropic-version` header. Pinned rather than tracking latest, because the Messages
        /// API's response shape is versioned and a silent bump would change parsing.
        #[serde(default = "default_anthropic_version")]
        version: String,
    },

    /// A local Ollama daemon.
    Ollama {
        /// Daemon root.
        #[serde(default = "default_ollama_base")]
        base_url: String,
    },

    /// Any OpenAI-compatible endpoint.
    Compatible {
        /// Header and URL convention to use.
        #[serde(default)]
        flavor: CompatibleFlavor,
        /// Credential; absent for servers that require no authentication.
        #[serde(default)]
        api_key: Option<SecretString>,
        /// API root.
        base_url: String,
        /// `api-version` query parameter, required by Azure OpenAI.
        #[serde(default)]
        api_version: Option<String>,
    },
}

fn default_openai_base() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_anthropic_base() -> String {
    "https://api.anthropic.com/v1".to_string()
}

fn default_anthropic_version() -> String {
    "2023-06-01".to_string()
}

fn default_ollama_base() -> String {
    "http://localhost:11434".to_string()
}

impl ProviderConfig {
    /// Which wire shape this configuration selects.
    #[must_use]
    pub const fn kind(&self) -> ProviderKind {
        match self {
            ProviderConfig::OpenAi { .. } => ProviderKind::OpenAi,
            ProviderConfig::Anthropic { .. } => ProviderKind::Anthropic,
            ProviderConfig::Ollama { .. } => ProviderKind::Ollama,
            ProviderConfig::Compatible { .. } => ProviderKind::Compatible,
        }
    }

    /// Endpoint root, for display in `LLM.INFO`.
    #[must_use]
    pub fn base_url(&self) -> &str {
        match self {
            ProviderConfig::OpenAi { base_url, .. }
            | ProviderConfig::Anthropic { base_url, .. }
            | ProviderConfig::Ollama { base_url }
            | ProviderConfig::Compatible { base_url, .. } => base_url,
        }
    }

    /// Build a default configuration for a provider kind.
    ///
    /// [`ProviderKind::Compatible`] has no defensible default base URL — an OpenAI-compatible
    /// server could be anywhere — so it is rejected here rather than guessed at.
    pub fn default_for(kind: ProviderKind) -> LlmResult<Self> {
        match kind {
            ProviderKind::OpenAi => Ok(ProviderConfig::OpenAi {
                api_key: SecretString::default(),
                base_url: default_openai_base(),
                organization: None,
                project: None,
            }),
            ProviderKind::Anthropic => Ok(ProviderConfig::Anthropic {
                api_key: SecretString::default(),
                base_url: default_anthropic_base(),
                version: default_anthropic_version(),
            }),
            ProviderKind::Ollama => Ok(ProviderConfig::Ollama {
                base_url: default_ollama_base(),
            }),
            ProviderKind::Compatible => Err(LlmError::configuration(
                "an OpenAI-compatible provider requires an explicit base_url",
            )),
        }
    }

    /// Overwrite the credential.
    pub fn set_api_key(&mut self, key: SecretString) {
        match self {
            ProviderConfig::OpenAi { api_key, .. } | ProviderConfig::Anthropic { api_key, .. } => {
                *api_key = key;
            }
            ProviderConfig::Compatible { api_key, .. } => *api_key = Some(key),
            // Ollama is a local daemon with no credential of its own.
            ProviderConfig::Ollama { .. } => {}
        }
    }

    /// Overwrite the endpoint root.
    pub fn set_base_url(&mut self, url: String) {
        match self {
            ProviderConfig::OpenAi { base_url, .. }
            | ProviderConfig::Anthropic { base_url, .. }
            | ProviderConfig::Ollama { base_url }
            | ProviderConfig::Compatible { base_url, .. } => *base_url = url,
        }
    }

    /// Validate that this configuration can actually be used.
    ///
    /// Catching a missing credential here means the failure surfaces at config load with a clear
    /// message, rather than as a 401 during a user's query.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::Configuration`] when a required field is empty.
    pub fn validate(&self) -> LlmResult<()> {
        match self {
            ProviderConfig::OpenAi { api_key, .. } if api_key.is_empty() => Err(
                LlmError::configuration("OpenAI provider requires an api_key (set OPENAI_API_KEY)"),
            ),
            ProviderConfig::Anthropic { api_key, .. } if api_key.is_empty() => {
                Err(LlmError::configuration(
                    "Anthropic provider requires an api_key (set ANTHROPIC_API_KEY)",
                ))
            }
            ProviderConfig::Compatible {
                flavor: CompatibleFlavor::AzureOpenAi,
                api_version,
                ..
            } if api_version.is_none() => Err(LlmError::configuration(
                "Azure OpenAI requires an api_version (for example '2024-10-21')",
            )),
            ProviderConfig::Compatible { base_url, .. } if base_url.is_empty() => Err(
                LlmError::configuration("OpenAI-compatible provider requires a base_url"),
            ),
            _ => Ok(()),
        }
    }
}

/// Per-token prices for a model, in USD per million tokens.
///
/// Only present when an operator configured it. Orbit-RS ships no built-in price table: a table
/// baked into a database binary goes stale silently, and a confidently wrong cost figure is worse
/// than an absent one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelPricing {
    /// USD per million input tokens.
    pub prompt_usd_per_million: f64,
    /// USD per million output tokens.
    pub completion_usd_per_million: f64,
}

impl ModelPricing {
    /// Compute the cost of a request.
    ///
    /// Returns `None` when the provider reported no usage at all — charging for a request whose
    /// token count nobody measured would be inventing a number.
    #[must_use]
    pub fn cost_of(&self, usage: &TokenUsage) -> Option<Cost> {
        if !usage.is_reported() {
            return None;
        }
        let per_token = |tokens: Option<u32>, per_million: f64| {
            f64::from(tokens.unwrap_or(0)) * per_million / 1_000_000.0
        };
        Some(Cost {
            prompt_usd: per_token(usage.prompt_tokens, self.prompt_usd_per_million),
            completion_usd: per_token(usage.completion_tokens, self.completion_usd_per_million),
        })
    }
}

/// A named, switchable model configuration.
///
/// This is the unit `LLM.USE` switches between and the unit a fallback chain is built from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelProfile {
    /// Profile name, unique within a registry.
    pub name: String,
    /// How to reach the provider.
    #[serde(flatten)]
    pub provider: ProviderConfig,
    /// Model identifier passed to the provider.
    pub model: String,
    /// Embedding model, when this profile is also used for embeddings.
    #[serde(default)]
    pub embedding_model: Option<String>,
    /// Default generation parameters, overridable per request.
    #[serde(default)]
    pub params: GenerationParams,
    /// Prices, when configured.
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    /// Profiles to try, in order, if this one fails.
    #[serde(default)]
    pub fallbacks: Vec<String>,
    /// Deadline for a single attempt.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Retry behavior for this profile.
    #[serde(default)]
    pub retry: RetryPolicy,
    /// Circuit breaker tuning for this profile.
    #[serde(default)]
    pub breaker: BreakerConfig,
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

impl ModelProfile {
    /// Build a profile with defaults for everything but the essentials.
    pub fn new(
        name: impl Into<String>,
        provider: ProviderConfig,
        model: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            provider,
            model: model.into(),
            embedding_model: None,
            params: GenerationParams::default(),
            pricing: None,
            fallbacks: Vec::new(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            retry: RetryPolicy::default(),
            breaker: BreakerConfig::default(),
        }
    }

    /// Set default generation parameters.
    #[must_use]
    pub fn with_params(mut self, params: GenerationParams) -> Self {
        self.params = params;
        self
    }

    /// Set the fallback chain.
    #[must_use]
    pub fn with_fallbacks(mut self, fallbacks: Vec<String>) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    /// Set the embedding model.
    #[must_use]
    pub fn with_embedding_model(mut self, model: impl Into<String>) -> Self {
        self.embedding_model = Some(model.into());
        self
    }

    /// Set prices.
    #[must_use]
    pub fn with_pricing(mut self, pricing: ModelPricing) -> Self {
        self.pricing = Some(pricing);
        self
    }

    /// Attempt deadline.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.max(1))
    }

    /// Validate the profile end to end.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::Configuration`] for an empty name or model, a self-referential fallback,
    /// or an unusable provider configuration.
    pub fn validate(&self) -> LlmResult<()> {
        if self.name.trim().is_empty() {
            return Err(LlmError::configuration("model profile requires a name"));
        }
        if self.model.trim().is_empty() {
            return Err(LlmError::configuration(format!(
                "profile '{}' requires a model", self.name
            )));
        }
        if self.fallbacks.iter().any(|f| f == &self.name) {
            return Err(LlmError::configuration(format!(
                "profile '{}' lists itself as a fallback, which would loop", self.name
            )));
        }
        self.provider.validate()
    }
}

/// The `[llm]` configuration section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// Whether the LLM subsystem is active.
    pub enabled: bool,
    /// Profile used when a request names none.
    pub default_profile: Option<String>,
    /// Profiles by name.
    ///
    /// `BTreeMap` rather than `HashMap` so `LLM.MODELS` and config dumps have a stable order —
    /// diffable output matters more here than lookup speed on a map of tens of entries.
    pub profiles: BTreeMap<String, ModelProfile>,
}

impl LlmConfig {
    /// Parse an `[llm]` section from TOML.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::Configuration`] if the document does not parse or a profile is invalid.
    pub fn from_toml_str(toml_str: &str) -> LlmResult<Self> {
        let mut config: LlmConfig = toml::from_str(toml_str)
            .map_err(|e| LlmError::configuration(format!("invalid [llm] configuration: {e}")))?;
        // The map key is authoritative: a profile keyed `[llm.profiles.fast]` is named `fast`,
        // whatever the body says, so the two cannot drift.
        for (key, profile) in config.profiles.iter_mut() {
            profile.name.clone_from(key);
        }
        Ok(config)
    }

    /// Layer environment variables over the parsed file.
    ///
    /// Recognized, in increasing precedence:
    ///
    /// | Variable | Effect |
    /// |---|---|
    /// | `OPENAI_API_KEY` | credential for every OpenAI profile lacking one |
    /// | `ANTHROPIC_API_KEY` | credential for every Anthropic profile lacking one |
    /// | `OLLAMA_HOST` | base URL for every Ollama profile |
    /// | `ORBIT_LLM_DEFAULT_PROFILE` | the default profile |
    /// | `ORBIT_LLM_<PROFILE>_API_KEY` | credential for one profile |
    /// | `ORBIT_LLM_<PROFILE>_BASE_URL` | base URL for one profile |
    /// | `ORBIT_LLM_<PROFILE>_MODEL` | model for one profile |
    ///
    /// `<PROFILE>` is the profile name uppercased with `-` and `.` replaced by `_`.
    pub fn apply_env_overrides(&mut self) {
        self.apply_env_overrides_from(&|key| std::env::var(key).ok());
    }

    /// Env layering against an injected lookup, so the precedence rules are testable without
    /// mutating the process environment (which races across parallel tests).
    pub fn apply_env_overrides_from(&mut self, lookup: &dyn Fn(&str) -> Option<String>) {
        let shared_openai = lookup("OPENAI_API_KEY");
        let shared_anthropic = lookup("ANTHROPIC_API_KEY");
        let ollama_host = lookup("OLLAMA_HOST");

        for (name, profile) in self.profiles.iter_mut() {
            let slug = env_slug(name);

            match &mut profile.provider {
                ProviderConfig::OpenAi { api_key, .. } => {
                    if api_key.is_empty() {
                        if let Some(shared) = &shared_openai {
                            *api_key = SecretString::new(shared.clone());
                        }
                    }
                }
                ProviderConfig::Anthropic { api_key, .. } => {
                    if api_key.is_empty() {
                        if let Some(shared) = &shared_anthropic {
                            *api_key = SecretString::new(shared.clone());
                        }
                    }
                }
                ProviderConfig::Ollama { base_url } => {
                    if let Some(host) = &ollama_host {
                        *base_url = host.clone();
                    }
                }
                ProviderConfig::Compatible { .. } => {}
            }

            // Per-profile variables win over the shared ones above.
            if let Some(key) = lookup(&format!("ORBIT_LLM_{slug}_API_KEY")) {
                profile.provider.set_api_key(SecretString::new(key));
            }
            if let Some(url) = lookup(&format!("ORBIT_LLM_{slug}_BASE_URL")) {
                profile.provider.set_base_url(url);
            }
            if let Some(model) = lookup(&format!("ORBIT_LLM_{slug}_MODEL")) {
                profile.model = model;
            }
        }

        if let Some(default) = lookup("ORBIT_LLM_DEFAULT_PROFILE") {
            self.default_profile = Some(default);
        }
    }

    /// Validate every profile and the default selection.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::Configuration`] if a profile is invalid, the default names a profile
    /// that does not exist, or a fallback points at an unregistered profile.
    pub fn validate(&self) -> LlmResult<()> {
        for profile in self.profiles.values() {
            profile.validate()?;
            // A fallback naming a profile that does not exist is a failover that will not fire —
            // exactly the kind of affordance that looks configured and does nothing.
            if let Some(missing) = profile
                .fallbacks
                .iter()
                .find(|f| !self.profiles.contains_key(*f))
            {
                return Err(LlmError::configuration(format!(
                    "profile '{}' falls back to '{missing}', which is not configured",
                    profile.name
                )));
            }
        }
        if let Some(default) = &self.default_profile {
            if !self.profiles.contains_key(default) {
                return Err(LlmError::configuration(format!(
                    "default_profile '{default}' is not configured"
                )));
            }
        }
        Ok(())
    }
}

/// Normalize a profile name into the env-var fragment it maps to.
fn env_slug(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' => c.to_ascii_uppercase(),
            'A'..='Z' | '0'..='9' => c,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
enabled = true
default_profile = "fast"

[profiles.fast]
provider = "ollama"
model = "llama3.2"
embedding_model = "nomic-embed-text"

[profiles.fast.params]
temperature = 0.3
max_tokens = 1024

[profiles.smart]
provider = "anthropic"
api_key = "sk-ant-from-file"
model = "claude-sonnet-4-5"
fallbacks = ["fast"]
timeout_ms = 45000

[profiles.smart.pricing]
prompt_usd_per_million = 3.0
completion_usd_per_million = 15.0
"#;

    fn sample() -> LlmConfig {
        LlmConfig::from_toml_str(SAMPLE).expect("sample config parses")
    }

    #[test]
    fn toml_parses_into_profiles() {
        let cfg = sample();
        assert!(cfg.enabled);
        assert_eq!(cfg.default_profile.as_deref(), Some("fast"));
        assert_eq!(cfg.profiles.len(), 2);

        let fast = &cfg.profiles["fast"];
        assert_eq!(fast.provider.kind(), ProviderKind::Ollama);
        assert_eq!(fast.model, "llama3.2");
        assert_eq!(fast.embedding_model.as_deref(), Some("nomic-embed-text"));
        assert_eq!(fast.params.temperature, Some(0.3));
        assert_eq!(fast.params.max_tokens, Some(1024));
        assert_eq!(fast.timeout_ms, DEFAULT_TIMEOUT_MS, "default applied");

        let smart = &cfg.profiles["smart"];
        assert_eq!(smart.provider.kind(), ProviderKind::Anthropic);
        assert_eq!(smart.fallbacks, vec!["fast".to_string()]);
        assert_eq!(smart.timeout_ms, 45_000);
    }

    #[test]
    fn profile_name_comes_from_the_map_key() {
        let cfg = sample();
        assert_eq!(cfg.profiles["fast"].name, "fast");
        assert_eq!(cfg.profiles["smart"].name, "smart");
    }

    #[test]
    fn sample_config_validates() {
        sample().validate().expect("sample config is valid");
    }

    #[test]
    fn env_fills_a_missing_credential() {
        let mut cfg = LlmConfig::from_toml_str(
            r#"
enabled = true
[profiles.gpt]
provider = "openai"
model = "gpt-4o-mini"
"#,
        )
        .expect("parses");

        assert!(cfg.profiles["gpt"].validate().is_err(), "no key yet");

        cfg.apply_env_overrides_from(&|key| match key {
            "OPENAI_API_KEY" => Some("sk-from-env".into()),
            _ => None,
        });

        cfg.profiles["gpt"]
            .validate()
            .expect("credential arrived from env");
    }

    #[test]
    fn per_profile_env_beats_shared_env_and_file() {
        let mut cfg = sample();
        cfg.apply_env_overrides_from(&|key| match key {
            "ANTHROPIC_API_KEY" => Some("sk-shared".into()),
            "ORBIT_LLM_SMART_API_KEY" => Some("sk-specific".into()),
            _ => None,
        });

        let ProviderConfig::Anthropic { api_key, .. } = &cfg.profiles["smart"].provider else {
            panic!("smart profile should be Anthropic");
        };
        assert_eq!(
            api_key.expose(),
            "sk-specific",
            "per-profile env must win over both the shared env var and the file value"
        );
    }

    #[test]
    fn shared_env_does_not_clobber_a_file_credential() {
        let mut cfg = sample();
        cfg.apply_env_overrides_from(&|key| match key {
            "ANTHROPIC_API_KEY" => Some("sk-shared".into()),
            _ => None,
        });

        let ProviderConfig::Anthropic { api_key, .. } = &cfg.profiles["smart"].provider else {
            panic!("smart profile should be Anthropic");
        };
        assert_eq!(
            api_key.expose(),
            "sk-ant-from-file",
            "the shared variable only fills gaps"
        );
    }

    #[test]
    fn env_overrides_base_url_model_and_default() {
        let mut cfg = sample();
        cfg.apply_env_overrides_from(&|key| match key {
            "OLLAMA_HOST" => Some("http://gpu-box:11434".into()),
            "ORBIT_LLM_FAST_MODEL" => Some("qwen3".into()),
            "ORBIT_LLM_DEFAULT_PROFILE" => Some("smart".into()),
            _ => None,
        });

        assert_eq!(cfg.profiles["fast"].provider.base_url(), "http://gpu-box:11434");
        assert_eq!(cfg.profiles["fast"].model, "qwen3");
        assert_eq!(cfg.default_profile.as_deref(), Some("smart"));
    }

    #[test]
    fn env_slug_normalizes_punctuation() {
        assert_eq!(env_slug("fast"), "FAST");
        assert_eq!(env_slug("gpt-4o-mini"), "GPT_4O_MINI");
        assert_eq!(env_slug("team.smart"), "TEAM_SMART");
    }

    #[test]
    fn dangling_fallback_is_rejected() {
        let cfg = LlmConfig::from_toml_str(
            r#"
enabled = true
[profiles.a]
provider = "ollama"
model = "llama3.2"
fallbacks = ["ghost"]
"#,
        )
        .expect("parses");

        let err = cfg.validate().expect_err("dangling fallback rejected");
        assert!(err.to_string().contains("'ghost'"), "got: {err}");
    }

    #[test]
    fn self_referential_fallback_is_rejected() {
        let profile = ModelProfile::new(
            "loop",
            ProviderConfig::Ollama {
                base_url: default_ollama_base(),
            },
            "llama3.2",
        )
        .with_fallbacks(vec!["loop".into()]);

        let err = profile.validate().expect_err("self-fallback rejected");
        assert!(err.to_string().contains("itself as a fallback"));
    }

    #[test]
    fn missing_default_profile_is_rejected() {
        let cfg = LlmConfig::from_toml_str(
            r#"
enabled = true
default_profile = "nope"
[profiles.a]
provider = "ollama"
model = "llama3.2"
"#,
        )
        .expect("parses");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn azure_requires_an_api_version() {
        let without = ProviderConfig::Compatible {
            flavor: CompatibleFlavor::AzureOpenAi,
            api_key: Some(SecretString::new("k")),
            base_url: "https://x.openai.azure.com".into(),
            api_version: None,
        };
        assert!(without.validate().is_err());

        let with = ProviderConfig::Compatible {
            flavor: CompatibleFlavor::AzureOpenAi,
            api_key: Some(SecretString::new("k")),
            base_url: "https://x.openai.azure.com".into(),
            api_version: Some("2024-10-21".into()),
        };
        with.validate().expect("api_version supplied");
    }

    #[test]
    fn compatible_has_no_guessed_default_base_url() {
        assert!(
            ProviderConfig::default_for(ProviderKind::Compatible).is_err(),
            "guessing where a compatible server lives would be a fabricated default"
        );
        for kind in [
            ProviderKind::OpenAi,
            ProviderKind::Anthropic,
            ProviderKind::Ollama,
        ] {
            ProviderConfig::default_for(kind).expect("well-known default exists");
        }
    }

    #[test]
    fn serialized_config_does_not_contain_the_credential() {
        let cfg = sample();
        let dumped = toml::to_string(&cfg).expect("config serializes");
        assert!(
            !dumped.contains("sk-ant-from-file"),
            "a config dump must not print a live credential:\n{dumped}"
        );
        assert!(dumped.contains(crate::secret::REDACTED));
    }

    #[test]
    fn pricing_is_absent_when_usage_is_unreported() {
        let pricing = ModelPricing {
            prompt_usd_per_million: 3.0,
            completion_usd_per_million: 15.0,
        };
        assert!(
            pricing.cost_of(&TokenUsage::default()).is_none(),
            "billing a request nobody counted invents a number"
        );

        let usage = TokenUsage {
            prompt_tokens: Some(1_000_000),
            completion_tokens: Some(1_000_000),
        };
        let cost = pricing.cost_of(&usage).expect("usage reported");
        assert!((cost.prompt_usd - 3.0).abs() < f64::EPSILON);
        assert!((cost.completion_usd - 15.0).abs() < f64::EPSILON);
        assert!((cost.total_usd() - 18.0).abs() < f64::EPSILON);
    }

    #[test]
    fn partial_usage_prices_only_the_reported_half() {
        let pricing = ModelPricing {
            prompt_usd_per_million: 3.0,
            completion_usd_per_million: 15.0,
        };
        let usage = TokenUsage {
            prompt_tokens: Some(1_000_000),
            completion_tokens: None,
        };
        let cost = pricing.cost_of(&usage).expect("prompt half reported");
        assert!((cost.prompt_usd - 3.0).abs() < f64::EPSILON);
        assert_eq!(cost.completion_usd, 0.0);
    }

    #[test]
    fn compatible_flavor_aliases_resolve() {
        assert_eq!(
            CompatibleFlavor::parse("azure").expect("parses"),
            CompatibleFlavor::AzureOpenAi
        );
        for alias in ["groq", "vllm", "together", "openrouter", "lmstudio"] {
            assert_eq!(
                CompatibleFlavor::parse(alias).expect("parses"),
                CompatibleFlavor::Generic,
                "alias {alias}"
            );
        }
        assert!(CompatibleFlavor::parse("bedrock").is_err());
    }

    #[test]
    fn ollama_ignores_credentials_because_it_has_none() {
        let mut provider = ProviderConfig::Ollama {
            base_url: default_ollama_base(),
        };
        provider.set_api_key(SecretString::new("ignored"));
        provider.validate().expect("still valid");
    }
}
