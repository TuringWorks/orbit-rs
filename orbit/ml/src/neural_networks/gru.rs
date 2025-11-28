//! GRU Neural Network implementation.

use async_trait::async_trait;
use ndarray::{Array1, Array2, Axis};
use crate::error::{MLError, Result};
use crate::neural_networks::{NetworkArchitecture, NeuralNetwork, Optimizer};
use rand::distributions::Uniform;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Cache for storing forward pass states (needed for BPTT)
#[derive(Debug, Clone, Default)]
struct GRUCache {
    /// Layer states: [layer_idx][time_step] -> (z, r, h_tilde, h, x)
    /// We store x (input) as well because it's needed for gradients
    layers: Vec<Vec<GRUTimeStep>>,
}

#[derive(Debug, Clone)]
struct GRUTimeStep {
    z: Array1<f64>,
    r: Array1<f64>,
    h_tilde: Array1<f64>,
    h: Array1<f64>,
    x: Array1<f64>, // Input to this layer at this step
}

/// Gradients for a GRU layer
#[derive(Debug, Clone, Default)]
struct GRULayerGradients {
    d_w_z: Array2<f64>,
    d_u_z: Array2<f64>,
    d_b_z: Array1<f64>,

    d_w_r: Array2<f64>,
    d_u_r: Array2<f64>,
    d_b_r: Array1<f64>,

    d_w_h: Array2<f64>,
    d_u_h: Array2<f64>,
    d_b_h: Array1<f64>,
}

/// GRU (Gated Recurrent Unit) Neural Network implementation
///
/// A type of recurrent neural network that uses gating mechanisms
/// to control information flow and address vanishing gradient problems.
#[derive(Debug, Clone)]
pub struct GRUNetwork {
    architecture: NetworkArchitecture,

    // Weights and biases for each layer
    // Stored as a list of (W_z, U_z, b_z, W_r, U_r, b_r, W_h, U_h, b_h) tuples
    // W: input weights, U: recurrent weights, b: biases
    // z: update gate, r: reset gate, h: candidate hidden state
    layers: Vec<GRULayerWeights>,

    // Cache for BPTT (interior mutability needed for forward pass)
    cache: Arc<RwLock<GRUCache>>,

    // Accumulated gradients
    gradients: Vec<GRULayerGradients>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GRULayerWeights {
    // Update gate
    w_z: Array2<f64>,
    u_z: Array2<f64>,
    b_z: Array1<f64>,

    // Reset gate
    w_r: Array2<f64>,
    u_r: Array2<f64>,
    b_r: Array1<f64>,

    // Candidate hidden state
    w_h: Array2<f64>,
    u_h: Array2<f64>,
    b_h: Array1<f64>,
}

impl GRUNetwork {
    /// Create a new GRU network with given architecture
    ///
    /// # Arguments
    /// * `architecture` - Network architecture specification
    ///
    /// # Returns
    /// A new GRU network instance
    pub async fn new(architecture: NetworkArchitecture) -> Result<Self> {
        let mut layers = Vec::new();
        let mut rng = rand::thread_rng();
        let range = Uniform::new(-0.1, 0.1);

        // Initialize weights for each GRU layer
        // Note: This assumes the architecture defines GRU layers correctly
        // We iterate through layers and initialize weights for GRU types

        let mut input_size = architecture.input_shape.iter().product::<usize>();

        for layer_config in &architecture.layers {
            if let crate::neural_networks::LayerType::GRU = layer_config.layer_type {
                let hidden_size = layer_config.units;

                // Create random matrices inline to avoid borrow issues
                let layer_weights = GRULayerWeights {
                    // Update gate
                    w_z: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u_z: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b_z: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),

                    // Reset gate
                    w_r: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u_r: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b_r: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),

                    // Candidate hidden state
                    w_h: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u_h: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b_h: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),
                };

                layers.push(layer_weights);
                input_size = hidden_size; // Output of this layer is input to next
            }
        }

        Ok(Self {
            architecture,
            layers,
            cache: Arc::new(RwLock::new(GRUCache::default())),
            gradients: Vec::new(),
        })
    }
}

#[async_trait]
impl NeuralNetwork for GRUNetwork {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        // Input shape: [batch_size, sequence_length, input_features]
        // But ndarray is 2D, so we might receive [batch_size, input_features] for a single step
        // or we need to handle 3D tensors.
        // For simplicity in this MVP, let's assume input is [sequence_length, input_features] (batch_size=1)
        // or [batch_size, input_features] and we treat it as a single step if not recurrent?
        // Actually, RNNs usually take [batch_size, seq_len, features].
        // Given the trait signature `forward(&self, input: &Array2<f64>) -> Result<Array2<f64>>`,
        // it seems we are constrained to 2D.
        // Let's assume input is [sequence_length, input_features] representing one sequence.

        let seq_len = input.nrows();
        // let _features = input.ncols();

        let mut current_input = input.clone();
        let mut layer_caches = Vec::new();

        for layer in &self.layers {
            let hidden_size = layer.w_z.ncols();
            let mut hidden_state = Array1::<f64>::zeros(hidden_size);
            let mut layer_output = Array2::<f64>::zeros((seq_len, hidden_size));
            let mut layer_time_steps = Vec::with_capacity(seq_len);

            for t in 0..seq_len {
                let x_t = current_input.row(t).to_owned();

                // Sigmoid function
                let sigmoid = |x: f64| 1.0 / (1.0 + (-x).exp());
                // Tanh function
                let tanh = |x: f64| x.tanh();

                // Update gate: z_t = sigmoid(W_z * x_t + U_z * h_{t-1} + b_z)
                let z_t = (&x_t.dot(&layer.w_z) + &hidden_state.dot(&layer.u_z) + &layer.b_z)
                    .mapv(sigmoid);

                // Reset gate: r_t = sigmoid(W_r * x_t + U_r * h_{t-1} + b_r)
                let r_t = (&x_t.dot(&layer.w_r) + &hidden_state.dot(&layer.u_r) + &layer.b_r)
                    .mapv(sigmoid);

                // Candidate hidden state: h_tilde_t = tanh(W_h * x_t + U_h * (r_t * h_{t-1}) + b_h)
                let r_h = &r_t * &hidden_state; // Element-wise multiplication
                let h_tilde_t =
                    (&x_t.dot(&layer.w_h) + &r_h.dot(&layer.u_h) + &layer.b_h).mapv(tanh);

                // New hidden state: h_t = (1 - z_t) * h_{t-1} + z_t * h_tilde_t
                let h_t = (&(1.0 - &z_t) * &hidden_state) + (&z_t * &h_tilde_t);

                // Store state for BPTT
                layer_time_steps.push(GRUTimeStep {
                    z: z_t.clone(),
                    r: r_t.clone(),
                    h_tilde: h_tilde_t.clone(),
                    h: h_t.clone(),
                    x: x_t,
                });

                hidden_state = h_t;
                layer_output.row_mut(t).assign(&hidden_state);
            }

            layer_caches.push(layer_time_steps);
            current_input = layer_output;
        }

        // Update cache
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

        // Initialize gradients
        self.gradients = self
            .layers
            .iter()
            .map(|l| GRULayerGradients {
                d_w_z: Array2::zeros(l.w_z.raw_dim()),
                d_u_z: Array2::zeros(l.u_z.raw_dim()),
                d_b_z: Array1::zeros(l.b_z.raw_dim()),
                d_w_r: Array2::zeros(l.w_r.raw_dim()),
                d_u_r: Array2::zeros(l.u_r.raw_dim()),
                d_b_r: Array1::zeros(l.b_r.raw_dim()),
                d_w_h: Array2::zeros(l.w_h.raw_dim()),
                d_u_h: Array2::zeros(l.u_h.raw_dim()),
                d_b_h: Array1::zeros(l.b_h.raw_dim()),
            })
            .collect();

        // Propagate gradients backward through layers
        let mut next_layer_grad = loss_gradient.clone(); // Gradient w.r.t output of current layer

        for (layer_idx, layer) in self.layers.iter().enumerate().rev() {
            let layer_cache = &cache.layers[layer_idx];
            let seq_len = layer_cache.len();
            let hidden_size = layer.w_z.ncols();
            let input_size = layer.w_z.nrows();

            let mut d_h_next = Array1::<f64>::zeros(hidden_size); // Gradient w.r.t h_{t} from t+1
            let mut d_prev_layer = Array2::<f64>::zeros((seq_len, input_size));

            let gradients = &mut self.gradients[layer_idx];

            for t in (0..seq_len).rev() {
                let step = &layer_cache[t];
                let h_prev = if t > 0 {
                    &layer_cache[t - 1].h
                } else {
                    &Array1::zeros(hidden_size)
                };

                // Gradient of loss w.r.t h_t
                // dL/dh_t = dL/dy_t (from next layer/loss) + dL/dh_{t+1} * dh_{t+1}/dh_t
                let d_h = &next_layer_grad.row(t) + &d_h_next;

                // h_t = (1 - z) * h_prev + z * h_tilde

                // 1. Gradient w.r.t h_tilde
                // d_h_tilde = d_h * z * (1 - tanh^2(h_tilde))
                let d_h_tilde = &d_h * &step.z * (1.0 - step.h_tilde.mapv(|x| x * x));

                // 2. Gradient w.r.t z
                // d_z = d_h * (h_tilde - h_prev) * z * (1 - z)
                let d_z = &d_h * &(&step.h_tilde - h_prev) * &step.z * (1.0 - &step.z);

                // 3. Gradient w.r.t r
                // r is inside h_tilde: h_tilde = tanh(Wx + U(r * h_prev) + b)
                // d_r = (d_h_tilde * U_h) * h_prev * r * (1 - r)
                // Note: U_h is (hidden, hidden). d_h_tilde is (hidden).
                // d_h_tilde * U_h^T gives gradient w.r.t (r * h_prev)
                let d_r_term = d_h_tilde.dot(&layer.u_h.t());
                let d_r = &d_r_term * h_prev * &step.r * (1.0 - &step.r);

                // Accumulate weight gradients
                // dW_z += x^T * d_z
                gradients.d_w_z = &gradients.d_w_z
                    + &step
                        .x
                        .clone()
                        .insert_axis(Axis(1))
                        .dot(&d_z.clone().insert_axis(Axis(0)));
                gradients.d_u_z = &gradients.d_u_z
                    + &h_prev
                        .clone()
                        .insert_axis(Axis(1))
                        .dot(&d_z.clone().insert_axis(Axis(0)));
                gradients.d_b_z = &gradients.d_b_z + &d_z;

                gradients.d_w_r = &gradients.d_w_r
                    + &step
                        .x
                        .clone()
                        .insert_axis(Axis(1))
                        .dot(&d_r.clone().insert_axis(Axis(0)));
                gradients.d_u_r = &gradients.d_u_r
                    + &h_prev
                        .clone()
                        .insert_axis(Axis(1))
                        .dot(&d_r.clone().insert_axis(Axis(0)));
                gradients.d_b_r = &gradients.d_b_r + &d_r;

                gradients.d_w_h = &gradients.d_w_h
                    + &step
                        .x
                        .clone()
                        .insert_axis(Axis(1))
                        .dot(&d_h_tilde.clone().insert_axis(Axis(0)));
                // For U_h, input is (r * h_prev)
                let r_h_prev = &step.r * h_prev;
                gradients.d_u_h = &gradients.d_u_h
                    + &r_h_prev
                        .insert_axis(Axis(1))
                        .dot(&d_h_tilde.clone().insert_axis(Axis(0)));
                gradients.d_b_h = &gradients.d_b_h + &d_h_tilde;

                // Compute d_h_prev (to pass to t-1)
                // d_h_prev = d_h * (1 - z)  (from h_t equation)
                //          + d_z * U_z      (from z gate)
                //          + d_r * U_r      (from r gate)
                //          + d_h_tilde * U_h * r (from h_tilde)

                let term1 = &d_h * (1.0 - &step.z);
                let term2 = d_z.dot(&layer.u_z.t());
                let term3 = d_r.dot(&layer.u_r.t());
                let term4 = d_h_tilde.dot(&layer.u_h.t()) * &step.r;

                d_h_next = term1 + term2 + term3 + term4;

                // Compute d_x (to pass to previous layer)
                // d_x = d_z * W_z + d_r * W_r + d_h_tilde * W_h
                let d_x = d_z.dot(&layer.w_z.t())
                    + d_r.dot(&layer.w_r.t())
                    + d_h_tilde.dot(&layer.w_h.t());
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

        for (i, layer) in self.layers.iter_mut().enumerate() {
            let grads = &self.gradients[i];

            // Apply optimizer updates
            // Note: Optimizer trait typically takes parameter name/key.
            // Here we have many parameters.
            // We might need to extend Optimizer to handle raw updates or map params.
            // For MVP, we'll do simple SGD update: param -= lr * grad

            // TODO: Use actual optimizer. For now, simple SGD with lr=0.01
            let lr = 0.01;

            layer.w_z = &layer.w_z - &(grads.d_w_z.mapv(|g| g * lr));
            layer.u_z = &layer.u_z - &(grads.d_u_z.mapv(|g| g * lr));
            layer.b_z = &layer.b_z - &(grads.d_b_z.mapv(|g| g * lr));

            layer.w_r = &layer.w_r - &(grads.d_w_r.mapv(|g| g * lr));
            layer.u_r = &layer.u_r - &(grads.d_u_r.mapv(|g| g * lr));
            layer.b_r = &layer.b_r - &(grads.d_b_r.mapv(|g| g * lr));

            layer.w_h = &layer.w_h - &(grads.d_w_h.mapv(|g| g * lr));
            layer.u_h = &layer.u_h - &(grads.d_u_h.mapv(|g| g * lr));
            layer.b_h = &layer.b_h - &(grads.d_b_h.mapv(|g| g * lr));
        }

        // Clear gradients
        self.gradients.clear();

        Ok(())
    }

    fn architecture(&self) -> &NetworkArchitecture {
        &self.architecture
    }

    fn parameter_count(&self) -> usize {
        let mut count = 0;
        for layer in &self.layers {
            count += layer.w_z.len();
            count += layer.u_z.len();
            count += layer.b_z.len();

            count += layer.w_r.len();
            count += layer.u_r.len();
            count += layer.b_r.len();

            count += layer.w_h.len();
            count += layer.u_h.len();
            count += layer.b_h.len();
        }
        count
    }

    async fn save_weights(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self.layers)
            .map_err(|e| MLError::neural_network(format!("Failed to serialize weights: {}", e)))
    }

    async fn load_weights(&mut self, weights: &[u8]) -> Result<()> {
        let layers: Vec<GRULayerWeights> = bincode::deserialize(weights).map_err(|e| {
            MLError::neural_network(format!("Failed to deserialize weights: {}", e))
        })?;

        if layers.len() != self.layers.len() {
            return Err(MLError::neural_network(
                "Loaded weights do not match network architecture",
            ));
        }

        self.layers = layers;
        // Clear cache and gradients as they are invalid for new weights
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
            network_type: crate::neural_networks::NetworkType::GRU,
            input_shape: vec![10],
            layers: vec![LayerConfig {
                layer_type: LayerType::GRU,
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
    async fn test_gru_creation() {
        let architecture = create_test_architecture();
        let network = GRUNetwork::new(architecture).await;
        assert!(network.is_ok());
    }

    #[tokio::test]
    async fn test_gru_forward() {
        let architecture = create_test_architecture();
        let network = GRUNetwork::new(architecture).await.unwrap();

        // Input: [sequence_length=3, features=10]
        let input = Array2::zeros((3, 10));
        let output = network.forward(&input).await;

        assert!(output.is_ok());
        let output = output.unwrap();
        assert_eq!(output.shape(), &[3, 5]); // [seq_len, hidden_size]
    }
}
