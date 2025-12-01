//! Agriculture, Food & Environment industry models
//!
//! Comprehensive ML models for agriculture and environment including:
//! - Agriculture & Agritech
//! - Food & Beverage
//! - Forestry, Fisheries & Natural Resources
//! - Environmental Monitoring & Climate Risk

pub mod agritech;
pub mod marine_exploration;
pub mod food_beverage;
pub mod environmental_monitoring;

// Re-export commonly used types
pub use agritech::*;
pub use marine_exploration::*;
pub use food_beverage::*;
pub use environmental_monitoring::*;
