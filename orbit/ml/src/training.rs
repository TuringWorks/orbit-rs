//! Training pipeline and job management.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

/// Comprehensive training configuration for ML models
///
/// Contains all parameters needed to configure the training process including
/// optimization settings, regularization, checkpointing, and monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Number of training epochs (complete passes through the dataset)
    pub epochs: usize,

    /// Learning rate for gradient descent optimization
    pub learning_rate: f64,

    /// Mini-batch size for stochastic gradient descent
    pub batch_size: usize,

    /// Fraction of data to use for validation (0.0 to 1.0)
    pub validation_split: f64,

    /// Optimization algorithm configuration
    pub optimizer: OptimizerType,

    /// Loss function for training objective
    pub loss_function: LossFunction,

    /// List of metrics to compute and track during training
    pub metrics: Vec<String>,

    /// Optional early stopping configuration to prevent overfitting
    pub early_stopping: Option<EarlyStoppingConfig>,

    /// Model checkpointing configuration for recovery and best model saving
    pub checkpointing: CheckpointConfig,

    /// Additional model-specific hyperparameters as key-value pairs
    pub hyperparameters: HashMap<String, serde_json::Value>,
}

/// Optimizer types available for training
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptimizerType {
    /// Stochastic Gradient Descent
    Sgd {
        /// Optional momentum factor (0.0 to 1.0)
        momentum: Option<f64>,
    },

    /// Adam optimizer with adaptive learning rates
    Adam {
        /// Exponential decay rate for first moment estimates (default: 0.9)
        beta1: f64,
        /// Exponential decay rate for second moment estimates (default: 0.999)
        beta2: f64,
        /// Small epsilon value for numerical stability (default: 1e-8)
        epsilon: f64,
    },

    /// AdaGrad optimizer with adaptive learning rates
    AdaGrad {
        /// Small epsilon value for numerical stability
        epsilon: f64,
    },

    /// RMSprop optimizer
    RmsProp {
        /// Decay factor for moving average (default: 0.99)
        alpha: f64,
        /// Small epsilon value for numerical stability
        epsilon: f64,
    },
}

/// Loss functions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossFunction {
    /// Mean Squared Error
    MeanSquaredError,

    /// Mean Absolute Error
    MeanAbsoluteError,

    /// Cross Entropy Loss
    CrossEntropy,

    /// Binary Cross Entropy Loss
    BinaryCrossEntropy,

    /// Huber Loss (less sensitive to outliers than MSE)
    HuberLoss {
        /// Threshold parameter for switching between L1 and L2 loss
        delta: f64,
    },

    /// Custom loss function with configurable parameters
    Custom {
        /// Name identifier for the custom loss function
        name: String,
        /// Loss function parameters as key-value pairs
        parameters: HashMap<String, f64>,
    },
}

/// Early stopping configuration to prevent overfitting
///
/// Monitors a specified metric and stops training when no improvement
/// is seen for a configured number of epochs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyStoppingConfig {
    /// Name of the metric to monitor (e.g., "val_loss", "val_accuracy")
    pub monitor: String,

    /// Minimum change in monitored metric to qualify as improvement
    pub min_delta: f64,

    /// Number of epochs with no improvement after which training stops
    pub patience: usize,

    /// Whether to restore model weights from the best epoch
    pub restore_best_weights: bool,
}

/// Model checkpointing configuration for training recovery and best model saving
///
/// Configures automatic saving of model checkpoints during training to enable
/// recovery from failures and preservation of the best performing models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Whether to enable automatic checkpointing during training
    pub enabled: bool,

    /// Frequency of checkpoint saves (every N epochs)
    pub save_every_n_epochs: usize,

    /// Maximum number of checkpoint files to retain (oldest are deleted)
    pub max_checkpoints: usize,

    /// Whether to save only checkpoints that improve the monitored metric
    pub save_best_only: bool,

    /// Name of metric to monitor for determining "best" model
    pub monitor_metric: Option<String>,
}

/// Training job status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TrainingStatus {
    /// Job is queued
    Queued,

    /// Job is running
    Running,

    /// Job completed successfully
    Completed,

    /// Job failed
    Failed,

    /// Job was cancelled
    Cancelled,

    /// Job is paused
    Paused,
}

/// Training job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingJob {
    /// Unique job identifier
    pub id: Uuid,

    /// Model name
    pub model_name: String,

    /// Model type
    pub model_type: String,

    /// Job status
    pub status: TrainingStatus,

    /// Training configuration
    pub config: TrainingConfig,

    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Start timestamp
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Completion timestamp
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Training progress (0.0 to 1.0)
    pub progress: f64,

    /// Current loss value
    pub loss: Option<f64>,

    /// Training metrics
    pub metrics: HashMap<String, f64>,
}

/// Training orchestrator that manages the model training process
///
/// Coordinates training execution using the provided configuration,
/// manages training jobs, and handles training lifecycle events.
pub struct Trainer {
    /// Training configuration to use for all training jobs
    config: TrainingConfig,
}

impl Trainer {
    /// Create a new trainer with the specified configuration
    ///
    /// # Arguments
    /// * `config` - Training configuration to use for all training jobs
    pub fn new(config: TrainingConfig) -> Self {
        Self { config }
    }

    /// Start a new training job for the specified model
    ///
    /// # Arguments
    /// * `_model_name` - Name of the model to train
    /// * `_data` - Training data as bytes
    ///
    /// # Returns
    /// A new training job instance or error if job creation fails
    ///
    /// # Note
    /// This is currently a stub implementation. Full training logic will be added.
    pub async fn train(&self, _model_name: &str, _data: &[u8]) -> Result<TrainingJob> {
        // TODO: Implement actual training logic
        let job = TrainingJob {
            id: Uuid::new_v4(),
            model_name: _model_name.to_string(),
            model_type: "neural_network".to_string(),
            status: TrainingStatus::Queued,
            config: self.config.clone(),
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            progress: 0.0,
            loss: None,
            metrics: HashMap::new(),
        };

        Ok(job)
    }
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 100,
            learning_rate: 0.001,
            batch_size: 32,
            validation_split: 0.2,
            optimizer: OptimizerType::Adam {
                beta1: 0.9,
                beta2: 0.999,
                epsilon: 1e-8,
            },
            loss_function: LossFunction::MeanSquaredError,
            metrics: vec!["accuracy".to_string()],
            early_stopping: Some(EarlyStoppingConfig {
                monitor: "val_loss".to_string(),
                min_delta: 0.001,
                patience: 10,
                restore_best_weights: true,
            }),
            checkpointing: CheckpointConfig {
                enabled: true,
                save_every_n_epochs: 10,
                max_checkpoints: 5,
                save_best_only: true,
                monitor_metric: Some("val_loss".to_string()),
            },
            hyperparameters: HashMap::new(),
        }
    }
}

impl TrainingConfig {
    /// Create a new training configuration with default values
    ///
    /// # Returns
    /// A new training configuration with sensible defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of training epochs
    ///
    /// # Arguments
    /// * `epochs` - Number of complete passes through the training dataset
    pub fn epochs(mut self, epochs: usize) -> Self {
        self.epochs = epochs;
        self
    }

    /// Set the learning rate for optimization
    ///
    /// # Arguments
    /// * `lr` - Learning rate value (typically between 0.0001 and 0.1)
    pub fn learning_rate(mut self, lr: f64) -> Self {
        self.learning_rate = lr;
        self
    }

    /// Set the mini-batch size for training
    ///
    /// # Arguments
    /// * `size` - Number of samples per batch (powers of 2 are typically optimal)
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Set the optimization algorithm
    ///
    /// # Arguments
    /// * `optimizer` - Optimizer type and configuration
    pub fn optimizer(mut self, optimizer: OptimizerType) -> Self {
        self.optimizer = optimizer;
        self
    }

    /// Set the loss function for training
    ///
    /// # Arguments
    /// * `loss` - Loss function type and configuration
    pub fn loss_function(mut self, loss: LossFunction) -> Self {
        self.loss_function = loss;
        self
    }

    /// Add a metric to track during training
    ///
    /// # Arguments
    /// * `metric` - Name of the metric (e.g., "accuracy", "precision")
    pub fn add_metric(mut self, metric: &str) -> Self {
        self.metrics.push(metric.to_string());
        self
    }

    /// Set a custom hyperparameter value
    ///
    /// # Arguments
    /// * `key` - Parameter name
    /// * `value` - Parameter value as JSON
    pub fn set_hyperparameter(mut self, key: &str, value: serde_json::Value) -> Self {
        self.hyperparameters.insert(key.to_string(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_config_default() {
        let config = TrainingConfig::default();
        assert_eq!(config.epochs, 100);
        assert_eq!(config.learning_rate, 0.001);
        assert_eq!(config.batch_size, 32);
    }

    #[test]
    fn test_training_config_builder() {
        let config = TrainingConfig::new()
            .epochs(200)
            .learning_rate(0.01)
            .batch_size(64)
            .add_metric("precision")
            .set_hyperparameter(
                "dropout",
                serde_json::Value::Number(serde_json::Number::from_f64(0.5).unwrap()),
            );

        assert_eq!(config.epochs, 200);
        assert_eq!(config.learning_rate, 0.01);
        assert_eq!(config.batch_size, 64);
        assert!(config.metrics.contains(&"precision".to_string()));
        assert!(config.hyperparameters.contains_key("dropout"));
    }

    #[tokio::test]
    async fn test_trainer_creation() {
        let config = TrainingConfig::default();
        let trainer = Trainer::new(config);

        let job = trainer.train("test_model", b"test_data").await.unwrap();
        assert_eq!(job.status, TrainingStatus::Queued);
        assert_eq!(job.model_name, "test_model");
    }
}
