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

pub mod physics;
pub mod graph_neural_networks;

// Re-export commonly used types
pub use physics::*;
pub use graph_neural_networks::*;

// Reference to shared infrastructure:
// - ../data_loaders.rs - Image, time-series, graph data loaders
// - ../neural_networks/candle_layers.rs - GPU-accelerated layers (ResNet, ViT, GNN)

