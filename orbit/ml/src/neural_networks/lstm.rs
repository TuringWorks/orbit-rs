//! LSTM Neural Network implementation.

use async_trait::async_trait;
use ndarray::Array2;

use crate::error::{MLError, Result};
use crate::neural_networks::{NetworkArchitecture, NeuralNetwork, Optimizer};

/// LSTM (Long Short-Term Memory) Neural Network implementation
///
/// A specialized recurrent neural network capable of learning long-term
/// dependencies through gating mechanisms (forget, input, output gates).
use ndarray::{s, Array1, Axis};
use rand::distributions::Uniform;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Cache for storing forward pass states (needed for BPTT)
#[derive(Debug, Clone, Default)]
struct LSTMCache {
    layers: Vec<Vec<LSTMTimeStep>>,
}

#[derive(Debug, Clone)]
struct LSTMTimeStep {
    f: Array1<f64>,
    i: Array1<f64>,
    c_tilde: Array1<f64>,
    c: Array1<f64>,
    o: Array1<f64>,
    h: Array1<f64>,
    x: Array1<f64>,
}

/// Gradients for an LSTM layer
#[derive(Debug, Clone, Default)]
struct LSTMLayerGradients {
    d_w_f: Array2<f64>,
    d_u_f: Array2<f64>,
    d_b_f: Array1<f64>,

    d_w_i: Array2<f64>,
    d_u_i: Array2<f64>,
    d_b_i: Array1<f64>,

    d_w_c: Array2<f64>,
    d_u_c: Array2<f64>,
    d_b_c: Array1<f64>,

    d_w_o: Array2<f64>,
    d_u_o: Array2<f64>,
    d_b_o: Array1<f64>,
}

/// LSTM (Long Short-Term Memory) Neural Network implementation
///
/// A specialized recurrent neural network capable of learning long-term
/// dependencies through gating mechanisms (forget, input, output gates).
#[derive(Debug, Clone)]
pub struct LSTMNetwork {
    architecture: NetworkArchitecture,
    layers: Vec<LSTMLayerWeights>,

    // Cache for BPTT
    cache: Arc<RwLock<LSTMCache>>,

    // Accumulated gradients
    gradients: Vec<LSTMLayerGradients>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LSTMLayerWeights {
    // Forget gate
    w_f: Array2<f64>,
    u_f: Array2<f64>,
    b_f: Array1<f64>,

    // Input gate
    w_i: Array2<f64>,
    u_i: Array2<f64>,
    b_i: Array1<f64>,

    // Cell candidate
    w_c: Array2<f64>,
    u_c: Array2<f64>,
    b_c: Array1<f64>,

    // Output gate
    w_o: Array2<f64>,
    u_o: Array2<f64>,
    b_o: Array1<f64>,
}

impl LSTMNetwork {
    /// Create a new LSTM network with given architecture
    ///
    /// # Arguments
    /// * `architecture` - Network architecture specification
    ///
    /// # Returns
    /// A new LSTM network instance
    pub async fn new(architecture: NetworkArchitecture) -> Result<Self> {
        let mut layers = Vec::new();
        let mut rng = rand::thread_rng();
        let range = Uniform::new(-0.1, 0.1);

        let mut input_size = architecture.input_shape.iter().product::<usize>();

        for layer_config in &architecture.layers {
            if let crate::neural_networks::LayerType::LSTM = layer_config.layer_type {
                let hidden_size = layer_config.units;

                // Create random matrices inline to avoid borrow issues
                let layer_weights = LSTMLayerWeights {
                    // Forget gate
                    w_f: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u_f: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b_f: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),

                    // Input gate
                    w_i: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u_i: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b_i: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),

                    // Cell candidate
                    w_c: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u_c: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b_c: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),

                    // Output gate
                    w_o: Array2::from_shape_fn((input_size, hidden_size), |_| rng.sample(range)),
                    u_o: Array2::from_shape_fn((hidden_size, hidden_size), |_| rng.sample(range)),
                    b_o: Array1::from_shape_fn(hidden_size, |_| rng.sample(range)),
                };

                layers.push(layer_weights);
                input_size = hidden_size;
            }
        }

        Ok(Self {
            architecture,
            layers,
            cache: Arc::new(RwLock::new(LSTMCache::default())),
            gradients: Vec::new(),
        })
    }
}

#[async_trait]
impl NeuralNetwork for LSTMNetwork {
    async fn forward(&self, input: &Array2<f64>) -> Result<Array2<f64>> {
        let seq_len = input.nrows();
        let mut current_input = input.clone();
        let mut layer_caches = Vec::new();

        for layer in &self.layers {
            let hidden_size = layer.w_f.ncols();
            let mut h_t = Array1::<f64>::zeros(hidden_size); // Hidden state
            let mut c_t = Array1::<f64>::zeros(hidden_size); // Cell state
            let mut layer_output = Array2::<f64>::zeros((seq_len, hidden_size));
            let mut layer_time_steps = Vec::with_capacity(seq_len);

            for t in 0..seq_len {
                let x_t = current_input.row(t).to_owned();

                let sigmoid = |x: f64| 1.0 / (1.0 + (-x).exp());
                let tanh = |x: f64| x.tanh();

                // Forget gate: f_t = sigmoid(W_f * x_t + U_f * h_{t-1} + b_f)
                let f_t = (&x_t.dot(&layer.w_f) + &h_t.dot(&layer.u_f) + &layer.b_f).mapv(sigmoid);

                // Input gate: i_t = sigmoid(W_i * x_t + U_i * h_{t-1} + b_i)
                let i_t = (&x_t.dot(&layer.w_i) + &h_t.dot(&layer.u_i) + &layer.b_i).mapv(sigmoid);

                // Cell candidate: c_tilde_t = tanh(W_c * x_t + U_c * h_{t-1} + b_c)
                let c_tilde_t =
                    (&x_t.dot(&layer.w_c) + &h_t.dot(&layer.u_c) + &layer.b_c).mapv(tanh);

                // Cell state: c_t = f_t * c_{t-1} + i_t * c_tilde_t
                c_t = (&f_t * &c_t) + (&i_t * &c_tilde_t);

                // Output gate: o_t = sigmoid(W_o * x_t + U_o * h_{t-1} + b_o)
                let o_t = (&x_t.dot(&layer.w_o) + &h_t.dot(&layer.u_o) + &layer.b_o).mapv(sigmoid);

                // Hidden state: h_t = o_t * tanh(c_t)
                h_t = &o_t * &c_t.mapv(tanh);

                layer_time_steps.push(LSTMTimeStep {
                    f: f_t.clone(),
                    i: i_t.clone(),
                    c_tilde: c_tilde_t.clone(),
                    c: c_t.clone(),
                    o: o_t.clone(),
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
            .map(|l| LSTMLayerGradients {
                d_w_f: Array2::zeros(l.w_f.raw_dim()),
                d_u_f: Array2::zeros(l.u_f.raw_dim()),
                d_b_f: Array1::zeros(l.b_f.raw_dim()),
                d_w_i: Array2::zeros(l.w_i.raw_dim()),
                d_u_i: Array2::zeros(l.u_i.raw_dim()),
                d_b_i: Array1::zeros(l.b_i.raw_dim()),
                d_w_c: Array2::zeros(l.w_c.raw_dim()),
                d_u_c: Array2::zeros(l.u_c.raw_dim()),
                d_b_c: Array1::zeros(l.b_c.raw_dim()),
                d_w_o: Array2::zeros(l.w_o.raw_dim()),
                d_u_o: Array2::zeros(l.u_o.raw_dim()),
                d_b_o: Array1::zeros(l.b_o.raw_dim()),
            })
            .collect();

        let mut next_layer_grad = loss_gradient.clone();

        for (layer_idx, layer) in self.layers.iter().enumerate().rev() {
            let layer_cache = &cache.layers[layer_idx];
            let seq_len = layer_cache.len();
            let hidden_size = layer.w_f.ncols();
            let input_size = layer.w_f.nrows();

            let mut d_h_next = Array1::<f64>::zeros(hidden_size);
            let mut d_c_next = Array1::<f64>::zeros(hidden_size);
            let mut d_prev_layer = Array2::<f64>::zeros((seq_len, input_size));

            let gradients = &mut self.gradients[layer_idx];

            for t in (0..seq_len).rev() {
                let step = &layer_cache[t];
                let h_prev = if t > 0 {
                    &layer_cache[t - 1].h
                } else {
                    &Array1::zeros(hidden_size)
                };
                let c_prev = if t > 0 {
                    &layer_cache[t - 1].c
                } else {
                    &Array1::zeros(hidden_size)
                };

                let d_h = &next_layer_grad.row(t) + &d_h_next;

                // h_t = o_t * tanh(c_t)
                // d_o = d_h * tanh(c_t) * o * (1 - o)
                let tanh_c = step.c.mapv(|x| x.tanh());
                let d_o = &d_h * &tanh_c * &step.o * (1.0 - &step.o);

                // d_c = d_h * o_t * (1 - tanh^2(c_t)) + d_c_next
                let d_c = &d_h * &step.o * (1.0 - tanh_c.mapv(|x| x * x)) + &d_c_next;

                // c_t = f_t * c_{t-1} + i_t * c_tilde_t

                // d_c_tilde = d_c * i_t * (1 - c_tilde^2)
                let d_c_tilde = &d_c * &step.i * (1.0 - step.c_tilde.mapv(|x| x * x));

                // d_i = d_c * c_tilde * i * (1 - i)
                let d_i = &d_c * &step.c_tilde * &step.i * (1.0 - &step.i);

                // d_f = d_c * c_{t-1} * f * (1 - f)
                let d_f = &d_c * c_prev * &step.f * (1.0 - &step.f);

                // Accumulate gradients
                // For each gate G (f, i, c, o):
                // dW_G += x^T * d_G
                // dU_G += h_{t-1}^T * d_G
                // db_G += d_G

                let x_t_t = step.x.clone().insert_axis(Axis(1));
                let h_prev_t = h_prev.clone().insert_axis(Axis(1));

                let d_f_row = d_f.clone().insert_axis(Axis(0));
                gradients.d_w_f = &gradients.d_w_f + &x_t_t.dot(&d_f_row);
                gradients.d_u_f = &gradients.d_u_f + &h_prev_t.dot(&d_f_row);
                gradients.d_b_f = &gradients.d_b_f + &d_f;

                let d_i_row = d_i.clone().insert_axis(Axis(0));
                gradients.d_w_i = &gradients.d_w_i + &x_t_t.dot(&d_i_row);
                gradients.d_u_i = &gradients.d_u_i + &h_prev_t.dot(&d_i_row);
                gradients.d_b_i = &gradients.d_b_i + &d_i;

                let d_c_tilde_row = d_c_tilde.clone().insert_axis(Axis(0));
                gradients.d_w_c = &gradients.d_w_c + &x_t_t.dot(&d_c_tilde_row);
                gradients.d_u_c = &gradients.d_u_c + &h_prev_t.dot(&d_c_tilde_row);
                gradients.d_b_c = &gradients.d_b_c + &d_c_tilde;

                let d_o_row = d_o.clone().insert_axis(Axis(0));
                gradients.d_w_o = &gradients.d_w_o + &x_t_t.dot(&d_o_row);
                gradients.d_u_o = &gradients.d_u_o + &h_prev_t.dot(&d_o_row);
                gradients.d_b_o = &gradients.d_b_o + &d_o;

                // Compute d_h_prev and d_c_prev
                d_c_next = &d_c * &step.f;

                let d_gates = d_f.dot(&layer.u_f.t())
                    + d_i.dot(&layer.u_i.t())
                    + d_c_tilde.dot(&layer.u_c.t())
                    + d_o.dot(&layer.u_o.t());
                d_h_next = d_gates;

                let d_x = d_f.dot(&layer.w_f.t())
                    + d_i.dot(&layer.w_i.t())
                    + d_c_tilde.dot(&layer.w_c.t())
                    + d_o.dot(&layer.w_o.t());
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
            layer.w_f = &layer.w_f - &(g.d_w_f.mapv(|x| x * lr));
            layer.u_f = &layer.u_f - &(g.d_u_f.mapv(|x| x * lr));
            layer.b_f = &layer.b_f - &(g.d_b_f.mapv(|x| x * lr));
            layer.w_i = &layer.w_i - &(g.d_w_i.mapv(|x| x * lr));
            layer.u_i = &layer.u_i - &(g.d_u_i.mapv(|x| x * lr));
            layer.b_i = &layer.b_i - &(g.d_b_i.mapv(|x| x * lr));
            layer.w_c = &layer.w_c - &(g.d_w_c.mapv(|x| x * lr));
            layer.u_c = &layer.u_c - &(g.d_u_c.mapv(|x| x * lr));
            layer.b_c = &layer.b_c - &(g.d_b_c.mapv(|x| x * lr));
            layer.w_o = &layer.w_o - &(g.d_w_o.mapv(|x| x * lr));
            layer.u_o = &layer.u_o - &(g.d_u_o.mapv(|x| x * lr));
            layer.b_o = &layer.b_o - &(g.d_b_o.mapv(|x| x * lr));
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
            count += layer.w_f.len();
            count += layer.u_f.len();
            count += layer.b_f.len();

            count += layer.w_i.len();
            count += layer.u_i.len();
            count += layer.b_i.len();

            count += layer.w_c.len();
            count += layer.u_c.len();
            count += layer.b_c.len();

            count += layer.w_o.len();
            count += layer.u_o.len();
            count += layer.b_o.len();
        }
        count
    }

    async fn save_weights(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self.layers)
            .map_err(|e| MLError::neural_network(format!("Failed to serialize weights: {}", e)))
    }

    async fn load_weights(&mut self, weights: &[u8]) -> Result<()> {
        let layers: Vec<LSTMLayerWeights> = bincode::deserialize(weights).map_err(|e| {
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
            network_type: crate::neural_networks::NetworkType::LSTM,
            input_shape: vec![10],
            layers: vec![LayerConfig {
                layer_type: LayerType::LSTM,
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
    async fn test_lstm_creation() {
        let architecture = create_test_architecture();
        let network = LSTMNetwork::new(architecture).await;
        assert!(network.is_ok());
    }

    #[tokio::test]
    async fn test_lstm_forward() {
        let architecture = create_test_architecture();
        let network = LSTMNetwork::new(architecture).await.unwrap();

        // Input: [sequence_length=3, features=10]
        let input = Array2::zeros((3, 10));
        let output = network.forward(&input).await;

        assert!(output.is_ok());
        let output = output.unwrap();
        assert_eq!(output.shape(), &[3, 5]); // [seq_len, hidden_size]
    }
}
