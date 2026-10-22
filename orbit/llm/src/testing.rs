//! In-process stub provider used to test the router without HTTP.
//!
//! The router's job — retry, timeout, breaker, fallback, accounting — is the part most likely to be
//! wrong and the part hardest to exercise against a real provider. A stub makes each of those
//! behaviors assertable deterministically, which is why the registry exposes
//! [`crate::LlmRegistry::register_with_provider`] as a seam.

use crate::error::LlmResult;
use crate::provider::{
    EmbeddingProvider, LlmProvider, ProviderChatOutput, ProviderEmbeddingOutput, ProviderKind,
};
use crate::providers::BuiltProvider;
use crate::types::{ChatRequest, EmbeddingRequest, FinishReason, TokenUsage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

type Responder = Box<dyn Fn(usize) -> LlmResult<String> + Send + Sync>;

/// A provider whose every response is scripted.
pub struct StubProvider {
    responder: Responder,
    calls: AtomicUsize,
    delay: Option<Duration>,
    model: String,
    usage: TokenUsage,
}

impl StubProvider {
    /// Build a stub whose responder receives the 0-based call index.
    pub fn new(responder: impl Fn(usize) -> LlmResult<String> + Send + Sync + 'static) -> Self {
        Self {
            responder: Box::new(responder),
            calls: AtomicUsize::new(0),
            delay: None,
            model: "stub-model".to_owned(),
            usage: TokenUsage {
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
            },
        }
    }

    /// Make every call take `delay`, to exercise the timeout path.
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Report a specific model name.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Report specific token counts, or none at all.
    #[must_use]
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = usage;
        self
    }

    /// How many times this stub has been called.
    #[must_use]
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    async fn next_response(&self) -> LlmResult<String> {
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        (self.responder)(index)
    }
}

#[async_trait::async_trait]
impl LlmProvider for StubProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Compatible
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn generate(&self, _request: &ChatRequest) -> LlmResult<ProviderChatOutput> {
        let text = self.next_response().await?;
        Ok(ProviderChatOutput {
            text,
            model: self.model.clone(),
            usage: self.usage,
            finish_reason: Some(FinishReason::Stop),
        })
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for StubProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Compatible
    }

    fn embedding_model(&self) -> &str {
        &self.model
    }

    async fn embed(&self, request: &EmbeddingRequest) -> LlmResult<ProviderEmbeddingOutput> {
        // Deterministic vectors derived from input length, so alignment is checkable.
        let text = self.next_response().await?;
        let base = text.len() as f32;
        Ok(ProviderEmbeddingOutput {
            embeddings: request
                .inputs
                .iter()
                .map(|input| vec![base, input.len() as f32])
                .collect(),
            model: self.model.clone(),
            usage: self.usage,
        })
    }
}

/// Wrap a stub as a [`BuiltProvider`], discarding the handle.
#[must_use]
pub fn stub_provider(
    responder: impl Fn(usize) -> LlmResult<String> + Send + Sync + 'static,
) -> BuiltProvider {
    built_from(Arc::new(StubProvider::new(responder)))
}

/// Wrap a stub as a [`BuiltProvider`], keeping a handle for call-count assertions.
#[must_use]
pub fn stub_with_handle(stub: StubProvider) -> (BuiltProvider, Arc<StubProvider>) {
    let stub = Arc::new(stub);
    (built_from(Arc::clone(&stub)), stub)
}

fn built_from(stub: Arc<StubProvider>) -> BuiltProvider {
    BuiltProvider {
        llm: Arc::clone(&stub) as Arc<dyn LlmProvider>,
        embedding: stub as Arc<dyn EmbeddingProvider>,
    }
}

/// An [`crate::LlmError::Api`] with the given status, for scripting failures.
#[must_use]
pub fn api_error(status: u16) -> crate::LlmError {
    crate::LlmError::Api {
        provider: "stub".to_owned(),
        status,
        body: format!("scripted {status}"),
        retry_after: None,
    }
}
