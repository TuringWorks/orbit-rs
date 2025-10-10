//! Neural Network implementations for Orbit ML.
//!
//! This module provides comprehensive neural network architectures including:
//! - Feedforward Neural Networks (FNN)
//! - Convolutional Neural Networks (CNN)
//! - Recurrent Neural Networks (RNN)
//! - Long Short-Term Memory Networks (LSTM)
//! - Gated Recurrent Units (GRU)

use async_trait::async_trait;
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{MLError, Result};

pub mod activations;
pub mod convolutional;
pub mod feedforward;
pub mod gru;
pub mod layers;
pub mod lstm;
pub mod optimizers;
pub mod recurrent;

pub use activations::*;
pub use convolutional::ConvolutionalNetwork;
pub use feedforward::FeedforwardNetwork;
pub use gru::GRUNetwork;
pub use layers::*;
pub use lstm::LSTMNetwork;
pub use optimizers::*;
pub use recurrent::RecurrentNetwork;

/// Neural network architecture types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    /// Feedforward Neural Network
    Feedforward,
    /// Convolutional Neural Network
    Convolutional,
    /// Recurrent Neural Network
    Recurrent,
    /// Long Short-Term Memory
    LSTM,
    /// Gated Recurrent Unit
    GRU,
    /// Custom architecture
    Custom { name: String },
}

/// Neural network layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    /// Layer type
    pub layer_type: LayerType,
    /// Number of units/neurons
    pub units: usize,
    /// Activation function
    pub activation: ActivationType,
    /// Dropout rate (0.0 to 1.0)
    pub dropout: Option<f64>,
    /// L1 regularization coefficient
    pub l1_regularization: Option<f64>,
    /// L2 regularization coefficient
    pub l2_regularization: Option<f64>,
    /// Additional parameters
    pub parameters: HashMap<String, f64>,
}

/// Layer types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    /// Dense/Fully connected layer
    Dense,
    /// Convolutional layer
    Conv1D {
        kernel_size: usize,
        stride: usize,
        padding: usize,
    },
    /// 2D Convolutional layer
    Conv2D {
        kernel_size: (usize, usize),
        stride: (usize, usize),
        padding: (usize, usize),
    },
    /// 3D Convolutional layer
    Conv3D {
        kernel_size: (usize, usize, usize),
        stride: (usize, usize, usize),
        padding: (usize, usize, usize),
    },
    /// Max pooling layer
    MaxPool1D { pool_size: usize, stride: usize },
    /// 2D Max pooling layer
    MaxPool2D {
        pool_size: (usize, usize),
        stride: (usize, usize),
    },
    /// Average pooling layer
    AvgPool1D { pool_size: usize, stride: usize },
    /// 2D Average pooling layer
    AvgPool2D {
        pool_size: (usize, usize),
        stride: (usize, usize),
    },
    /// Recurrent layer
    RNN,
    /// LSTM layer
    LSTM,
    /// GRU layer
    GRU,
    /// Batch normalization
    BatchNorm,
    /// Layer normalization
    LayerNorm,
    /// Dropout layer
    Dropout,
    /// Flatten layer
    Flatten,
    /// Reshape layer
    Reshape { shape: Vec<usize> },
}

/// Activation function types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivationType {
    /// Linear activation (identity)
    Linear,
    /// Rectified Linear Unit
    ReLU,
    /// Leaky ReLU
    LeakyReLU { alpha: f64 },
    /// Exponential Linear Unit
    ELU { alpha: f64 },
    /// Swish activation
    Swish,
    /// GELU activation
    GELU,
    /// Sigmoid activation
    Sigmoid,
    /// Hyperbolic tangent
    Tanh,
    /// Softmax activation
    Softmax,
    /// Log softmax
    LogSoftmax,
}

/// Neural network architecture specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkArchitecture {
    /// Network type
    pub network_type: NetworkType,
    /// Input shape
    pub input_shape: Vec<usize>,
    /// Layer configurations
    pub layers: Vec<LayerConfig>,
    /// Output shape
    pub output_shape: Vec<usize>,
    /// Loss function
    pub loss_function: String,
    /// Optimizer configuration
    pub optimizer: OptimizerConfig,
}

/// Optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    /// Optimizer type
    pub optimizer_type: OptimizerType,
    /// Learning rate
    pub learning_rate: f64,
    /// Additional parameters
    pub parameters: HashMap<String, f64>,
}

/// Optimizer types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptimizerType {
    /// Stochastic Gradient Descent
    SGD,
    /// Adam optimizer
    Adam,
    /// AdaGrad optimizer
    AdaGrad,
    /// RMSprop optimizer
    RMSprop,
    /// AdamW optimizer
    AdamW,
}

/// Generic neural network trait
#[async_trait]
pub trait NeuralNetwork: Send + Sync {
    /// Forward pass through the network
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>>;

    /// Backward pass (compute gradients)
    async fn backward(&mut self, loss_gradient: &Array2<f64>) -> Result<()>;

    /// Update weights using optimizer
    async fn update_weights(&mut self, optimizer: &dyn Optimizer) -> Result<()>;

    /// Get network architecture
    fn architecture(&self) -> &NetworkArchitecture;

    /// Get number of parameters
    fn parameter_count(&self) -> usize;

    /// Save network weights
    async fn save_weights(&self) -> Result<Vec<u8>>;

    /// Load network weights
    async fn load_weights(&mut self, weights: &[u8]) -> Result<()>;
}

/// Neural network builder
#[derive(Default)]
pub struct NeuralNetworkBuilder {
    network_type: Option<NetworkType>,
    input_shape: Option<Vec<usize>>,
    layers: Vec<LayerConfig>,
    output_shape: Option<Vec<usize>>,
    loss_function: Option<String>,
    optimizer: Option<OptimizerConfig>,
}

impl NeuralNetworkBuilder {
    /// Create a new neural network builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set network type
    pub fn network_type(mut self, network_type: NetworkType) -> Self {
        self.network_type = Some(network_type);
        self
    }

    /// Set input shape
    pub fn input_shape(mut self, shape: Vec<usize>) -> Self {
        self.input_shape = Some(shape);
        self
    }

    /// Add a layer
    pub fn add_layer(mut self, layer: LayerConfig) -> Self {
        self.layers.push(layer);
        self
    }

    /// Add a dense layer
    pub fn dense(mut self, units: usize, activation: ActivationType) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::Dense,
            units,
            activation,
            dropout: None,
            l1_regularization: None,
            l2_regularization: None,
            parameters: HashMap::new(),
        });
        self
    }

    /// Add a convolutional 2D layer
    pub fn conv2d(
        mut self,
        filters: usize,
        kernel_size: (usize, usize),
        stride: (usize, usize),
        padding: (usize, usize),
        activation: ActivationType,
    ) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::Conv2D {
                kernel_size,
                stride,
                padding,
            },
            units: filters,
            activation,
            dropout: None,
            l1_regularization: None,
            l2_regularization: None,
            parameters: HashMap::new(),
        });
        self
    }

    /// Add max pooling 2D layer
    pub fn max_pool2d(mut self, pool_size: (usize, usize), stride: (usize, usize)) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::MaxPool2D { pool_size, stride },
            units: 0, // Not applicable
            activation: ActivationType::Linear,
            dropout: None,
            l1_regularization: None,
            l2_regularization: None,
            parameters: HashMap::new(),
        });
        self
    }

    /// Add LSTM layer
    pub fn lstm(mut self, units: usize, activation: ActivationType) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::LSTM,
            units,
            activation,
            dropout: None,
            l1_regularization: None,
            l2_regularization: None,
            parameters: HashMap::new(),
        });
        self
    }

    /// Add GRU layer
    pub fn gru(mut self, units: usize, activation: ActivationType) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::GRU,
            units,
            activation,
            dropout: None,
            l1_regularization: None,
            l2_regularization: None,
            parameters: HashMap::new(),
        });
        self
    }

    /// Add dropout layer
    pub fn dropout(mut self, rate: f64) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::Dropout,
            units: 0,
            activation: ActivationType::Linear,
            dropout: Some(rate),
            l1_regularization: None,
            l2_regularization: None,
            parameters: HashMap::new(),
        });
        self
    }

    /// Set output shape
    pub fn output_shape(mut self, shape: Vec<usize>) -> Self {
        self.output_shape = Some(shape);
        self
    }

    /// Set loss function
    pub fn loss_function(mut self, loss: &str) -> Self {
        self.loss_function = Some(loss.to_string());
        self
    }

    /// Set optimizer
    pub fn optimizer(mut self, optimizer: OptimizerConfig) -> Self {
        self.optimizer = Some(optimizer);
        self
    }

    // Legacy compatibility methods
    pub fn layers(self, layer_sizes: &[usize]) -> Self {
        let mut builder = self;
        for &size in layer_sizes {
            builder = builder.dense(size, ActivationType::ReLU);
        }
        builder
    }

    pub fn activation(self, _activation: &str) -> Self {
        // This method is kept for compatibility but the activation
        // is now specified per layer in the dense() method
        self
    }

    /// Build the neural network
    pub async fn build(self) -> Result<Box<dyn NeuralNetwork>> {
        let network_type = self.network_type.unwrap_or(NetworkType::Feedforward);

        let input_shape = self.input_shape.unwrap_or_else(|| vec![1]);

        let output_shape = self.output_shape.unwrap_or_else(|| {
            if let Some(last_layer) = self.layers.last() {
                vec![last_layer.units]
            } else {
                vec![1]
            }
        });

        let loss_function = self.loss_function.unwrap_or_else(|| "mse".to_string());

        let optimizer = self.optimizer.unwrap_or_else(|| OptimizerConfig {
            optimizer_type: OptimizerType::Adam,
            learning_rate: 0.001,
            parameters: HashMap::new(),
        });

        let architecture = NetworkArchitecture {
            network_type: network_type.clone(),
            input_shape,
            layers: self.layers,
            output_shape,
            loss_function,
            optimizer,
        };

        // Build specific network type
        match network_type {
            NetworkType::Feedforward => Ok(Box::new(FeedforwardNetwork::new(architecture).await?)),
            NetworkType::Convolutional => {
                Ok(Box::new(ConvolutionalNetwork::new(architecture).await?))
            }
            NetworkType::Recurrent => Ok(Box::new(RecurrentNetwork::new(architecture).await?)),
            NetworkType::LSTM => Ok(Box::new(LSTMNetwork::new(architecture).await?)),
            NetworkType::GRU => Ok(Box::new(GRUNetwork::new(architecture).await?)),
            NetworkType::Custom { name } => Err(MLError::neural_network(format!(
                "Custom network type '{}' not implemented",
                name
            ))),
        }
    }
}

/// Convenience functions for common network architectures
impl NeuralNetworkBuilder {
    /// Create a simple feedforward classifier
    pub fn feedforward_classifier(input_size: usize, num_classes: usize) -> Self {
        Self::new()
            .network_type(NetworkType::Feedforward)
            .input_shape(vec![input_size])
            .dense(128, ActivationType::ReLU)
            .dropout(0.2)
            .dense(64, ActivationType::ReLU)
            .dropout(0.2)
            .dense(num_classes, ActivationType::Softmax)
            .output_shape(vec![num_classes])
            .loss_function("categorical_crossentropy")
            .optimizer(OptimizerConfig {
                optimizer_type: OptimizerType::Adam,
                learning_rate: 0.001,
                parameters: HashMap::new(),
            })
    }

    /// Create a CNN for image classification
    pub fn cnn_classifier(input_shape: Vec<usize>, num_classes: usize) -> Self {
        Self::new()
            .network_type(NetworkType::Convolutional)
            .input_shape(input_shape)
            .conv2d(32, (3, 3), (1, 1), (1, 1), ActivationType::ReLU)
            .max_pool2d((2, 2), (2, 2))
            .conv2d(64, (3, 3), (1, 1), (1, 1), ActivationType::ReLU)
            .max_pool2d((2, 2), (2, 2))
            .conv2d(128, (3, 3), (1, 1), (1, 1), ActivationType::ReLU)
            .add_layer(LayerConfig {
                layer_type: LayerType::Flatten,
                units: 0,
                activation: ActivationType::Linear,
                dropout: None,
                l1_regularization: None,
                l2_regularization: None,
                parameters: HashMap::new(),
            })
            .dense(256, ActivationType::ReLU)
            .dropout(0.5)
            .dense(num_classes, ActivationType::Softmax)
            .output_shape(vec![num_classes])
            .loss_function("categorical_crossentropy")
    }

    /// Create an LSTM for sequence processing
    pub fn lstm_classifier(input_shape: Vec<usize>, num_classes: usize) -> Self {
        Self::new()
            .network_type(NetworkType::LSTM)
            .input_shape(input_shape)
            .lstm(128, ActivationType::Tanh)
            .dropout(0.2)
            .lstm(64, ActivationType::Tanh)
            .dropout(0.2)
            .dense(num_classes, ActivationType::Softmax)
            .output_shape(vec![num_classes])
            .loss_function("categorical_crossentropy")
    }

    /// Legacy feedforward method for compatibility
    pub fn feedforward() -> Self {
        Self::new().network_type(NetworkType::Feedforward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_builder_creation() {
        let builder = NeuralNetworkBuilder::new()
            .network_type(NetworkType::Feedforward)
            .input_shape(vec![784])
            .dense(128, ActivationType::ReLU)
            .dense(10, ActivationType::Softmax)
            .output_shape(vec![10]);

        assert_eq!(builder.layers.len(), 2);
        assert_eq!(builder.input_shape, Some(vec![784]));
        assert_eq!(builder.output_shape, Some(vec![10]));
    }

    #[test]
    fn test_feedforward_classifier_preset() {
        let builder = NeuralNetworkBuilder::feedforward_classifier(784, 10);
        assert_eq!(builder.layers.len(), 5); // 2 dense + 2 dropout + 1 output
        assert!(matches!(
            builder.network_type,
            Some(NetworkType::Feedforward)
        ));
    }

    #[test]
    fn test_legacy_compatibility() {
        let builder = NeuralNetworkBuilder::feedforward()
            .layers(&[128, 64, 10])
            .activation("relu");

        assert_eq!(builder.layers.len(), 3);
        assert!(matches!(
            builder.network_type,
            Some(NetworkType::Feedforward)
        ));
    }

    #[test]
    fn test_activation_serialization() {
        let activation = ActivationType::LeakyReLU { alpha: 0.01 };
        let serialized = serde_json::to_string(&activation).unwrap();
        let deserialized: ActivationType = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            ActivationType::LeakyReLU { alpha } => assert_eq!(alpha, 0.01),
            _ => panic!("Deserialization failed"),
        }
    }
}
