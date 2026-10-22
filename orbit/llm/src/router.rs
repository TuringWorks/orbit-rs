//! The request path: resolve → breaker → timeout → retry → fallback → account.
//!
//! Providers do HTTP shaping and nothing else. Every resilience behavior lives here, so it is
//! identical across backends rather than reimplemented per backend — and testable against an
//! in-process stub rather than against a live API.
//!
//! Ordering matters and is deliberate:
//!
//! 1. **Breaker before attempt** — a known-dead provider costs zero, not one full timeout.
//! 2. **Timeout inside retry** — each attempt gets the full deadline; a slow first attempt does not
//!    consume the second attempt's budget.
//! 3. **Retry inside fallback** — transient faults are absorbed at the primary before paying the
//!    latency of switching providers.
//! 4. **Accounting after everything** — including which fallbacks fired, because a failover nobody
//!    can see is an outage nobody can see.

use crate::error::{LlmError, LlmResult};
use crate::registry::{LlmRegistry, RegisteredModel};
use crate::types::{ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Routes requests through the registry, applying resilience and accounting.
#[derive(Debug, Clone)]
pub struct Router {
    registry: Arc<LlmRegistry>,
}

impl Router {
    /// Build a router over a registry.
    #[must_use]
    pub fn new(registry: Arc<LlmRegistry>) -> Self {
        Self { registry }
    }

    /// The registry this router reads.
    #[must_use]
    pub fn registry(&self) -> &Arc<LlmRegistry> {
        &self.registry
    }

    /// Generate text, falling back through the chain if needed.
    ///
    /// `profile` names a registered profile; `None` uses the registry default. Parameters on
    /// `request` override the profile's defaults field by field.
    ///
    /// # Errors
    ///
    /// Returns [`LlmError::UnknownProfile`] or [`LlmError::NoDefaultProfile`] if resolution fails,
    /// and [`LlmError::AllProvidersFailed`] if every profile in the chain failed.
    pub async fn generate(
        &self,
        profile: Option<&str>,
        request: ChatRequest,
    ) -> LlmResult<ChatResponse> {
        let primary = self.registry.resolve(profile)?;
        let chain = self.registry.attempt_chain(&primary.profile.name);
        let mut attempted = Vec::with_capacity(chain.len());
        let mut last_error: Option<LlmError> = None;

        for model in &chain {
            let is_fallback = !attempted.is_empty();
            let name = model.profile.name.as_str();

            if !model.breaker.allows_request() {
                let failures = model.breaker.consecutive_failures();
                debug!(profile = name, failures, "skipping profile: circuit open");
                attempted.push(name.to_owned());
                last_error = Some(LlmError::CircuitOpen {
                    profile: name.to_owned(),
                    failures,
                });
                continue;
            }

            // The profile's defaults, overlaid with anything the caller specified.
            let effective = ChatRequest {
                messages: request.messages.clone(),
                params: model.profile.params.merged_with(&request.params),
            };

            let started = Instant::now();
            let llm = Arc::clone(&model.provider.llm);
            match attempt_with_retry(model, || {
                let effective = effective.clone();
                let llm = Arc::clone(&llm);
                async move { llm.generate(&effective).await }
            })
            .await
            {
                Ok(output) => {
                    let latency = started.elapsed();
                    let cost = model
                        .profile
                        .pricing
                        .and_then(|pricing| pricing.cost_of(&output.usage));

                    model
                        .counters
                        .record_success(&output.usage, cost, latency.as_millis() as u64);
                    if is_fallback {
                        model.counters.record_fallback_use();
                        // Attribute the failover to where the request started, so an operator
                        // reading the primary's stats sees that it is shedding traffic.
                        if let Some(first) = chain.first() {
                            first.counters.record_fallback_fired();
                        }
                        warn!(
                            requested = %primary.profile.name,
                            served_by = name,
                            skipped = ?attempted,
                            "LLM request served by a fallback profile"
                        );
                    }

                    return Ok(ChatResponse {
                        text: output.text,
                        model: output.model,
                        profile: name.to_owned(),
                        usage: output.usage,
                        cost,
                        finish_reason: output.finish_reason,
                        latency,
                        fallbacks_used: attempted,
                    });
                }
                Err(err) => {
                    model.counters.record_failure();
                    warn!(profile = name, error = %err, "LLM attempt failed");
                    attempted.push(name.to_owned());
                    last_error = Some(err);
                }
            }
        }

        Err(finish_failed(attempted, last_error, &primary.profile.name))
    }

    /// Produce embeddings, falling back through the chain if needed.
    ///
    /// # Errors
    ///
    /// As [`Router::generate`], plus [`LlmError::Unsupported`] surfacing from a provider with no
    /// embeddings endpoint.
    pub async fn embed(
        &self,
        profile: Option<&str>,
        request: EmbeddingRequest,
    ) -> LlmResult<EmbeddingResponse> {
        let primary = self.registry.resolve(profile)?;
        let chain = self.registry.attempt_chain(&primary.profile.name);
        let mut attempted = Vec::with_capacity(chain.len());
        let mut last_error: Option<LlmError> = None;

        for model in &chain {
            let name = model.profile.name.as_str();

            if !model.breaker.allows_request() {
                attempted.push(name.to_owned());
                last_error = Some(LlmError::CircuitOpen {
                    profile: name.to_owned(),
                    failures: model.breaker.consecutive_failures(),
                });
                continue;
            }

            let started = Instant::now();
            let embedder = Arc::clone(&model.provider.embedding);
            match attempt_with_retry(model, || {
                let request = request.clone();
                let embedder = Arc::clone(&embedder);
                async move { embedder.embed(&request).await }
            })
            .await
            {
                Ok(output) => {
                    let latency = started.elapsed();
                    model
                        .counters
                        .record_success(&output.usage, None, latency.as_millis() as u64);
                    if !attempted.is_empty() {
                        model.counters.record_fallback_use();
                        warn!(
                            requested = %primary.profile.name,
                            served_by = name,
                            "embedding request served by a fallback profile"
                        );
                    }
                    return Ok(EmbeddingResponse {
                        embeddings: output.embeddings,
                        model: output.model,
                        profile: name.to_owned(),
                        usage: output.usage,
                        latency,
                    });
                }
                Err(err) => {
                    model.counters.record_failure();
                    attempted.push(name.to_owned());
                    last_error = Some(err);
                }
            }
        }

        Err(finish_failed(attempted, last_error, &primary.profile.name))
    }
}

/// Collapse an exhausted chain into one error.
///
/// A single-profile failure reports its own error unwrapped — wrapping it in "all 1 profiles
/// failed" adds noise without adding information.
fn finish_failed(attempted: Vec<String>, last_error: Option<LlmError>, primary: &str) -> LlmError {
    let last = last_error.unwrap_or_else(|| LlmError::UnknownProfile {
        name: primary.to_owned(),
    });
    if attempted.len() <= 1 {
        return last;
    }
    LlmError::AllProvidersFailed {
        attempted,
        last: Box::new(last),
    }
}

/// Run one profile's attempts: timeout per attempt, retry per policy, breaker updated throughout.
async fn attempt_with_retry<T, F, Fut>(model: &RegisteredModel, mut call: F) -> LlmResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = LlmResult<T>>,
{
    let policy = &model.profile.retry;
    let timeout = model.profile.timeout();
    let provider_name = model.profile.provider.kind().to_string();
    let mut attempts = 0_u32;

    loop {
        attempts += 1;
        let outcome = run_once(&provider_name, timeout, call()).await;

        match outcome {
            Ok(value) => {
                model.breaker.record_success();
                return Ok(value);
            }
            Err(err) => {
                if err.indicates_provider_unhealthy() {
                    model.breaker.record_failure();
                }
                if !err.is_retryable() || !policy.should_retry(attempts) {
                    return Err(err);
                }
                let delay = policy.delay_for_attempt(attempts, err.retry_after());
                debug!(
                    profile = %model.profile.name,
                    attempt = attempts,
                    delay_ms = delay.as_millis() as u64,
                    error = %err,
                    "retrying LLM request"
                );
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

/// Apply the per-attempt deadline.
async fn run_once<T>(
    provider: &str,
    timeout: Duration,
    future: impl std::future::Future<Output = LlmResult<T>>,
) -> LlmResult<T> {
    let started = Instant::now();
    match tokio::time::timeout(timeout, future).await {
        Ok(result) => result,
        Err(_) => Err(LlmError::Timeout {
            provider: provider.to_owned(),
            elapsed: started.elapsed(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::breaker::BreakerConfig;
    use crate::config::{ModelPricing, ModelProfile, ProviderConfig};
    use crate::retry::RetryPolicy;
    use crate::testing::{api_error, stub_with_handle, StubProvider};
    use crate::types::{GenerationParams, TokenUsage};

    fn base_profile(name: &str) -> ModelProfile {
        ModelProfile {
            retry: RetryPolicy::none(),
            timeout_ms: 500,
            ..ModelProfile::new(
                name,
                ProviderConfig::Ollama {
                    base_url: "http://localhost:11434".into(),
                },
                "stub",
            )
        }
    }

    fn router_with(entries: Vec<(ModelProfile, StubProvider)>) -> (Router, Vec<Arc<StubProvider>>) {
        let registry = Arc::new(LlmRegistry::new());
        let handles = entries
            .into_iter()
            .map(|(profile, stub)| {
                let (built, handle) = stub_with_handle(stub);
                registry
                    .register_with_provider(profile, built)
                    .expect("registers");
                handle
            })
            .collect();
        (Router::new(Arc::clone(&registry)), handles)
    }

    fn ask() -> ChatRequest {
        ChatRequest::prompt("question", None)
    }

    #[tokio::test]
    async fn a_healthy_primary_answers_without_failover() {
        let (router, _) = router_with(vec![(
            base_profile("main"),
            StubProvider::new(|_| Ok("answer".into())),
        )]);

        let response = router.generate(None, ask()).await.expect("succeeds");
        assert_eq!(response.text, "answer");
        assert_eq!(response.profile, "main");
        assert!(response.fallbacks_used.is_empty());
    }

    #[tokio::test]
    async fn a_failing_primary_fails_over_and_the_failover_is_visible() {
        let (router, handles) = router_with(vec![
            (
                base_profile("primary").with_fallbacks(vec!["backup".into()]),
                StubProvider::new(|_| Err(api_error(503))),
            ),
            (
                base_profile("backup"),
                StubProvider::new(|_| Ok("from backup".into())),
            ),
        ]);

        let response = router
            .generate(Some("primary"), ask())
            .await
            .expect("succeeds");

        assert_eq!(response.text, "from backup");
        assert_eq!(response.profile, "backup");
        assert_eq!(
            response.fallbacks_used,
            vec!["primary".to_string()],
            "a silent failover is an outage nobody can see"
        );
        assert_eq!(handles[0].calls(), 1);
        assert_eq!(handles[1].calls(), 1);

        let primary = router.registry().summary("primary").expect("exists");
        let backup = router.registry().summary("backup").expect("exists");
        assert_eq!(primary.usage.failures, 1);
        assert_eq!(primary.usage.fallbacks_fired, 1);
        assert_eq!(backup.usage.fallback_uses, 1);
    }

    #[tokio::test]
    async fn a_whole_failed_chain_reports_every_profile_tried() {
        let (router, _) = router_with(vec![
            (
                base_profile("primary").with_fallbacks(vec!["backup".into()]),
                StubProvider::new(|_| Err(api_error(503))),
            ),
            (
                base_profile("backup"),
                StubProvider::new(|_| Err(api_error(500))),
            ),
        ]);

        let err = router
            .generate(Some("primary"), ask())
            .await
            .expect_err("both failed");
        let LlmError::AllProvidersFailed { attempted, .. } = &err else {
            panic!("expected AllProvidersFailed, got {err:?}");
        };
        assert_eq!(attempted, &["primary".to_string(), "backup".to_string()]);
    }

    #[tokio::test]
    async fn a_lone_profile_failure_is_not_wrapped() {
        let (router, _) = router_with(vec![(
            base_profile("only"),
            StubProvider::new(|_| Err(api_error(401))),
        )]);

        let err = router.generate(None, ask()).await.expect_err("fails");
        assert!(
            matches!(err, LlmError::Api { status: 401, .. }),
            "wrapping a single failure adds noise, not information; got {err:?}"
        );
    }

    #[tokio::test]
    async fn a_retryable_failure_is_retried_within_the_budget() {
        let profile = ModelProfile {
            retry: RetryPolicy {
                max_retries: 2,
                initial_backoff_ms: 1,
                max_backoff_ms: 2,
                backoff_multiplier_pct: 200,
            },
            ..base_profile("flaky")
        };
        let (router, handles) = router_with(vec![(
            profile,
            StubProvider::new(|call| {
                if call < 2 {
                    Err(api_error(503))
                } else {
                    Ok("recovered".into())
                }
            }),
        )]);

        let response = router.generate(None, ask()).await.expect("recovers");
        assert_eq!(response.text, "recovered");
        assert_eq!(handles[0].calls(), 3, "two failures then a success");
    }

    #[tokio::test]
    async fn a_non_retryable_failure_is_not_retried() {
        let profile = ModelProfile {
            retry: RetryPolicy {
                max_retries: 5,
                initial_backoff_ms: 1,
                max_backoff_ms: 2,
                backoff_multiplier_pct: 200,
            },
            ..base_profile("authfail")
        };
        let (router, handles) =
            router_with(vec![(profile, StubProvider::new(|_| Err(api_error(401))))]);

        router.generate(None, ask()).await.expect_err("401");
        assert_eq!(
            handles[0].calls(),
            1,
            "retrying a 401 burns quota to reach the same answer"
        );
    }

    #[tokio::test]
    async fn the_retry_budget_is_finite() {
        let profile = ModelProfile {
            retry: RetryPolicy {
                max_retries: 3,
                initial_backoff_ms: 1,
                max_backoff_ms: 2,
                backoff_multiplier_pct: 200,
            },
            ..base_profile("down")
        };
        let (router, handles) =
            router_with(vec![(profile, StubProvider::new(|_| Err(api_error(503))))]);

        router.generate(None, ask()).await.expect_err("always down");
        assert_eq!(handles[0].calls(), 4, "the initial attempt plus 3 retries");
    }

    #[tokio::test]
    async fn a_hung_provider_hits_the_deadline_instead_of_hanging_the_query() {
        let profile = ModelProfile {
            timeout_ms: 50,
            ..base_profile("slow")
        };
        let (router, _) = router_with(vec![(
            profile,
            StubProvider::new(|_| Ok("eventually".into())).with_delay(Duration::from_secs(30)),
        )]);

        let started = Instant::now();
        let err = router.generate(None, ask()).await.expect_err("times out");
        assert!(matches!(err, LlmError::Timeout { .. }), "got {err:?}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the deadline must bound the request, not the provider's patience"
        );
    }

    #[tokio::test]
    async fn a_timeout_on_the_primary_fails_over() {
        let (router, _) = router_with(vec![
            (
                ModelProfile {
                    timeout_ms: 30,
                    ..base_profile("slow").with_fallbacks(vec!["quick".into()])
                },
                StubProvider::new(|_| Ok("never".into())).with_delay(Duration::from_secs(30)),
            ),
            (
                base_profile("quick"),
                StubProvider::new(|_| Ok("fast answer".into())),
            ),
        ]);

        let response = router
            .generate(Some("slow"), ask())
            .await
            .expect("fails over");
        assert_eq!(response.text, "fast answer");
        assert_eq!(response.fallbacks_used, vec!["slow".to_string()]);
    }

    #[tokio::test]
    async fn an_open_breaker_short_circuits_instead_of_calling_the_provider() {
        let profile = ModelProfile {
            breaker: BreakerConfig {
                failure_threshold: 2,
                open_duration_ms: 60_000,
                success_threshold: 1,
            },
            ..base_profile("dead")
        };
        let (router, handles) =
            router_with(vec![(profile, StubProvider::new(|_| Err(api_error(503))))]);

        for _ in 0..2 {
            router.generate(None, ask()).await.expect_err("down");
        }
        assert_eq!(handles[0].calls(), 2);

        let err = router
            .generate(None, ask())
            .await
            .expect_err("circuit open");
        assert!(matches!(err, LlmError::CircuitOpen { .. }), "got {err:?}");
        assert_eq!(
            handles[0].calls(),
            2,
            "a known-dead provider must cost zero, not one full timeout"
        );
    }

    #[tokio::test]
    async fn a_client_error_does_not_open_the_breaker() {
        let profile = ModelProfile {
            breaker: BreakerConfig {
                failure_threshold: 2,
                open_duration_ms: 60_000,
                success_threshold: 1,
            },
            ..base_profile("picky")
        };
        let (router, handles) = router_with(vec![(
            profile,
            StubProvider::new(|call| {
                if call < 3 {
                    Err(api_error(400))
                } else {
                    Ok("fine".into())
                }
            }),
        )]);

        for _ in 0..3 {
            router.generate(None, ask()).await.expect_err("bad request");
        }
        let response = router.generate(None, ask()).await.expect("still reachable");
        assert_eq!(
            response.text, "fine",
            "one caller's malformed prompt must not take a healthy provider out of service"
        );
        assert_eq!(handles[0].calls(), 4);
    }

    #[tokio::test]
    async fn an_open_breaker_falls_through_to_the_next_profile() {
        let (router, handles) = router_with(vec![
            (
                ModelProfile {
                    breaker: BreakerConfig {
                        failure_threshold: 1,
                        open_duration_ms: 60_000,
                        success_threshold: 1,
                    },
                    ..base_profile("dead").with_fallbacks(vec!["alive".into()])
                },
                StubProvider::new(|_| Err(api_error(503))),
            ),
            (
                base_profile("alive"),
                StubProvider::new(|_| Ok("healthy".into())),
            ),
        ]);

        // First call trips the breaker and fails over.
        router
            .generate(Some("dead"), ask())
            .await
            .expect("fails over");
        let calls_after_first = handles[0].calls();

        let response = router
            .generate(Some("dead"), ask())
            .await
            .expect("fails over");
        assert_eq!(response.profile, "alive");
        assert_eq!(
            handles[0].calls(),
            calls_after_first,
            "the open circuit was not dialled again"
        );
    }

    #[tokio::test]
    async fn request_parameters_override_profile_defaults() {
        let profile = base_profile("params").with_params(GenerationParams {
            temperature: Some(0.1),
            max_tokens: Some(100),
            ..Default::default()
        });
        let registry = Arc::new(LlmRegistry::new());
        let captured = Arc::new(std::sync::Mutex::new(GenerationParams::default()));

        struct Capturing(Arc<std::sync::Mutex<GenerationParams>>);

        #[async_trait::async_trait]
        impl crate::provider::LlmProvider for Capturing {
            fn kind(&self) -> crate::ProviderKind {
                crate::ProviderKind::Compatible
            }
            fn model(&self) -> &str {
                "capture"
            }
            async fn generate(
                &self,
                request: &ChatRequest,
            ) -> LlmResult<crate::provider::ProviderChatOutput> {
                *self.0.lock().expect("lock") = request.params.clone();
                Ok(crate::provider::ProviderChatOutput {
                    text: "ok".into(),
                    model: "capture".into(),
                    usage: TokenUsage::default(),
                    finish_reason: None,
                })
            }
        }

        #[async_trait::async_trait]
        impl crate::provider::EmbeddingProvider for Capturing {
            fn kind(&self) -> crate::ProviderKind {
                crate::ProviderKind::Compatible
            }
            fn embedding_model(&self) -> &str {
                "capture"
            }
            async fn embed(
                &self,
                _: &EmbeddingRequest,
            ) -> LlmResult<crate::provider::ProviderEmbeddingOutput> {
                unreachable!("not exercised by this test")
            }
        }

        let capturing = Arc::new(Capturing(Arc::clone(&captured)));
        registry
            .register_with_provider(
                profile,
                crate::providers::BuiltProvider {
                    llm: Arc::clone(&capturing) as Arc<dyn crate::provider::LlmProvider>,
                    embedding: capturing as Arc<dyn crate::provider::EmbeddingProvider>,
                },
            )
            .expect("registers");

        let router = Router::new(registry);
        router
            .generate(
                None,
                ask().with_params(GenerationParams {
                    temperature: Some(0.9),
                    ..Default::default()
                }),
            )
            .await
            .expect("succeeds");

        let seen = captured.lock().expect("lock").clone();
        assert_eq!(seen.temperature, Some(0.9), "the request wins");
        assert_eq!(seen.max_tokens, Some(100), "the profile fills the gap");
    }

    #[tokio::test]
    async fn cost_is_computed_only_when_the_profile_is_priced() {
        let (unpriced_router, _) = router_with(vec![(
            base_profile("free"),
            StubProvider::new(|_| Ok("x".into())),
        )]);
        let response = unpriced_router
            .generate(None, ask())
            .await
            .expect("succeeds");
        assert!(
            response.cost.is_none(),
            "an unconfigured price must read as unknown, not as zero"
        );

        let (priced_router, _) = router_with(vec![(
            base_profile("paid").with_pricing(ModelPricing {
                prompt_usd_per_million: 1_000_000.0,
                completion_usd_per_million: 1_000_000.0,
            }),
            StubProvider::new(|_| Ok("x".into())).with_usage(TokenUsage {
                prompt_tokens: Some(2),
                completion_tokens: Some(3),
            }),
        )]);
        let cost = priced_router
            .generate(None, ask())
            .await
            .expect("succeeds")
            .cost
            .expect("priced");
        assert!((cost.total_usd() - 5.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn unreported_usage_stays_unreported_through_the_router() {
        let (router, _) = router_with(vec![(
            base_profile("local"),
            StubProvider::new(|_| Ok("x".into())).with_usage(TokenUsage::default()),
        )]);

        let response = router.generate(None, ask()).await.expect("succeeds");
        assert_eq!(response.usage.total(), None);
        assert!(!router
            .registry()
            .summary("local")
            .expect("exists")
            .usage
            .tokens_are_complete());
    }

    #[tokio::test]
    async fn switching_the_default_changes_which_model_answers() {
        let (router, _) = router_with(vec![
            (
                base_profile("a"),
                StubProvider::new(|_| Ok("from a".into())),
            ),
            (
                base_profile("b"),
                StubProvider::new(|_| Ok("from b".into())),
            ),
        ]);

        assert_eq!(
            router.generate(None, ask()).await.expect("a").text,
            "from a"
        );
        router.registry().set_default("b").expect("registered");
        assert_eq!(
            router.generate(None, ask()).await.expect("b").text,
            "from b",
            "LLM.USE must take effect on the next request, with no restart"
        );
    }

    #[tokio::test]
    async fn embeddings_route_and_account_like_generations() {
        let (router, _) = router_with(vec![(
            base_profile("embed"),
            StubProvider::new(|_| Ok("seed".into())),
        )]);

        let response = router
            .embed(None, EmbeddingRequest::new(["one", "two-two"]))
            .await
            .expect("succeeds");

        assert_eq!(response.embeddings.len(), 2);
        assert_eq!(response.dimensions(), Some(2));
        assert_eq!(response.embeddings[0][1], 3.0, "len(\"one\")");
        assert_eq!(response.embeddings[1][1], 7.0, "len(\"two-two\")");
        assert_eq!(
            router
                .registry()
                .summary("embed")
                .expect("exists")
                .usage
                .requests,
            1
        );
    }

    #[tokio::test]
    async fn embeddings_fail_over_too() {
        let (router, _) = router_with(vec![
            (
                base_profile("primary").with_fallbacks(vec!["backup".into()]),
                StubProvider::new(|_| Err(api_error(500))),
            ),
            (
                base_profile("backup"),
                StubProvider::new(|_| Ok("seed".into())),
            ),
        ]);

        let response = router
            .embed(Some("primary"), EmbeddingRequest::new(["x"]))
            .await
            .expect("fails over");
        assert_eq!(response.profile, "backup");
    }

    #[tokio::test]
    async fn the_response_names_the_model_the_provider_reported() {
        // The profile is named "alias" but the provider reports a versioned model id; the response
        // must carry what actually ran, not what was asked for.
        let (router, _) = router_with(vec![(
            base_profile("alias"),
            StubProvider::new(|_| Ok("x".into())).with_model("llama3.2:3b-instruct-q4"),
        )]);

        let response = router.generate(None, ask()).await.expect("succeeds");
        assert_eq!(response.profile, "alias");
        assert_eq!(response.model, "llama3.2:3b-instruct-q4");
    }

    #[tokio::test]
    async fn an_unknown_profile_is_reported_not_silently_defaulted() {
        let (router, _) = router_with(vec![(
            base_profile("only"),
            StubProvider::new(|_| Ok("x".into())),
        )]);

        let err = router
            .generate(Some("ghost"), ask())
            .await
            .expect_err("unknown");
        assert!(
            matches!(err, LlmError::UnknownProfile { .. }),
            "quietly answering from a different model than asked for is worse than failing"
        );
    }

    #[tokio::test]
    async fn an_empty_registry_reports_no_default() {
        let router = Router::new(Arc::new(LlmRegistry::new()));
        assert!(matches!(
            router.generate(None, ask()).await.expect_err("empty"),
            LlmError::NoDefaultProfile
        ));
    }
}
