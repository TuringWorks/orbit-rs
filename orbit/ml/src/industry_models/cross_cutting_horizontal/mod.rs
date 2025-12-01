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

pub mod physics;

// Re-export commonly used types
pub use physics::*;

// Reference to shared infrastructure:
// - ../data_loaders.rs - Image, time-series, graph data loaders
// - ../neural_networks/candle_layers.rs - GPU-accelerated layers (ResNet, ViT, GNN)
