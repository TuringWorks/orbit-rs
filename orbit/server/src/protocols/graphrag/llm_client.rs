//! LLM Client for GraphRAG
//!
//! This module provides LLM client implementations for various providers
//! including OpenAI, Anthropic, Ollama, and local LLM APIs.

use orbit_shared::graphrag::LLMProvider;
use orbit_shared::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};

/// LLM generation request
#[derive(Debug, Clone)]
pub struct LLMGenerationRequest {
    /// Prompt text
    pub prompt: String,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature for generation
    pub temperature: Option<f32>,
    /// System message (optional)
    pub system_message: Option<String>,
}

/// LLM generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMGenerationResponse {
    /// Generated text
    pub text: String,
    /// Tokens used
    pub tokens_used: Option<u32>,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Model used
    pub model: String,
}

/// LLM client trait
#[async_trait::async_trait]
pub trait LLMClient: Send + Sync {
    /// Generate text from a prompt
    async fn generate(&self, request: LLMGenerationRequest) -> OrbitResult<LLMGenerationResponse>;

    /// Get the model name
    fn model_name(&self) -> &str;
}

/// OpenAI LLM client
pub struct OpenAIClient {
    api_key: String,
    model: String,
    base_url: String,
    default_temperature: f32,
    default_max_tokens: u32,
}

impl OpenAIClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://api.openai.com/v1".to_string(),
            default_temperature: 0.7,
            default_max_tokens: 2048,
        }
    }
}

#[async_trait::async_trait]
impl LLMClient for OpenAIClient {
    async fn generate(&self, request: LLMGenerationRequest) -> OrbitResult<LLMGenerationResponse> {
        use reqwest::Client;

        let client = Client::new();
        let url = format!("{}/chat/completions", self.base_url);

        let mut messages = Vec::new();
        if let Some(system_msg) = request.system_message {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_msg
            }));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": request.prompt
        }));

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": request.temperature.unwrap_or(self.default_temperature),
            "max_tokens": request.max_tokens.unwrap_or(self.default_max_tokens),
        });

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| OrbitError::internal(format!("OpenAI API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OrbitError::internal(format!(
                "OpenAI API error ({}): {}",
                status, error_text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to parse OpenAI response: {}", e)))?;

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| OrbitError::internal("Invalid OpenAI response format"))?
            .to_string();

        let tokens_used = json["usage"]["total_tokens"].as_u64().map(|v| v as u32);

        Ok(LLMGenerationResponse {
            text,
            tokens_used,
            finish_reason: json["choices"][0]["finish_reason"]
                .as_str()
                .map(|s| s.to_string()),
            model: self.model.clone(),
        })
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// Ollama LLM client
pub struct OllamaClient {
    model: String,
    base_url: String,
    default_temperature: f32,
}

impl OllamaClient {
    pub fn new(model: String) -> Self {
        Self {
            model,
            base_url: "http://localhost:11434".to_string(),
            default_temperature: 0.7,
        }
    }

    pub fn with_endpoint(model: String, endpoint: String) -> Self {
        Self {
            model,
            base_url: endpoint,
            default_temperature: 0.7,
        }
    }
}

#[async_trait::async_trait]
impl LLMClient for OllamaClient {
    async fn generate(&self, request: LLMGenerationRequest) -> OrbitResult<LLMGenerationResponse> {
        use reqwest::Client;

        let client = Client::new();
        let url = format!("{}/api/generate", self.base_url);

        let mut prompt = request.prompt;
        if let Some(system_msg) = request.system_message {
            prompt = format!("{}\n\n{}", system_msg, prompt);
        }

        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": request.temperature.unwrap_or(self.default_temperature),
                "num_predict": request.max_tokens.unwrap_or(2048),
            }
        });

        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| OrbitError::internal(format!("Ollama API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OrbitError::internal(format!(
                "Ollama API error ({}): {}",
                status, error_text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to parse Ollama response: {}", e)))?;

        let text = json["response"]
            .as_str()
            .ok_or_else(|| OrbitError::internal("Invalid Ollama response format"))?
            .to_string();

        Ok(LLMGenerationResponse {
            text,
            tokens_used: None,
            finish_reason: Some("stop".to_string()),
            model: self.model.clone(),
        })
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// Local LLM client (generic HTTP API)
pub struct LocalLLMClient {
    endpoint: String,
    model: String,
    default_temperature: f32,
    default_max_tokens: u32,
}

impl LocalLLMClient {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            endpoint,
            model,
            default_temperature: 0.7,
            default_max_tokens: 2048,
        }
    }
}

#[async_trait::async_trait]
impl LLMClient for LocalLLMClient {
    async fn generate(&self, request: LLMGenerationRequest) -> OrbitResult<LLMGenerationResponse> {
        use reqwest::Client;

        let client = Client::new();

        let mut prompt = request.prompt;
        if let Some(system_msg) = request.system_message {
            prompt = format!("{}\n\n{}", system_msg, prompt);
        }

        // Try OpenAI-compatible format first
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": request.temperature.unwrap_or(self.default_temperature),
            "max_tokens": request.max_tokens.unwrap_or(self.default_max_tokens),
        });

        let response = client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| OrbitError::internal(format!("Local LLM API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OrbitError::internal(format!(
                "Local LLM API error ({}): {}",
                status, error_text
            )));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| {
            OrbitError::internal(format!("Failed to parse local LLM response: {}", e))
        })?;

        // Try OpenAI-compatible format
        let text = if let Some(text) = json["choices"][0]["message"]["content"].as_str() {
            text.to_string()
        } else if let Some(text) = json["response"].as_str() {
            text.to_string()
        } else if let Some(text) = json["text"].as_str() {
            text.to_string()
        } else {
            return Err(OrbitError::internal("Invalid local LLM response format"));
        };

        Ok(LLMGenerationResponse {
            text,
            tokens_used: json["usage"]["total_tokens"].as_u64().map(|v| v as u32),
            finish_reason: json["choices"][0]["finish_reason"]
                .as_str()
                .map(|s| s.to_string()),
            model: self.model.clone(),
        })
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// Create LLM client from provider configuration
pub fn create_llm_client(provider: &LLMProvider) -> OrbitResult<Box<dyn LLMClient>> {
    match provider {
        LLMProvider::OpenAI {
            api_key,
            model,
            temperature: _,
            max_tokens: _,
        } => Ok(Box::new(OpenAIClient::new(api_key.clone(), model.clone()))),
        LLMProvider::Ollama {
            model,
            temperature: _,
        } => Ok(Box::new(OllamaClient::new(model.clone()))),
        LLMProvider::Local {
            endpoint,
            model,
            temperature: _,
            max_tokens: _,
        } => Ok(Box::new(LocalLLMClient::new(
            endpoint.clone(),
            model.clone(),
        ))),
        LLMProvider::Anthropic {
            api_key: _,
            model: _,
            temperature: _,
            max_tokens: _,
        } => {
            // TODO: Implement Anthropic client
            Err(OrbitError::internal("Anthropic client not yet implemented"))
        }
    }
}
