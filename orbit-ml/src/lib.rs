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
//! - **Industry Models**: Specialized models for healthcare, fintech, defense, etc.
//! - **SQL Integration**: Native SQL syntax for all ML operations
//! - **GPU Acceleration**: CUDA support for training and inference
//! - **Distributed Training**: Multi-node, multi-GPU capabilities
//!
//! ## Quick Start
//!
//! ```rust
//! use orbit_ml::{MLEngine, NeuralNetwork, TrainingConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create ML engine
//! let engine = MLEngine::new().await?;
//!
//! // Create and train a neural network
//! let nn = NeuralNetwork::feedforward()
//!     .layers(&[64, 32, 16, 1])
//!     .activation("relu")
//!     .build()?;
//!
//! let config = TrainingConfig::default()
//!     .epochs(100)
//!     .learning_rate(0.001);
//!
//! let trained_model = engine.train(nn, &training_data, config).await?;
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
/// Pre-built models for various industry verticals
pub mod industry_models;

// SQL extensions
/// SQL function extensions for ML operations
pub mod sql_extensions;

// Utilities and common functionality
/// Data processing and manipulation utilities
pub mod data;
pub mod inference;
pub mod models;
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
pub use training::{Trainer, TrainingConfig};
pub use transformers::{Transformer, TransformerBuilder};

// Feature-gated exports
#[cfg(feature = "python")]
pub use multi_language::python::PythonMLEngine;

#[cfg(feature = "javascript")]
pub use multi_language::javascript::JavaScriptMLEngine;

#[cfg(feature = "lua")]
pub use multi_language::lua::LuaMLEngine;

#[cfg(feature = "industry-healthcare")]
pub use industry_models::healthcare::HealthcareModels;

#[cfg(feature = "industry-fintech")]
pub use industry_models::fintech::FintechModels;

#[cfg(feature = "industry-adtech")]
pub use industry_models::adtech::AdtechModels;

#[cfg(feature = "industry-defense")]
pub use industry_models::defense::DefenseModels;

#[cfg(feature = "industry-logistics")]
pub use industry_models::logistics::LogisticsModels;

#[cfg(feature = "industry-banking")]
pub use industry_models::banking::BankingModels;

#[cfg(feature = "industry-insurance")]
pub use industry_models::insurance::InsuranceModels;

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

    #[tokio::test]
    async fn test_engine_creation() {
        let result = MLEngine::new().await;
        assert!(result.is_ok());
    }
}
