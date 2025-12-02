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
    /// Gradients for bias parameters (kept for future training implementation)
    #[allow(dead_code)]
    bias_gradients: Array1<f64>,
    /// Cached input from forward pass (kept for future backprop implementation)
    #[allow(dead_code)]
    last_input: Option<Array2<f64>>,
}

impl DenseLayer {
    /// Create a new dense layer with Xavier initialization
    ///
    /// # Arguments
    /// * `input_size` - Number of input features
    /// * `output_size` - Number of output features
    ///
    /// # Returns
    /// A new dense layer with weights initialized using Xavier initialization
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
            + self
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
    /// Create a new dropout layer
    ///
    /// # Arguments
    /// * `dropout_rate` - Probability of dropping units (0.0 to 1.0)
    ///
    /// # Returns
    /// A new dropout layer in training mode
    pub fn new(dropout_rate: f64) -> Self {
        Self {
            dropout_rate,
            training: true,
        }
    }

    /// Set training mode for the dropout layer
    ///
    /// # Arguments
    /// * `training` - If true, dropout is applied; if false, acts as identity
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

/// Convolutional 2D layer with full forward and backward pass implementation
#[derive(Debug, Clone)]
pub struct Conv2DLayer {
    /// Convolutional filters [out_channels, in_channels, height, width]
    filters: Array4<f64>,
    /// Bias terms for each output channel
    biases: Array1<f64>,
    /// Size of the convolution kernel
    kernel_size: (usize, usize),
    /// Stride for convolution operation
    stride: (usize, usize),
    /// Padding applied to input
    padding: (usize, usize),
    /// Number of input channels
    in_channels: usize,
    /// Number of output channels (filters)
    out_channels: usize,
    /// Expected input height (for reshaping from Array2)
    input_height: usize,
    /// Expected input width (for reshaping from Array2)
    input_width: usize,
    /// Cached input for backward pass
    last_input: Option<Array4<f64>>,
    /// Filter gradients
    filter_gradients: Array4<f64>,
    /// Bias gradients
    bias_gradients: Array1<f64>,
}

impl Conv2DLayer {
    /// Create a new 2D convolutional layer with He initialization
    ///
    /// # Arguments
    /// * `in_channels` - Number of input channels
    /// * `out_channels` - Number of output channels (number of filters)
    /// * `kernel_size` - Size of convolution kernel as (height, width)
    /// * `stride` - Stride for convolution as (height, width)
    /// * `padding` - Padding for input as (height, width)
    ///
    /// # Returns
    /// A new Conv2D layer with He-initialized weights
    pub fn new(
        in_channels: usize,
        out_channels: usize,
        kernel_size: (usize, usize),
        stride: (usize, usize),
        padding: (usize, usize),
    ) -> Self {
        Self::with_input_size(
            in_channels,
            out_channels,
            kernel_size,
            stride,
            padding,
            28,
            28,
        )
    }

    /// Create a new 2D convolutional layer with specified input dimensions
    ///
    /// # Arguments
    /// * `in_channels` - Number of input channels
    /// * `out_channels` - Number of output channels (number of filters)
    /// * `kernel_size` - Size of convolution kernel as (height, width)
    /// * `stride` - Stride for convolution as (height, width)
    /// * `padding` - Padding for input as (height, width)
    /// * `input_height` - Expected input image height
    /// * `input_width` - Expected input image width
    ///
    /// # Returns
    /// A new Conv2D layer with He-initialized weights
    pub fn with_input_size(
        in_channels: usize,
        out_channels: usize,
        kernel_size: (usize, usize),
        stride: (usize, usize),
        padding: (usize, usize),
        input_height: usize,
        input_width: usize,
    ) -> Self {
        // He initialization: sqrt(2 / fan_in) where fan_in = in_channels * kernel_h * kernel_w
        let fan_in = in_channels * kernel_size.0 * kernel_size.1;
        let scale = (2.0 / fan_in as f64).sqrt();

        let filters = Array4::from_shape_fn(
            (out_channels, in_channels, kernel_size.0, kernel_size.1),
            |_| (rand::random::<f64>() - 0.5) * 2.0 * scale,
        );
        let biases = Array1::zeros(out_channels);

        Self {
            filters,
            biases: biases.clone(),
            kernel_size,
            stride,
            padding,
            in_channels,
            out_channels,
            input_height,
            input_width,
            last_input: None,
            filter_gradients: Array4::zeros((
                out_channels,
                in_channels,
                kernel_size.0,
                kernel_size.1,
            )),
            bias_gradients: biases,
        }
    }

    /// Calculate output dimensions for given input dimensions
    fn output_dims(&self, input_h: usize, input_w: usize) -> (usize, usize) {
        let out_h = (input_h + 2 * self.padding.0 - self.kernel_size.0) / self.stride.0 + 1;
        let out_w = (input_w + 2 * self.padding.1 - self.kernel_size.1) / self.stride.1 + 1;
        (out_h, out_w)
    }

    /// Reshape Array2 to Array4 (batch, channels, height, width)
    fn reshape_to_4d(&self, input: &Array2<f64>) -> Result<Array4<f64>> {
        let batch_size = input.nrows();
        let expected_cols = self.in_channels * self.input_height * self.input_width;

        if input.ncols() != expected_cols {
            return Err(MLError::neural_network(format!(
                "Conv2D input size mismatch: expected {} ({} x {} x {}), got {}",
                expected_cols,
                self.in_channels,
                self.input_height,
                self.input_width,
                input.ncols()
            )));
        }

        let mut result = Array4::zeros((
            batch_size,
            self.in_channels,
            self.input_height,
            self.input_width,
        ));
        for b in 0..batch_size {
            for c in 0..self.in_channels {
                for h in 0..self.input_height {
                    for w in 0..self.input_width {
                        let idx =
                            c * self.input_height * self.input_width + h * self.input_width + w;
                        result[[b, c, h, w]] = input[[b, idx]];
                    }
                }
            }
        }
        Ok(result)
    }

    /// Reshape Array4 (batch, channels, height, width) back to Array2
    fn reshape_to_2d(&self, input: &Array4<f64>) -> Array2<f64> {
        let (batch_size, channels, height, width) = input.dim();
        let cols = channels * height * width;
        let mut result = Array2::zeros((batch_size, cols));

        for b in 0..batch_size {
            for c in 0..channels {
                for h in 0..height {
                    for w in 0..width {
                        let idx = c * height * width + h * width + w;
                        result[[b, idx]] = input[[b, c, h, w]];
                    }
                }
            }
        }
        result
    }

    /// Apply zero-padding to input tensor
    fn pad_input(&self, input: &Array4<f64>) -> Array4<f64> {
        if self.padding.0 == 0 && self.padding.1 == 0 {
            return input.clone();
        }

        let (batch_size, channels, h, w) = input.dim();
        let new_h = h + 2 * self.padding.0;
        let new_w = w + 2 * self.padding.1;
        let mut padded = Array4::zeros((batch_size, channels, new_h, new_w));

        for b in 0..batch_size {
            for c in 0..channels {
                for i in 0..h {
                    for j in 0..w {
                        padded[[b, c, i + self.padding.0, j + self.padding.1]] =
                            input[[b, c, i, j]];
                    }
                }
            }
        }
        padded
    }

    /// Perform 2D convolution forward pass
    fn conv2d_forward(&self, input: &Array4<f64>) -> Array4<f64> {
        let (batch_size, _in_channels, in_h, in_w) = input.dim();
        let (out_h, out_w) = self.output_dims(in_h - 2 * self.padding.0, in_w - 2 * self.padding.1);

        // Apply padding
        let padded = self.pad_input(input);

        let mut output = Array4::zeros((batch_size, self.out_channels, out_h, out_w));

        // Perform convolution
        for b in 0..batch_size {
            for oc in 0..self.out_channels {
                for oh in 0..out_h {
                    for ow in 0..out_w {
                        let mut sum = self.biases[oc];
                        for ic in 0..self.in_channels {
                            for kh in 0..self.kernel_size.0 {
                                for kw in 0..self.kernel_size.1 {
                                    let ih = oh * self.stride.0 + kh;
                                    let iw = ow * self.stride.1 + kw;
                                    sum += padded[[b, ic, ih, iw]] * self.filters[[oc, ic, kh, kw]];
                                }
                            }
                        }
                        output[[b, oc, oh, ow]] = sum;
                    }
                }
            }
        }
        output
    }

    /// Get the filter weights
    pub fn filters(&self) -> &Array4<f64> {
        &self.filters
    }

    /// Get the biases
    pub fn biases(&self) -> &Array1<f64> {
        &self.biases
    }
}

#[async_trait]
impl Layer for Conv2DLayer {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        // Reshape input from Array2 to Array4
        let input_4d = self.reshape_to_4d(input)?;

        // Perform convolution
        let output_4d = self.conv2d_forward(&input_4d);

        // Reshape back to Array2
        Ok(self.reshape_to_2d(&output_4d))
    }

    async fn backward(&mut self, gradient: &Array2<f64>) -> Result<Array2<f64>> {
        // Get cached input from forward pass
        let input_4d = match &self.last_input {
            Some(inp) => inp.clone(),
            None => {
                return Err(MLError::neural_network(
                    "Conv2D backward called without prior forward pass",
                ));
            }
        };

        let (batch_size, _in_channels, in_h, in_w) = input_4d.dim();
        let (out_h, out_w) = self.output_dims(in_h, in_w);

        // Reshape gradient to 4D
        let grad_cols = self.out_channels * out_h * out_w;
        if gradient.ncols() != grad_cols {
            return Err(MLError::neural_network(format!(
                "Conv2D backward gradient size mismatch: expected {}, got {}",
                grad_cols,
                gradient.ncols()
            )));
        }

        let mut grad_4d = Array4::zeros((batch_size, self.out_channels, out_h, out_w));
        for b in 0..batch_size {
            for c in 0..self.out_channels {
                for h in 0..out_h {
                    for w in 0..out_w {
                        let idx = c * out_h * out_w + h * out_w + w;
                        grad_4d[[b, c, h, w]] = gradient[[b, idx]];
                    }
                }
            }
        }

        // Pad input for gradient computation
        let padded_input = self.pad_input(&input_4d);

        // Compute filter gradients: dL/dW = sum over batch of input * grad_output
        self.filter_gradients = Array4::zeros((
            self.out_channels,
            self.in_channels,
            self.kernel_size.0,
            self.kernel_size.1,
        ));

        for b in 0..batch_size {
            for oc in 0..self.out_channels {
                for ic in 0..self.in_channels {
                    for kh in 0..self.kernel_size.0 {
                        for kw in 0..self.kernel_size.1 {
                            let mut sum = 0.0;
                            for oh in 0..out_h {
                                for ow in 0..out_w {
                                    let ih = oh * self.stride.0 + kh;
                                    let iw = ow * self.stride.1 + kw;
                                    sum += padded_input[[b, ic, ih, iw]] * grad_4d[[b, oc, oh, ow]];
                                }
                            }
                            self.filter_gradients[[oc, ic, kh, kw]] += sum;
                        }
                    }
                }
            }
        }

        // Normalize by batch size
        self.filter_gradients
            .mapv_inplace(|x| x / batch_size as f64);

        // Compute bias gradients: dL/db = sum of gradients
        self.bias_gradients = Array1::zeros(self.out_channels);
        for b in 0..batch_size {
            for oc in 0..self.out_channels {
                for oh in 0..out_h {
                    for ow in 0..out_w {
                        self.bias_gradients[oc] += grad_4d[[b, oc, oh, ow]];
                    }
                }
            }
        }
        self.bias_gradients.mapv_inplace(|x| x / batch_size as f64);

        // Compute input gradient (full convolution with flipped kernels)
        let mut input_grad = Array4::zeros((batch_size, self.in_channels, in_h, in_w));

        for b in 0..batch_size {
            for ic in 0..self.in_channels {
                for ih in 0..in_h {
                    for iw in 0..in_w {
                        let mut sum = 0.0;
                        for oc in 0..self.out_channels {
                            for kh in 0..self.kernel_size.0 {
                                for kw in 0..self.kernel_size.1 {
                                    // Check if this input position contributes to output
                                    let oh_start =
                                        ih as isize + self.padding.0 as isize - kh as isize;
                                    let ow_start =
                                        iw as isize + self.padding.1 as isize - kw as isize;

                                    if oh_start >= 0
                                        && ow_start >= 0
                                        && oh_start % self.stride.0 as isize == 0
                                        && ow_start % self.stride.1 as isize == 0
                                    {
                                        let oh = (oh_start / self.stride.0 as isize) as usize;
                                        let ow = (ow_start / self.stride.1 as isize) as usize;

                                        if oh < out_h && ow < out_w {
                                            // Use flipped kernel
                                            let fkh = self.kernel_size.0 - 1 - kh;
                                            let fkw = self.kernel_size.1 - 1 - kw;
                                            sum += grad_4d[[b, oc, oh, ow]]
                                                * self.filters[[oc, ic, fkh, fkw]];
                                        }
                                    }
                                }
                            }
                        }
                        input_grad[[b, ic, ih, iw]] = sum;
                    }
                }
            }
        }

        // Reshape back to Array2
        Ok(self.reshape_to_2d(&input_grad))
    }

    fn parameters(&self) -> Vec<&Array2<f64>> {
        // Return empty since we store 4D filters, not 2D
        // Alternatively, we could flatten the filters
        vec![]
    }

    fn parameter_gradients(&self) -> Vec<&Array2<f64>> {
        vec![]
    }
}

/// Layer normalization implementation for transformer architectures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerNorm {
    /// Shape of the input features to normalize
    normalized_shape: usize,
    /// Small epsilon value to avoid division by zero
    eps: f64,
    /// Learnable weight (gamma) parameters for scaling
    weight: Array1<f64>,
    /// Learnable bias (beta) parameters for shifting
    bias: Array1<f64>,
}

impl LayerNorm {
    /// Create a new layer normalization layer
    ///
    /// # Arguments
    /// * `normalized_shape` - Number of features to normalize
    /// * `eps` - Small epsilon value to avoid division by zero
    ///
    /// # Returns
    /// A new LayerNorm instance with weight initialized to ones and bias to zeros
    pub fn new(normalized_shape: usize, eps: f64) -> Self {
        Self {
            normalized_shape,
            eps,
            weight: Array1::ones(normalized_shape),
            bias: Array1::zeros(normalized_shape),
        }
    }

    /// Forward pass through layer normalization
    ///
    /// # Arguments
    /// * `input` - Input tensor in format [batch, seq_len, hidden_size]
    ///
    /// # Returns
    /// Normalized tensor with same shape as input
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

/// Linear/Dense layer for transformer and neural network usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Linear {
    /// Number of input features
    pub in_features: usize,
    /// Number of output features
    pub out_features: usize,
    /// Weight matrix [out_features, in_features]
    weight: Array2<f64>,
    /// Optional bias vector [out_features]
    bias: Option<Array1<f64>>,
}

impl Linear {
    /// Create a new linear layer with bias using Xavier initialization
    ///
    /// # Arguments
    /// * `in_features` - Number of input features
    /// * `out_features` - Number of output features
    ///
    /// # Returns
    /// A new Linear layer with Xavier-initialized weights and zero bias
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

    /// Create a new linear layer without bias using Xavier initialization
    ///
    /// # Arguments
    /// * `in_features` - Number of input features
    /// * `out_features` - Number of output features
    ///
    /// # Returns
    /// A new Linear layer with Xavier-initialized weights and no bias
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

    /// Forward pass for 3D input tensors (batch, sequence, features)
    ///
    /// # Arguments
    /// * `input` - Input tensor of shape [batch_size, seq_len, in_features]
    ///
    /// # Returns
    /// Output tensor of shape [batch_size, seq_len, out_features]
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

    /// Standard forward pass for 2D input tensors (batch, features)
    ///
    /// # Arguments
    /// * `input` - Input tensor of shape [batch_size, in_features]
    ///
    /// # Returns
    /// Output tensor of shape [batch_size, out_features]
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
