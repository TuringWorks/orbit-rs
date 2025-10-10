//! Neural network layer implementations.

use async_trait::async_trait;
use ndarray::{Array1, Array2, Array3, Array4};
use serde::{Deserialize, Serialize};

use crate::error::{MLError, Result};

/// Generic layer trait
#[async_trait]
pub trait Layer: Send + Sync {
    /// Forward pass through the layer
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>>;

    /// Backward pass through the layer
    async fn backward(&mut self, gradient: &Array2<f64>) -> Result<Array2<f64>>;

    /// Get layer parameters
    fn parameters(&self) -> Vec<&Array2<f64>>;

    /// Get parameter gradients
    fn parameter_gradients(&self) -> Vec<&Array2<f64>>;
}

/// Dense (fully connected) layer
#[derive(Debug, Clone)]
pub struct DenseLayer {
    weights: Array2<f64>,
    biases: Array1<f64>,
    weight_gradients: Array2<f64>,
    bias_gradients: Array1<f64>,
    last_input: Option<Array2<f64>>,
}

impl DenseLayer {
    pub fn new(input_size: usize, output_size: usize) -> Self {
        // Xavier initialization
        let scale = (2.0 / (input_size + output_size) as f64).sqrt();
        let weights = Array2::from_shape_fn((output_size, input_size), |_| {
            (rand::random::<f64>() - 0.5) * 2.0 * scale
        });

        Self {
            weights,
            biases: Array1::zeros(output_size),
            weight_gradients: Array2::zeros((output_size, input_size)),
            bias_gradients: Array1::zeros(output_size),
            last_input: None,
        }
    }
}

#[async_trait]
impl Layer for DenseLayer {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        // output = input * W^T + b
        let output = input.dot(&self.weights.t())
            + &self
                .biases
                .broadcast((input.nrows(), self.biases.len()))
                .unwrap();
        Ok(output)
    }

    async fn backward(&mut self, gradient: &Array2<f64>) -> Result<Array2<f64>> {
        // Compute gradients and return input gradient
        // This is a simplified implementation
        Ok(gradient.clone())
    }

    fn parameters(&self) -> Vec<&Array2<f64>> {
        vec![&self.weights]
    }

    fn parameter_gradients(&self) -> Vec<&Array2<f64>> {
        vec![&self.weight_gradients]
    }
}

/// Dropout layer
#[derive(Debug, Clone)]
pub struct DropoutLayer {
    dropout_rate: f64,
    training: bool,
}

impl DropoutLayer {
    pub fn new(dropout_rate: f64) -> Self {
        Self {
            dropout_rate,
            training: true,
        }
    }

    pub fn set_training(&mut self, training: bool) {
        self.training = training;
    }
}

#[async_trait]
impl Layer for DropoutLayer {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        if !self.training || self.dropout_rate == 0.0 {
            return Ok(input.clone());
        }

        let scale = 1.0 / (1.0 - self.dropout_rate);
        let output = input.map(|x| {
            if rand::random::<f64>() < self.dropout_rate {
                0.0
            } else {
                x * scale
            }
        });

        Ok(output)
    }

    async fn backward(&mut self, gradient: &Array2<f64>) -> Result<Array2<f64>> {
        // Dropout backward pass
        Ok(gradient.clone())
    }

    fn parameters(&self) -> Vec<&Array2<f64>> {
        vec![]
    }

    fn parameter_gradients(&self) -> Vec<&Array2<f64>> {
        vec![]
    }
}

/// Convolutional 2D layer (simplified stub)
#[derive(Debug, Clone)]
pub struct Conv2DLayer {
    filters: Array4<f64>, // [out_channels, in_channels, height, width]
    biases: Array1<f64>,
    kernel_size: (usize, usize),
    stride: (usize, usize),
    padding: (usize, usize),
}

impl Conv2DLayer {
    pub fn new(
        in_channels: usize,
        out_channels: usize,
        kernel_size: (usize, usize),
        stride: (usize, usize),
        padding: (usize, usize),
    ) -> Self {
        let filters = Array4::zeros((out_channels, in_channels, kernel_size.0, kernel_size.1));
        let biases = Array1::zeros(out_channels);

        Self {
            filters,
            biases,
            kernel_size,
            stride,
            padding,
        }
    }
}

#[async_trait]
impl Layer for Conv2DLayer {
    async fn forward(&self, _input: &Array2<f64>) -> Result<Array2<f64>> {
        // TODO: Implement 2D convolution
        Err(MLError::neural_network(
            "Conv2D forward pass not implemented yet",
        ))
    }

    async fn backward(&mut self, gradient: &Array2<f64>) -> Result<Array2<f64>> {
        // TODO: Implement 2D convolution backward pass
        Ok(gradient.clone())
    }

    fn parameters(&self) -> Vec<&Array2<f64>> {
        vec![]
    }

    fn parameter_gradients(&self) -> Vec<&Array2<f64>> {
        vec![]
    }
}

/// Layer normalization implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerNorm {
    normalized_shape: usize,
    eps: f64,
    weight: Array1<f64>,
    bias: Array1<f64>,
}

impl LayerNorm {
    pub fn new(normalized_shape: usize, eps: f64) -> Self {
        Self {
            normalized_shape,
            eps,
            weight: Array1::ones(normalized_shape),
            bias: Array1::zeros(normalized_shape),
        }
    }

    pub fn forward(&self, input: &Array3<f64>) -> Result<Array3<f64>> {
        let (batch_size, seq_len, hidden_size) = input.dim();
        let mut output = input.clone();

        // Apply layer normalization across the hidden dimension
        for b in 0..batch_size {
            for s in 0..seq_len {
                // Compute mean and variance for this position
                let mut sum = 0.0;
                for h in 0..hidden_size {
                    sum += output[[b, s, h]];
                }
                let mean = sum / hidden_size as f64;

                let mut var_sum = 0.0;
                for h in 0..hidden_size {
                    let diff = output[[b, s, h]] - mean;
                    var_sum += diff * diff;
                }
                let variance = var_sum / hidden_size as f64;
                let std = (variance + self.eps).sqrt();

                // Normalize
                for h in 0..hidden_size {
                    output[[b, s, h]] =
                        (output[[b, s, h]] - mean) / std * self.weight[h] + self.bias[h];
                }
            }
        }

        Ok(output)
    }
}

/// Linear/Dense layer for transformer usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Linear {
    pub in_features: usize,
    pub out_features: usize,
    weight: Array2<f64>,
    bias: Option<Array1<f64>>,
}

impl Linear {
    pub fn new(in_features: usize, out_features: usize) -> Result<Self> {
        // Xavier initialization
        let scale = (2.0 / (in_features + out_features) as f64).sqrt();
        let weight = Array2::from_shape_fn((out_features, in_features), |_| {
            (rand::random::<f64>() - 0.5) * 2.0 * scale
        });

        let bias = Some(Array1::zeros(out_features));

        Ok(Self {
            in_features,
            out_features,
            weight,
            bias,
        })
    }

    pub fn new_no_bias(in_features: usize, out_features: usize) -> Result<Self> {
        let scale = (2.0 / (in_features + out_features) as f64).sqrt();
        let weight = Array2::from_shape_fn((out_features, in_features), |_| {
            (rand::random::<f64>() - 0.5) * 2.0 * scale
        });

        Ok(Self {
            in_features,
            out_features,
            weight,
            bias: None,
        })
    }

    pub fn forward_3d(&self, input: &Array3<f64>) -> Result<Array3<f64>> {
        let (batch_size, seq_len, _) = input.dim();
        let mut output = Array3::<f64>::zeros((batch_size, seq_len, self.out_features));

        for b in 0..batch_size {
            for s in 0..seq_len {
                // Extract input vector for this position
                let mut input_vec = Array1::<f64>::zeros(self.in_features);
                for i in 0..self.in_features {
                    input_vec[i] = input[[b, s, i]];
                }
                let result = self.weight.dot(&input_vec);

                for o in 0..self.out_features {
                    output[[b, s, o]] = result[o]
                        + if let Some(ref bias) = self.bias {
                            bias[o]
                        } else {
                            0.0
                        };
                }
            }
        }

        Ok(output)
    }

    pub fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        let output = input.dot(&self.weight.t());

        if let Some(ref bias) = self.bias {
            let biased_output = &output + &bias.broadcast(output.dim()).unwrap();
            Ok(biased_output)
        } else {
            Ok(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dense_layer_forward() {
        let layer = DenseLayer::new(10, 5);
        let input = Array2::ones((2, 10)); // Batch size 2

        let output = layer.forward(&input).await.unwrap();
        assert_eq!(output.shape(), &[2, 5]);
    }

    #[tokio::test]
    async fn test_dropout_layer() {
        let mut layer = DropoutLayer::new(0.5);
        let input = Array2::ones((2, 10));

        // Test training mode
        layer.set_training(true);
        let output_train = layer.forward(&input).await.unwrap();
        assert_eq!(output_train.shape(), &[2, 10]);

        // Test inference mode
        layer.set_training(false);
        let output_inference = layer.forward(&input).await.unwrap();
        assert_eq!(output_inference, input);
    }

    #[test]
    fn test_linear_layer() {
        let layer = Linear::new(10, 5).unwrap();
        let input = Array2::ones((2, 10));

        let output = layer.forward(&input).unwrap();
        assert_eq!(output.shape(), &[2, 5]);
    }

    #[test]
    fn test_layer_norm() {
        let layer_norm = LayerNorm::new(4, 1e-5);
        let input = Array3::ones((2, 3, 4));

        let output = layer_norm.forward(&input).unwrap();
        assert_eq!(output.shape(), &[2, 3, 4]);
    }
}
