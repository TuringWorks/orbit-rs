//! Industry-specific machine learning models
//!
//! This module provides pre-built ML models for various industry verticals including:
//! - Business: Healthcare, Fintech, Banking, Insurance, Adtech, Defense, Logistics
//! - Advanced AI: Physical AI, Drug Discovery, Genomics, Physics, Industrial AI, IoT
//! - Critical Industry: Retail, Fashion, FMCG/CPG, Supply Chain, Critical Equipment
//! - Heavy Industry: Aerospace, Petroleum, Robotics, Energy, Manufacturing
//! - Food Service: Fast Food, Restaurants, Industrial Supplies
//! - Specialized Manufacturing: Automotive, Consumer Electronics

// Common infrastructure
pub mod common;

// Re-export common types
pub use common::{
    IndustryModel, IndustryModelError, ModelConfig, ModelMetrics, ModelRegistry, Result,
    TrainingConfig,
};

#[cfg(feature = "industry-healthcare")]
/// Healthcare-specific machine learning models and utilities
pub mod healthcare;

#[cfg(feature = "industry-fintech")]
/// Financial technology machine learning models and utilities
pub mod fintech;

#[cfg(feature = "industry-adtech")]
/// Advertising technology machine learning models and utilities
pub mod adtech;

#[cfg(feature = "industry-defense")]
/// Defense and security machine learning models and utilities
pub mod defense;

#[cfg(feature = "industry-logistics")]
/// Logistics and supply chain machine learning models and utilities
pub mod logistics;

#[cfg(feature = "industry-banking")]
/// Banking and financial services machine learning models and utilities
pub mod banking;

#[cfg(feature = "industry-insurance")]
/// Insurance industry machine learning models and utilities
pub mod insurance;
