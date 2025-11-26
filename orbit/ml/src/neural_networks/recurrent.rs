//! Recurrent Neural Network implementation.

use async_trait::async_trait;
use ndarray::Array2;

use crate::error::{MLError, Result};
use crate::neural_networks::{NetworkArchitecture, NeuralNetwork, Optimizer};

/// Recurrent Neural Network implementation
///
/// A neural network with feedback connections that can process
/// sequences of data by maintaining internal state across time steps.
use ndarray::{s, Array1, Axis};
use rand::distributions::Uniform;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Cache for storing forward pass states (needed for BPTT)
#[derive(Debug, Clone, Default)]
struct RNNCache {
    layers: Vec<Vec<RNNTimeStep>>,
}

#[derive(Debug, Clone)]
struct RNNTimeStep {
    h: Array1<f64>,
    x: Array1<f64>,
}

/// Gradients for an RNN layer
#[derive(Debug, Clone, Default)]
struct RNNLayerGradients {
    d_w: Array2<f64>,
    d_u: Array2<f64>,
    d_b: Array1<f64>,
}

/// Recurrent Neural Network implementation
///
/// A neural network with feedback connections that can process
/// sequences of data by maintaining internal state across time steps.
#[derive(Debug, Clone)]
pub struct RecurrentNetwork {
    architecture: NetworkArchitecture,
    layers: Vec<RNNLayerWeights>,

    // Cache for BPTT
    cache: Arc<RwLock<RNNCache>>,

    // Accumulated gradients
    gradients: Vec<RNNLayerGradients>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RNNLayerWeights {
    // Input weights
    w: Array2<f64>,
    // Recurrent weights
    u: Array2<f64>,
    // Bias
    b: Array1<f64>,
}

impl RecurrentNetwork {
    /// Create a new recurrent neural network with given architecture
    ///
    /// # Arguments
    /// * `architecture` - Network architecture specification
    ///
    /// # Returns
    /// A new RNN instance
    pub async fn new(architecture: NetworkArchitecture) -> Result<Self> {
        let mut layers = Vec::new();
        let mut rng = rand::thread_rng();
        let range = Uniform::new(-0.1, 0.1);

        let mut input_size = architecture.input_shape.iter().product::<usize>();

        for layer_config in &architecture.layers {
            if let crate::neural_networks::LayerType::RNN = layer_config.layer_type {
                let hidden_size = layer_config.units;

                // Create random matrices inline to avoid borrow issues
                let layer_weights = RNNLayerWeights {
                    w: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),
                };

                layers.push(layer_weights);
                input_size = hidden_size;
            }
        }

        Ok(Self {
            architecture,
            layers,
            cache: Arc::new(RwLock::new(RNNCache::default())),
            gradients: Vec::new(),
        })
    }
}

#[async_trait]
impl NeuralNetwork for RecurrentNetwork {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        let seq_len = input.nrows();
        let mut current_input = input.clone();
        let mut layer_caches = Vec::new();

        for layer in &self.layers {
            let hidden_size = layer.w.ncols();
            let mut h_t = Array1::<f64>::zeros(hidden_size); // Hidden state
            let mut layer_output = Array2::<f64>::zeros((seq_len, hidden_size));
            let mut layer_time_steps = Vec::with_capacity(seq_len);

            for t in 0..seq_len {
                let x_t = current_input.row(t).to_owned();

                let tanh = |x: f64| x.tanh();

                // h_t = tanh(W * x_t + U * h_{t-1} + b)
                h_t = (&x_t.dot(&layer.w) + &h_t.dot(&layer.u) + &layer.b).mapv(tanh);

                layer_time_steps.push(RNNTimeStep {
                    h: h_t.clone(),
                    x: x_t,
                });

                layer_output.row_mut(t).assign(&h_t);
            }

            layer_caches.push(layer_time_steps);
            current_input = layer_output;
        }

        if let Ok(mut cache) = self.cache.write() {
            cache.layers = layer_caches;
        }

        Ok(current_input)
    }

    async fn backward(&mut self, loss_gradient: &Array2<f64>) -> Result<()> {
        let cache = self.cache.read().unwrap().clone();
        if cache.layers.is_empty() {
            return Err(MLError::internal("No forward pass cache found"));
        }

        self.gradients = self
            .layers
            .iter()
            .map(|l| RNNLayerGradients {
                d_w: Array2::zeros(l.w.raw_dim()),
                d_u: Array2::zeros(l.u.raw_dim()),
                d_b: Array1::zeros(l.b.raw_dim()),
            })
            .collect();

        let mut next_layer_grad = loss_gradient.clone();

        for (layer_idx, layer) in self.layers.iter().enumerate().rev() {
            let layer_cache = &cache.layers[layer_idx];
            let seq_len = layer_cache.len();
            let hidden_size = layer.w.ncols();
            let input_size = layer.w.nrows();

            let mut d_h_next = Array1::<f64>::zeros(hidden_size);
            let mut d_prev_layer = Array2::<f64>::zeros((seq_len, input_size));

            let gradients = &mut self.gradients[layer_idx];

            for t in (0..seq_len).rev() {
                let step = &layer_cache[t];
                let h_prev = if t > 0 {
                    &layer_cache[t - 1].h
                } else {
                    &Array1::zeros(hidden_size)
                };

                // dL/dh_t = dL/dy_t + dL/dh_{t+1} * dh_{t+1}/dh_t
                let d_h = &next_layer_grad.row(t) + &d_h_next;

                // h_t = tanh(z), where z = Wx + Uh + b
                // dh/dz = 1 - h_t^2
                let d_z = &d_h * (1.0 - step.h.mapv(|x| x * x));

                // Accumulate gradients
                // dW += x^T * d_z
                let x_t_t = step.x.clone().insert_axis(Axis(1));
                let d_z_row = d_z.clone().insert_axis(Axis(0));

                gradients.d_w = &gradients.d_w + &x_t_t.dot(&d_z_row);

                // dU += h_{t-1}^T * d_z
                let h_prev_t = h_prev.clone().insert_axis(Axis(1));
                gradients.d_u = &gradients.d_u + &h_prev_t.dot(&d_z_row);

                // db += d_z
                gradients.d_b = &gradients.d_b + &d_z;

                // Compute d_h_prev
                // d_h_prev = d_z * U^T
                d_h_next = d_z.dot(&layer.u.t());

                // Compute d_x
                // d_x = d_z * W^T
                let d_x = d_z.dot(&layer.w.t());
                d_prev_layer.row_mut(t).assign(&d_x);
            }
            next_layer_grad = d_prev_layer;
        }
        Ok(())
    }

    async fn update_weights(&mut self, _optimizer: &dyn Optimizer) -> Result<()> {
        if self.gradients.is_empty() {
            return Ok(());
        }
        let lr = 0.01; // TODO: Use optimizer

        for (i, layer) in self.layers.iter_mut().enumerate() {
            let g = &self.gradients[i];
            layer.w = &layer.w - &(g.d_w.mapv(|x| x * lr));
            layer.u = &layer.u - &(g.d_u.mapv(|x| x * lr));
            layer.b = &layer.b - &(g.d_b.mapv(|x| x * lr));
        }
        self.gradients.clear();
        Ok(())
    }

    fn architecture(&self) -> &NetworkArchitecture {
        &self.architecture
    }

    fn parameter_count(&self) -> usize {
        let mut count = 0;
        for layer in &self.layers {
            count += layer.w.len();
            count += layer.u.len();
            count += layer.b.len();
        }
        count
    }

    async fn save_weights(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self.layers)
            .map_err(|e| MLError::neural_network(format!("Failed to serialize weights: {}", e)))
    }

    async fn load_weights(&mut self, weights: &[u8]) -> Result<()> {
        let layers: Vec<RNNLayerWeights> = bincode::deserialize(weights).map_err(|e| {
            MLError::neural_network(format!("Failed to deserialize weights: {}", e))
        })?;

        if layers.len() != self.layers.len() {
            return Err(MLError::neural_network(
                "Loaded weights do not match network architecture",
            ));
        }

        self.layers = layers;
        if let Ok(mut cache) = self.cache.write() {
            cache.layers.clear();
        }
        self.gradients.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural_networks::{
        ActivationType, LayerConfig, LayerType, OptimizerConfig, OptimizerType,
    };
    use std::collections::HashMap;

    fn create_test_architecture() -> NetworkArchitecture {
        NetworkArchitecture {
            network_type: crate::neural_networks::NetworkType::Recurrent,
            input_shape: vec![10],
            layers: vec![LayerConfig {
                layer_type: LayerType::RNN,
                units: 5,
                activation: ActivationType::Tanh,
                dropout: None,
                l1_regularization: None,
                l2_regularization: None,
                parameters: HashMap::new(),
            }],
            output_shape: vec![5],
            loss_function: "mse".to_string(),
            optimizer: OptimizerConfig {
                optimizer_type: OptimizerType::Adam,
                learning_rate: 0.001,
                parameters: HashMap::new(),
            },
        }
    }

    #[tokio::test]
    async fn test_rnn_creation() {
        let architecture = create_test_architecture();
        let network = RecurrentNetwork::new(architecture).await;
        assert!(network.is_ok());
    }

    #[tokio::test]
    async fn test_rnn_forward() {
        let architecture = create_test_architecture();
        let network = RecurrentNetwork::new(architecture).await.unwrap();

        // Input: [sequence_length=3, features=10]
        let input = Array2::zeros((3, 10));
        let output = network.forward(&input).await;

        assert!(output.is_ok());
        let output = output.unwrap();
        assert_eq!(output.shape(), &[3, 5]); // [seq_len, hidden_size]
    }
}
