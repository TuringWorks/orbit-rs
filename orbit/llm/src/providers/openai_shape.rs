//! The OpenAI `/chat/completions` and `/embeddings` wire shape.
//!
//! Shared by [`super::openai`] and [`super::compatible`], because Azure OpenAI, vLLM, Groq,
//! Together, OpenRouter, LM Studio, DeepSeek, and Fireworks all speak it. The two callers differ
//! only in authentication and URL construction.
//!
//! Body construction and response parsing are pure functions taking and returning values, so the
//! emitted wire format is asserted by unit tests rather than by reading the code.

use crate::error::LlmResult;
use crate::http::missing_field;
use crate::provider::{ProviderChatOutput, ProviderEmbeddingOutput};
use crate::types::{ChatRequest, EmbeddingRequest, FinishReason, TokenUsage};
use serde_json::{json, Map, Value};

/// Which field name to use for the output-token cap.
///
/// OpenAI's own API renamed `max_tokens` to `max_completion_tokens` and rejects the old name for
/// reasoning models (o-series, GPT-5). Third-party OpenAI-compatible servers overwhelmingly
/// implement only the original `max_tokens`. Sending the wrong one is a hard 400, so the choice is
/// made per caller rather than guessed from the model name — a name-prefix heuristic would break
/// the first time a vendor ships a model that does not match the pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxTokensField {
    /// `max_tokens` — third-party compatible servers.
    Legacy,
    /// `max_completion_tokens` — OpenAI's current API.
    Completion,
}

impl MaxTokensField {
    const fn as_str(self) -> &'static str {
        match self {
            MaxTokensField::Legacy => "max_tokens",
            MaxTokensField::Completion => "max_completion_tokens",
        }
    }
}

/// Build a `/chat/completions` request body.
///
/// Every parameter present in `request.params` appears in the output. There is no path by which a
/// configured temperature is accepted and then dropped.
#[must_use]
pub fn chat_body(model: &str, request: &ChatRequest, max_tokens_field: MaxTokensField) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
        .collect();

    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("messages".into(), json!(messages));

    let params = &request.params;
    if let Some(temperature) = params.temperature {
        body.insert("temperature".into(), json!(temperature));
    }
    if let Some(max_tokens) = params.max_tokens {
        body.insert(max_tokens_field.as_str().into(), json!(max_tokens));
    }
    if let Some(top_p) = params.top_p {
        body.insert("top_p".into(), json!(top_p));
    }
    if !params.stop.is_empty() {
        body.insert("stop".into(), json!(params.stop));
    }
    if let Some(seed) = params.seed {
        body.insert("seed".into(), json!(seed));
    }

    Value::Object(body)
}

/// Parse a `/chat/completions` response.
///
/// # Errors
///
/// Returns [`crate::LlmError::MalformedResponse`] when the documented fields are absent.
pub fn parse_chat(
    provider: &str,
    fallback_model: &str,
    body: &Value,
) -> LlmResult<ProviderChatOutput> {
    let choice = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .ok_or_else(|| missing_field(provider, "choices[0]"))?;

    let text = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| missing_field(provider, "choices[0].message.content"))?
        .to_owned();

    Ok(ProviderChatOutput {
        text,
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_owned(),
        usage: parse_usage(body),
        finish_reason: choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(FinishReason::from_wire),
    })
}

/// Read the `usage` object, leaving unreported counts absent.
fn parse_usage(body: &Value) -> TokenUsage {
    let field = |name: &str| {
        body.pointer(&format!("/usage/{name}"))
            .and_then(Value::as_u64)
            .map(|v| v as u32)
    };
    TokenUsage {
        prompt_tokens: field("prompt_tokens"),
        completion_tokens: field("completion_tokens"),
    }
}

/// Build an `/embeddings` request body.
#[must_use]
pub fn embedding_body(model: &str, request: &EmbeddingRequest) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("input".into(), json!(request.inputs));
    if let Some(dimensions) = request.dimensions {
        body.insert("dimensions".into(), json!(dimensions));
    }
    Value::Object(body)
}

/// Parse an `/embeddings` response.
///
/// Vectors are reordered by the response's `index` field. OpenAI documents that the `data` array
/// may not be in request order, and returning vectors misaligned with their inputs is a silent
/// data-corruption bug that no error surfaces.
///
/// # Errors
///
/// Returns [`crate::LlmError::MalformedResponse`] when `data` is absent or an entry lacks an
/// embedding.
pub fn parse_embeddings(
    provider: &str,
    fallback_model: &str,
    body: &Value,
) -> LlmResult<ProviderEmbeddingOutput> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| missing_field(provider, "data"))?;

    let mut indexed: Vec<(usize, Vec<f32>)> = data
        .iter()
        .enumerate()
        .map(|(position, entry)| {
            let vector = entry
                .get("embedding")
                .and_then(Value::as_array)
                .ok_or_else(|| missing_field(provider, "data[].embedding"))?
                .iter()
                .map(|v| v.as_f64().unwrap_or_default() as f32)
                .collect();
            let index = entry
                .get("index")
                .and_then(Value::as_u64)
                .map_or(position, |i| i as usize);
            Ok((index, vector))
        })
        .collect::<LlmResult<Vec<_>>>()?;

    indexed.sort_by_key(|(index, _)| *index);

    Ok(ProviderEmbeddingOutput {
        embeddings: indexed.into_iter().map(|(_, vector)| vector).collect(),
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_owned(),
        usage: TokenUsage {
            prompt_tokens: body
                .pointer("/usage/prompt_tokens")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
            // An embeddings call generates no completion tokens; that is a fact about the
            // operation, not an unreported measurement, so zero is the honest value.
            completion_tokens: Some(0),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GenerationParams, Message};

    /// Assert a JSON number matches an `f32`-sourced value after widening to `f64`.
    fn assert_close(actual: &Value, expected: f32) {
        let actual = actual
            .as_f64()
            .unwrap_or_else(|| panic!("not a number: {actual}"));
        assert!(
            (actual - f64::from(expected)).abs() < 1e-6,
            "expected ~{expected}, got {actual}"
        );
    }

    fn request() -> ChatRequest {
        ChatRequest {
            messages: vec![Message::system("be terse"), Message::user("hello")],
            params: GenerationParams {
                temperature: Some(0.25),
                max_tokens: Some(512),
                top_p: Some(0.8),
                stop: vec!["END".into()],
                seed: Some(42),
            },
        }
    }

    #[test]
    fn chat_body_carries_every_configured_parameter() {
        let body = chat_body("gpt-4o-mini", &request(), MaxTokensField::Legacy);

        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "be terse");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["max_tokens"], 512);
        // Compared with a tolerance: `GenerationParams` holds `f32`, and serializing widens to the
        // nearest f64, so 0.25 is exact but 0.8 is not.
        assert_close(&body["temperature"], 0.25);
        assert_close(&body["top_p"], 0.8);
        assert_eq!(body["stop"][0], "END");
        assert_eq!(body["seed"], 42);
    }

    #[test]
    fn openai_native_uses_max_completion_tokens() {
        let body = chat_body("gpt-5", &request(), MaxTokensField::Completion);
        assert_eq!(body["max_completion_tokens"], 512);
        assert!(
            body.get("max_tokens").is_none(),
            "sending both names is a 400 on OpenAI's reasoning models"
        );
    }

    #[test]
    fn unset_parameters_are_omitted_not_defaulted() {
        let bare = ChatRequest::prompt("hi", None);
        let body = chat_body("m", &bare, MaxTokensField::Legacy);

        for absent in ["temperature", "max_tokens", "top_p", "stop", "seed"] {
            assert!(
                body.get(absent).is_none(),
                "{absent} must be absent, not defaulted — a default temperature is a claim we did not make"
            );
        }
        assert_eq!(body["messages"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn parse_chat_reads_the_documented_shape() {
        let body = json!({
            "model": "gpt-4o-mini-2024-07-18",
            "choices": [{
                "message": { "role": "assistant", "content": "hi there" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 12, "completion_tokens": 3, "total_tokens": 15 }
        });
        let out = parse_chat("openai", "gpt-4o-mini", &body).expect("parses");

        assert_eq!(out.text, "hi there");
        assert_eq!(out.model, "gpt-4o-mini-2024-07-18");
        assert_eq!(out.usage.prompt_tokens, Some(12));
        assert_eq!(out.usage.completion_tokens, Some(3));
        assert_eq!(out.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn parse_chat_leaves_unreported_usage_absent() {
        let body = json!({
            "choices": [{ "message": { "content": "x" } }]
        });
        let out = parse_chat("compatible", "local-model", &body).expect("parses");

        assert_eq!(
            out.model, "local-model",
            "falls back to the configured name"
        );
        assert_eq!(out.usage.prompt_tokens, None);
        assert_eq!(out.usage.completion_tokens, None);
        assert!(!out.usage.is_reported());
        assert_eq!(out.finish_reason, None);
    }

    #[test]
    fn parse_chat_reports_a_missing_body() {
        let err = parse_chat("openai", "m", &json!({ "choices": [] })).expect_err("no choices");
        assert!(err.to_string().contains("choices[0]"));

        let err = parse_chat("openai", "m", &json!({ "choices": [{}] })).expect_err("no content");
        assert!(err.to_string().contains("message.content"));
    }

    #[test]
    fn parse_chat_preserves_a_length_truncation() {
        let body = json!({
            "choices": [{ "message": { "content": "trunc" }, "finish_reason": "length" }]
        });
        let out = parse_chat("openai", "m", &body).expect("parses");
        assert_eq!(
            out.finish_reason,
            Some(FinishReason::Length),
            "a truncated answer must not read as a complete one"
        );
    }

    #[test]
    fn embedding_body_includes_dimensions_only_when_asked() {
        let plain = EmbeddingRequest::new(["a", "b"]);
        let body = embedding_body("text-embedding-3-small", &plain);
        assert_eq!(body["input"][0], "a");
        assert_eq!(body["input"][1], "b");
        assert!(body.get("dimensions").is_none());

        let truncated = EmbeddingRequest {
            inputs: vec!["a".into()],
            dimensions: Some(256),
        };
        assert_eq!(embedding_body("m", &truncated)["dimensions"], 256);
    }

    #[test]
    fn embeddings_are_realigned_with_their_inputs() {
        // OpenAI documents that `data` need not arrive in request order.
        let body = json!({
            "model": "text-embedding-3-small",
            "data": [
                { "index": 2, "embedding": [3.0] },
                { "index": 0, "embedding": [1.0] },
                { "index": 1, "embedding": [2.0] }
            ],
            "usage": { "prompt_tokens": 9 }
        });
        let out = parse_embeddings("openai", "m", &body).expect("parses");

        assert_eq!(
            out.embeddings,
            vec![vec![1.0], vec![2.0], vec![3.0]],
            "vectors misaligned with their inputs is silent data corruption"
        );
        assert_eq!(out.usage.prompt_tokens, Some(9));
        assert_eq!(out.usage.completion_tokens, Some(0));
    }

    #[test]
    fn embeddings_without_index_keep_response_order() {
        let body = json!({
            "data": [
                { "embedding": [1.0] },
                { "embedding": [2.0] }
            ]
        });
        let out = parse_embeddings("compatible", "m", &body).expect("parses");
        assert_eq!(out.embeddings, vec![vec![1.0], vec![2.0]]);
    }

    #[test]
    fn embeddings_report_a_missing_data_array() {
        let err = parse_embeddings("openai", "m", &json!({})).expect_err("no data");
        assert!(err.to_string().contains("'data'"));
    }
}
