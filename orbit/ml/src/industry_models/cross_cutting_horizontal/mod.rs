//! Cross-Cutting Horizontal Use Cases
//!
//! Foundational ML capabilities that cut across all industries:
//! - Forecasting: demand, risk, time series
//! - Recommendations & Personalization
//! - Anomaly / Fraud Detection
//! - Computer Vision: inspection, recognition, OCR
//! - NLP: search, chatbots, summarization
//! - Optimization & Control
//! - Generative AI
//! - Graph Neural Networks
//! - Survival Analysis
//! - Tree Ensembles (XGBoost, Random Forest)

pub mod anomaly_detection;
pub mod generative_models;
pub mod graph_neural_networks;
pub mod physics;
pub mod recommender_systems;
pub mod reinforcement_learning;
pub mod survival_analysis;
pub mod time_series_models;
pub mod tree_ensembles;

// Re-export commonly used types
pub use anomaly_detection::*;
pub use generative_models::*;
pub use graph_neural_networks::*;
pub use physics::*;
pub use recommender_systems::*;
pub use reinforcement_learning::*;
pub use survival_analysis::*;
pub use time_series_models::*;
pub use tree_ensembles::*;

// Reference to shared infrastructure:
// - ../data_loaders.rs - Image, time-series, graph data loaders
// - ../neural_networks/candle_layers.rs - GPU-accelerated layers (ResNet, ViT, GNN)
