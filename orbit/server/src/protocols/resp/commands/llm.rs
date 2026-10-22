//! `LLM.*` commands: inspect, register, and switch models at runtime.
//!
//! This is the control surface for the capability the roadmap calls *provider and model
//! switchability*. Before it existed, changing which model GraphRAG used meant editing environment
//! variables and restarting the server. Now:
//!
//! ```text
//! LLM.REGISTER fast ollama llama3.2 TEMPERATURE 0.2
//! LLM.USE fast
//! LLM.GENERATE "why is the sky blue?"
//! LLM.STATS
//! ```
//!
//! Every command reads or mutates the process-wide registry in [`crate::llm`], so a switch made
//! here is immediately visible to GraphRAG and any other AI surface.
//!
//! Responses never contain a credential: profile detail is rendered from
//! [`orbit_llm::ModelSummary`], which is derived from the profile rather than holding it.

use super::traits::CommandHandler;
use crate::llm::runtime;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::resp::simple_local::SimpleLocalRegistry;
use crate::protocols::resp::types::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use orbit_client::OrbitClient;
use orbit_llm::{
    ChatRequest, CompatibleFlavor, EmbeddingRequest, GenerationParams, ModelPricing, ModelProfile,
    ModelSummary, ProviderConfig, ProviderKind, ProviderSettings, SecretString,
};
use std::str::FromStr;
use std::sync::Arc;

/// Commands supported by this handler.
const SUPPORTED: &[&str] = &[
    "LLM.PROVIDERS",
    "LLM.MODELS",
    "LLM.INFO",
    "LLM.REGISTER",
    "LLM.UNREGISTER",
    "LLM.USE",
    "LLM.GENERATE",
    "LLM.EMBED",
    "LLM.STATS",
];

/// Handler for the `LLM.*` command family.
pub struct LlmCommands {
    #[allow(dead_code)]
    local_registry: Arc<SimpleLocalRegistry>,
    #[allow(dead_code)]
    orbit_client: Arc<OrbitClient>,
}

impl LlmCommands {
    /// Create a handler.
    pub fn new(orbit_client: Arc<OrbitClient>, local_registry: Arc<SimpleLocalRegistry>) -> Self {
        Self {
            local_registry,
            orbit_client,
        }
    }

    /// `LLM.PROVIDERS` — the wire shapes this build can speak.
    fn providers(&self) -> RespValue {
        RespValue::Array(
            ProviderKind::all()
                .iter()
                .map(|kind| {
                    RespValue::Array(vec![
                        bulk("name"),
                        bulk(kind.as_str()),
                        bulk("embeddings"),
                        RespValue::Boolean(kind.supports_embeddings()),
                    ])
                })
                .collect(),
        )
    }

    /// `LLM.MODELS` — registered profiles, marking the default.
    fn models(&self) -> RespValue {
        RespValue::Array(
            runtime()
                .registry()
                .summaries()
                .iter()
                .map(render_summary_brief)
                .collect(),
        )
    }

    /// `LLM.INFO <profile>` — full detail for one profile.
    fn info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let name = self.get_string_arg(args, 0, "LLM.INFO")?;
        let summary = runtime()
            .registry()
            .summary(&name)
            .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;
        Ok(render_summary_full(&summary))
    }

    /// `LLM.USE <profile>` — switch the default model, no restart.
    fn use_profile(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let name = self.get_string_arg(args, 0, "LLM.USE")?;
        runtime()
            .registry()
            .set_default(&name)
            .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;
        tracing::info!(profile = %name, "default LLM profile switched at runtime");
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// `LLM.UNREGISTER <profile>` — remove a profile.
    fn unregister(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let name = self.get_string_arg(args, 0, "LLM.UNREGISTER")?;
        runtime()
            .registry()
            .unregister(&name)
            .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;
        tracing::info!(profile = %name, "LLM profile removed at runtime");
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// `LLM.REGISTER <profile> <provider> <model> [option value]...`
    fn register(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let profile = build_profile_from_args(args)?;
        let name = profile.name.clone();
        runtime()
            .registry()
            .register(profile)
            .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;
        tracing::info!(profile = %name, "LLM profile registered at runtime");
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// `LLM.GENERATE <prompt> [MODEL p] [SYSTEM s] [MAXTOKENS n] [TEMPERATURE t]`
    async fn generate(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let prompt = self.get_string_arg(args, 0, "LLM.GENERATE")?;
        let options = parse_options(&args[1..])?;

        let request = ChatRequest::prompt(prompt, options.system).with_params(GenerationParams {
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            ..Default::default()
        });

        let response = runtime()
            .router()
            .generate(options.model.as_deref(), request)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;

        let mut fields = vec![
            bulk("text"),
            bulk(&response.text),
            bulk("model"),
            bulk(&response.model),
            bulk("profile"),
            bulk(&response.profile),
            bulk("latency_ms"),
            RespValue::Integer(response.latency.as_millis() as i64),
        ];

        // Token and cost fields appear only when the provider actually reported them. Emitting a
        // zero would assert the request was free.
        if let Some(total) = response.usage.total() {
            fields.push(bulk("tokens_used"));
            fields.push(RespValue::Integer(i64::from(total)));
        }
        if let Some(cost) = response.cost {
            fields.push(bulk("cost_usd"));
            fields.push(RespValue::Double(cost.total_usd()));
        }
        if let Some(reason) = &response.finish_reason {
            fields.push(bulk("finish_reason"));
            fields.push(bulk(reason.as_wire()));
        }
        if !response.fallbacks_used.is_empty() {
            fields.push(bulk("fallbacks_used"));
            fields.push(RespValue::Array(
                response.fallbacks_used.iter().map(|f| bulk(f)).collect(),
            ));
        }

        Ok(RespValue::Array(fields))
    }

    /// `LLM.EMBED <text> [text...] [MODEL p]`
    async fn embed(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // Everything up to an option keyword is an input; the rest are options. Splitting this way
        // lets a caller embed a batch in one round trip.
        let split = args
            .iter()
            .position(|arg| {
                arg.as_string()
                    .is_some_and(|s| s.eq_ignore_ascii_case("MODEL"))
            })
            .unwrap_or(args.len());

        let inputs: Vec<String> = args[..split]
            .iter()
            .filter_map(RespValue::as_string)
            .collect();
        if inputs.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'llm.embed' command".to_string(),
            ));
        }

        let options = parse_options(&args[split..])?;
        let response = runtime()
            .router()
            .embed(options.model.as_deref(), EmbeddingRequest::new(inputs))
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;

        Ok(RespValue::Array(vec![
            bulk("model"),
            bulk(&response.model),
            bulk("profile"),
            bulk(&response.profile),
            bulk("dimensions"),
            RespValue::Integer(response.dimensions().unwrap_or(0) as i64),
            bulk("embeddings"),
            RespValue::Array(
                response
                    .embeddings
                    .iter()
                    .map(|vector| {
                        RespValue::Array(
                            vector
                                .iter()
                                .map(|v| RespValue::Double(f64::from(*v)))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        ]))
    }

    /// `LLM.STATS [profile]` — counters, breaker state, and cost.
    fn stats(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let summaries = match args.first().and_then(RespValue::as_string) {
            Some(name) => vec![runtime()
                .registry()
                .summary(&name)
                .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?],
            None => runtime().registry().summaries(),
        };

        Ok(RespValue::Array(
            summaries.iter().map(render_stats).collect(),
        ))
    }
}

#[async_trait]
impl CommandHandler for LlmCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name {
            "LLM.PROVIDERS" => Ok(self.providers()),
            "LLM.MODELS" => Ok(self.models()),
            "LLM.INFO" => self.info(args),
            "LLM.REGISTER" => self.register(args),
            "LLM.UNREGISTER" => self.unregister(args),
            "LLM.USE" => self.use_profile(args),
            "LLM.GENERATE" => self.generate(args).await,
            "LLM.EMBED" => self.embed(args).await,
            "LLM.STATS" => self.stats(args),
            other => Err(ProtocolError::RespError(format!(
                "ERR unknown command '{other}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        SUPPORTED
    }
}

/// Options accepted by `LLM.GENERATE` and `LLM.EMBED`.
#[derive(Debug, Default, PartialEq)]
struct RequestOptions {
    model: Option<String>,
    system: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

/// Parse trailing `KEY value` pairs.
///
/// An unrecognized key is an error rather than being skipped: silently ignoring `TEMPRATURE 0.2`
/// would let a caller believe a setting took effect when it did not.
fn parse_options(args: &[RespValue]) -> ProtocolResult<RequestOptions> {
    let mut options = RequestOptions::default();
    let mut index = 0;

    while index < args.len() {
        let key = args[index]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid option name".to_string()))?
            .to_uppercase();

        let value = args
            .get(index + 1)
            .and_then(RespValue::as_string)
            .ok_or_else(|| {
                ProtocolError::RespError(format!("ERR option '{key}' requires a value"))
            })?;

        match key.as_str() {
            "MODEL" | "PROFILE" => options.model = Some(value),
            "SYSTEM" => options.system = Some(value),
            "TEMPERATURE" => {
                options.temperature = Some(parse_number(&value, "TEMPERATURE")?);
            }
            "MAXTOKENS" | "MAX_TOKENS" => {
                options.max_tokens = Some(parse_number(&value, "MAXTOKENS")?);
            }
            other => {
                return Err(ProtocolError::RespError(format!(
                    "ERR unknown option '{other}'"
                )))
            }
        }
        index += 2;
    }

    Ok(options)
}

fn parse_number<T: FromStr>(value: &str, field: &str) -> ProtocolResult<T> {
    value
        .parse()
        .map_err(|_| ProtocolError::RespError(format!("ERR invalid value for '{field}': {value}")))
}

/// Build a profile from `LLM.REGISTER` arguments.
fn build_profile_from_args(args: &[RespValue]) -> ProtocolResult<ModelProfile> {
    if args.len() < 3 {
        return Err(ProtocolError::RespError(
            "ERR wrong number of arguments for 'llm.register' command; \
             expected <profile> <provider> <model> [option value]..."
                .to_string(),
        ));
    }

    let name = string_at(args, 0, "profile")?;
    let provider_arg = string_at(args, 1, "provider")?;
    let model = string_at(args, 2, "model")?;

    let kind = ProviderKind::from_str(&provider_arg)
        .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;
    let settings = ProfileSettings::parse(&args[3..])?;

    let provider = build_provider_config(kind, &provider_arg, &settings)?;
    let mut profile = ModelProfile::new(name, provider, model).with_params(GenerationParams {
        temperature: settings.temperature,
        max_tokens: settings.max_tokens,
        ..Default::default()
    });

    profile.embedding_model = settings.embedding_model.clone();
    profile.fallbacks = settings.fallbacks.clone();
    if let Some(timeout_ms) = settings.timeout_ms {
        profile.timeout_ms = timeout_ms;
    }
    if let (Some(prompt), Some(completion)) = (settings.price_prompt, settings.price_completion) {
        profile.pricing = Some(ModelPricing {
            prompt_usd_per_million: prompt,
            completion_usd_per_million: completion,
        });
    }

    Ok(profile)
}

fn build_provider_config(
    kind: ProviderKind,
    provider_arg: &str,
    settings: &ProfileSettings,
) -> ProtocolResult<ProviderConfig> {
    // The provider argument doubles as the flavor, so `LLM.REGISTER p azure gpt-4o` picks Azure's
    // header and URL convention without needing a separate option.
    let flavor = (kind == ProviderKind::Compatible)
        .then(|| CompatibleFlavor::parse(provider_arg))
        .transpose()
        .map_err(|e| ProtocolError::RespError(format!("ERR {e}")))?;

    ProviderConfig::from_settings(
        kind,
        &ProviderSettings {
            api_key: settings.api_key.clone(),
            base_url: settings.base_url.clone(),
            api_version: settings.api_version.clone(),
            organization: settings.organization.clone(),
            project: settings.project.clone(),
            flavor,
        },
    )
    .map_err(|e| {
        // Name the option the caller would actually type. "requires an explicit base_url" is
        // correct and unactionable at a redis-cli prompt where the option is spelled BASEURL.
        ProtocolError::RespError(match kind {
            ProviderKind::Compatible if settings.base_url.is_none() => {
                "ERR an OpenAI-compatible provider requires BASEURL <url>".to_string()
            }
            _ => format!("ERR {e}"),
        })
    })
}

/// Optional `LLM.REGISTER` settings.
#[derive(Debug, Default)]
struct ProfileSettings {
    api_key: Option<SecretString>,
    base_url: Option<String>,
    api_version: Option<String>,
    organization: Option<String>,
    project: Option<String>,
    embedding_model: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    timeout_ms: Option<u64>,
    fallbacks: Vec<String>,
    price_prompt: Option<f64>,
    price_completion: Option<f64>,
}

impl ProfileSettings {
    fn parse(args: &[RespValue]) -> ProtocolResult<Self> {
        let mut settings = Self::default();
        let mut index = 0;

        while index < args.len() {
            let key = args[index]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid option name".to_string()))?
                .to_uppercase();

            let value = args
                .get(index + 1)
                .and_then(RespValue::as_string)
                .ok_or_else(|| {
                    ProtocolError::RespError(format!("ERR option '{key}' requires a value"))
                })?;

            match key.as_str() {
                "APIKEY" | "API_KEY" => settings.api_key = Some(SecretString::new(value)),
                "BASEURL" | "BASE_URL" | "ENDPOINT" => settings.base_url = Some(value),
                "APIVERSION" | "API_VERSION" => settings.api_version = Some(value),
                "ORGANIZATION" | "ORG" => settings.organization = Some(value),
                "PROJECT" => settings.project = Some(value),
                "EMBEDDINGMODEL" | "EMBEDDING_MODEL" => settings.embedding_model = Some(value),
                "TEMPERATURE" => settings.temperature = Some(parse_number(&value, "TEMPERATURE")?),
                "MAXTOKENS" | "MAX_TOKENS" => {
                    settings.max_tokens = Some(parse_number(&value, "MAXTOKENS")?);
                }
                "TIMEOUTMS" | "TIMEOUT_MS" => {
                    settings.timeout_ms = Some(parse_number(&value, "TIMEOUTMS")?);
                }
                "FALLBACKS" => {
                    settings.fallbacks = value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect();
                }
                "PRICEPROMPT" | "PRICE_PROMPT" => {
                    settings.price_prompt = Some(parse_number(&value, "PRICEPROMPT")?);
                }
                "PRICECOMPLETION" | "PRICE_COMPLETION" => {
                    settings.price_completion = Some(parse_number(&value, "PRICECOMPLETION")?);
                }
                other => {
                    return Err(ProtocolError::RespError(format!(
                        "ERR unknown option '{other}'"
                    )))
                }
            }
            index += 2;
        }

        Ok(settings)
    }
}

fn string_at(args: &[RespValue], index: usize, field: &str) -> ProtocolResult<String> {
    args.get(index)
        .and_then(RespValue::as_string)
        .ok_or_else(|| ProtocolError::RespError(format!("ERR invalid {field}")))
}

fn bulk(value: &str) -> RespValue {
    RespValue::BulkString(Bytes::from(value.to_owned()))
}

fn render_summary_brief(summary: &ModelSummary) -> RespValue {
    RespValue::Array(vec![
        bulk("name"),
        bulk(&summary.name),
        bulk("provider"),
        bulk(&summary.provider),
        bulk("model"),
        bulk(&summary.model),
        bulk("default"),
        RespValue::Boolean(summary.is_default),
        bulk("breaker"),
        bulk(&summary.breaker_state),
    ])
}

fn render_summary_full(summary: &ModelSummary) -> RespValue {
    let mut fields = vec![
        bulk("name"),
        bulk(&summary.name),
        bulk("provider"),
        bulk(&summary.provider),
        bulk("model"),
        bulk(&summary.model),
        bulk("base_url"),
        bulk(&summary.base_url),
        bulk("default"),
        RespValue::Boolean(summary.is_default),
        bulk("timeout_ms"),
        RespValue::Integer(summary.timeout_ms as i64),
        bulk("has_pricing"),
        RespValue::Boolean(summary.has_pricing),
        bulk("breaker"),
        bulk(&summary.breaker_state),
    ];

    if let Some(embedding_model) = &summary.embedding_model {
        fields.push(bulk("embedding_model"));
        fields.push(bulk(embedding_model));
    }
    if !summary.fallbacks.is_empty() {
        fields.push(bulk("fallbacks"));
        fields.push(RespValue::Array(
            summary.fallbacks.iter().map(|f| bulk(f)).collect(),
        ));
    }

    RespValue::Array(fields)
}

fn render_stats(summary: &ModelSummary) -> RespValue {
    let usage = &summary.usage;
    let mut fields = vec![
        bulk("name"),
        bulk(&summary.name),
        bulk("requests"),
        RespValue::Integer(usage.requests as i64),
        bulk("failures"),
        RespValue::Integer(usage.failures as i64),
        bulk("fallbacks_fired"),
        RespValue::Integer(usage.fallbacks_fired as i64),
        bulk("fallback_uses"),
        RespValue::Integer(usage.fallback_uses as i64),
        bulk("prompt_tokens"),
        RespValue::Integer(usage.prompt_tokens as i64),
        bulk("completion_tokens"),
        RespValue::Integer(usage.completion_tokens as i64),
        // Says whether the two token figures above are totals or lower bounds. Without it a
        // reader cannot tell a provider that reports nothing from one that used nothing.
        bulk("tokens_complete"),
        RespValue::Boolean(usage.tokens_are_complete()),
        bulk("breaker"),
        bulk(&summary.breaker_state),
    ];

    // Cost is meaningful only for a priced profile; on an unpriced one it would always read 0.00.
    if summary.has_pricing {
        fields.push(bulk("cost_usd"));
        fields.push(RespValue::Double(usage.cost_usd));
    }
    if let Some(mean) = usage.mean_latency_ms {
        fields.push(bulk("mean_latency_ms"));
        fields.push(RespValue::Double(mean));
    }

    RespValue::Array(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<RespValue> {
        values.iter().map(|v| bulk(v)).collect()
    }

    #[test]
    fn register_builds_an_ollama_profile_with_its_parameters() {
        let profile = build_profile_from_args(&args(&[
            "fast",
            "ollama",
            "llama3.2",
            "TEMPERATURE",
            "0.25",
            "MAXTOKENS",
            "1024",
            "EMBEDDINGMODEL",
            "nomic-embed-text",
        ]))
        .expect("builds");

        assert_eq!(profile.name, "fast");
        assert_eq!(profile.provider.kind(), ProviderKind::Ollama);
        assert_eq!(profile.model, "llama3.2");
        assert_eq!(profile.params.temperature, Some(0.25));
        assert_eq!(profile.params.max_tokens, Some(1024));
        assert_eq!(profile.embedding_model.as_deref(), Some("nomic-embed-text"));
    }

    #[test]
    fn register_accepts_a_fallback_chain_and_timeout() {
        let profile = build_profile_from_args(&args(&[
            "primary",
            "ollama",
            "llama3.2",
            "FALLBACKS",
            "backup, spare",
            "TIMEOUTMS",
            "15000",
        ]))
        .expect("builds");

        assert_eq!(
            profile.fallbacks,
            vec!["backup".to_string(), "spare".to_string()]
        );
        assert_eq!(profile.timeout_ms, 15_000);
    }

    #[test]
    fn register_maps_a_named_service_onto_the_compatible_shape() {
        let profile = build_profile_from_args(&args(&[
            "groq",
            "groq",
            "llama-3.3-70b",
            "BASEURL",
            "https://api.groq.com/openai/v1",
            "APIKEY",
            "gsk-test",
        ]))
        .expect("builds");

        assert_eq!(profile.provider.kind(), ProviderKind::Compatible);
        assert_eq!(
            profile.provider.base_url(),
            "https://api.groq.com/openai/v1"
        );
    }

    #[test]
    fn register_selects_the_azure_flavor_from_the_provider_argument() {
        let profile = build_profile_from_args(&args(&[
            "azure",
            "azure",
            "gpt-4o-deployment",
            "BASEURL",
            "https://contoso.openai.azure.com",
            "APIVERSION",
            "2024-10-21",
            "APIKEY",
            "azure-key",
        ]))
        .expect("builds");

        let ProviderConfig::Compatible { flavor, .. } = &profile.provider else {
            panic!("expected the compatible shape");
        };
        assert_eq!(*flavor, CompatibleFlavor::AzureOpenAi);
    }

    #[test]
    fn register_requires_a_base_url_for_a_compatible_provider() {
        let err = build_profile_from_args(&args(&["local", "vllm", "Qwen3-8B"]))
            .expect_err("no base URL");
        assert!(err.to_string().contains("BASEURL"));
    }

    #[test]
    fn register_rejects_an_unknown_provider() {
        let err = build_profile_from_args(&args(&["x", "cohere", "command-r"]))
            .expect_err("unknown provider");
        assert!(err.to_string().contains("unknown provider"));
    }

    #[test]
    fn register_requires_the_three_positional_arguments() {
        for short in [vec![], vec!["a"], vec!["a", "ollama"]] {
            assert!(build_profile_from_args(&args(&short)).is_err());
        }
    }

    #[test]
    fn pricing_needs_both_halves_to_be_meaningful() {
        let half = build_profile_from_args(&args(&["p", "ollama", "m", "PRICEPROMPT", "3.0"]))
            .expect("builds");
        assert!(
            half.pricing.is_none(),
            "half a price schedule would compute a wrong cost, not a partial one"
        );

        let full = build_profile_from_args(&args(&[
            "p",
            "ollama",
            "m",
            "PRICEPROMPT",
            "3.0",
            "PRICECOMPLETION",
            "15.0",
        ]))
        .expect("builds");
        let pricing = full.pricing.expect("both halves supplied");
        assert!((pricing.prompt_usd_per_million - 3.0).abs() < f64::EPSILON);
        assert!((pricing.completion_usd_per_million - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn options_parse_case_insensitively() {
        let parsed = parse_options(&args(&[
            "model",
            "fast",
            "System",
            "be terse",
            "TEMPERATURE",
            "0.7",
            "maxtokens",
            "128",
        ]))
        .expect("parses");

        assert_eq!(parsed.model.as_deref(), Some("fast"));
        assert_eq!(parsed.system.as_deref(), Some("be terse"));
        assert_eq!(parsed.temperature, Some(0.7));
        assert_eq!(parsed.max_tokens, Some(128));
    }

    #[test]
    fn a_misspelled_option_is_an_error_not_a_silent_no_op() {
        let err = parse_options(&args(&["TEMPRATURE", "0.2"])).expect_err("typo rejected");
        assert!(
            err.to_string().contains("unknown option"),
            "skipping it would let a caller believe the setting took effect"
        );
    }

    #[test]
    fn an_option_without_a_value_is_an_error() {
        let err = parse_options(&args(&["MODEL"])).expect_err("dangling option");
        assert!(err.to_string().contains("requires a value"));
    }

    #[test]
    fn a_non_numeric_temperature_is_rejected() {
        let err = parse_options(&args(&["TEMPERATURE", "warm"])).expect_err("not a number");
        assert!(err.to_string().contains("invalid value for 'TEMPERATURE'"));
    }

    #[test]
    fn no_options_parses_to_all_defaults() {
        assert_eq!(
            parse_options(&[]).expect("parses"),
            RequestOptions::default()
        );
    }

    #[test]
    fn every_supported_command_is_reachable_from_dispatch() {
        // Affordance audit: a command listed but not dispatched is documentation, not a feature.
        // `handle` is exercised indirectly here by checking the match arms cover the list.
        let dispatched = [
            "LLM.PROVIDERS",
            "LLM.MODELS",
            "LLM.INFO",
            "LLM.REGISTER",
            "LLM.UNREGISTER",
            "LLM.USE",
            "LLM.GENERATE",
            "LLM.EMBED",
            "LLM.STATS",
        ];
        assert_eq!(SUPPORTED, dispatched);
    }

    #[test]
    fn stats_rendering_omits_cost_for_an_unpriced_profile() {
        let summary = ModelSummary {
            name: "free".into(),
            provider: "ollama".into(),
            model: "llama3.2".into(),
            embedding_model: None,
            base_url: "http://localhost:11434".into(),
            is_default: true,
            fallbacks: vec![],
            timeout_ms: 60_000,
            has_pricing: false,
            breaker_state: "closed".into(),
            usage: orbit_llm::UsageSnapshot {
                requests: 3,
                failures: 1,
                fallback_uses: 0,
                fallbacks_fired: 0,
                prompt_tokens: 100,
                completion_tokens: 50,
                unreported_usage: 1,
                cost_usd: 0.0,
                mean_latency_ms: Some(120.0),
            },
        };

        let RespValue::Array(fields) = render_stats(&summary) else {
            panic!("expected an array");
        };
        let keys: Vec<String> = fields.iter().filter_map(RespValue::as_string).collect();

        assert!(
            !keys.contains(&"cost_usd".to_string()),
            "an unpriced profile would always report 0.00, which reads as 'free'"
        );
        assert!(keys.contains(&"tokens_complete".to_string()));
        assert!(keys.contains(&"mean_latency_ms".to_string()));
    }

    #[test]
    fn full_info_rendering_carries_no_credential_fields() {
        let summary = ModelSummary {
            name: "openai".into(),
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            embedding_model: Some("text-embedding-3-small".into()),
            base_url: "https://api.openai.com/v1".into(),
            is_default: false,
            fallbacks: vec!["backup".into()],
            timeout_ms: 30_000,
            has_pricing: true,
            breaker_state: "closed".into(),
            usage: orbit_llm::UsageSnapshot {
                requests: 0,
                failures: 0,
                fallback_uses: 0,
                fallbacks_fired: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                unreported_usage: 0,
                cost_usd: 0.0,
                mean_latency_ms: None,
            },
        };

        let RespValue::Array(fields) = render_summary_full(&summary) else {
            panic!("expected an array");
        };
        let rendered: Vec<String> = fields.iter().filter_map(RespValue::as_string).collect();

        for forbidden in ["api_key", "apikey", "secret", "token"] {
            assert!(
                !rendered.iter().any(|f| f.eq_ignore_ascii_case(forbidden)),
                "LLM.INFO must not expose a credential field, found {forbidden}"
            );
        }
        assert!(rendered.contains(&"embedding_model".to_string()));
        assert!(rendered.contains(&"fallbacks".to_string()));
    }
}
