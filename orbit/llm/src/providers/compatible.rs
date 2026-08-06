//! Any endpoint speaking OpenAI's `/chat/completions`.
//!
//! One implementation covers Azure OpenAI, vLLM, Groq, Together, OpenRouter, LM Studio, DeepSeek,
//! Fireworks, and any local server that implements the same route. The differences are entirely in
//! authentication and URL construction, which is what [`CompatibleFlavor`] selects.
//!
//! Note the `max_tokens` choice: third-party servers implement the original field name, not
//! OpenAI's newer `max_completion_tokens`. See [`super::openai_shape::MaxTokensField`].

use super::openai_shape::{
    chat_body, embedding_body, parse_chat, parse_embeddings, MaxTokensField,
};
use crate::config::CompatibleFlavor;
use crate::error::LlmResult;
use crate::http::{parse_json, shared_client, transport_error};
use crate::provider::{
    EmbeddingProvider, LlmProvider, ProviderChatOutput, ProviderEmbeddingOutput, ProviderKind,
};
use crate::secret::SecretString;
use crate::types::{ChatRequest, EmbeddingRequest};
use serde_json::Value;

const PROVIDER: &str = "compatible";

/// Client for an OpenAI-compatible endpoint.
#[derive(Debug, Clone)]
pub struct CompatibleProvider {
    flavor: CompatibleFlavor,
    api_key: Option<SecretString>,
    base_url: String,
    api_version: Option<String>,
    model: String,
    embedding_model: String,
}

impl CompatibleProvider {
    /// Build a client.
    ///
    /// When no embedding model is named the chat model is reused: many compatible servers host a
    /// single model and serve both routes from it. Reporting the model back in the response means
    /// this substitution is visible rather than assumed.
    pub fn new(
        flavor: CompatibleFlavor,
        api_key: Option<SecretString>,
        base_url: String,
        api_version: Option<String>,
        model: String,
        embedding_model: Option<String>,
    ) -> Self {
        let embedding_model = embedding_model.unwrap_or_else(|| model.clone());
        Self {
            flavor,
            api_key,
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_version,
            model,
            embedding_model,
        }
    }

    /// Full URL for a route.
    ///
    /// Azure puts the deployment name in the path and the API version in the query string; every
    /// other flavor appends the route to the base URL.
    #[must_use]
    pub fn url_for(&self, route: Route, model: &str) -> String {
        match self.flavor {
            CompatibleFlavor::Generic => format!("{}{}", self.base_url, route.generic_path()),
            CompatibleFlavor::AzureOpenAi => {
                let version = self.api_version.as_deref().unwrap_or_default();
                format!(
                    "{}/openai/deployments/{model}{}?api-version={version}",
                    self.base_url,
                    route.generic_path()
                )
            }
        }
    }

    fn request(&self, url: String) -> reqwest::RequestBuilder {
        let builder = shared_client().post(url);
        match (&self.api_key, self.flavor) {
            (Some(key), CompatibleFlavor::AzureOpenAi) => builder.header("api-key", key.expose()),
            (Some(key), CompatibleFlavor::Generic) => builder.bearer_auth(key.expose()),
            // Unauthenticated local servers are the common case for vLLM and LM Studio.
            (None, _) => builder,
        }
    }
}

/// Routes this provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Text generation.
    Chat,
    /// Embeddings.
    Embeddings,
}

impl Route {
    const fn generic_path(self) -> &'static str {
        match self {
            Route::Chat => "/chat/completions",
            Route::Embeddings => "/embeddings",
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for CompatibleProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Compatible
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(&self, request: &ChatRequest) -> LlmResult<ProviderChatOutput> {
        let body = chat_body(&self.model, request, MaxTokensField::Legacy);
        let response = self
            .request(self.url_for(Route::Chat, &self.model))
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(PROVIDER, e))?;

        let json: Value = parse_json(PROVIDER, response).await?;
        parse_chat(PROVIDER, &self.model, &json)
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for CompatibleProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Compatible
    }

    fn embedding_model(&self) -> &str {
        &self.embedding_model
    }

    async fn embed(&self, request: &EmbeddingRequest) -> LlmResult<ProviderEmbeddingOutput> {
        let body = embedding_body(&self.embedding_model, request);
        let response = self
            .request(self.url_for(Route::Embeddings, &self.embedding_model))
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(PROVIDER, e))?;

        let json: Value = parse_json(PROVIDER, response).await?;
        parse_embeddings(PROVIDER, &self.embedding_model, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generic() -> CompatibleProvider {
        CompatibleProvider::new(
            CompatibleFlavor::Generic,
            Some(SecretString::new("gsk-test")),
            "https://api.groq.com/openai/v1/".into(),
            None,
            "llama-3.3-70b".into(),
            None,
        )
    }

    fn azure() -> CompatibleProvider {
        CompatibleProvider::new(
            CompatibleFlavor::AzureOpenAi,
            Some(SecretString::new("azure-key")),
            "https://contoso.openai.azure.com".into(),
            Some("2024-10-21".into()),
            "gpt-4o-deployment".into(),
            Some("embed-deployment".into()),
        )
    }

    #[test]
    fn generic_url_appends_the_route() {
        assert_eq!(
            generic().url_for(Route::Chat, "llama-3.3-70b"),
            "https://api.groq.com/openai/v1/chat/completions"
        );
        assert_eq!(
            generic().url_for(Route::Embeddings, "m"),
            "https://api.groq.com/openai/v1/embeddings"
        );
    }

    #[test]
    fn azure_url_carries_the_deployment_and_api_version() {
        assert_eq!(
            azure().url_for(Route::Chat, "gpt-4o-deployment"),
            "https://contoso.openai.azure.com/openai/deployments/gpt-4o-deployment/chat/completions?api-version=2024-10-21"
        );
        assert_eq!(
            azure().url_for(Route::Embeddings, "embed-deployment"),
            "https://contoso.openai.azure.com/openai/deployments/embed-deployment/embeddings?api-version=2024-10-21"
        );
    }

    #[test]
    fn compatible_servers_get_the_legacy_max_tokens_field() {
        let request = ChatRequest::prompt("hi", None).with_params(crate::types::GenerationParams {
            max_tokens: Some(128),
            ..Default::default()
        });
        let body = chat_body("llama-3.3-70b", &request, MaxTokensField::Legacy);
        assert_eq!(body["max_tokens"], 128);
        assert!(
            body.get("max_completion_tokens").is_none(),
            "third-party servers implement the original field name"
        );
    }

    #[test]
    fn embedding_model_defaults_to_the_chat_model() {
        assert_eq!(generic().embedding_model(), "llama-3.3-70b");
        assert_eq!(azure().embedding_model(), "embed-deployment");
    }

    #[test]
    fn unauthenticated_local_servers_are_supported() {
        let local = CompatibleProvider::new(
            CompatibleFlavor::Generic,
            None,
            "http://localhost:8000/v1".into(),
            None,
            "Qwen3-8B".into(),
            None,
        );
        assert_eq!(
            local.url_for(Route::Chat, "Qwen3-8B"),
            "http://localhost:8000/v1/chat/completions"
        );
        assert_eq!(LlmProvider::kind(&local), ProviderKind::Compatible);
    }

    #[test]
    fn debug_output_does_not_leak_the_key() {
        assert!(!format!("{:?}", generic()).contains("gsk-test"));
        assert!(!format!("{:?}", azure()).contains("azure-key"));
    }
}
