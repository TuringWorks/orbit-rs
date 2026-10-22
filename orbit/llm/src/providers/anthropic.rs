//! Anthropic's Messages API.
//!
//! This closes the pre-existing defect where `create_llm_client` returned
//! `Err("Anthropic client not yet implemented")` for a provider the configuration enum already
//! advertised.
//!
//! Three details differ from the OpenAI shape and each one is a hard failure if got wrong:
//!
//! 1. The system prompt is a **top-level `system` field**, not a message with `role: "system"`.
//!    Sending it as a message is rejected.
//! 2. `max_tokens` is **required**. There is no server-side default to fall back on, which is why
//!    [`crate::config::ModelProfile::validate`] insists an Anthropic profile carries one.
//! 3. Authentication is the `x-api-key` header plus a pinned `anthropic-version`, not bearer auth.

use crate::error::{LlmError, LlmResult};
use crate::http::{missing_field, parse_json, shared_client, transport_error};
use crate::provider::{
    EmbeddingProvider, LlmProvider, ProviderChatOutput, ProviderEmbeddingOutput, ProviderKind,
};
use crate::secret::SecretString;
use crate::types::{ChatRequest, EmbeddingRequest, FinishReason, TokenUsage};
use serde_json::{json, Map, Value};

const PROVIDER: &str = "anthropic";

/// Client for Anthropic's Messages API.
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    api_key: SecretString,
    base_url: String,
    version: String,
    model: String,
}

impl AnthropicProvider {
    /// Build a client.
    pub fn new(api_key: SecretString, base_url: String, version: String, model: String) -> Self {
        Self {
            api_key,
            base_url: base_url.trim_end_matches('/').to_owned(),
            version,
            model,
        }
    }
}

/// Build a `/messages` request body.
///
/// # Errors
///
/// Returns [`LlmError::Configuration`] when `max_tokens` is absent, and [`LlmError::Unsupported`]
/// when the caller asked for a deterministic seed — the Messages API has no seed parameter, and
/// dropping the request silently would let a caller believe their run is reproducible when it is
/// not.
pub fn messages_body(model: &str, request: &ChatRequest) -> LlmResult<Value> {
    let max_tokens = request.params.max_tokens.ok_or_else(|| {
        LlmError::configuration(
            "Anthropic requires max_tokens; set it on the profile or the request",
        )
    })?;

    if request.params.seed.is_some() {
        return Err(LlmError::Unsupported {
            provider: PROVIDER.to_owned(),
            capability: "deterministic seeding",
        });
    }

    let messages: Vec<Value> = request
        .conversation()
        .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
        .collect();

    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("max_tokens".into(), json!(max_tokens));
    body.insert("messages".into(), json!(messages));

    if let Some(system) = request.system_message() {
        body.insert("system".into(), json!(system));
    }
    if let Some(temperature) = request.params.temperature {
        body.insert("temperature".into(), json!(temperature));
    }
    if let Some(top_p) = request.params.top_p {
        body.insert("top_p".into(), json!(top_p));
    }
    if !request.params.stop.is_empty() {
        body.insert("stop_sequences".into(), json!(request.params.stop));
    }

    Ok(Value::Object(body))
}

/// Parse a `/messages` response.
///
/// The body carries `content` as an array of typed blocks; text blocks are concatenated in order
/// and non-text blocks (tool use, thinking) are skipped rather than stringified.
///
/// # Errors
///
/// Returns [`LlmError::MalformedResponse`] when `content` is absent or contains no text block.
pub fn parse_messages(fallback_model: &str, body: &Value) -> LlmResult<ProviderChatOutput> {
    let blocks = body
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| missing_field(PROVIDER, "content"))?;

    let text = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");

    if text.is_empty() && !blocks.is_empty() {
        return Err(missing_field(PROVIDER, "content[].text"));
    }

    Ok(ProviderChatOutput {
        text,
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_owned(),
        usage: TokenUsage {
            prompt_tokens: body
                .pointer("/usage/input_tokens")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
            completion_tokens: body
                .pointer("/usage/output_tokens")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
        },
        finish_reason: body
            .get("stop_reason")
            .and_then(Value::as_str)
            .map(FinishReason::from_wire),
    })
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(&self, request: &ChatRequest) -> LlmResult<ProviderChatOutput> {
        let body = messages_body(&self.model, request)?;
        let response = shared_client()
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", &self.version)
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(PROVIDER, e))?;

        let json: Value = parse_json(PROVIDER, response).await?;
        parse_messages(&self.model, &json)
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for AnthropicProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    fn embedding_model(&self) -> &str {
        // Anthropic ships no embeddings endpoint; there is no model name to report.
        ""
    }

    async fn embed(&self, _request: &EmbeddingRequest) -> LlmResult<ProviderEmbeddingOutput> {
        // Returning zero vectors here would be worse than failing: a zero vector indexes cleanly
        // and then silently ruins every similarity search that touches it.
        Err(LlmError::Unsupported {
            provider: PROVIDER.to_owned(),
            capability: "embeddings",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GenerationParams, Message};

    fn request_with(params: GenerationParams) -> ChatRequest {
        ChatRequest {
            messages: vec![
                Message::system("be terse"),
                Message::user("hello"),
                Message::assistant("hi"),
                Message::user("again"),
            ],
            params,
        }
    }

    fn basic_params() -> GenerationParams {
        GenerationParams {
            max_tokens: Some(1024),
            temperature: Some(0.4),
            ..Default::default()
        }
    }

    #[test]
    fn system_prompt_is_a_top_level_field_not_a_message() {
        let body =
            messages_body("claude-sonnet-4-5", &request_with(basic_params())).expect("body builds");

        assert_eq!(body["system"], "be terse");
        let messages = body["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 3, "the system turn is not a message");
        assert!(
            messages.iter().all(|m| m["role"] != "system"),
            "Anthropic rejects a system-role message"
        );
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[2]["role"], "user");
    }

    #[test]
    fn system_field_is_omitted_when_absent() {
        let req = ChatRequest::prompt("hi", None).with_params(basic_params());
        let body = messages_body("claude-sonnet-4-5", &req).expect("body builds");
        assert!(body.get("system").is_none());
    }

    #[test]
    fn max_tokens_is_required_by_the_api_and_by_us() {
        let req = ChatRequest::prompt("hi", None);
        let err = messages_body("claude-sonnet-4-5", &req).expect_err("max_tokens required");
        assert!(err.to_string().contains("max_tokens"));
    }

    #[test]
    fn stop_sequences_use_the_anthropic_field_name() {
        let params = GenerationParams {
            stop: vec!["STOP".into()],
            ..basic_params()
        };
        let body = messages_body("m", &request_with(params)).expect("body builds");
        assert_eq!(body["stop_sequences"][0], "STOP");
        assert!(
            body.get("stop").is_none(),
            "'stop' is the OpenAI field name"
        );
    }

    #[test]
    fn seed_is_refused_rather_than_silently_dropped() {
        let params = GenerationParams {
            seed: Some(7),
            ..basic_params()
        };
        let err = messages_body("m", &request_with(params)).expect_err("seed unsupported");
        assert!(err.to_string().contains("deterministic seeding"));
        assert!(!err.is_retryable());
    }

    #[test]
    fn parse_reads_the_documented_response_shape() {
        let body = json!({
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "model": "claude-sonnet-4-5-20250929",
            "content": [{ "type": "text", "text": "the answer" }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 20, "output_tokens": 7 }
        });
        let out = parse_messages("claude-sonnet-4-5", &body).expect("parses");

        assert_eq!(out.text, "the answer");
        assert_eq!(out.model, "claude-sonnet-4-5-20250929");
        assert_eq!(out.usage.prompt_tokens, Some(20));
        assert_eq!(out.usage.completion_tokens, Some(7));
        assert_eq!(out.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn end_turn_and_max_tokens_map_onto_the_common_reasons() {
        let truncated = json!({
            "content": [{ "type": "text", "text": "cut" }],
            "stop_reason": "max_tokens"
        });
        assert_eq!(
            parse_messages("m", &truncated)
                .expect("parses")
                .finish_reason,
            Some(FinishReason::Length)
        );
    }

    #[test]
    fn multiple_text_blocks_are_concatenated_in_order() {
        let body = json!({
            "content": [
                { "type": "text", "text": "part one " },
                { "type": "thinking", "thinking": "ignored" },
                { "type": "text", "text": "part two" }
            ]
        });
        let out = parse_messages("m", &body).expect("parses");
        assert_eq!(out.text, "part one part two");
    }

    #[test]
    fn a_response_with_no_text_block_is_an_error_not_an_empty_answer() {
        let body = json!({
            "content": [{ "type": "tool_use", "name": "x" }]
        });
        let err = parse_messages("m", &body).expect_err("no text block");
        assert!(err.to_string().contains("content[].text"));
    }

    #[test]
    fn an_empty_content_array_yields_empty_text() {
        // Distinct from the case above: the model genuinely returned nothing, which is a valid
        // (if unhelpful) response rather than a shape mismatch.
        let out = parse_messages("m", &json!({ "content": [] })).expect("parses");
        assert!(out.text.is_empty());
    }

    #[tokio::test]
    async fn embeddings_are_refused_not_faked() {
        let provider = AnthropicProvider::new(
            SecretString::new("k"),
            "https://api.anthropic.com/v1".into(),
            "2023-06-01".into(),
            "claude-sonnet-4-5".into(),
        );
        let err = provider
            .embed(&EmbeddingRequest::new(["x"]))
            .await
            .expect_err("no embeddings endpoint exists");
        assert!(err.to_string().contains("embeddings"));
    }

    #[test]
    fn debug_output_does_not_leak_the_key() {
        let provider = AnthropicProvider::new(
            SecretString::new("sk-ant-secret"),
            "https://api.anthropic.com/v1".into(),
            "2023-06-01".into(),
            "claude-sonnet-4-5".into(),
        );
        assert!(!format!("{provider:?}").contains("sk-ant-secret"));
    }
}
