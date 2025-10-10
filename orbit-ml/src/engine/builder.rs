use crate::config::MLConfig;
use crate::engine::MLEngine;
use crate::error::Result;

#[derive(Default)]
pub struct MLEngineBuilder {
    config: Option<MLConfig>,
}

impl MLEngineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(mut self, config: MLConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub async fn build(self) -> Result<MLEngine> {
        let config = self.config.unwrap_or_default();
        MLEngine::with_config(config).await
    }
}
