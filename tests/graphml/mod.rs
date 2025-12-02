// GraphML (Graph Machine Learning) Tests
// These tests are disabled by default and enabled only when GRAPHML_FEATURES flag is set
//
// Note: Only existing test modules are included. Additional modules can be added as they are implemented:
// - graph_neural_networks_tests.rs
// - link_prediction_tests.rs
// - node_classification_tests.rs
// - graph_algorithms_tests.rs
// - integration_tests.rs

#[cfg(feature = "graphml-features")]
mod node_embeddings_tests;