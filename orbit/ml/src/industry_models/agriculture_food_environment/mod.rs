//! Agriculture, Food & Environment industry models
//!
//! Comprehensive ML models for agriculture and environment including:
//! - Agriculture & Agritech
//! - Food & Beverage
//! - Forestry, Fisheries & Natural Resources
//! - Environmental Monitoring & Climate Risk

pub mod agritech;
pub mod environmental_monitoring;
pub mod food_beverage;
pub mod marine_exploration;

// Re-export commonly used types
pub use agritech::*;
pub use environmental_monitoring::*;
pub use food_beverage::*;
pub use marine_exploration::*;
