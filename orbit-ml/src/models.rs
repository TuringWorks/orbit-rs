//! Model management and registry.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{MLError, Result};

/// Generic trait for ML models providing core functionality
///
/// All models in the Orbit ML system implement this trait to provide
/// a consistent interface for inference, metadata access, and resource management.
#[async_trait]
pub trait Model: Send + Sync {
    /// Get model metadata including name, version, and performance metrics
    ///
    /// # Returns
    /// Reference to the model's metadata structure
    fn metadata(&self) -> &ModelMetadata;

    /// Perform inference on input data
    ///
    /// # Arguments
    /// * `input` - Input feature vector for prediction
    ///
    /// # Returns
    /// Prediction output vector or error if inference fails
    async fn predict(&self, input: &[f64]) -> Result<Vec<f64>>;

    /// Get the model's memory footprint in bytes
    ///
    /// # Returns
    /// Total size of model parameters and buffers in bytes
    fn size_bytes(&self) -> usize;

    /// Check if the model is currently loaded in memory
    ///
    /// # Returns
    /// True if model is ready for inference, false otherwise
    fn is_loaded(&self) -> bool;
}

/// Model metadata containing identification, configuration, and performance information
///
/// This structure stores all relevant information about a model including
/// its identity, version, performance metrics, and configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Unique model identifier
    pub id: Uuid,
    /// Human-readable model name
    pub name: String,
    /// Type of model (e.g., "neural_network", "decision_tree", "transformer")
    pub model_type: String,
    /// Semantic version string (e.g., "1.0.0")
    pub version: String,
    /// Optional human-readable description of the model
    pub description: Option<String>,
    /// Timestamp when the model was first created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp of the last model update
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Model size in bytes (parameters + metadata)
    pub size_bytes: usize,
    /// Model-specific configuration parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Performance metrics (accuracy, loss, etc.)
    pub metrics: HashMap<String, f64>,
    /// Categorical labels for model organization and search
    pub tags: Vec<String>,
}

/// Central registry for managing model metadata and lifecycle
///
/// Provides a centralized location for storing, retrieving, and managing
/// model metadata across the Orbit ML system.
#[derive(Debug)]
pub struct ModelRegistry {
    /// Storage for model metadata indexed by model name
    models: HashMap<String, ModelMetadata>,
}

impl ModelRegistry {
    /// Create a new empty model registry
    ///
    /// # Returns
    /// A new registry instance or error if initialization fails
    pub async fn new() -> Result<Self> {
        Ok(Self {
            models: HashMap::new(),
        })
    }

    /// Register a new model in the registry
    ///
    /// # Arguments
    /// * `metadata` - Complete model metadata to register
    ///
    /// # Returns
    /// Ok(()) if registration succeeds
    ///
    /// # Note
    /// If a model with the same name exists, it will be replaced
    pub async fn register_model(&mut self, metadata: ModelMetadata) -> Result<()> {
        self.models.insert(metadata.name.clone(), metadata);
        Ok(())
    }

    /// Retrieve model metadata by name
    ///
    /// # Arguments
    /// * `name` - Name of the model to retrieve
    ///
    /// # Returns
    /// Some(metadata) if model exists, None otherwise
    pub async fn get_model(&self, name: &str) -> Result<Option<ModelMetadata>> {
        Ok(self.models.get(name).cloned())
    }

    /// List metadata for all registered models
    ///
    /// # Returns
    /// Vector containing metadata for all models in the registry
    pub async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        Ok(self.models.values().cloned().collect())
    }

    /// Remove a model from the registry
    ///
    /// # Arguments
    /// * `name` - Name of the model to delete
    ///
    /// # Returns
    /// Ok(()) if deletion succeeds, error if model not found
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
