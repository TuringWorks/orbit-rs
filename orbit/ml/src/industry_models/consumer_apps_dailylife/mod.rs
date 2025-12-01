//! Consumer Apps & Daily-Life Services industry models
//!
//! Comprehensive ML models for consumer applications including:
//! - Personal Finance & Budgeting Apps
//! - Health & Fitness Apps
//! - Home & IoT

pub mod repair_services;
pub mod personal_finance;
pub mod health_fitness;
pub mod smart_home;

// Re-export commonly used types
pub use repair_services::*;
pub use personal_finance::*;
pub use health_fitness::*;
pub use smart_home::*;
