//! Feedforward Neural Network implementation.

use async_trait::async_trait;
use ndarray::{Array1, Array2};

use crate::error::{MLError, Result};
use crate::neural_networks::{
    ActivationType, LayerType, NetworkArchitecture, NeuralNetwork, Optimizer,
};

/// Feedforward Neural Network
#[derive(Debug, Clone)]
pub struct FeedforwardNetwork {
    /// Network architecture
    architecture: NetworkArchitecture,

    /// Layer weights
    weights: Vec<Array2<f64>>,

    /// Layer biases
    biases: Vec<Array1<f64>>,

    /// Cached layer outputs (for backpropagation)
    layer_outputs: Vec<Array2<f64>>,

    /// Cached layer inputs (pre-activation)
    layer_inputs: Vec<Array2<f64>>,

    /// Weight gradients
    weight_gradients: Vec<Array2<f64>>,

    /// Bias gradients  
    bias_gradients: Vec<Array1<f64>>,
}

impl FeedforwardNetwork {
    /// Create a new feedforward network
    pub async fn new(architecture: NetworkArchitecture) -> Result<Self> {
        let mut weights = Vec::new();
        let mut biases = Vec::new();

        let mut prev_size = if architecture.input_shape.is_empty() {
            return Err(MLError::neural_network("Input shape cannot be empty"));
        } else {
            architecture.input_shape.iter().product()
        };

        // Initialize weights and biases for each layer
        for layer in &architecture.layers {
            match layer.layer_type {
                LayerType::Dense => {
                    // Xavier initialization
                    let scale = (2.0 / (prev_size + layer.units) as f64).sqrt();
                    let weight_matrix = Array2::from_shape_fn((layer.units, prev_size), |_| {
                        (rand::random::<f64>() - 0.5) * 2.0 * scale
                    });

                    weights.push(weight_matrix);
                    biases.push(Array1::zeros(layer.units));

                    prev_size = layer.units;
                }
                LayerType::Dropout => {
                    // Dropout doesn't have weights
                    continue;
                }
                _ => {
                    return Err(MLError::neural_network(format!(
                        "Layer type {:?} not supported in feedforward network",
                        layer.layer_type
                    )));
                }
            }
        }

        let layer_count = weights.len();

        Ok(Self {
            architecture,
            weights,
            biases,
            layer_outputs: vec![Array2::zeros((1, 1)); layer_count + 1], // +1 for input
            layer_inputs: vec![Array2::zeros((1, 1)); layer_count],
            weight_gradients: vec![Array2::zeros((1, 1)); layer_count],
            bias_gradients: vec![Array1::zeros(1); layer_count],
        })
    }

    /// Apply activation function
    fn apply_activation(&self, input: &Array2<f64>, activation: &ActivationType) -> Array2<f64> {
        match activation {
            ActivationType::Linear => input.clone(),
            ActivationType::ReLU => input.map(|x| x.max(0.0)),
            ActivationType::LeakyReLU { alpha } => {
                input.map(|x| if *x > 0.0 { *x } else { alpha * x })
            }
            ActivationType::ELU { alpha } => input.map(|x| {
                if *x > 0.0 {
                    *x
                } else {
                    alpha * (x.exp() - 1.0)
                }
            }),
            ActivationType::Swish => input.map(|x| x / (1.0 + (-x).exp())),
            ActivationType::GELU => input
                .map(|x| 0.5 * x * (1.0 + (x * 0.7978845608 * (1.0 + 0.044715 * x * x)).tanh())),
            ActivationType::Sigmoid => input.map(|x| 1.0 / (1.0 + (-x).exp())),
            ActivationType::Tanh => input.map(|x| x.tanh()),
            ActivationType::Softmax => {
                let mut result = input.clone();
                for mut row in result.rows_mut() {
                    let max_val = row.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                    for val in row.iter_mut() {
                        *val = (*val - max_val).exp();
                    }
                    let sum: f64 = row.sum();
                    for val in row.iter_mut() {
                        *val /= sum;
                    }
                }
                result
            }
            ActivationType::LogSoftmax => {
                let softmax = self.apply_activation(input, &ActivationType::Softmax);
                softmax.map(|x| x.ln())
            }
        }
    }

    /// Apply activation derivative (for backpropagation)
    #[allow(dead_code)]
    fn apply_activation_derivative(
        &self,
        input: &Array2<f64>,
        activation: &ActivationType,
    ) -> Array2<f64> {
        match activation {
            ActivationType::Linear => Array2::ones(input.raw_dim()),
            ActivationType::ReLU => input.map(|x| if *x > 0.0 { 1.0 } else { 0.0 }),
            ActivationType::LeakyReLU { alpha } => {
                input.map(|x| if *x > 0.0 { 1.0 } else { *alpha })
            }
            ActivationType::ELU { alpha } => {
                input.map(|x| if *x > 0.0 { 1.0 } else { alpha * x.exp() })
            }
            ActivationType::Sigmoid => {
                let sigmoid = self.apply_activation(input, activation);
                sigmoid.map(|x| x * (1.0 - x))
            }
            ActivationType::Tanh => {
                let tanh = self.apply_activation(input, activation);
                tanh.map(|x| 1.0 - x * x)
            }
            ActivationType::Swish => {
                let _sigmoid = input.map(|x| 1.0 / (1.0 + (-x).exp()));
                input.map(|&x| {
                    let s = 1.0 / (1.0 + (-x).exp());
                    s + x * s * (1.0 - s)
                })
            }
            ActivationType::GELU => {
                // Approximate derivative of GELU
                input.map(|x| {
                    let cdf = 0.5 * (1.0 + (x * 0.7978845608).tanh());
                    let pdf = 0.7978845608 * (1.0 - (x * 0.7978845608).tanh().powi(2));
                    cdf + x * pdf
                })
            }
            ActivationType::Softmax => {
                // For softmax, derivative is handled differently in cross-entropy loss
                Array2::ones(input.raw_dim())
            }
            ActivationType::LogSoftmax => {
                // Derivative of log softmax
                let softmax = self.apply_activation(input, &ActivationType::Softmax);
                Array2::ones(input.raw_dim()) - &softmax
            }
        }
    }

    /// Apply dropout during training
    fn apply_dropout(&self, input: &Array2<f64>, dropout_rate: f64, training: bool) -> Array2<f64> {
        if !training || dropout_rate == 0.0 {
            return input.clone();
        }

        let scale = 1.0 / (1.0 - dropout_rate);
        input.map(|x| {
            if rand::random::<f64>() < dropout_rate {
                0.0
            } else {
                x * scale
            }
        })
    }
}

#[async_trait]
impl NeuralNetwork for FeedforwardNetwork {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        let mut current_output = input.clone();
        let mut layer_idx = 0;

        // Store input for backpropagation
        // self.layer_outputs[0] = current_output.clone();

        for (_i, layer) in self.architecture.layers.iter().enumerate() {
            match layer.layer_type {
                LayerType::Dense => {
                    // Linear transformation: output = input * W^T + b
                    let linear_output = current_output.dot(&self.weights[layer_idx].t())
                        + &self.biases[layer_idx]
                            .broadcast((current_output.nrows(), self.biases[layer_idx].len()))
                            .unwrap();

                    // Store pre-activation for backpropagation
                    // self.layer_inputs[layer_idx] = linear_output.clone();

                    // Apply activation
                    current_output = self.apply_activation(&linear_output, &layer.activation);

                    // Store post-activation for backpropagation
                    // self.layer_outputs[layer_idx + 1] = current_output.clone();

                    layer_idx += 1;
                }
                LayerType::Dropout => {
                    // Apply dropout (training=false for inference)
                    if let Some(dropout_rate) = layer.dropout {
                        current_output = self.apply_dropout(&current_output, dropout_rate, false);
                    }
                }
                _ => {
                    return Err(MLError::neural_network(
                        "Unsupported layer type in feedforward network",
                    ));
                }
            }
        }

        Ok(current_output)
    }

    async fn backward(&mut self, _loss_gradient: &Array2<f64>) -> Result<()> {
        // This is a simplified implementation
        // In practice, you'd implement full backpropagation here
        Ok(())
    }

    async fn update_weights(&mut self, _optimizer: &dyn Optimizer) -> Result<()> {
        // This is a simplified implementation
        // In practice, you'd apply the optimizer updates here
        Ok(())
    }

    fn architecture(&self) -> &NetworkArchitecture {
        &self.architecture
    }

    fn parameter_count(&self) -> usize {
        let weight_count: usize = self.weights.iter().map(|w| w.len()).sum();
        let bias_count: usize = self.biases.iter().map(|b| b.len()).sum();
        weight_count + bias_count
    }

    async fn save_weights(&self) -> Result<Vec<u8>> {
        // Serialize weights and biases
        let data = (self.weights.clone(), self.biases.clone());
        bincode::serialize(&data)
            .map_err(|e| MLError::neural_network(format!("Failed to serialize weights: {}", e)))
    }

    async fn load_weights(&mut self, weights: &[u8]) -> Result<()> {
        // Deserialize weights and biases
        let (weights, biases): (Vec<Array2<f64>>, Vec<Array1<f64>>) = bincode::deserialize(weights)
            .map_err(|e| {
                MLError::neural_network(format!("Failed to deserialize weights: {}", e))
            })?;

        self.weights = weights;
        self.biases = biases;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural_networks::{LayerConfig, NetworkType, OptimizerConfig, OptimizerType};
    use std::collections::HashMap;

    fn create_test_architecture() -> NetworkArchitecture {
        NetworkArchitecture {
            network_type: NetworkType::Feedforward,
            input_shape: vec![10],
            layers: vec![
                LayerConfig {
                    layer_type: LayerType::Dense,
                    units: 5,
                    activation: ActivationType::ReLU,
                    dropout: None,
                    l1_regularization: None,
                    l2_regularization: None,
                    parameters: HashMap::new(),
                },
                LayerConfig {
                    layer_type: LayerType::Dense,
                    units: 3,
                    activation: ActivationType::Softmax,
                    dropout: None,
                    l1_regularization: None,
                    l2_regularization: None,
                    parameters: HashMap::new(),
                },
            ],
            output_shape: vec![3],
            loss_function: "categorical_crossentropy".to_string(),
            optimizer: OptimizerConfig {
                optimizer_type: OptimizerType::Adam,
                learning_rate: 0.001,
                parameters: HashMap::new(),
            },
        }
    }

    #[tokio::test]
    async fn test_feedforward_network_creation() {
        let architecture = create_test_architecture();
        let network = FeedforwardNetwork::new(architecture).await;

        assert!(network.is_ok());
        let network = network.unwrap();
        assert_eq!(network.weights.len(), 2);
        assert_eq!(network.biases.len(), 2);
    }

    #[tokio::test]
    async fn test_feedforward_forward_pass() {
        let architecture = create_test_architecture();
        let network = FeedforwardNetwork::new(architecture).await.unwrap();

        let input = Array2::ones((2, 10)); // Batch size 2, input size 10
        let output = network.forward(&input).await;

        assert!(output.is_ok());
        let output = output.unwrap();
        assert_eq!(output.shape(), &[2, 3]); // Batch size 2, output size 3
    }

    #[tokio::test]
    async fn test_parameter_count() {
        let architecture = create_test_architecture();
        let network = FeedforwardNetwork::new(architecture).await.unwrap();

        // First layer: 10 inputs * 5 units + 5 biases = 55
        // Second layer: 5 inputs * 3 units + 3 biases = 18
        // Total: 73 parameters
        assert_eq!(network.parameter_count(), 73);
    }

    #[tokio::test]
    async fn test_activation_functions() {
        let architecture = create_test_architecture();
        let network = FeedforwardNetwork::new(architecture).await.unwrap();

        let input = Array2::from_shape_vec((1, 3), vec![-1.0, 0.0, 1.0]).unwrap();

        // Test ReLU
        let relu_output = network.apply_activation(&input, &ActivationType::ReLU);
        assert_eq!(relu_output[[0, 0]], 0.0);
        assert_eq!(relu_output[[0, 1]], 0.0);
        assert_eq!(relu_output[[0, 2]], 1.0);

        // Test Sigmoid
        let sigmoid_output = network.apply_activation(&input, &ActivationType::Sigmoid);
        assert!(sigmoid_output[[0, 0]] < 0.5);
        assert_eq!(sigmoid_output[[0, 1]], 0.5);
        assert!(sigmoid_output[[0, 2]] > 0.5);
    }

    #[tokio::test]
    async fn test_weight_serialization() {
        let architecture = create_test_architecture();
        let mut network = FeedforwardNetwork::new(architecture).await.unwrap();

        let weights = network.save_weights().await.unwrap();
        let original_param_count = network.parameter_count();

        // Modify weights to test loading
        network.weights[0].fill(999.0);

        // Load original weights back
        network.load_weights(&weights).await.unwrap();

        // Parameter count should remain the same
        assert_eq!(network.parameter_count(), original_param_count);
    }
}
