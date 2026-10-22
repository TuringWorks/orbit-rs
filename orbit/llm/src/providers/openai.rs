//! OpenAI's own API.

use super::openai_shape::{
    chat_body, embedding_body, parse_chat, parse_embeddings, MaxTokensField,
};
use crate::error::LlmResult;
use crate::http::{parse_json, shared_client, transport_error};
use crate::provider::{
    EmbeddingProvider, LlmProvider, ProviderChatOutput, ProviderEmbeddingOutput, ProviderKind,
};
use crate::secret::SecretString;
use crate::types::{ChatRequest, EmbeddingRequest};
use serde_json::Value;

const PROVIDER: &str = "openai";

/// Model used when a profile requests embeddings without naming an embedding model.
///
/// Unlike a fabricated default *value*, this is a documented fallback identifier the caller can
/// override; it is reported back in the response so nobody has to guess what ran.
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";

/// Client for `api.openai.com` and API-identical proxies.
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    api_key: SecretString,
    base_url: String,
    organization: Option<String>,
    project: Option<String>,
    model: String,
    embedding_model: String,
}

impl OpenAiProvider {
    /// Build a client.
    pub fn new(
        api_key: SecretString,
        base_url: String,
        organization: Option<String>,
        project: Option<String>,
        model: String,
        embedding_model: Option<String>,
    ) -> Self {
        Self {
            api_key,
            base_url: base_url.trim_end_matches('/').to_owned(),
            organization,
            project,
            model,
            embedding_model: embedding_model.unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.to_owned()),
        }
    }

    fn request(&self, path: &str) -> reqwest::RequestBuilder {
        let builder = shared_client()
            .post(format!("{}{path}", self.base_url))
            .bearer_auth(self.api_key.expose());

        let builder = match &self.organization {
            Some(org) => builder.header("OpenAI-Organization", org),
            None => builder,
        };
        match &self.project {
            Some(project) => builder.header("OpenAI-Project", project),
            None => builder,
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAi
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(&self, request: &ChatRequest) -> LlmResult<ProviderChatOutput> {
        let body = chat_body(&self.model, request, MaxTokensField::Completion);
        let response = self
            .request("/chat/completions")
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(PROVIDER, e))?;

        let json: Value = parse_json(PROVIDER, response).await?;
        parse_chat(PROVIDER, &self.model, &json)
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for OpenAiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAi
    }

    fn embedding_model(&self) -> &str {
        &self.embedding_model
    }

    async fn embed(&self, request: &EmbeddingRequest) -> LlmResult<ProviderEmbeddingOutput> {
        let body = embedding_body(&self.embedding_model, request);
        let response = self
            .request("/embeddings")
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

    fn provider() -> OpenAiProvider {
        OpenAiProvider::new(
            SecretString::new("sk-test"),
            "https://api.openai.com/v1/".into(),
            Some("org-1".into()),
            None,
            "gpt-4o-mini".into(),
            None,
        )
    }

    #[test]
    fn trailing_slash_in_base_url_does_not_double_up() {
        let p = provider();
        assert_eq!(p.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn embedding_model_falls_back_to_a_named_default() {
        assert_eq!(provider().embedding_model(), DEFAULT_EMBEDDING_MODEL);

        let explicit = OpenAiProvider::new(
            SecretString::new("k"),
            "https://api.openai.com/v1".into(),
            None,
            None,
            "gpt-4o".into(),
            Some("text-embedding-3-large".into()),
        );
        assert_eq!(explicit.embedding_model(), "text-embedding-3-large");
    }

    #[test]
    fn declares_its_kind_and_model() {
        let p = provider();
        assert_eq!(LlmProvider::kind(&p), ProviderKind::OpenAi);
        assert_eq!(p.model(), "gpt-4o-mini");
    }

    #[test]
    fn debug_output_does_not_leak_the_key() {
        assert!(!format!("{:?}", provider()).contains("sk-test"));
    }
}
