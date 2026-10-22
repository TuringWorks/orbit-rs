//! Ollama's native API.
//!
//! Ollama uses `/api/chat` with generation knobs nested under `options`, and names the token cap
//! `num_predict`. It *does* report token counts (`prompt_eval_count` / `eval_count`), so usage is
//! genuinely available here — unlike most local OpenAI-compatible servers.

use crate::error::LlmResult;
use crate::http::{missing_field, parse_json, shared_client, transport_error};
use crate::provider::{
    EmbeddingProvider, LlmProvider, ProviderChatOutput, ProviderEmbeddingOutput, ProviderKind,
};
use crate::types::{ChatRequest, EmbeddingRequest, FinishReason, TokenUsage};
use serde_json::{json, Map, Value};

const PROVIDER: &str = "ollama";

/// Embedding model used when a profile names none.
const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text";

/// Client for a local or remote Ollama daemon.
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    base_url: String,
    model: String,
    embedding_model: String,
}

impl OllamaProvider {
    /// Build a client.
    pub fn new(base_url: String, model: String, embedding_model: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            model,
            embedding_model: embedding_model.unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.to_owned()),
        }
    }
}

/// Build an `/api/chat` request body.
///
/// `stream` is explicitly `false`: Ollama streams by default, and an unset flag would produce a
/// newline-delimited body that the single-object parser cannot read.
#[must_use]
pub fn chat_body(model: &str, request: &ChatRequest) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
        .collect();

    let params = &request.params;
    let mut options = Map::new();
    if let Some(temperature) = params.temperature {
        options.insert("temperature".into(), json!(temperature));
    }
    if let Some(max_tokens) = params.max_tokens {
        options.insert("num_predict".into(), json!(max_tokens));
    }
    if let Some(top_p) = params.top_p {
        options.insert("top_p".into(), json!(top_p));
    }
    if let Some(seed) = params.seed {
        options.insert("seed".into(), json!(seed));
    }
    if !params.stop.is_empty() {
        options.insert("stop".into(), json!(params.stop));
    }

    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("messages".into(), json!(messages));
    body.insert("stream".into(), json!(false));
    if !options.is_empty() {
        body.insert("options".into(), Value::Object(options));
    }

    Value::Object(body)
}

/// Parse an `/api/chat` response.
///
/// # Errors
///
/// Returns [`crate::LlmError::MalformedResponse`] when `message.content` is absent.
pub fn parse_chat(fallback_model: &str, body: &Value) -> LlmResult<ProviderChatOutput> {
    let text = body
        .pointer("/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| missing_field(PROVIDER, "message.content"))?
        .to_owned();

    Ok(ProviderChatOutput {
        text,
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_owned(),
        usage: TokenUsage {
            prompt_tokens: body
                .get("prompt_eval_count")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
            completion_tokens: body
                .get("eval_count")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
        },
        finish_reason: body
            .get("done_reason")
            .and_then(Value::as_str)
            .map(FinishReason::from_wire),
    })
}

/// Build an `/api/embed` request body.
#[must_use]
pub fn embedding_body(model: &str, request: &EmbeddingRequest) -> Value {
    json!({ "model": model, "input": request.inputs })
}

/// Parse an `/api/embed` response.
///
/// # Errors
///
/// Returns [`crate::LlmError::MalformedResponse`] when `embeddings` is absent.
pub fn parse_embeddings(fallback_model: &str, body: &Value) -> LlmResult<ProviderEmbeddingOutput> {
    let embeddings = body
        .get("embeddings")
        .and_then(Value::as_array)
        .ok_or_else(|| missing_field(PROVIDER, "embeddings"))?
        .iter()
        .map(|vector| {
            vector
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .map(|v| v.as_f64().unwrap_or_default() as f32)
                        .collect::<Vec<f32>>()
                })
                .ok_or_else(|| missing_field(PROVIDER, "embeddings[]"))
        })
        .collect::<LlmResult<Vec<_>>>()?;

    Ok(ProviderEmbeddingOutput {
        embeddings,
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_owned(),
        usage: TokenUsage {
            prompt_tokens: body
                .get("prompt_eval_count")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
            completion_tokens: Some(0),
        },
    })
}

#[async_trait::async_trait]
impl LlmProvider for OllamaProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(&self, request: &ChatRequest) -> LlmResult<ProviderChatOutput> {
        let body = chat_body(&self.model, request);
        let response = shared_client()
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(PROVIDER, e))?;

        let json: Value = parse_json(PROVIDER, response).await?;
        parse_chat(&self.model, &json)
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for OllamaProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    fn embedding_model(&self) -> &str {
        &self.embedding_model
    }

    async fn embed(&self, request: &EmbeddingRequest) -> LlmResult<ProviderEmbeddingOutput> {
        let body = embedding_body(&self.embedding_model, request);
        let response = shared_client()
            .post(format!("{}/api/embed", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(PROVIDER, e))?;

        let json: Value = parse_json(PROVIDER, response).await?;
        parse_embeddings(&self.embedding_model, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GenerationParams, Message};

    #[test]
    fn generation_knobs_go_under_options_with_ollama_names() {
        let request = ChatRequest {
            messages: vec![Message::system("sys"), Message::user("hi")],
            params: GenerationParams {
                temperature: Some(0.5),
                max_tokens: Some(256),
                top_p: Some(0.9),
                stop: vec!["END".into()],
                seed: Some(3),
            },
        };
        let body = chat_body("llama3.2", &request);

        assert_eq!(body["model"], "llama3.2");
        assert_eq!(body["stream"], false, "the single-object parser needs this");
        assert_eq!(body["options"]["temperature"], 0.5);
        assert_eq!(
            body["options"]["num_predict"], 256,
            "Ollama's name for the token cap"
        );
        // `top_p` is an `f32`; serializing widens it to the nearest `f64`, so 0.9 is not exact.
        let top_p = body["options"]["top_p"].as_f64().expect("number");
        assert!((top_p - 0.9).abs() < 1e-6, "got {top_p}");
        assert_eq!(body["options"]["seed"], 3);
        assert_eq!(body["options"]["stop"][0], "END");
        assert!(
            body.get("max_tokens").is_none(),
            "top-level max_tokens is ignored by Ollama"
        );
    }

    #[test]
    fn the_system_turn_stays_a_message() {
        let request = ChatRequest::prompt("hi", Some("sys".into()));
        let body = chat_body("llama3.2", &request);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "sys");
    }

    #[test]
    fn options_is_omitted_when_nothing_is_configured() {
        let body = chat_body("llama3.2", &ChatRequest::prompt("hi", None));
        assert!(body.get("options").is_none());
    }

    #[test]
    fn parse_reads_ollamas_token_counters() {
        let body = json!({
            "model": "llama3.2",
            "message": { "role": "assistant", "content": "answer" },
            "done": true,
            "done_reason": "stop",
            "prompt_eval_count": 31,
            "eval_count": 12
        });
        let out = parse_chat("llama3.2", &body).expect("parses");

        assert_eq!(out.text, "answer");
        assert_eq!(out.usage.prompt_tokens, Some(31));
        assert_eq!(out.usage.completion_tokens, Some(12));
        assert_eq!(out.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn a_length_stop_is_reported_as_truncation() {
        let body = json!({
            "message": { "content": "cut" },
            "done_reason": "length"
        });
        assert_eq!(
            parse_chat("m", &body).expect("parses").finish_reason,
            Some(FinishReason::Length)
        );
    }

    #[test]
    fn a_missing_message_is_an_error() {
        let err = parse_chat("m", &json!({ "done": true })).expect_err("no message");
        assert!(err.to_string().contains("message.content"));
    }

    #[test]
    fn embeddings_round_trip() {
        let body = embedding_body("nomic-embed-text", &EmbeddingRequest::new(["a", "b"]));
        assert_eq!(body["input"][1], "b");

        let response = json!({
            "model": "nomic-embed-text",
            "embeddings": [[0.1, 0.2], [0.3, 0.4]],
            "prompt_eval_count": 4
        });
        let out = parse_embeddings("nomic-embed-text", &response).expect("parses");
        assert_eq!(out.embeddings.len(), 2);
        assert_eq!(out.embeddings[1], vec![0.3, 0.4]);
        assert_eq!(out.usage.prompt_tokens, Some(4));
    }

    #[test]
    fn a_missing_embeddings_array_is_an_error() {
        let err = parse_embeddings("m", &json!({})).expect_err("no embeddings");
        assert!(err.to_string().contains("embeddings"));
    }

    #[test]
    fn defaults_are_named_not_guessed() {
        let p = OllamaProvider::new("http://localhost:11434/".into(), "llama3.2".into(), None);
        assert_eq!(p.base_url, "http://localhost:11434");
        assert_eq!(p.embedding_model(), DEFAULT_EMBEDDING_MODEL);
        assert_eq!(LlmProvider::kind(&p), ProviderKind::Ollama);
    }
}
