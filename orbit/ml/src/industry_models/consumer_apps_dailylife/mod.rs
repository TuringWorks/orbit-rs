//! Consumer Apps & Daily-Life Services industry models
//!
//! Comprehensive ML models for consumer applications including:
//! - Personal Finance & Budgeting Apps
//! - Health & Fitness Apps
//! - Home & IoT

pub mod health_fitness;
pub mod personal_finance;
pub mod repair_services;
pub mod smart_home;

// Re-export commonly used types
pub use health_fitness::*;
pub use personal_finance::*;
pub use repair_services::*;
pub use smart_home::*;
