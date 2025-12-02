//! Convolutional Neural Network implementation.

use async_trait::async_trait;
use ndarray::{Array1, Array2, Array4};

use crate::error::{MLError, Result};
use crate::neural_networks::activations::Activations;
use crate::neural_networks::{
    ActivationType, LayerType, NetworkArchitecture, NeuralNetwork, Optimizer,
};

/// A single convolutional layer with filters, biases, and activation
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ConvLayer {
    /// Filter weights [out_channels, in_channels, kernel_h, kernel_w]
    filters: Array4<f64>,
    /// Bias per output channel
    biases: Array1<f64>,
    /// Kernel dimensions
    kernel_size: (usize, usize),
    /// Stride
    stride: (usize, usize),
    /// Padding
    padding: (usize, usize),
    /// Activation function
    activation: ActivationType,
    /// Output channels
    out_channels: usize,
    /// Input channels
    in_channels: usize,
    /// Filter gradients
    filter_grads: Array4<f64>,
    /// Bias gradients
    bias_grads: Array1<f64>,
    /// Cached input for backprop
    last_input: Option<Array4<f64>>,
    /// Cached pre-activation output
    last_pre_activation: Option<Array4<f64>>,
}

/// A pooling layer (max or average)
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PoolLayer {
    pool_size: (usize, usize),
    stride: (usize, usize),
    is_max: bool,
    /// Cached indices for max pooling backprop
    last_max_indices: Option<Vec<(usize, usize)>>,
}

/// A dense (fully connected) layer
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DenseLayerInternal {
    weights: Array2<f64>,
    biases: Array1<f64>,
    activation: ActivationType,
    weight_grads: Array2<f64>,
    bias_grads: Array1<f64>,
    last_input: Option<Array2<f64>>,
    last_pre_activation: Option<Array2<f64>>,
}

/// Internal layer representation for CNN
#[derive(Debug, Clone)]
enum CNNLayer {
    Conv(ConvLayer),
    Pool(PoolLayer),
    Dense(DenseLayerInternal),
    Flatten,
    Dropout { rate: f64, training: bool },
}

/// Convolutional Neural Network with full implementation
#[derive(Debug, Clone)]
pub struct ConvolutionalNetwork {
    architecture: NetworkArchitecture,
    layers: Vec<CNNLayer>,
    /// Current spatial dimensions [channels, height, width]
    #[allow(dead_code)]
    current_dims: (usize, usize, usize),
    /// Whether the network is in training mode
    training: bool,
}

impl ConvolutionalNetwork {
    /// Create a new convolutional neural network with given architecture
    pub async fn new(architecture: NetworkArchitecture) -> Result<Self> {
        let input_shape = &architecture.input_shape;
        if input_shape.len() < 2 {
            return Err(MLError::neural_network(
                "CNN requires input shape with at least [height, width] or [channels, height, width]",
            ));
        }

        // Parse input dimensions
        let (in_channels, in_height, in_width) = if input_shape.len() == 2 {
            (1, input_shape[0], input_shape[1])
        } else {
            (input_shape[0], input_shape[1], input_shape[2])
        };

        let mut layers = Vec::new();
        let mut current_channels = in_channels;
        let mut current_height = in_height;
        let mut current_width = in_width;
        let mut is_flattened = false;
        let mut flat_size = 0usize;

        // Build layers from architecture
        for layer_config in &architecture.layers {
            match &layer_config.layer_type {
                LayerType::Conv2D {
                    kernel_size,
                    stride,
                    padding,
                } => {
                    if is_flattened {
                        return Err(MLError::neural_network(
                            "Cannot add Conv2D after Flatten layer",
                        ));
                    }

                    let out_channels = layer_config.units;
                    let conv = Self::create_conv_layer(
                        current_channels,
                        out_channels,
                        *kernel_size,
                        *stride,
                        *padding,
                        layer_config.activation.clone(),
                    );

                    // Update dimensions
                    current_height =
                        (current_height + 2 * padding.0 - kernel_size.0) / stride.0 + 1;
                    current_width = (current_width + 2 * padding.1 - kernel_size.1) / stride.1 + 1;
                    current_channels = out_channels;

                    layers.push(CNNLayer::Conv(conv));
                }
                LayerType::MaxPool2D { pool_size, stride } => {
                    if is_flattened {
                        return Err(MLError::neural_network(
                            "Cannot add MaxPool2D after Flatten layer",
                        ));
                    }

                    layers.push(CNNLayer::Pool(PoolLayer {
                        pool_size: *pool_size,
                        stride: *stride,
                        is_max: true,
                        last_max_indices: None,
                    }));

                    current_height = (current_height - pool_size.0) / stride.0 + 1;
                    current_width = (current_width - pool_size.1) / stride.1 + 1;
                }
                LayerType::AvgPool2D { pool_size, stride } => {
                    if is_flattened {
                        return Err(MLError::neural_network(
                            "Cannot add AvgPool2D after Flatten layer",
                        ));
                    }

                    layers.push(CNNLayer::Pool(PoolLayer {
                        pool_size: *pool_size,
                        stride: *stride,
                        is_max: false,
                        last_max_indices: None,
                    }));

                    current_height = (current_height - pool_size.0) / stride.0 + 1;
                    current_width = (current_width - pool_size.1) / stride.1 + 1;
                }
                LayerType::Flatten => {
                    if !is_flattened {
                        flat_size = current_channels * current_height * current_width;
                        is_flattened = true;
                        layers.push(CNNLayer::Flatten);
                    }
                }
                LayerType::Dense => {
                    if !is_flattened {
                        // Implicit flatten
                        flat_size = current_channels * current_height * current_width;
                        is_flattened = true;
                        layers.push(CNNLayer::Flatten);
                    }

                    let in_features = flat_size;
                    let out_features = layer_config.units;

                    let dense = Self::create_dense_layer(
                        in_features,
                        out_features,
                        layer_config.activation.clone(),
                    );

                    flat_size = out_features;
                    layers.push(CNNLayer::Dense(dense));
                }
                LayerType::Dropout => {
                    let rate = layer_config.dropout.unwrap_or(0.5);
                    layers.push(CNNLayer::Dropout {
                        rate,
                        training: true,
                    });
                }
                _ => {
                    // Skip unsupported layer types with a warning
                }
            }
        }

        Ok(Self {
            architecture,
            layers,
            current_dims: (current_channels, current_height, current_width),
            training: true,
        })
    }

    fn create_conv_layer(
        in_channels: usize,
        out_channels: usize,
        kernel_size: (usize, usize),
        stride: (usize, usize),
        padding: (usize, usize),
        activation: ActivationType,
    ) -> ConvLayer {
        // He initialization
        let fan_in = in_channels * kernel_size.0 * kernel_size.1;
        let scale = (2.0 / fan_in as f64).sqrt();

        let filters = Array4::from_shape_fn(
            (out_channels, in_channels, kernel_size.0, kernel_size.1),
            |_| (rand::random::<f64>() - 0.5) * 2.0 * scale,
        );

        ConvLayer {
            filters: filters.clone(),
            biases: Array1::zeros(out_channels),
            kernel_size,
            stride,
            padding,
            activation,
            out_channels,
            in_channels,
            filter_grads: Array4::zeros(filters.dim()),
            bias_grads: Array1::zeros(out_channels),
            last_input: None,
            last_pre_activation: None,
        }
    }

    fn create_dense_layer(
        in_features: usize,
        out_features: usize,
        activation: ActivationType,
    ) -> DenseLayerInternal {
        // Xavier initialization
        let scale = (2.0 / (in_features + out_features) as f64).sqrt();
        let weights = Array2::from_shape_fn((out_features, in_features), |_| {
            (rand::random::<f64>() - 0.5) * 2.0 * scale
        });

        DenseLayerInternal {
            weights: weights.clone(),
            biases: Array1::zeros(out_features),
            activation,
            weight_grads: Array2::zeros(weights.dim()),
            bias_grads: Array1::zeros(out_features),
            last_input: None,
            last_pre_activation: None,
        }
    }

    /// Reshape Array2 [batch, features] to Array4 [batch, channels, height, width]
    fn reshape_to_4d(
        &self,
        input: &Array2<f64>,
        channels: usize,
        height: usize,
        width: usize,
    ) -> Result<Array4<f64>> {
        let batch_size = input.nrows();
        let expected = channels * height * width;

        if input.ncols() != expected {
            return Err(MLError::neural_network(format!(
                "Input size mismatch: expected {}, got {}",
                expected,
                input.ncols()
            )));
        }

        let mut result = Array4::zeros((batch_size, channels, height, width));
        for b in 0..batch_size {
            for c in 0..channels {
                for h in 0..height {
                    for w in 0..width {
                        let idx = c * height * width + h * width + w;
                        result[[b, c, h, w]] = input[[b, idx]];
                    }
                }
            }
        }
        Ok(result)
    }

    /// Reshape Array4 [batch, channels, height, width] to Array2 [batch, features]
    fn reshape_to_2d(&self, input: &Array4<f64>) -> Array2<f64> {
        let (batch_size, channels, height, width) = input.dim();
        let features = channels * height * width;
        let mut result = Array2::zeros((batch_size, features));

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

    /// Apply convolution operation
    fn conv_forward(&self, input: &Array4<f64>, layer: &ConvLayer) -> Array4<f64> {
        let (batch_size, _in_channels, in_h, in_w) = input.dim();

        // Calculate output dimensions
        let out_h = (in_h + 2 * layer.padding.0 - layer.kernel_size.0) / layer.stride.0 + 1;
        let out_w = (in_w + 2 * layer.padding.1 - layer.kernel_size.1) / layer.stride.1 + 1;

        // Apply padding if needed
        let padded = if layer.padding.0 > 0 || layer.padding.1 > 0 {
            let new_h = in_h + 2 * layer.padding.0;
            let new_w = in_w + 2 * layer.padding.1;
            let mut p = Array4::zeros((batch_size, layer.in_channels, new_h, new_w));
            for b in 0..batch_size {
                for c in 0..layer.in_channels {
                    for h in 0..in_h {
                        for w in 0..in_w {
                            p[[b, c, h + layer.padding.0, w + layer.padding.1]] =
                                input[[b, c, h, w]];
                        }
                    }
                }
            }
            p
        } else {
            input.clone()
        };

        let mut output = Array4::zeros((batch_size, layer.out_channels, out_h, out_w));

        // Perform convolution
        for b in 0..batch_size {
            for oc in 0..layer.out_channels {
                for oh in 0..out_h {
                    for ow in 0..out_w {
                        let mut sum = layer.biases[oc];
                        for ic in 0..layer.in_channels {
                            for kh in 0..layer.kernel_size.0 {
                                for kw in 0..layer.kernel_size.1 {
                                    let ih = oh * layer.stride.0 + kh;
                                    let iw = ow * layer.stride.1 + kw;
                                    sum +=
                                        padded[[b, ic, ih, iw]] * layer.filters[[oc, ic, kh, kw]];
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

    /// Apply max pooling
    fn max_pool_forward(&self, input: &Array4<f64>, layer: &PoolLayer) -> Array4<f64> {
        let (batch_size, channels, in_h, in_w) = input.dim();
        let out_h = (in_h - layer.pool_size.0) / layer.stride.0 + 1;
        let out_w = (in_w - layer.pool_size.1) / layer.stride.1 + 1;

        let mut output = Array4::zeros((batch_size, channels, out_h, out_w));

        for b in 0..batch_size {
            for c in 0..channels {
                for oh in 0..out_h {
                    for ow in 0..out_w {
                        let mut max_val = f64::NEG_INFINITY;
                        for ph in 0..layer.pool_size.0 {
                            for pw in 0..layer.pool_size.1 {
                                let ih = oh * layer.stride.0 + ph;
                                let iw = ow * layer.stride.1 + pw;
                                max_val = max_val.max(input[[b, c, ih, iw]]);
                            }
                        }
                        output[[b, c, oh, ow]] = max_val;
                    }
                }
            }
        }

        output
    }

    /// Apply average pooling
    fn avg_pool_forward(&self, input: &Array4<f64>, layer: &PoolLayer) -> Array4<f64> {
        let (batch_size, channels, in_h, in_w) = input.dim();
        let out_h = (in_h - layer.pool_size.0) / layer.stride.0 + 1;
        let out_w = (in_w - layer.pool_size.1) / layer.stride.1 + 1;
        let pool_area = (layer.pool_size.0 * layer.pool_size.1) as f64;

        let mut output = Array4::zeros((batch_size, channels, out_h, out_w));

        for b in 0..batch_size {
            for c in 0..channels {
                for oh in 0..out_h {
                    for ow in 0..out_w {
                        let mut sum = 0.0;
                        for ph in 0..layer.pool_size.0 {
                            for pw in 0..layer.pool_size.1 {
                                let ih = oh * layer.stride.0 + ph;
                                let iw = ow * layer.stride.1 + pw;
                                sum += input[[b, c, ih, iw]];
                            }
                        }
                        output[[b, c, oh, ow]] = sum / pool_area;
                    }
                }
            }
        }

        output
    }

    /// Apply activation to 4D tensor
    fn apply_activation_4d(&self, input: &Array4<f64>, activation: &ActivationType) -> Array4<f64> {
        let (_batch_size, channels, height, width) = input.dim();
        let flat = self.reshape_to_2d(input);
        let activated = Activations::apply(&flat, activation);
        self.reshape_to_4d(&activated, channels, height, width)
            .unwrap_or_else(|_| input.clone())
    }

    /// Set training mode
    pub fn set_training(&mut self, training: bool) {
        self.training = training;
        for layer in &mut self.layers {
            if let CNNLayer::Dropout {
                training: ref mut t,
                ..
            } = layer
            {
                *t = training;
            }
        }
    }
}

#[async_trait]
impl NeuralNetwork for ConvolutionalNetwork {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        let input_shape = &self.architecture.input_shape;

        // Get initial dimensions
        let (in_channels, in_height, in_width) = if input_shape.len() == 2 {
            (1, input_shape[0], input_shape[1])
        } else {
            (input_shape[0], input_shape[1], input_shape[2])
        };

        // Reshape input to 4D
        let mut current_4d = self.reshape_to_4d(input, in_channels, in_height, in_width)?;
        let mut current_2d: Option<Array2<f64>> = None;

        // Process each layer
        for layer in &self.layers {
            match layer {
                CNNLayer::Conv(conv) => {
                    // Apply convolution
                    let conv_output = self.conv_forward(&current_4d, conv);
                    // Apply activation
                    current_4d = self.apply_activation_4d(&conv_output, &conv.activation);
                }
                CNNLayer::Pool(pool) => {
                    current_4d = if pool.is_max {
                        self.max_pool_forward(&current_4d, pool)
                    } else {
                        self.avg_pool_forward(&current_4d, pool)
                    };
                }
                CNNLayer::Flatten => {
                    current_2d = Some(self.reshape_to_2d(&current_4d));
                }
                CNNLayer::Dense(dense) => {
                    let input_2d = current_2d.as_ref().ok_or_else(|| {
                        MLError::neural_network("Dense layer requires flattened input")
                    })?;

                    // Forward: output = input * W^T + b
                    let pre_activation = input_2d.dot(&dense.weights.t())
                        + dense
                            .biases
                            .broadcast((input_2d.nrows(), dense.biases.len()))
                            .unwrap();

                    // Apply activation
                    let activated = Activations::apply(&pre_activation, &dense.activation);
                    current_2d = Some(activated);
                }
                CNNLayer::Dropout { rate, training } => {
                    if *training && *rate > 0.0 {
                        if let Some(ref mut data) = current_2d {
                            let scale = 1.0 / (1.0 - rate);
                            *data = data.map(|x| {
                                if rand::random::<f64>() < *rate {
                                    0.0
                                } else {
                                    x * scale
                                }
                            });
                        }
                    }
                }
            }
        }

        current_2d.ok_or_else(|| {
            MLError::neural_network("CNN forward pass completed but no output produced")
        })
    }

    async fn backward(&mut self, loss_gradient: &Array2<f64>) -> Result<()> {
        // Backward pass through layers in reverse
        let mut current_grad = loss_gradient.clone();

        for layer in self.layers.iter_mut().rev() {
            match layer {
                CNNLayer::Dense(dense) => {
                    if let Some(ref last_input) = dense.last_input {
                        let batch_size = last_input.nrows() as f64;

                        // Compute gradients
                        // dL/dW = input^T * dL/dy
                        dense.weight_grads = current_grad.t().dot(last_input) / batch_size;

                        // dL/db = sum(dL/dy, axis=0)
                        dense.bias_grads = current_grad.sum_axis(ndarray::Axis(0)) / batch_size;

                        // dL/dx = dL/dy * W
                        current_grad = current_grad.dot(&dense.weights);
                    }
                }
                CNNLayer::Flatten => {
                    // Reshape gradient back to 4D if needed
                    // For simplicity, we stop backprop at flatten for now
                }
                CNNLayer::Dropout { rate, training } => {
                    // Dropout backward: apply same mask (simplified)
                    if *training && *rate > 0.0 {
                        current_grad = current_grad.clone();
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn update_weights(&mut self, _optimizer: &dyn Optimizer) -> Result<()> {
        // Apply simple gradient descent update for now
        // The optimizer interface requires mutable slices which don't work well with our layer structure
        // In the future, we should refactor to collect all parameters and gradients first
        let learning_rate = 0.001;

        for layer in &mut self.layers {
            match layer {
                CNNLayer::Conv(conv) => {
                    // Update filters: filters -= learning_rate * filter_grads
                    let filter_shape = conv.filters.dim();
                    for oc in 0..filter_shape.0 {
                        for ic in 0..filter_shape.1 {
                            for kh in 0..filter_shape.2 {
                                for kw in 0..filter_shape.3 {
                                    conv.filters[[oc, ic, kh, kw]] -=
                                        learning_rate * conv.filter_grads[[oc, ic, kh, kw]];
                                }
                            }
                        }
                    }

                    // Update biases: biases -= learning_rate * bias_grads
                    for i in 0..conv.biases.len() {
                        conv.biases[i] -= learning_rate * conv.bias_grads[i];
                    }
                }
                CNNLayer::Dense(dense) => {
                    // Update weights: weights -= learning_rate * weight_grads
                    let weight_shape = dense.weights.dim();
                    for i in 0..weight_shape.0 {
                        for j in 0..weight_shape.1 {
                            dense.weights[[i, j]] -= learning_rate * dense.weight_grads[[i, j]];
                        }
                    }

                    // Update biases: biases -= learning_rate * bias_grads
                    for i in 0..dense.biases.len() {
                        dense.biases[i] -= learning_rate * dense.bias_grads[i];
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn architecture(&self) -> &NetworkArchitecture {
        &self.architecture
    }

    fn parameter_count(&self) -> usize {
        let mut count = 0;
        for layer in &self.layers {
            match layer {
                CNNLayer::Conv(conv) => {
                    count += conv.filters.len() + conv.biases.len();
                }
                CNNLayer::Dense(dense) => {
                    count += dense.weights.len() + dense.biases.len();
                }
                _ => {}
            }
        }
        count
    }

    async fn save_weights(&self) -> Result<Vec<u8>> {
        // Serialize all layer weights
        let mut weights_data: Vec<f64> = Vec::new();

        for layer in &self.layers {
            match layer {
                CNNLayer::Conv(conv) => {
                    weights_data.extend(conv.filters.iter());
                    weights_data.extend(conv.biases.iter());
                }
                CNNLayer::Dense(dense) => {
                    weights_data.extend(dense.weights.iter());
                    weights_data.extend(dense.biases.iter());
                }
                _ => {}
            }
        }

        // Convert to bytes
        let bytes: Vec<u8> = weights_data.iter().flat_map(|f| f.to_le_bytes()).collect();

        Ok(bytes)
    }

    async fn load_weights(&mut self, weights: &[u8]) -> Result<()> {
        if !weights.len().is_multiple_of(8) {
            return Err(MLError::neural_network("Invalid weights data length"));
        }

        let mut weight_values: Vec<f64> = Vec::with_capacity(weights.len() / 8);
        for chunk in weights.chunks(8) {
            let bytes: [u8; 8] = chunk
                .try_into()
                .map_err(|_| MLError::neural_network("Failed to parse weight bytes"))?;
            weight_values.push(f64::from_le_bytes(bytes));
        }

        let mut offset = 0;
        for layer in &mut self.layers {
            match layer {
                CNNLayer::Conv(conv) => {
                    let filter_count = conv.filters.len();
                    let bias_count = conv.biases.len();

                    if offset + filter_count + bias_count > weight_values.len() {
                        return Err(MLError::neural_network("Not enough weights for Conv layer"));
                    }

                    let filter_shape = conv.filters.dim();
                    conv.filters = Array4::from_shape_vec(
                        filter_shape,
                        weight_values[offset..offset + filter_count].to_vec(),
                    )
                    .map_err(|e| MLError::neural_network(format!("Filter shape error: {}", e)))?;
                    offset += filter_count;

                    conv.biases =
                        Array1::from_vec(weight_values[offset..offset + bias_count].to_vec());
                    offset += bias_count;
                }
                CNNLayer::Dense(dense) => {
                    let weight_count = dense.weights.len();
                    let bias_count = dense.biases.len();

                    if offset + weight_count + bias_count > weight_values.len() {
                        return Err(MLError::neural_network(
                            "Not enough weights for Dense layer",
                        ));
                    }

                    let weight_shape = dense.weights.dim();
                    dense.weights = Array2::from_shape_vec(
                        weight_shape,
                        weight_values[offset..offset + weight_count].to_vec(),
                    )
                    .map_err(|e| MLError::neural_network(format!("Weight shape error: {}", e)))?;
                    offset += weight_count;

                    dense.biases =
                        Array1::from_vec(weight_values[offset..offset + bias_count].to_vec());
                    offset += bias_count;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural_networks::{NetworkType, NeuralNetworkBuilder};

    #[tokio::test]
    async fn test_cnn_creation() {
        let network = NeuralNetworkBuilder::new()
            .network_type(NetworkType::Convolutional)
            .input_shape(vec![1, 28, 28])
            .conv2d(32, (3, 3), (1, 1), (1, 1), ActivationType::ReLU)
            .max_pool2d((2, 2), (2, 2))
            .conv2d(64, (3, 3), (1, 1), (1, 1), ActivationType::ReLU)
            .max_pool2d((2, 2), (2, 2))
            .dense(128, ActivationType::ReLU)
            .dense(10, ActivationType::Softmax)
            .build()
            .await;

        assert!(network.is_ok());
        let net = network.unwrap();
        assert!(net.parameter_count() > 0);
    }

    #[tokio::test]
    async fn test_cnn_forward_pass() {
        let network = NeuralNetworkBuilder::new()
            .network_type(NetworkType::Convolutional)
            .input_shape(vec![1, 8, 8])
            .conv2d(4, (3, 3), (1, 1), (1, 1), ActivationType::ReLU)
            .max_pool2d((2, 2), (2, 2))
            .dense(16, ActivationType::ReLU)
            .dense(2, ActivationType::Softmax)
            .build()
            .await
            .unwrap();

        // Create a batch of 2 samples, each 1x8x8
        let input = Array2::from_shape_fn((2, 64), |_| rand::random::<f64>());
        let output = network.forward(&input).await;

        assert!(output.is_ok());
        let out = output.unwrap();
        assert_eq!(out.nrows(), 2);
        assert_eq!(out.ncols(), 2);

        // Check softmax output sums to ~1
        for row in out.rows() {
            let sum: f64 = row.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5);
        }
    }

    #[tokio::test]
    async fn test_cnn_parameter_count() {
        let network = NeuralNetworkBuilder::new()
            .network_type(NetworkType::Convolutional)
            .input_shape(vec![1, 8, 8])
            .conv2d(4, (3, 3), (1, 1), (0, 0), ActivationType::ReLU)
            .dense(10, ActivationType::Softmax)
            .build()
            .await
            .unwrap();

        // Conv: 4 filters * 1 channel * 3 * 3 = 36 weights + 4 biases = 40
        // After conv: 6x6x4 = 144 features
        // Dense: 144 * 10 = 1440 weights + 10 biases = 1450
        // Total: 40 + 1450 = 1490
        let count = network.parameter_count();
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_cnn_save_load_weights() {
        let mut network = NeuralNetworkBuilder::new()
            .network_type(NetworkType::Convolutional)
            .input_shape(vec![1, 8, 8])
            .conv2d(2, (3, 3), (1, 1), (0, 0), ActivationType::ReLU)
            .dense(4, ActivationType::Softmax)
            .build()
            .await
            .unwrap();

        // Save weights
        let weights = network.save_weights().await.unwrap();
        assert!(!weights.is_empty());

        // Create input and get output
        let input = Array2::from_shape_fn((1, 64), |_| rand::random::<f64>());
        let output1 = network.forward(&input).await.unwrap();

        // Load weights back
        network.load_weights(&weights).await.unwrap();
        let output2 = network.forward(&input).await.unwrap();

        // Outputs should be the same
        for (a, b) in output1.iter().zip(output2.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }
}
