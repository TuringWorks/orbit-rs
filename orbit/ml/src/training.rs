//! Training pipeline and job management.

use std::collections::HashMap;
use std::sync::Arc;

use ndarray::Array2;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::Result;
use crate::neural_networks::{NeuralNetwork, Optimizer};

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

impl LossFunction {
    /// Compute loss between predictions and targets
    ///
    /// # Arguments
    /// * `predictions` - Model output predictions
    /// * `targets` - Ground truth target values
    ///
    /// # Returns
    /// The computed loss value
    pub fn compute(&self, predictions: &Array2<f64>, targets: &Array2<f64>) -> f64 {
        match self {
            LossFunction::MeanSquaredError => {
                let diff = predictions - targets;
                let squared = diff.mapv(|x| x * x);
                squared.mean().unwrap_or(0.0)
            }
            LossFunction::MeanAbsoluteError => {
                let diff = predictions - targets;
                let abs_diff = diff.mapv(|x| x.abs());
                abs_diff.mean().unwrap_or(0.0)
            }
            LossFunction::CrossEntropy => {
                // Cross-entropy: -sum(targets * log(predictions))
                let epsilon = 1e-15;
                let clipped = predictions.mapv(|x| x.max(epsilon).min(1.0 - epsilon));
                let log_pred = clipped.mapv(|x| x.ln());
                let loss = -(targets * &log_pred).sum();
                loss / predictions.shape()[0] as f64
            }
            LossFunction::BinaryCrossEntropy => {
                // Binary cross-entropy: -mean(targets * log(pred) + (1-targets) * log(1-pred))
                let epsilon = 1e-15;
                let clipped = predictions.mapv(|x| x.max(epsilon).min(1.0 - epsilon));
                let term1 = targets * clipped.mapv(|x| x.ln());
                let term2 = (1.0 - targets) * clipped.mapv(|x| (1.0 - x).ln());
                -(term1 + term2).mean().unwrap_or(0.0)
            }
            LossFunction::HuberLoss { delta } => {
                // Huber loss: L2 for small errors, L1 for large errors
                let diff = predictions - targets;
                let abs_diff = diff.mapv(|x| x.abs());
                let loss = abs_diff.mapv(|x| {
                    if x <= *delta {
                        0.5 * x * x
                    } else {
                        delta * (x - 0.5 * delta)
                    }
                });
                loss.mean().unwrap_or(0.0)
            }
            LossFunction::Custom { .. } => {
                // Fall back to MSE for custom loss functions
                let diff = predictions - targets;
                let squared = diff.mapv(|x| x * x);
                squared.mean().unwrap_or(0.0)
            }
        }
    }

    /// Compute loss gradient with respect to predictions
    ///
    /// # Arguments
    /// * `predictions` - Model output predictions
    /// * `targets` - Ground truth target values
    ///
    /// # Returns
    /// Gradient of loss with respect to predictions
    pub fn gradient(&self, predictions: &Array2<f64>, targets: &Array2<f64>) -> Array2<f64> {
        let batch_size = predictions.shape()[0] as f64;

        match self {
            LossFunction::MeanSquaredError => {
                // d/dx (mean((pred - target)^2)) = 2 * (pred - target) / batch_size
                (predictions - targets) * (2.0 / batch_size)
            }
            LossFunction::MeanAbsoluteError => {
                // d/dx (mean(|pred - target|)) = sign(pred - target) / batch_size
                let diff = predictions - targets;
                diff.mapv(|x| x.signum() / batch_size)
            }
            LossFunction::CrossEntropy => {
                // d/dx (-sum(target * log(pred))) = -target / pred
                let epsilon = 1e-15;
                let clipped = predictions.mapv(|x| x.max(epsilon).min(1.0 - epsilon));
                -(targets / &clipped) / batch_size
            }
            LossFunction::BinaryCrossEntropy => {
                // d/dx BCE = (pred - target) / (pred * (1 - pred))
                let epsilon = 1e-15;
                let clipped = predictions.mapv(|x| x.max(epsilon).min(1.0 - epsilon));
                let denom = &clipped * &clipped.mapv(|x| 1.0 - x);
                (predictions - targets) / (denom + epsilon) / batch_size
            }
            LossFunction::HuberLoss { delta } => {
                let diff = predictions - targets;
                diff.mapv(|x| {
                    if x.abs() <= *delta {
                        x / batch_size
                    } else {
                        delta * x.signum() / batch_size
                    }
                })
            }
            LossFunction::Custom { .. } => {
                // Fall back to MSE gradient
                (predictions - targets) * (2.0 / batch_size)
            }
        }
    }
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

    /// Start a new training job for the specified model (legacy interface)
    ///
    /// # Arguments
    /// * `model_name` - Name of the model to train
    /// * `_data` - Training data as bytes (not used in legacy interface)
    ///
    /// # Returns
    /// A new training job instance in Queued state
    pub async fn train(&self, model_name: &str, _data: &[u8]) -> Result<TrainingJob> {
        let job = TrainingJob {
            id: Uuid::new_v4(),
            model_name: model_name.to_string(),
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

    /// Train a neural network with the provided data
    ///
    /// # Arguments
    /// * `network` - Neural network to train (wrapped in Arc<RwLock>)
    /// * `train_x` - Training input data [samples, features]
    /// * `train_y` - Training target data [samples, outputs]
    /// * `val_x` - Optional validation input data
    /// * `val_y` - Optional validation target data
    ///
    /// # Returns
    /// Training history with losses and metrics per epoch
    pub async fn train_network(
        &self,
        network: Arc<RwLock<Box<dyn NeuralNetwork>>>,
        train_x: &Array2<f64>,
        train_y: &Array2<f64>,
        val_x: Option<&Array2<f64>>,
        val_y: Option<&Array2<f64>>,
    ) -> Result<TrainingHistory> {
        let mut history = TrainingHistory::new();
        let num_samples = train_x.shape()[0];
        let num_batches = num_samples.div_ceil(self.config.batch_size);

        // Create optimizer
        let optimizer = self.create_optimizer();

        // Early stopping state
        let mut best_val_loss = f64::MAX;
        let mut patience_counter = 0;
        let mut best_weights: Option<Vec<u8>> = None;

        for _epoch in 0..self.config.epochs {
            let mut epoch_loss = 0.0;
            let mut epoch_correct = 0usize;
            let mut epoch_total = 0usize;

            // Mini-batch training
            for batch_idx in 0..num_batches {
                let start = batch_idx * self.config.batch_size;
                let end = (start + self.config.batch_size).min(num_samples);
                let batch_size = end - start;

                // Extract batch using safe slice operations
                let batch_x = extract_batch(train_x, start, end);
                let batch_y = extract_batch(train_y, start, end);

                // Forward pass
                let predictions = {
                    let net = network.read().await;
                    net.forward(&batch_x).await?
                };

                // Compute loss
                let batch_loss = self.config.loss_function.compute(&predictions, &batch_y);
                epoch_loss += batch_loss * batch_size as f64;

                // Compute accuracy for classification tasks
                if self.config.metrics.contains(&"accuracy".to_string()) {
                    let (correct, total) = compute_accuracy(&predictions, &batch_y);
                    epoch_correct += correct;
                    epoch_total += total;
                }

                // Compute loss gradient
                let loss_gradient = self.config.loss_function.gradient(&predictions, &batch_y);

                // Backward pass and weight update
                {
                    let mut net = network.write().await;
                    net.backward(&loss_gradient).await?;
                    net.update_weights(optimizer.as_ref()).await?;
                }
            }

            // Compute epoch metrics
            let avg_loss = epoch_loss / num_samples as f64;
            history.train_loss.push(avg_loss);

            if epoch_total > 0 {
                let accuracy = epoch_correct as f64 / epoch_total as f64;
                history.train_accuracy.push(accuracy);
            }

            // Validation
            if let (Some(vx), Some(vy)) = (val_x, val_y) {
                let val_predictions = {
                    let net = network.read().await;
                    net.forward(vx).await?
                };
                let val_loss = self.config.loss_function.compute(&val_predictions, vy);
                history.val_loss.push(val_loss);

                if self.config.metrics.contains(&"accuracy".to_string()) {
                    let (correct, total) = compute_accuracy(&val_predictions, vy);
                    if total > 0 {
                        history.val_accuracy.push(correct as f64 / total as f64);
                    }
                }

                // Early stopping check
                if let Some(ref es_config) = self.config.early_stopping {
                    if val_loss < best_val_loss - es_config.min_delta {
                        best_val_loss = val_loss;
                        patience_counter = 0;

                        if es_config.restore_best_weights {
                            let net = network.read().await;
                            best_weights = Some(net.save_weights().await?);
                        }
                    } else {
                        patience_counter += 1;
                        if patience_counter >= es_config.patience {
                            // Restore best weights if configured
                            if es_config.restore_best_weights {
                                if let Some(ref weights) = best_weights {
                                    let mut net = network.write().await;
                                    net.load_weights(weights).await?;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        history.epochs_completed = history.train_loss.len();
        Ok(history)
    }

    /// Create an optimizer from the configuration
    fn create_optimizer(&self) -> Box<dyn Optimizer> {
        use crate::neural_networks::optimizers::{
            AdaGradOptimizer, AdamOptimizer, RMSpropOptimizer, SGDOptimizer,
        };

        match &self.config.optimizer {
            OptimizerType::Sgd { momentum } => Box::new(SGDOptimizer::new(
                self.config.learning_rate,
                momentum.unwrap_or(0.0),
            )),
            OptimizerType::Adam {
                beta1,
                beta2,
                epsilon,
            } => Box::new(AdamOptimizer::new(
                self.config.learning_rate,
                *beta1,
                *beta2,
                *epsilon,
            )),
            OptimizerType::AdaGrad { epsilon } => {
                Box::new(AdaGradOptimizer::new(self.config.learning_rate, *epsilon))
            }
            OptimizerType::RmsProp { alpha, epsilon } => Box::new(RMSpropOptimizer::new(
                self.config.learning_rate,
                *alpha,
                *epsilon,
            )),
        }
    }
}

/// Training history tracking losses and metrics per epoch
#[derive(Debug, Clone, Default)]
pub struct TrainingHistory {
    /// Training loss per epoch
    pub train_loss: Vec<f64>,
    /// Validation loss per epoch
    pub val_loss: Vec<f64>,
    /// Training accuracy per epoch
    pub train_accuracy: Vec<f64>,
    /// Validation accuracy per epoch
    pub val_accuracy: Vec<f64>,
    /// Number of epochs completed
    pub epochs_completed: usize,
}

impl TrainingHistory {
    /// Create a new empty training history
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the final training loss
    pub fn final_train_loss(&self) -> Option<f64> {
        self.train_loss.last().copied()
    }

    /// Get the final validation loss
    pub fn final_val_loss(&self) -> Option<f64> {
        self.val_loss.last().copied()
    }

    /// Get the best (lowest) validation loss
    pub fn best_val_loss(&self) -> Option<f64> {
        self.val_loss
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
    }
}

/// Compute classification accuracy
fn compute_accuracy(predictions: &Array2<f64>, targets: &Array2<f64>) -> (usize, usize) {
    let mut correct = 0;
    let total = predictions.shape()[0];

    for i in 0..total {
        let pred_row = predictions.row(i);
        let target_row = targets.row(i);

        // Find argmax for both
        let pred_class = pred_row
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let target_class = target_row
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        if pred_class == target_class {
            correct += 1;
        }
    }

    (correct, total)
}

/// Extract a batch of rows from an array (safe alternative to slicing)
fn extract_batch(data: &Array2<f64>, start: usize, end: usize) -> Array2<f64> {
    let num_cols = data.shape()[1];
    let batch_size = end - start;

    let mut batch = Array2::zeros((batch_size, num_cols));
    for (batch_row, data_row) in (start..end).enumerate() {
        for col in 0..num_cols {
            batch[[batch_row, col]] = data[[data_row, col]];
        }
    }
    batch
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
    use ndarray::array;

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

    #[test]
    fn test_mse_loss() {
        let predictions = array![[1.0, 2.0], [3.0, 4.0]];
        let targets = array![[1.0, 2.0], [3.0, 4.0]];

        let loss = LossFunction::MeanSquaredError.compute(&predictions, &targets);
        assert!(loss.abs() < 1e-10, "MSE should be 0 for identical arrays");

        let predictions2 = array![[1.0, 2.0], [3.0, 4.0]];
        let targets2 = array![[2.0, 3.0], [4.0, 5.0]];
        let loss2 = LossFunction::MeanSquaredError.compute(&predictions2, &targets2);
        assert!((loss2 - 1.0).abs() < 1e-10, "MSE should be 1.0");
    }

    #[test]
    fn test_mae_loss() {
        let predictions = array![[1.0, 2.0], [3.0, 4.0]];
        let targets = array![[2.0, 3.0], [4.0, 5.0]];

        let loss = LossFunction::MeanAbsoluteError.compute(&predictions, &targets);
        assert!((loss - 1.0).abs() < 1e-10, "MAE should be 1.0");
    }

    #[test]
    fn test_huber_loss() {
        let predictions = array![[0.0], [0.0]];
        let targets = array![[0.5], [0.5]];

        let loss = LossFunction::HuberLoss { delta: 1.0 }.compute(&predictions, &targets);
        // For |error| < delta, Huber = 0.5 * error^2 = 0.5 * 0.25 = 0.125
        assert!((loss - 0.125).abs() < 1e-10);
    }

    #[test]
    fn test_loss_gradient() {
        let predictions = array![[1.0], [2.0]];
        let targets = array![[1.5], [2.5]];

        let grad = LossFunction::MeanSquaredError.gradient(&predictions, &targets);
        // Gradient should be 2 * (pred - target) / batch_size
        // = 2 * [-0.5, -0.5] / 2 = [-0.5, -0.5]
        assert!((grad[[0, 0]] - (-0.5)).abs() < 1e-10);
        assert!((grad[[1, 0]] - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn test_compute_accuracy() {
        let predictions = array![[0.1, 0.9], [0.8, 0.2], [0.3, 0.7]];
        let targets = array![[0.0, 1.0], [1.0, 0.0], [0.0, 1.0]];

        let (correct, total) = compute_accuracy(&predictions, &targets);
        assert_eq!(total, 3);
        assert_eq!(correct, 3); // All predictions match targets
    }

    #[test]
    fn test_training_history() {
        let mut history = TrainingHistory::new();
        history.train_loss.push(1.0);
        history.train_loss.push(0.5);
        history.train_loss.push(0.25);
        history.val_loss.push(1.1);
        history.val_loss.push(0.6);
        history.val_loss.push(0.3);

        assert_eq!(history.final_train_loss(), Some(0.25));
        assert_eq!(history.final_val_loss(), Some(0.3));
        assert_eq!(history.best_val_loss(), Some(0.3));
    }

    #[test]
    fn test_create_optimizer_sgd() {
        let config = TrainingConfig::new()
            .learning_rate(0.01)
            .optimizer(OptimizerType::Sgd {
                momentum: Some(0.9),
            });
        let trainer = Trainer::new(config);
        let optimizer = trainer.create_optimizer();
        assert_eq!(optimizer.learning_rate(), 0.01);
    }

    #[test]
    fn test_create_optimizer_adam() {
        let config = TrainingConfig::new()
            .learning_rate(0.001)
            .optimizer(OptimizerType::Adam {
                beta1: 0.9,
                beta2: 0.999,
                epsilon: 1e-8,
            });
        let trainer = Trainer::new(config);
        let optimizer = trainer.create_optimizer();
        assert_eq!(optimizer.learning_rate(), 0.001);
    }
}
