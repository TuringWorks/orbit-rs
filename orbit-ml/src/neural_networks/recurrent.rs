//! Recurrent Neural Network implementation.

use async_trait::async_trait;
use ndarray::Array2;

use crate::error::{MLError, Result};
use crate::neural_networks::{NetworkArchitecture, NeuralNetwork, Optimizer};

#[derive(Debug, Clone)]
pub struct RecurrentNetwork {
    architecture: NetworkArchitecture,
}

impl RecurrentNetwork {
    pub async fn new(architecture: NetworkArchitecture) -> Result<Self> {
        Ok(Self { architecture })
    }
}

#[async_trait]
impl NeuralNetwork for RecurrentNetwork {
    async fn forward(&self, _input: &Array2<f64>) -> Result<Array2<f64>> {
        Err(MLError::neural_network(
            "RNN forward pass not implemented yet",
        ))
    }

    async fn backward(&mut self, _loss_gradient: &Array2<f64>) -> Result<()> {
        Ok(())
    }

    async fn update_weights(&mut self, _optimizer: &dyn Optimizer) -> Result<()> {
        Ok(())
    }

    fn architecture(&self) -> &NetworkArchitecture {
        &self.architecture
    }

    fn parameter_count(&self) -> usize {
        0
    }

    async fn save_weights(&self) -> Result<Vec<u8>> {
        Ok(vec![])
    }

    async fn load_weights(&mut self, _weights: &[u8]) -> Result<()> {
        Ok(())
    }
}
