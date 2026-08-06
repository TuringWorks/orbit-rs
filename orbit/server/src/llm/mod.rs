//! Server-wide LLM runtime.
//!
//! Holds the [`LlmRegistry`] and [`Router`] that every AI surface — GraphRAG, the RESP `LLM.*`
//! commands, and future SQL/MCP entry points — shares, so a model registered or switched through
//! one surface is visible to all of them.
//!
//! # Why a process-global
//!
//! `GraphRAGActor` is a `Serialize`/`Deserialize` value type that is constructed per request today;
//! it cannot own a router. The pre-existing code worked around this by calling
//! `std::env::var("OPENAI_API_KEY")` inline in a RESP command handler with a hardcoded `"gpt-4"`.
//! A single lazily-initialized runtime replaces those scattered reads with one bootstrap that is
//! configurable, inspectable, and mutable at runtime.
//!
//! # Bootstrap order (12-factor III: config in the environment)
//!
//! 1. The `[llm]` section of the server config file, if one is found.
//! 2. Environment overrides layered on top ([`orbit_llm::LlmConfig::apply_env_overrides`]).
//! 3. Well-known credentials that name no profile of their own — `ANTHROPIC_API_KEY`,
//!    `OPENAI_API_KEY`, `OLLAMA_MODEL` — registered as conventional profiles. This preserves the
//!    behavior GraphRAG had before this module existed, so an existing deployment keeps working
//!    with no config file at all.

use orbit_llm::{LlmConfig, LlmRegistry, ModelProfile, ProviderConfig, Router, SecretString};
use std::sync::{Arc, OnceLock};
use tracing::{debug, info, warn};

/// Config-file locations searched when `ORBIT_LLM_CONFIG` is unset.
///
/// Mirrors the search order documented in `main.rs` so the LLM section is found in the same place
/// as the rest of the server configuration.
const CONFIG_SEARCH_PATHS: &[&str] = &[
    "./config/orbit-server.toml",
    "/app/config/orbit-server.toml",
    "/etc/orbit/orbit-server.toml",
];

/// Default model registered from `OLLAMA_MODEL`'s sibling variable when only a host is set.
const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";

/// The shared registry and router.
#[derive(Debug, Clone)]
pub struct LlmRuntime {
    registry: Arc<LlmRegistry>,
    router: Router,
}

impl LlmRuntime {
    /// Build a runtime over an existing registry.
    #[must_use]
    pub fn new(registry: Arc<LlmRegistry>) -> Self {
        let router = Router::new(Arc::clone(&registry));
        Self { registry, router }
    }

    /// The shared registry — the surface `LLM.REGISTER` / `LLM.USE` mutate.
    #[must_use]
    pub fn registry(&self) -> &Arc<LlmRegistry> {
        &self.registry
    }

    /// The router — the surface `LLM.GENERATE` / GraphRAG call.
    #[must_use]
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// Whether any model is configured.
    ///
    /// Callers should check this before offering an AI feature: a clear "no model configured"
    /// beats a per-request `NoDefaultProfile` error deep inside a query.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.registry.is_empty()
    }
}

static RUNTIME: OnceLock<LlmRuntime> = OnceLock::new();

/// The process-wide runtime, bootstrapped on first use.
///
/// Bootstrapping never fails: an unreadable config file or an invalid profile is logged and skipped
/// so the database still starts. A database that refuses to boot because an optional LLM
/// credential is malformed has turned a degraded feature into an outage.
#[must_use]
pub fn runtime() -> &'static LlmRuntime {
    RUNTIME.get_or_init(|| LlmRuntime::new(Arc::new(bootstrap_registry())))
}

/// Install a runtime explicitly, before anything calls [`runtime`].
///
/// Returns `false` if one is already installed — the first caller wins, and silently replacing a
/// live registry would strand any profile registered against it.
pub fn install(runtime: LlmRuntime) -> bool {
    RUNTIME.set(runtime).is_ok()
}

/// Build a registry from config file, environment, and well-known credentials.
fn bootstrap_registry() -> LlmRegistry {
    let mut config = load_config().unwrap_or_default();
    config.apply_env_overrides();

    let registry = LlmRegistry::new();

    register_config_profiles(&registry, &config);

    register_env_profiles(&registry, &config);

    // Applied after the env profiles so a configured default can point at one of them.
    if let Some(default) = &config.default_profile {
        if let Err(e) = registry.set_default(default) {
            warn!(profile = %default, error = %e, "configured default LLM profile is unusable");
        }
    }

    if registry.is_empty() {
        info!(
            "no LLM profile configured; AI features will report that no model is available \
             (set OPENAI_API_KEY, ANTHROPIC_API_KEY, or OLLAMA_MODEL, or add an [llm] section)"
        );
    } else {
        info!(
            profiles = ?registry.profile_names(),
            default = ?registry.default_profile(),
            "LLM runtime ready"
        );
    }

    registry
}

/// Register the profiles a config file declared, honoring its `enabled` flag.
///
/// `enabled = false` has to actually disable them. A flag that can be flipped without changing any
/// behavior is a decorative parameter, and this one reads as a kill switch. Environment credentials
/// are unaffected: exporting `OPENAI_API_KEY` is a deliberate act by whoever runs the process, not
/// a stale line in a checked-in file.
fn register_config_profiles(registry: &LlmRegistry, config: &LlmConfig) {
    if !config.enabled {
        if !config.profiles.is_empty() {
            info!(
                profiles = config.profiles.len(),
                "[llm] section has enabled = false; its profiles are not registered"
            );
        }
        return;
    }

    for profile in config.profiles.values() {
        match registry.register(profile.clone()) {
            Ok(()) => debug!(profile = %profile.name, "registered LLM profile from configuration"),
            Err(e) => warn!(
                profile = %profile.name,
                error = %e,
                "skipping LLM profile: it is not usable as configured"
            ),
        }
    }
}

/// Register conventional profiles for credentials found in the environment.
///
/// Skips any name the config file already claimed: an explicit profile is a deliberate choice and
/// must not be overwritten by an ambient variable.
fn register_env_profiles(registry: &LlmRegistry, config: &LlmConfig) {
    let candidates = [
        env_openai_profile(),
        env_anthropic_profile(),
        env_ollama_profile(),
    ];

    for profile in candidates.into_iter().flatten() {
        if config.profiles.contains_key(&profile.name) {
            debug!(
                profile = %profile.name,
                "environment credential ignored: the config file defines this profile"
            );
            continue;
        }
        let name = profile.name.clone();
        match registry.register(profile) {
            Ok(()) => info!(profile = %name, "registered LLM profile from the environment"),
            Err(e) => warn!(profile = %name, error = %e, "environment LLM profile is unusable"),
        }
    }
}

fn env_openai_profile() -> Option<ModelProfile> {
    let api_key = non_empty_env("OPENAI_API_KEY")?;
    Some(ModelProfile::new(
        "openai",
        ProviderConfig::OpenAi {
            api_key: SecretString::new(api_key),
            base_url: non_empty_env("OPENAI_BASE_URL")
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            organization: non_empty_env("OPENAI_ORG_ID"),
            project: non_empty_env("OPENAI_PROJECT_ID"),
        },
        // Previously hardcoded to "gpt-4" at the call site, which no deployment could change.
        non_empty_env("OPENAI_MODEL").unwrap_or_else(|| "gpt-4o-mini".to_string()),
    ))
}

fn env_anthropic_profile() -> Option<ModelProfile> {
    let api_key = non_empty_env("ANTHROPIC_API_KEY")?;
    Some(
        ModelProfile::new(
            "anthropic",
            ProviderConfig::Anthropic {
                api_key: SecretString::new(api_key),
                base_url: non_empty_env("ANTHROPIC_BASE_URL")
                    .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string()),
                version: non_empty_env("ANTHROPIC_VERSION")
                    .unwrap_or_else(|| "2023-06-01".to_string()),
            },
            non_empty_env("ANTHROPIC_MODEL").unwrap_or_else(|| "claude-sonnet-4-5".to_string()),
        )
        .with_params(orbit_llm::GenerationParams {
            // The Messages API requires a cap and offers no server-side default.
            max_tokens: Some(orbit_llm::compat::ANTHROPIC_REQUIRED_MAX_TOKENS),
            ..Default::default()
        }),
    )
}

fn env_ollama_profile() -> Option<ModelProfile> {
    // Either variable is enough: a host with no model named still points at a working daemon.
    let model = non_empty_env("OLLAMA_MODEL")
        .or_else(|| non_empty_env("OLLAMA_HOST").map(|_| DEFAULT_OLLAMA_MODEL.to_string()))?;

    let mut profile = ModelProfile::new(
        "ollama",
        ProviderConfig::Ollama {
            base_url: non_empty_env("OLLAMA_HOST")
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
        },
        model,
    );
    if let Some(embedding) = non_empty_env("OLLAMA_EMBEDDING_MODEL") {
        profile.embedding_model = Some(embedding);
    }
    Some(profile)
}

/// Read an environment variable, treating an empty value as absent.
///
/// An exported-but-empty variable is a misconfiguration, not a credential; accepting it produces a
/// profile that fails on its first request instead of at startup.
fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Load the `[llm]` section from the server config file, if one can be found and parsed.
fn load_config() -> Option<LlmConfig> {
    let path = config_path()?;
    let content = std::fs::read_to_string(&path)
        .inspect_err(|e| debug!(path = %path, error = %e, "no readable server config"))
        .ok()?;

    let document: toml::Value = toml::from_str(&content)
        .inspect_err(|e| warn!(path = %path, error = %e, "server config is not valid TOML"))
        .ok()?;

    let section = document.get("llm")?;
    let config: LlmConfig = section
        .clone()
        .try_into()
        .inspect_err(|e| warn!(path = %path, error = %e, "[llm] section is invalid; ignoring it"))
        .ok()?;

    info!(path = %path, profiles = config.profiles.len(), "loaded [llm] configuration");
    Some(normalize_profile_names(config))
}

/// Backfill each profile's name from its map key.
///
/// `LlmConfig::from_toml_str` does this for a whole document; the section is extracted by value
/// here, so the same normalization has to be applied.
fn normalize_profile_names(mut config: LlmConfig) -> LlmConfig {
    for (key, profile) in config.profiles.iter_mut() {
        profile.name.clone_from(key);
    }
    config
}

fn config_path() -> Option<String> {
    if let Some(explicit) = non_empty_env("ORBIT_LLM_CONFIG") {
        return Some(explicit);
    }
    CONFIG_SEARCH_PATHS
        .iter()
        .find(|path| std::path::Path::new(path).is_file())
        .map(|path| (*path).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_runtime_over_an_empty_registry_reports_itself_unconfigured() {
        let runtime = LlmRuntime::new(Arc::new(LlmRegistry::new()));
        assert!(!runtime.is_configured());
    }

    #[test]
    fn a_registered_profile_makes_the_runtime_configured() {
        let registry = LlmRegistry::new();
        registry
            .register(ModelProfile::new(
                "local",
                ProviderConfig::Ollama {
                    base_url: "http://localhost:11434".into(),
                },
                "llama3.2",
            ))
            .expect("registers");

        let runtime = LlmRuntime::new(Arc::new(registry));
        assert!(runtime.is_configured());
        assert_eq!(
            runtime.registry().default_profile().as_deref(),
            Some("local")
        );
    }

    #[test]
    fn the_router_and_registry_share_one_state() {
        let runtime = LlmRuntime::new(Arc::new(LlmRegistry::new()));
        runtime
            .registry()
            .register(ModelProfile::new(
                "later",
                ProviderConfig::Ollama {
                    base_url: "http://localhost:11434".into(),
                },
                "llama3.2",
            ))
            .expect("registers");

        assert!(
            runtime.router().registry().contains("later"),
            "a model registered through one surface must be visible to all of them"
        );
    }

    fn config_with(enabled: bool) -> LlmConfig {
        let mut config = LlmConfig::from_toml_str(
            r#"
[profiles.fast]
provider = "ollama"
model = "llama3.2"
"#,
        )
        .expect("parses");
        config.enabled = enabled;
        config
    }

    #[test]
    fn the_enabled_flag_actually_gates_registration() {
        let disabled = LlmRegistry::new();
        register_config_profiles(&disabled, &config_with(false));
        assert!(
            disabled.is_empty(),
            "a kill switch that registers the profiles anyway is a decorative parameter"
        );

        let enabled = LlmRegistry::new();
        register_config_profiles(&enabled, &config_with(true));
        assert_eq!(enabled.profile_names(), vec!["fast"]);
    }

    #[test]
    fn profile_names_are_backfilled_from_their_keys() {
        let section: toml::Value = toml::from_str(
            r#"
enabled = true
[profiles.fast]
provider = "ollama"
model = "llama3.2"
"#,
        )
        .expect("parses");

        let config: LlmConfig = section.try_into().expect("converts");
        let normalized = normalize_profile_names(config);
        assert_eq!(normalized.profiles["fast"].name, "fast");
    }

    #[test]
    fn an_exported_but_empty_variable_reads_as_absent() {
        // Uses a name no other test touches, so the process-wide mutation cannot race.
        let key = "ORBIT_LLM_TEST_EMPTY_VAR";
        // SAFETY: single-threaded within this test and the key is unique to it.
        unsafe {
            std::env::set_var(key, "   ");
        }
        assert_eq!(
            non_empty_env(key),
            None,
            "a blank value is not a credential"
        );
        unsafe {
            std::env::set_var(key, " value ");
        }
        assert_eq!(non_empty_env(key), Some("value".to_string()));
        unsafe {
            std::env::remove_var(key);
        }
        assert_eq!(non_empty_env(key), None);
    }
}
