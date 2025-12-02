//! Inference pipeline and job management.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::Result;
use crate::models::ModelRegistry;

/// Inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Batch size for inference
    pub batch_size: usize,

    /// Maximum timeout for inference in seconds
    pub timeout_seconds: u64,

    /// Enable model warmup
    pub enable_warmup: bool,

    /// Warmup iterations
    pub warmup_iterations: usize,

    /// Enable prediction caching
    pub enable_caching: bool,

    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,

    /// Output format
    pub output_format: OutputFormat,

    /// Include confidence scores
    pub include_confidence: bool,

    /// Include explanation/interpretability
    pub include_explanation: bool,

    /// Additional parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Output format options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Raw numerical output
    Raw,

    /// JSON formatted output
    Json,

    /// CSV formatted output
    Csv,

    /// Structured predictions with metadata
    Structured,
}

/// Inference job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceJob {
    /// Unique job identifier
    pub id: Uuid,

    /// Model name
    pub model_name: String,

    /// Input data
    pub input_data: Vec<u8>,

    /// Inference configuration
    pub config: InferenceConfig,

    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Inference result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    /// Job identifier
    pub job_id: Uuid,

    /// Model name
    pub model_name: String,

    /// Predictions
    pub predictions: Vec<Prediction>,

    /// Inference metrics
    pub metrics: InferenceMetrics,

    /// Processing timestamp
    pub processed_at: chrono::DateTime<chrono::Utc>,
}

/// Individual prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Prediction value(s)
    pub value: serde_json::Value,

    /// Confidence score (0.0 to 1.0)
    pub confidence: Option<f64>,

    /// Prediction probabilities (for classification)
    pub probabilities: Option<HashMap<String, f64>>,

    /// Explanation/interpretation
    pub explanation: Option<PredictionExplanation>,
}

/// Prediction explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionExplanation {
    /// Feature importance scores
    pub feature_importance: Option<HashMap<String, f64>>,

    /// SHAP values
    pub shap_values: Option<Vec<f64>>,

    /// Attention weights (for transformers)
    pub attention_weights: Option<Vec<Vec<f64>>>,

    /// Textual explanation
    pub text_explanation: Option<String>,
}

/// Inference metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceMetrics {
    /// Inference time in milliseconds
    pub inference_time_ms: f64,

    /// Preprocessing time in milliseconds
    pub preprocessing_time_ms: f64,

    /// Postprocessing time in milliseconds
    pub postprocessing_time_ms: f64,

    /// Total time in milliseconds
    pub total_time_ms: f64,

    /// Memory usage in bytes
    pub memory_usage_bytes: usize,

    /// Batch size processed
    pub batch_size: usize,

    /// Throughput (predictions per second)
    pub throughput: f64,
}

/// Predictor interface
pub struct Predictor {
    /// Inference configuration
    config: InferenceConfig,
    /// Optional reference to model registry for model lookup
    registry: Option<Arc<RwLock<ModelRegistry>>>,
}

impl Predictor {
    /// Create a new predictor without a registry
    pub fn new(config: InferenceConfig) -> Self {
        Self {
            config,
            registry: None,
        }
    }

    /// Create a new predictor with a model registry
    pub fn with_registry(config: InferenceConfig, registry: Arc<RwLock<ModelRegistry>>) -> Self {
        Self {
            config,
            registry: Some(registry),
        }
    }

    /// Run inference using raw bytes input
    ///
    /// # Arguments
    /// * `model_name` - Name of the model to use for inference
    /// * `input` - Raw input data as bytes
    ///
    /// # Returns
    /// Inference result containing predictions and metrics
    pub async fn predict(&self, model_name: &str, input: &[u8]) -> Result<InferenceResult> {
        let job_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();

        // Parse input as f64 features
        let preprocess_start = std::time::Instant::now();
        let features = self.parse_input(input)?;
        let preprocessing_time = preprocess_start.elapsed().as_secs_f64() * 1000.0;

        // Run inference
        let inference_start = std::time::Instant::now();
        let raw_predictions = self.run_model_inference(model_name, &features).await?;
        let inference_time = inference_start.elapsed().as_secs_f64() * 1000.0;

        // Postprocess predictions
        let postprocess_start = std::time::Instant::now();
        let predictions = self.postprocess_predictions(&raw_predictions);
        let postprocessing_time = postprocess_start.elapsed().as_secs_f64() * 1000.0;

        let total_time = start_time.elapsed().as_secs_f64() * 1000.0;
        let batch_size = self.calculate_batch_size(&features);
        let throughput = if total_time > 0.0 {
            (batch_size as f64 * 1000.0) / total_time
        } else {
            0.0
        };

        let metrics = InferenceMetrics {
            inference_time_ms: inference_time,
            preprocessing_time_ms: preprocessing_time,
            postprocessing_time_ms: postprocessing_time,
            total_time_ms: total_time,
            memory_usage_bytes: features.len() * std::mem::size_of::<f64>() + input.len(),
            batch_size,
            throughput,
        };

        Ok(InferenceResult {
            job_id,
            model_name: model_name.to_string(),
            predictions,
            metrics,
            processed_at: chrono::Utc::now(),
        })
    }

    /// Run inference with pre-parsed feature vectors
    ///
    /// # Arguments
    /// * `model_name` - Name of the model to use
    /// * `features` - Feature vector for inference
    ///
    /// # Returns
    /// Raw prediction values from the model
    pub async fn predict_features(&self, model_name: &str, features: &[f64]) -> Result<Vec<f64>> {
        self.run_model_inference(model_name, features).await
    }

    /// Parse input bytes into feature vector
    fn parse_input(&self, input: &[u8]) -> Result<Vec<f64>> {
        // Try to parse as JSON array of numbers first
        if let Ok(json_str) = std::str::from_utf8(input) {
            if let Ok(values) = serde_json::from_str::<Vec<f64>>(json_str) {
                return Ok(values);
            }
            // Try parsing as JSON object with "features" key
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(json_str) {
                if let Some(features) = obj.get("features") {
                    if let Ok(values) = serde_json::from_value::<Vec<f64>>(features.clone()) {
                        return Ok(values);
                    }
                }
            }
        }

        // Fallback: interpret as raw f64 bytes (little-endian)
        if input.len().is_multiple_of(8) {
            let features: Vec<f64> = input
                .chunks_exact(8)
                .map(|chunk| {
                    let bytes: [u8; 8] = chunk.try_into().unwrap();
                    f64::from_le_bytes(bytes)
                })
                .collect();
            return Ok(features);
        }

        // If all else fails, normalize bytes to [0, 1] range
        Ok(input.iter().map(|&b| b as f64 / 255.0).collect())
    }

    /// Run actual model inference
    async fn run_model_inference(&self, model_name: &str, features: &[f64]) -> Result<Vec<f64>> {
        // Try to get model from registry if available
        if let Some(registry) = &self.registry {
            let registry_guard = registry.read().await;
            if let Some(model) = registry_guard.get_model_instance(model_name) {
                // Run inference on the loaded model
                return model.predict(features).await;
            }
        }

        // Fallback: simple linear model for demonstration
        // In production, this would load the model from storage
        self.fallback_inference(features)
    }

    /// Fallback inference when no model is loaded
    /// Uses a simple linear transformation for demonstration
    fn fallback_inference(&self, features: &[f64]) -> Result<Vec<f64>> {
        // Simple aggregation-based prediction
        // Sum features with declining weights, normalize to [0, 1]
        if features.is_empty() {
            return Ok(vec![0.5]); // Default prediction
        }

        let weighted_sum: f64 = features
            .iter()
            .enumerate()
            .map(|(i, &f)| f * (1.0 / (i as f64 + 1.0)))
            .sum();

        let normalized = 1.0 / (1.0 + (-weighted_sum).exp()); // Sigmoid

        Ok(vec![normalized])
    }

    /// Convert raw predictions to Prediction structs
    fn postprocess_predictions(&self, raw: &[f64]) -> Vec<Prediction> {
        raw.iter()
            .map(|&value| {
                // For classification, compute confidence from prediction value
                let confidence = if self.config.include_confidence {
                    Some(compute_confidence(value))
                } else {
                    None
                };

                // For binary classification, compute probabilities
                let probabilities = if raw.len() == 1 {
                    let prob = value.clamp(0.0, 1.0);
                    Some(HashMap::from([
                        ("positive".to_string(), prob),
                        ("negative".to_string(), 1.0 - prob),
                    ]))
                } else if raw.len() > 1 {
                    // Softmax for multi-class
                    let softmax = softmax_normalize(raw);
                    Some(
                        softmax
                            .into_iter()
                            .enumerate()
                            .map(|(i, p)| (format!("class_{}", i), p))
                            .collect(),
                    )
                } else {
                    None
                };

                Prediction {
                    value: serde_json::Number::from_f64(value)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null),
                    confidence,
                    probabilities,
                    explanation: None,
                }
            })
            .collect()
    }

    /// Calculate effective batch size from features
    fn calculate_batch_size(&self, features: &[f64]) -> usize {
        // Assume features represent a single sample
        // For batched inference, this would be calculated differently
        if features.is_empty() {
            0
        } else {
            1.max(features.len() / self.config.batch_size.max(1))
        }
    }
}

/// Compute confidence score from prediction value
fn compute_confidence(value: f64) -> f64 {
    // Confidence is higher when prediction is closer to 0 or 1
    let distance_from_center = (value - 0.5).abs() * 2.0;
    0.5 + distance_from_center * 0.5
}

/// Apply softmax normalization to convert logits to probabilities
fn softmax_normalize(values: &[f64]) -> Vec<f64> {
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exp_sum: f64 = values.iter().map(|&v| (v - max_val).exp()).sum();
    values
        .iter()
        .map(|&v| (v - max_val).exp() / exp_sum)
        .collect()
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            batch_size: 1,
            timeout_seconds: 30,
            enable_warmup: false,
            warmup_iterations: 10,
            enable_caching: false,
            cache_ttl_seconds: 300,
            output_format: OutputFormat::Raw,
            include_confidence: false,
            include_explanation: false,
            parameters: HashMap::new(),
        }
    }
}

impl InferenceConfig {
    /// Create a new inference configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set batch size
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Set timeout
    pub fn timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Enable warmup
    pub fn enable_warmup(mut self, warmup_iterations: usize) -> Self {
        self.enable_warmup = true;
        self.warmup_iterations = warmup_iterations;
        self
    }

    /// Enable caching
    pub fn enable_caching(mut self, ttl_seconds: u64) -> Self {
        self.enable_caching = true;
        self.cache_ttl_seconds = ttl_seconds;
        self
    }

    /// Set output format
    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    /// Include confidence scores
    pub fn include_confidence(mut self) -> Self {
        self.include_confidence = true;
        self
    }

    /// Include explanations
    pub fn include_explanation(mut self) -> Self {
        self.include_explanation = true;
        self
    }

    /// Set parameter
    pub fn set_parameter(mut self, key: &str, value: serde_json::Value) -> Self {
        self.parameters.insert(key.to_string(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_config_default() {
        let config = InferenceConfig::default();
        assert_eq!(config.batch_size, 1);
        assert_eq!(config.timeout_seconds, 30);
        assert!(!config.enable_warmup);
        assert!(!config.enable_caching);
    }

    #[test]
    fn test_inference_config_builder() {
        let config = InferenceConfig::new()
            .batch_size(16)
            .timeout_seconds(60)
            .enable_warmup(5)
            .enable_caching(600)
            .include_confidence()
            .include_explanation()
            .set_parameter(
                "temperature",
                serde_json::Value::Number(serde_json::Number::from_f64(0.7).unwrap()),
            );

        assert_eq!(config.batch_size, 16);
        assert_eq!(config.timeout_seconds, 60);
        assert!(config.enable_warmup);
        assert_eq!(config.warmup_iterations, 5);
        assert!(config.enable_caching);
        assert_eq!(config.cache_ttl_seconds, 600);
        assert!(config.include_confidence);
        assert!(config.include_explanation);
        assert!(config.parameters.contains_key("temperature"));
    }

    #[tokio::test]
    async fn test_predictor_creation() {
        let config = InferenceConfig::default();
        let predictor = Predictor::new(config);

        let result = predictor
            .predict("test_model", b"test_input")
            .await
            .unwrap();
        assert_eq!(result.model_name, "test_model");
        assert_eq!(result.predictions.len(), 1);
        assert!(result.metrics.total_time_ms >= 0.0);
    }

    #[test]
    fn test_prediction_serialization() {
        let prediction = Prediction {
            value: serde_json::Value::Number(serde_json::Number::from_f64(0.75).unwrap()),
            confidence: Some(0.9),
            probabilities: Some(HashMap::from([
                ("class_a".to_string(), 0.75),
                ("class_b".to_string(), 0.25),
            ])),
            explanation: None,
        };

        let json = serde_json::to_string(&prediction).unwrap();
        let deserialized: Prediction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.confidence, Some(0.9));
    }
}
