//! # Orbit ML - Machine Learning and Deep Learning Engine
//!
//! A comprehensive ML/DL framework integrated into the Orbit database system,
//! providing neural networks, transformers, graph neural networks, and multi-language support.
//!
//! ## Features
//!
//! - **Neural Networks**: Feedforward, CNN, RNN, LSTM, GRU architectures
//! - **Transformers**: BERT, GPT, Vision Transformers with attention mechanisms
//! - **Graph Neural Networks**: GCN, GraphSAGE, GAT for graph-based learning
//! - **Multi-Language Support**: Python, JavaScript, Lua integration
//! - **Industry Models**: *Experimental scaffolding, off by default.* The healthcare, fintech,
//!   defense, logistics, banking, and insurance subtrees are method signatures with unimplemented
//!   bodies. Build with `--features experimental-industry-models` to compile them.
//! - **SQL Integration**: Native SQL syntax for all ML operations
//! - **GPU Acceleration**: CUDA support for training and inference
//! - **Distributed Training**: Multi-node, multi-GPU capabilities
//!
//! ## Quick Start
//!
//! ```rust
//! use orbit_ml::{MLEngine, NeuralNetworkBuilder, TrainingConfig};
//! use orbit_ml::engine::MLEngineInterface;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create ML engine
//! let engine = MLEngine::new().await?;
//!
//! // Create and train a neural network
//! let _nn = NeuralNetworkBuilder::feedforward()
//!     .layers(&[64, 32, 16, 1])
//!     .activation("relu")
//!     .build().await?;
//!
//! let config = TrainingConfig::default()
//!     .epochs(100)
//!     .learning_rate(0.001);
//!
//! // Example training data (in practice, this would be your actual data)
//! let training_data = vec![0u8; 1024]; // Mock training data
//!
//! let _trained_job = engine.train_model(
//!     "my_model".to_string(),
//!     "neural_network".to_string(),
//!     training_data,
//!     config
//! ).await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![forbid(unsafe_code)]

// Core modules
pub mod config;
pub mod engine;
pub mod error;
/// Performance metrics and monitoring
pub mod metrics;

// ML Architecture modules
/// Graph Neural Network implementations and architectures
pub mod graph_neural_networks;
pub mod neural_networks;
pub mod transformers;

// Multi-language support
/// Multi-language runtime integration (Python, JavaScript, Lua)
pub mod multi_language;

// Industry-specific models
//
// EXPERIMENTAL AND OFF BY DEFAULT. This subtree is scaffolding: roughly 470 `// TODO: Implement`
// method bodies across seven verticals, with no training, no inference, and no tests behind them.
// Shipping it in a default-on crate advertises capability that does not exist, so it is gated until
// a vertical is real. Enable with `--features experimental-industry-models` if you are working on
// it. See `specifications/COMPETITIVE_ANALYSIS.md` §2.5 and `AI_LLM_ROADMAP.md` decision D7.
/// Pre-built models for various industry verticals (experimental scaffolding).
#[cfg(feature = "experimental-industry-models")]
pub mod industry_models;

// SQL extensions
/// SQL function extensions for ML operations
pub mod sql_extensions;

// Utilities and common functionality
/// Data processing and manipulation utilities
pub mod data;
/// Data loading infrastructure for training
pub mod data_loaders;
pub mod inference;
pub mod models;
/// Streaming ML inference for real-time data processing
pub mod streaming_inference;
pub mod training;
/// General utility functions and helpers
pub mod utils;

// Re-exports for convenience
pub use config::MLConfig;
pub use engine::{MLEngine, MLEngineBuilder};
pub use error::{MLError, Result};
pub use graph_neural_networks::{GNNBuilder, GraphNeuralNetwork};
pub use inference::{InferenceConfig, Predictor};
pub use models::{Model, ModelMetadata, ModelRegistry};
pub use neural_networks::{NeuralNetwork, NeuralNetworkBuilder};
pub use streaming_inference::{
    AggregatedInference, InferenceAnomaly, InferenceAnomalyDetector, InferenceEvent,
    InferenceOutput, StreamingInferenceConfig, StreamingInferencePipeline,
    StreamingInferencePipelineBuilder, StreamingInferenceStats, WindowAggregation,
    WindowedInferenceAggregator,
};
pub use training::{Trainer, TrainingConfig};
pub use transformers::{Transformer, TransformerBuilder};

// Feature-gated exports
#[cfg(feature = "python")]
pub use multi_language::python::PythonMLEngine;

#[cfg(feature = "javascript")]
pub use multi_language::javascript::JavaScriptMLEngine;

#[cfg(feature = "lua")]
pub use multi_language::lua::LuaMLEngine;

#[cfg(all(
    feature = "experimental-industry-models",
    feature = "industry-healthcare"
))]
pub use industry_models::healthcare_pharma_lifesciences as healthcare;

#[cfg(all(feature = "experimental-industry-models", feature = "industry-fintech"))]
pub use industry_models::finance_banking_insurance as fintech;

#[cfg(all(feature = "experimental-industry-models", feature = "industry-adtech"))]
pub use industry_models::arts_design_creative as adtech;

#[cfg(all(feature = "experimental-industry-models", feature = "industry-defense"))]
pub use industry_models::government_defense_publicsector as defense;

#[cfg(all(
    feature = "experimental-industry-models",
    feature = "industry-logistics"
))]
pub use industry_models::transportation_logistics_travel as logistics;

#[cfg(all(feature = "experimental-industry-models", feature = "industry-banking"))]
pub use industry_models::finance_banking_insurance as banking;

#[cfg(all(
    feature = "experimental-industry-models",
    feature = "industry-insurance"
))]
pub use industry_models::finance_banking_insurance as insurance;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library information
pub fn version() -> &'static str {
    VERSION
}

/// Check if a feature is enabled at compile time
#[allow(clippy::match_like_matches_macro)] // Each branch has different cfg! conditions
pub fn has_feature(feature: &str) -> bool {
    match feature {
        "neural-networks" => cfg!(feature = "neural-networks"),
        "transformers" => cfg!(feature = "transformers"),
        "graph-neural-networks" => cfg!(feature = "graph-neural-networks"),
        "python" => cfg!(feature = "python"),
        "javascript" => cfg!(feature = "javascript"),
        "lua" => cfg!(feature = "lua"),
        "gpu" => cfg!(feature = "gpu"),
        "distributed" => cfg!(feature = "distributed"),
        "experimental-industry-models" => cfg!(feature = "experimental-industry-models"),
        "industry-healthcare" => cfg!(feature = "industry-healthcare"),
        "industry-fintech" => cfg!(feature = "industry-fintech"),
        "industry-adtech" => cfg!(feature = "industry-adtech"),
        "industry-defense" => cfg!(feature = "industry-defense"),
        "industry-logistics" => cfg!(feature = "industry-logistics"),
        "industry-banking" => cfg!(feature = "industry-banking"),
        "industry-insurance" => cfg!(feature = "industry-insurance"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }

    #[test]
    fn test_feature_detection() {
        // These should always be true with default features
        assert!(has_feature("neural-networks"));
        assert!(has_feature("transformers"));
        assert!(has_feature("graph-neural-networks"));
    }

    #[test]
    fn industry_scaffolding_is_off_by_default() {
        // The `industry_models` subtree is unimplemented stubs; a default build must not advertise
        // it. See specifications/COMPETITIVE_ANALYSIS.md §2.5.
        #[cfg(not(feature = "experimental-industry-models"))]
        assert!(!has_feature("experimental-industry-models"));
        #[cfg(not(feature = "industry-healthcare"))]
        assert!(!has_feature("industry-healthcare"));
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let result = MLEngine::new().await;
        assert!(result.is_ok());
    }
}
