use crate::error::{MLError, Result};
use crate::models::{Model, ModelRegistry};
use std::sync::Arc;

#[derive(Debug)]
pub struct ModelManager {
    registry: Arc<ModelRegistry>,
}

impl ModelManager {
    pub async fn new(registry: Arc<ModelRegistry>) -> Result<Self> {
        Ok(Self { registry })
    }

    pub async fn load_model(&self, _name: &str) -> Result<Arc<dyn Model>> {
        Err(MLError::not_found("Model not implemented"))
    }

    pub async fn delete_model(&self, _name: &str) -> Result<()> {
        Ok(())
    }
}
