pub struct NeuralNetwork;
pub struct NeuralNetworkBuilder;

impl NeuralNetwork {
    pub fn feedforward() -> NeuralNetworkBuilder {
        NeuralNetworkBuilder
    }
}

impl NeuralNetworkBuilder {
    pub fn layers(self, _layers: &[usize]) -> Self {
        self
    }

    pub fn activation(self, _activation: &str) -> Self {
        self
    }

    pub fn build(self) -> crate::error::Result<NeuralNetwork> {
        Ok(NeuralNetwork)
    }
}