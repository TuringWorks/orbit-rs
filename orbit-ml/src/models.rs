//! Model management and registry.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{MLError, Result};

/// Generic trait for ML models
#[async_trait]
pub trait Model: Send + Sync {
    /// Get model metadata
    fn metadata(&self) -> &ModelMetadata;

    /// Predict on input data
    async fn predict(&self, input: &[f64]) -> Result<Vec<f64>>;

    /// Get model size in bytes
    fn size_bytes(&self) -> usize;

    /// Check if model is loaded
    fn is_loaded(&self) -> bool;
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: Uuid,
    pub name: String,
    pub model_type: String,
    pub version: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub size_bytes: usize,
    pub parameters: HashMap<String, serde_json::Value>,
    pub metrics: HashMap<String, f64>,
    pub tags: Vec<String>,
}

/// Model registry for managing models
#[derive(Debug)]
pub struct ModelRegistry {
    models: HashMap<String, ModelMetadata>,
}

impl ModelRegistry {
    /// Create a new model registry
    pub async fn new() -> Result<Self> {
        Ok(Self {
            models: HashMap::new(),
        })
    }

    /// Register a new model
    pub async fn register_model(&mut self, metadata: ModelMetadata) -> Result<()> {
        self.models.insert(metadata.name.clone(), metadata);
        Ok(())
    }

    /// Get model metadata by name
    pub async fn get_model(&self, name: &str) -> Result<Option<ModelMetadata>> {
        Ok(self.models.get(name).cloned())
    }

    /// List all models
    pub async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        Ok(self.models.values().cloned().collect())
    }

    /// Delete a model
    pub async fn delete_model(&mut self, name: &str) -> Result<()> {
        if self.models.remove(name).is_some() {
            Ok(())
        } else {
            Err(MLError::not_found(format!("Model: {}", name)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_registry() {
        let mut registry = ModelRegistry::new().await.unwrap();

        let metadata = ModelMetadata {
            id: Uuid::new_v4(),
            name: "test_model".to_string(),
            model_type: "neural_network".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            size_bytes: 1024,
            parameters: HashMap::new(),
            metrics: HashMap::new(),
            tags: vec!["test".to_string()],
        };

        registry.register_model(metadata.clone()).await.unwrap();

        let retrieved = registry.get_model("test_model").await.unwrap().unwrap();
        assert_eq!(retrieved.name, "test_model");

        let models = registry.list_models().await.unwrap();
        assert_eq!(models.len(), 1);

        registry.delete_model("test_model").await.unwrap();
        let models = registry.list_models().await.unwrap();
        assert_eq!(models.len(), 0);
    }
}
