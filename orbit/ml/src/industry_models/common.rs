// Common traits and types for industry-specific ML models

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

/// Result type for industry model operations
pub type Result<T> = std::result::Result<T, IndustryModelError>;

/// Error types for industry models
#[derive(Debug, Clone)]
pub enum IndustryModelError {
    /// Training failed
    TrainingError(String),
    /// Prediction failed
    PredictionError(String),
    /// Evaluation failed
    EvaluationError(String),
    /// Model not found
    ModelNotFound(String),
    /// Invalid input data
    InvalidInput(String),
    /// Serialization error
    SerializationError(String),
}

impl fmt::Display for IndustryModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndustryModelError::TrainingError(msg) => write!(f, "Training error: {}", msg),
            IndustryModelError::PredictionError(msg) => write!(f, "Prediction error: {}", msg),
            IndustryModelError::EvaluationError(msg) => write!(f, "Evaluation error: {}", msg),
            IndustryModelError::ModelNotFound(msg) => write!(f, "Model not found: {}", msg),
            IndustryModelError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            IndustryModelError::SerializationError(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
        }
    }
}

impl Error for IndustryModelError {}

/// Metrics for evaluating model performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    /// Accuracy (0.0 to 1.0)
    pub accuracy: f64,
    /// Precision (0.0 to 1.0)
    pub precision: f64,
    /// Recall (0.0 to 1.0)
    pub recall: f64,
    /// F1 score (0.0 to 1.0)
    pub f1_score: f64,
    /// Area under ROC curve (optional, 0.0 to 1.0)
    pub auc_roc: Option<f64>,
    /// Mean absolute error (for regression tasks)
    pub mae: Option<f64>,
    /// Root mean squared error (for regression tasks)
    pub rmse: Option<f64>,
    /// Custom metrics specific to the model
    pub custom_metrics: Option<std::collections::HashMap<String, f64>>,
}

impl ModelMetrics {
    /// Create new metrics with default values
    pub fn new() -> Self {
        Self {
            accuracy: 0.0,
            precision: 0.0,
            recall: 0.0,
            f1_score: 0.0,
            auc_roc: None,
            mae: None,
            rmse: None,
            custom_metrics: None,
        }
    }

    /// Calculate F1 score from precision and recall
    pub fn calculate_f1(&mut self) {
        if self.precision + self.recall > 0.0 {
            self.f1_score = 2.0 * (self.precision * self.recall) / (self.precision + self.recall);
        }
    }

    /// Add a custom metric
    pub fn add_custom_metric(&mut self, name: String, value: f64) {
        if self.custom_metrics.is_none() {
            self.custom_metrics = Some(std::collections::HashMap::new());
        }
        if let Some(metrics) = &mut self.custom_metrics {
            metrics.insert(name, value);
        }
    }
}

impl Default for ModelMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Base trait for all industry-specific ML models
#[async_trait::async_trait]
pub trait IndustryModel: Send + Sync {
    /// Get the model type identifier
    fn model_type(&self) -> &str;

    /// Get the model version
    fn version(&self) -> &str;

    /// Train the model with the given data
    async fn train(&mut self, data: &[u8]) -> Result<ModelMetrics>;

    /// Make predictions on the given input
    async fn predict(&self, input: &[u8]) -> Result<Vec<f32>>;

    /// Evaluate the model on test data
    async fn evaluate(&self, test_data: &[u8]) -> Result<ModelMetrics>;

    /// Serialize the model to bytes
    fn serialize(&self) -> Result<Vec<u8>> {
        Err(IndustryModelError::SerializationError(
            "Serialization not implemented".to_string(),
        ))
    }

    /// Deserialize the model from bytes
    fn deserialize(&mut self, _data: &[u8]) -> Result<()> {
        Err(IndustryModelError::SerializationError(
            "Deserialization not implemented".to_string(),
        ))
    }
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model name
    pub name: String,
    /// Model version
    pub version: String,
    /// Hyperparameters
    pub hyperparameters: std::collections::HashMap<String, serde_json::Value>,
    /// Training configuration
    pub training_config: Option<TrainingConfig>,
}

/// Training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Number of epochs
    pub epochs: usize,
    /// Batch size
    pub batch_size: usize,
    /// Learning rate
    pub learning_rate: f64,
    /// Early stopping patience
    pub early_stopping_patience: Option<usize>,
    /// Validation split ratio
    pub validation_split: f64,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 100,
            batch_size: 32,
            learning_rate: 0.001,
            early_stopping_patience: Some(10),
            validation_split: 0.2,
        }
    }
}

/// Model registry for managing industry models
pub struct ModelRegistry {
    models: std::collections::HashMap<String, Box<dyn IndustryModel>>,
}

impl ModelRegistry {
    /// Create a new model registry
    pub fn new() -> Self {
        Self {
            models: std::collections::HashMap::new(),
        }
    }

    /// Register a model
    pub fn register(&mut self, name: String, model: Box<dyn IndustryModel>) {
        self.models.insert(name, model);
    }

    /// Get a model by name
    pub fn get(&self, name: &str) -> Option<&dyn IndustryModel> {
        self.models.get(name).map(|m| m.as_ref())
    }

    /// Get a mutable reference to a model by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut (dyn IndustryModel + '_)> {
        self.models.get_mut(name).map(move |m| m.as_mut())
    }

    /// List all registered models
    pub fn list_models(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }

    /// Remove a model from the registry
    pub fn remove(&mut self, name: &str) -> Option<Box<dyn IndustryModel>> {
        self.models.remove(name)
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_metrics_new() {
        let metrics = ModelMetrics::new();
        assert_eq!(metrics.accuracy, 0.0);
        assert_eq!(metrics.precision, 0.0);
        assert_eq!(metrics.recall, 0.0);
        assert_eq!(metrics.f1_score, 0.0);
    }

    #[test]
    fn test_model_metrics_calculate_f1() {
        let mut metrics = ModelMetrics::new();
        metrics.precision = 0.8;
        metrics.recall = 0.6;
        metrics.calculate_f1();
        assert!((metrics.f1_score - 0.6857).abs() < 0.001);
    }

    #[test]
    fn test_model_metrics_add_custom_metric() {
        let mut metrics = ModelMetrics::new();
        metrics.add_custom_metric("custom_score".to_string(), 0.95);
        assert!(metrics.custom_metrics.is_some());
        let custom = metrics.custom_metrics.unwrap();
        assert_eq!(custom.get("custom_score"), Some(&0.95));
    }

    #[test]
    fn test_model_registry() {
        let mut registry = ModelRegistry::new();
        assert_eq!(registry.list_models().len(), 0);
    }
}
