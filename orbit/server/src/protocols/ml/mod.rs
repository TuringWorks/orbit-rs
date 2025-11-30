//! Machine Learning Module for Orbit-RS
//!
//! Provides in-database machine learning capabilities accessible via SQL functions.
//! Supports distributed training, real-time inference, and seamless integration
//! with the existing SQL engine and vector operations.

pub mod engines;
pub mod functions;
pub mod models;
pub mod sql_integration;

use orbit_shared::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ML function execution result
pub type MLResult<T> = OrbitResult<T>;

/// ML function execution error
pub type MLError = OrbitError;

use orbit_ml::models::{FeatureSpec, FeatureType, ModelMetadata, OutputSpec, OutputType};

// Local re-exports or type aliases if needed
// For now we use orbit_ml types directly

/// Training request for ML models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainModelRequest {
    /// Model name
    pub model_name: String,
    /// Algorithm to use
    pub algorithm: String,
    /// Training features
    pub features: Vec<Vec<f64>>,
    /// Training targets (optional for unsupervised)
    pub targets: Option<Vec<f64>>,
    /// Training parameters
    pub parameters: HashMap<String, String>,
    /// Validation split ratio
    pub validation_split: Option<f64>,
}

/// Prediction request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictRequest {
    /// Model name
    pub model_name: String,
    /// Input features
    pub features: Vec<Vec<f64>>,
    /// Return prediction probabilities (for classification)
    pub return_probabilities: bool,
}

/// Prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    /// Predictions
    pub predictions: Vec<f64>,
    /// Prediction probabilities (if requested)
    pub probabilities: Option<Vec<Vec<f64>>>,
    /// Model metadata
    pub model_metadata: ModelMetadata,
}

/// Model evaluation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateRequest {
    /// Model name
    pub model_name: String,
    /// Test features
    pub test_features: Vec<Vec<f64>>,
    /// Test targets
    pub test_targets: Vec<f64>,
    /// Evaluation metrics to compute
    pub metrics: Vec<String>,
}

/// Model evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Computed metrics
    pub metrics: HashMap<String, f64>,
    /// Predictions on test set
    pub predictions: Vec<f64>,
    /// Model metadata
    pub model_metadata: ModelMetadata,
}

/// Model update request for online learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateModelRequest {
    /// Model name
    pub model_name: String,
    /// New training features
    pub new_features: Vec<Vec<f64>>,
    /// New training targets
    pub new_targets: Option<Vec<f64>>,
    /// Update parameters
    pub parameters: HashMap<String, String>,
}

/// ML function registry for SQL integration
pub struct MLFunctionRegistry {
    /// Registered ML functions
    functions: HashMap<String, Box<dyn MLFunction>>,
    /// Model storage backend
    #[allow(dead_code)]
    model_storage: Box<dyn ModelStorage>,
}

impl MLFunctionRegistry {
    /// Create new ML function registry
    pub fn new(model_storage: Box<dyn ModelStorage>) -> Self {
        Self {
            functions: HashMap::new(),
            model_storage,
        }
    }

    /// Register an ML function
    pub fn register_function(&mut self, name: String, function: Box<dyn MLFunction>) {
        self.functions.insert(name, function);
    }

    /// Execute an ML function
    pub async fn execute_function(&self, name: &str, args: Vec<MLValue>) -> MLResult<MLValue> {
        match self.functions.get(name) {
            Some(function) => function.execute(args).await,
            None => Err(MLError::internal(format!("Unknown ML function: {name}"))),
        }
    }
}

/// Trait for ML function implementations
#[async_trait::async_trait]
pub trait MLFunction: Send + Sync {
    /// Execute the ML function
    async fn execute(&self, args: Vec<MLValue>) -> MLResult<MLValue>;

    /// Get function signature
    fn signature(&self) -> FunctionSignature;
}

/// ML function signature
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    /// Function name
    pub name: String,
    /// Input parameters
    pub parameters: Vec<ParameterSpec>,
    /// Return type
    pub return_type: FeatureType,
    /// Function description
    pub description: String,
}

/// Parameter specification
#[derive(Debug, Clone)]
pub struct ParameterSpec {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: FeatureType,
    /// Whether parameter is required
    pub required: bool,
    /// Default value (if any)
    pub default: Option<String>,
}

/// ML value types used in function execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MLValue {
    /// Null value
    Null,
    /// Boolean value
    Boolean(bool),
    /// Integer value
    Integer(i64),
    /// Float value
    Float(f64),
    /// String value
    String(String),
    /// Vector value
    Vector(Vec<f64>),
    /// Array of values
    Array(Vec<MLValue>),
    /// Map/object value
    Map(HashMap<String, MLValue>),
}

/// Trait for model storage backends
#[async_trait::async_trait]
pub trait ModelStorage: Send + Sync {
    /// Save a trained model
    async fn save_model(&self, metadata: &ModelMetadata, model_data: &[u8]) -> MLResult<()>;

    /// Load a trained model
    async fn load_model(&self, model_name: &str) -> MLResult<(ModelMetadata, Vec<u8>)>;

    /// List available models
    async fn list_models(&self) -> MLResult<Vec<ModelMetadata>>;

    /// Delete a model
    async fn delete_model(&self, model_name: &str) -> MLResult<()>;

    /// Get model metadata
    async fn get_metadata(&self, model_name: &str) -> MLResult<ModelMetadata>;
}

/// Utility functions for ML operations
pub mod utils {
    use super::{MLError, MLResult};

    /// Convert SQL array to vector
    pub fn sql_array_to_vector(array: &[f64]) -> Vec<f64> {
        array.to_vec()
    }

    /// Convert vector to SQL array format
    pub fn vector_to_sql_array(vector: &[f64]) -> Vec<f64> {
        vector.to_vec()
    }

    /// Validate feature dimensions
    pub fn validate_features(features: &[Vec<f64>], expected_dim: usize) -> MLResult<()> {
        for (i, feature) in features.iter().enumerate() {
            if feature.len() != expected_dim {
                return Err(MLError::internal(format!(
                    "Feature {} has dimension {}, expected {}",
                    i,
                    feature.len(),
                    expected_dim
                )));
            }
        }
        Ok(())
    }

    /// Calculate basic statistics
    pub fn calculate_stats(values: &[f64]) -> (f64, f64, f64, f64) {
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        let count = values.len() as f64;

        (mean, std_dev, variance, count)
    }
}

#[cfg(test)]
mod tests {
    use super::{utils, FeatureSpec, FeatureType, MLValue};
    use std::collections::HashMap;

    #[test]
    fn test_feature_spec_creation() {
        let spec = FeatureSpec {
            name: "age".to_string(),
            dtype: FeatureType::Float64,
            required: true,
            constraints: HashMap::new(),
        };

        assert_eq!(spec.name, "age");
        assert!(spec.required);
    }

    #[test]
    fn test_ml_value_types() {
        let _int_val = MLValue::Integer(42);
        let _float_val = MLValue::Float(std::f64::consts::PI);
        let vector_val = MLValue::Vector(vec![1.0, 2.0, 3.0]);

        match _int_val {
            MLValue::Integer(i) => assert_eq!(i, 42),
            _ => panic!("Expected integer value"),
        }

        match vector_val {
            MLValue::Vector(v) => assert_eq!(v.len(), 3),
            _ => panic!("Expected vector value"),
        }
    }

    #[test]
    fn test_utils_calculate_stats() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (mean, std_dev, variance, count) = utils::calculate_stats(&values);

        assert!((mean - 3.0).abs() < 1e-10);
        assert_eq!(count, 5.0);
        assert!(variance > 0.0);
        assert!(std_dev > 0.0);
    }
}
