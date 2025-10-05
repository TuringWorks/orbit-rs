// GraphML (Graph Machine Learning) Tests
// These tests are disabled by default and enabled only when GRAPHML_FEATURES flag is set

#[cfg(feature = "graphml-features")]
mod node_embeddings_tests;

#[cfg(feature = "graphml-features")]
mod graph_neural_networks_tests;

#[cfg(feature = "graphml-features")]
mod link_prediction_tests;

#[cfg(feature = "graphml-features")]
mod node_classification_tests;

#[cfg(feature = "graphml-features")]
mod graph_algorithms_tests;

#[cfg(feature = "graphml-features")]
mod integration_tests;