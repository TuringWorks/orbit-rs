pub struct GraphNeuralNetwork;
pub struct GNNBuilder;

impl GNNBuilder {
    pub fn build(self) -> crate::error::Result<GraphNeuralNetwork> {
        Ok(GraphNeuralNetwork)
    }
}
