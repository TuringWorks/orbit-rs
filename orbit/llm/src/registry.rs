//! The registry of named, switchable model profiles.
//!
//! This is what makes "switch the model without restarting the server" true. A profile is
//! registered, replaced, or removed at runtime; the default can be reassigned by name; and every
//! read is a lock-free-enough `RwLock` read that clones an `Arc`.
//!
//! Reads happen on every request and writes only on a configuration change, so the lock is held
//! for the duration of a map lookup and never across an `await`.

use crate::breaker::{BreakerState, CircuitBreaker};
use crate::config::{LlmConfig, ModelProfile};
use crate::error::{LlmError, LlmResult};
use crate::providers::{build_provider, BuiltProvider};
use crate::usage::{ProfileCounters, UsageSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// A profile that has been resolved into a live provider.
///
/// The breaker and counters live here rather than in the router so they survive across requests and
/// are discarded together with the profile they describe.
pub struct RegisteredModel {
    /// The configuration this was built from.
    pub profile: ModelProfile,
    /// The live provider.
    pub provider: BuiltProvider,
    /// This profile's circuit breaker.
    pub breaker: CircuitBreaker,
    /// This profile's counters.
    pub counters: ProfileCounters,
}

impl std::fmt::Debug for RegisteredModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredModel")
            .field("profile", &self.profile)
            .field("breaker", &self.breaker.state())
            .finish_non_exhaustive()
    }
}

/// A profile as reported by `LLM.MODELS` / `LLM.INFO`.
///
/// Derived from the profile rather than holding it, so there is no path by which a credential
/// reaches a command response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSummary {
    /// Profile name.
    pub name: String,
    /// Provider shape.
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// Embedding model, when configured.
    pub embedding_model: Option<String>,
    /// Endpoint root.
    pub base_url: String,
    /// Whether this is the registry's default.
    pub is_default: bool,
    /// Fallback chain.
    pub fallbacks: Vec<String>,
    /// Attempt deadline.
    pub timeout_ms: u64,
    /// Whether prices are configured; `false` means cost will be reported as unknown.
    pub has_pricing: bool,
    /// Current breaker state.
    pub breaker_state: String,
    /// Counters for this profile.
    pub usage: UsageSnapshot,
}

#[derive(Default)]
struct RegistryInner {
    models: BTreeMap<String, Arc<RegisteredModel>>,
    default_profile: Option<String>,
}

/// Named model profiles, mutable at runtime.
#[derive(Default)]
pub struct LlmRegistry {
    inner: RwLock<RegistryInner>,
}

impl std::fmt::Debug for LlmRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmRegistry")
            .field("profiles", &self.profile_names())
            .field("default", &self.default_profile())
            .finish()
    }
}

impl LlmRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a registry from a validated configuration.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::Configuration`] if the configuration does not validate or a profile
    /// cannot be turned into a provider.
    pub fn from_config(config: &LlmConfig) -> LlmResult<Self> {
        config.validate()?;
        let registry = Self::new();
        for profile in config.profiles.values() {
            registry.register(profile.clone())?;
        }
        if let Some(default) = &config.default_profile {
            registry.set_default(default)?;
        }
        Ok(registry)
    }

    /// Add or replace a profile.
    ///
    /// Replacing resets the breaker and counters: the new configuration has not failed yet, and
    /// inheriting an open circuit would reject requests to a provider that was never called.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::Configuration`] if the profile is invalid.
    pub fn register(&self, profile: ModelProfile) -> LlmResult<()> {
        let provider = build_provider(&profile)?;
        let breaker = CircuitBreaker::new(profile.breaker.clone());
        let name = profile.name.clone();

        let model = Arc::new(RegisteredModel {
            profile,
            provider,
            breaker,
            counters: ProfileCounters::default(),
        });

        let mut inner = self.write();
        inner.models.insert(name.clone(), model);
        // First profile registered becomes the default, so a single-profile deployment needs no
        // separate default_profile setting.
        if inner.default_profile.is_none() {
            inner.default_profile = Some(name);
        }
        Ok(())
    }

    /// Register a profile with a caller-supplied provider.
    ///
    /// The seam used by tests and by any future in-process provider that is not built from HTTP
    /// configuration. The profile is still validated for name and fallback sanity.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::Configuration`] for an unnamed profile or a self-referential fallback.
    pub fn register_with_provider(
        &self,
        profile: ModelProfile,
        provider: BuiltProvider,
    ) -> LlmResult<()> {
        if profile.name.trim().is_empty() {
            return Err(LlmError::configuration("model profile requires a name"));
        }
        if profile.fallbacks.iter().any(|f| f == &profile.name) {
            return Err(LlmError::configuration(format!(
                "profile '{}' lists itself as a fallback, which would loop",
                profile.name
            )));
        }

        let breaker = CircuitBreaker::new(profile.breaker.clone());
        let name = profile.name.clone();
        let model = Arc::new(RegisteredModel {
            profile,
            provider,
            breaker,
            counters: ProfileCounters::default(),
        });

        let mut inner = self.write();
        inner.models.insert(name.clone(), model);
        if inner.default_profile.is_none() {
            inner.default_profile = Some(name);
        }
        Ok(())
    }

    /// Remove a profile.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::UnknownProfile`] if it is not registered, or
    /// [`LlmError::Configuration`] if another profile falls back to it — removing it would turn a
    /// configured failover into one that cannot fire.
    pub fn unregister(&self, name: &str) -> LlmResult<()> {
        let mut inner = self.write();
        if !inner.models.contains_key(name) {
            return Err(LlmError::UnknownProfile {
                name: name.to_owned(),
            });
        }
        if let Some(dependent) = inner
            .models
            .values()
            .find(|m| m.profile.fallbacks.iter().any(|f| f == name))
        {
            return Err(LlmError::configuration(format!(
                "cannot remove '{name}': profile '{}' falls back to it",
                dependent.profile.name
            )));
        }

        inner.models.remove(name);
        if inner.default_profile.as_deref() == Some(name) {
            // Promote deterministically rather than leaving the registry with no default: a
            // silently defaultless registry fails every unnamed request afterwards.
            inner.default_profile = inner.models.keys().next().cloned();
        }
        Ok(())
    }

    /// Make `name` the default profile.
    ///
    /// This is `LLM.USE`: the switch takes effect for the next request, with no restart.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::UnknownProfile`] if it is not registered.
    pub fn set_default(&self, name: &str) -> LlmResult<()> {
        let mut inner = self.write();
        if !inner.models.contains_key(name) {
            return Err(LlmError::UnknownProfile {
                name: name.to_owned(),
            });
        }
        inner.default_profile = Some(name.to_owned());
        Ok(())
    }

    /// The default profile's name, if one is set.
    #[must_use]
    pub fn default_profile(&self) -> Option<String> {
        self.read().default_profile.clone()
    }

    /// Resolve a profile by name, or the default when `name` is `None`.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::UnknownProfile`] or [`LlmError::NoDefaultProfile`].
    pub fn resolve(&self, name: Option<&str>) -> LlmResult<Arc<RegisteredModel>> {
        let inner = self.read();
        let name = match name {
            Some(name) => name.to_owned(),
            None => inner
                .default_profile
                .clone()
                .ok_or(LlmError::NoDefaultProfile)?,
        };
        inner
            .models
            .get(&name)
            .cloned()
            .ok_or(LlmError::UnknownProfile { name })
    }

    /// Registered profile names, in stable order.
    #[must_use]
    pub fn profile_names(&self) -> Vec<String> {
        self.read().models.keys().cloned().collect()
    }

    /// Whether a profile is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.read().models.contains_key(name)
    }

    /// Number of registered profiles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.read().models.len()
    }

    /// Whether the registry holds no profiles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Summaries of every profile, in stable order.
    #[must_use]
    pub fn summaries(&self) -> Vec<ModelSummary> {
        let inner = self.read();
        let default = inner.default_profile.clone();
        inner
            .models
            .values()
            .map(|model| summarize(model, default.as_deref()))
            .collect()
    }

    /// Summary of one profile.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::UnknownProfile`] if it is not registered.
    pub fn summary(&self, name: &str) -> LlmResult<ModelSummary> {
        let inner = self.read();
        let default = inner.default_profile.clone();
        inner
            .models
            .get(name)
            .map(|model| summarize(model, default.as_deref()))
            .ok_or_else(|| LlmError::UnknownProfile {
                name: name.to_owned(),
            })
    }

    /// Resolve `name`'s attempt order: the profile itself followed by its fallback chain.
    ///
    /// Chains are followed transitively and de-duplicated, so a cycle terminates instead of
    /// looping. Fallbacks naming an unregistered profile are skipped — the chain is best-effort at
    /// call time, while [`LlmConfig::validate`] rejects dangling references at load time.
    #[must_use]
    pub fn attempt_chain(&self, name: &str) -> Vec<Arc<RegisteredModel>> {
        let inner = self.read();
        let mut chain = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut queue = vec![name.to_owned()];

        while let Some(current) = queue.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            let Some(model) = inner.models.get(&current) else {
                continue;
            };
            chain.push(Arc::clone(model));
            // Reversed so the queue (a stack) pops them in declaration order.
            queue.extend(model.profile.fallbacks.iter().rev().cloned());
        }

        chain
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, RegistryInner> {
        // A poisoned lock means a writer panicked mid-update. The registry holds no invariant that
        // a panic could half-break — every mutation is a single map operation — so recovering is
        // strictly better than propagating a panic into every subsequent request.
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, RegistryInner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

fn summarize(model: &RegisteredModel, default: Option<&str>) -> ModelSummary {
    let profile = &model.profile;
    ModelSummary {
        name: profile.name.clone(),
        provider: profile.provider.kind().to_string(),
        model: profile.model.clone(),
        embedding_model: profile.embedding_model.clone(),
        base_url: profile.provider.base_url().to_owned(),
        is_default: default == Some(profile.name.as_str()),
        fallbacks: profile.fallbacks.clone(),
        timeout_ms: profile.timeout_ms,
        has_pricing: profile.pricing.is_some(),
        breaker_state: model.breaker.state().as_str().to_owned(),
        usage: model.counters.snapshot(),
    }
}

/// Breaker state for a profile, for `LLM.STATS`.
#[must_use]
pub fn breaker_state_of(model: &RegisteredModel) -> BreakerState {
    model.breaker.state()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderConfig;
    use crate::testing::stub_provider;

    fn profile(name: &str) -> ModelProfile {
        ModelProfile::new(
            name,
            ProviderConfig::Ollama {
                base_url: "http://localhost:11434".into(),
            },
            "llama3.2",
        )
    }

    fn registry_with(names: &[&str]) -> LlmRegistry {
        let registry = LlmRegistry::new();
        for name in names {
            registry.register(profile(name)).expect("registers");
        }
        registry
    }

    #[test]
    fn first_registration_becomes_the_default() {
        let registry = registry_with(&["a", "b"]);
        assert_eq!(registry.default_profile().as_deref(), Some("a"));
        assert_eq!(registry.len(), 2);
        assert!(!registry.is_empty());
    }

    #[test]
    fn switching_the_default_takes_effect_immediately() {
        let registry = registry_with(&["a", "b"]);
        registry.set_default("b").expect("b is registered");

        let resolved = registry.resolve(None).expect("default resolves");
        assert_eq!(
            resolved.profile.name, "b",
            "LLM.USE must change the next request's model without a restart"
        );
    }

    #[test]
    fn switching_to_an_unregistered_profile_is_rejected() {
        let registry = registry_with(&["a"]);
        let err = registry.set_default("ghost").expect_err("not registered");
        assert!(matches!(err, LlmError::UnknownProfile { .. }));
        assert_eq!(
            registry.default_profile().as_deref(),
            Some("a"),
            "a rejected switch must not clear the working default"
        );
    }

    #[test]
    fn re_registering_replaces_the_profile_in_place() {
        let registry = registry_with(&["a"]);
        let mut updated = profile("a");
        updated.model = "qwen3".into();
        registry.register(updated).expect("replaces");

        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.resolve(Some("a")).expect("resolves").profile.model,
            "qwen3"
        );
    }

    #[test]
    fn re_registering_clears_an_open_circuit() {
        let registry = registry_with(&["a"]);
        let before = registry.resolve(Some("a")).expect("resolves");
        for _ in 0..10 {
            before.breaker.record_failure();
        }
        assert_eq!(before.breaker.state(), BreakerState::Open);

        registry.register(profile("a")).expect("replaces");
        let after = registry.resolve(Some("a")).expect("resolves");
        assert_eq!(
            after.breaker.state(),
            BreakerState::Closed,
            "a reconfigured provider has not failed yet"
        );
    }

    #[test]
    fn resolve_reports_an_unknown_profile_by_name() {
        let registry = registry_with(&["a"]);
        let err = registry.resolve(Some("ghost")).expect_err("unknown");
        assert!(err.to_string().contains("'ghost'"));
    }

    #[test]
    fn an_empty_registry_has_no_default() {
        let registry = LlmRegistry::new();
        assert!(registry.is_empty());
        assert!(matches!(
            registry.resolve(None).expect_err("no default"),
            LlmError::NoDefaultProfile
        ));
    }

    #[test]
    fn removing_the_default_promotes_another_profile() {
        let registry = registry_with(&["a", "b"]);
        registry.unregister("a").expect("removes");

        assert_eq!(
            registry.default_profile().as_deref(),
            Some("b"),
            "leaving the registry defaultless would fail every unnamed request"
        );
    }

    #[test]
    fn removing_the_last_profile_leaves_no_default() {
        let registry = registry_with(&["a"]);
        registry.unregister("a").expect("removes");
        assert_eq!(registry.default_profile(), None);
        assert!(registry.is_empty());
    }

    #[test]
    fn a_profile_another_falls_back_to_cannot_be_removed() {
        let registry = LlmRegistry::new();
        registry.register(profile("backup")).expect("registers");
        registry
            .register(profile("primary").with_fallbacks(vec!["backup".into()]))
            .expect("registers");

        let err = registry.unregister("backup").expect_err("still referenced");
        assert!(err.to_string().contains("'primary' falls back to it"));
        assert!(registry.contains("backup"));
    }

    #[test]
    fn attempt_chain_follows_fallbacks_in_declaration_order() {
        let registry = LlmRegistry::new();
        registry.register(profile("c")).expect("registers");
        registry.register(profile("b")).expect("registers");
        registry
            .register(profile("a").with_fallbacks(vec!["b".into(), "c".into()]))
            .expect("registers");

        let chain: Vec<_> = registry
            .attempt_chain("a")
            .iter()
            .map(|m| m.profile.name.clone())
            .collect();
        assert_eq!(chain, vec!["a", "b", "c"]);
    }

    #[test]
    fn attempt_chain_is_transitive() {
        let registry = LlmRegistry::new();
        registry.register(profile("c")).expect("registers");
        registry
            .register(profile("b").with_fallbacks(vec!["c".into()]))
            .expect("registers");
        registry
            .register(profile("a").with_fallbacks(vec!["b".into()]))
            .expect("registers");

        let chain: Vec<_> = registry
            .attempt_chain("a")
            .iter()
            .map(|m| m.profile.name.clone())
            .collect();
        assert_eq!(chain, vec!["a", "b", "c"]);
    }

    #[test]
    fn a_fallback_cycle_terminates() {
        let registry = LlmRegistry::new();
        // Registered without cross-validation, then wired into a cycle — exactly the state a pair
        // of runtime LLM.REGISTER calls can produce.
        registry.register(profile("b")).expect("registers");
        registry
            .register(profile("a").with_fallbacks(vec!["b".into()]))
            .expect("registers");
        registry
            .register(profile("b").with_fallbacks(vec!["a".into()]))
            .expect("registers");

        let chain: Vec<_> = registry
            .attempt_chain("a")
            .iter()
            .map(|m| m.profile.name.clone())
            .collect();
        assert_eq!(chain, vec!["a", "b"], "each profile is attempted once");
    }

    #[test]
    fn attempt_chain_skips_unregistered_fallbacks() {
        let registry = LlmRegistry::new();
        registry
            .register_with_provider(
                profile("a").with_fallbacks(vec!["ghost".into()]),
                stub_provider(|_| Ok("ok".into())),
            )
            .expect("registers");

        let chain = registry.attempt_chain("a");
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn summaries_are_ordered_and_flag_the_default() {
        let registry = registry_with(&["zeta", "alpha"]);
        registry.set_default("zeta").expect("registered");

        let summaries = registry.summaries();
        assert_eq!(
            summaries
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"],
            "stable order makes LLM.MODELS output diffable"
        );
        assert!(summaries.iter().any(|s| s.is_default && s.name == "zeta"));
        assert_eq!(summaries.iter().filter(|s| s.is_default).count(), 1);
    }

    #[test]
    fn a_summary_carries_no_credential() {
        let registry = LlmRegistry::new();
        registry
            .register(ModelProfile::new(
                "openai",
                ProviderConfig::OpenAi {
                    api_key: crate::SecretString::new("sk-must-not-escape"),
                    base_url: "https://api.openai.com/v1".into(),
                    organization: None,
                    project: None,
                },
                "gpt-4o-mini",
            ))
            .expect("registers");

        let summary = registry.summary("openai").expect("exists");
        let rendered = serde_json::to_string(&summary).expect("serializes");
        assert!(!rendered.contains("sk-must-not-escape"), "got: {rendered}");
    }

    #[test]
    fn a_summary_reports_whether_cost_can_be_computed() {
        let registry = LlmRegistry::new();
        registry.register(profile("unpriced")).expect("registers");
        registry
            .register(profile("priced").with_pricing(crate::config::ModelPricing {
                prompt_usd_per_million: 1.0,
                completion_usd_per_million: 2.0,
            }))
            .expect("registers");

        assert!(!registry.summary("unpriced").expect("exists").has_pricing);
        assert!(registry.summary("priced").expect("exists").has_pricing);
    }

    #[test]
    fn from_config_registers_every_profile_and_the_default() {
        let config = LlmConfig::from_toml_str(
            r#"
enabled = true
default_profile = "second"

[profiles.first]
provider = "ollama"
model = "llama3.2"

[profiles.second]
provider = "ollama"
model = "qwen3"
"#,
        )
        .expect("parses");

        let registry = LlmRegistry::from_config(&config).expect("builds");
        assert_eq!(registry.profile_names(), vec!["first", "second"]);
        assert_eq!(registry.default_profile().as_deref(), Some("second"));
    }

    #[test]
    fn a_self_referential_fallback_is_refused_even_via_the_test_seam() {
        let registry = LlmRegistry::new();
        let err = registry
            .register_with_provider(
                profile("loop").with_fallbacks(vec!["loop".into()]),
                stub_provider(|_| Ok("ok".into())),
            )
            .expect_err("self-fallback rejected");
        assert!(err.to_string().contains("itself as a fallback"));
    }
}
