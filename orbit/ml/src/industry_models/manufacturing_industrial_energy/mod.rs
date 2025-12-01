//! Manufacturing, Industrial & Energy industry models
//!
//! Comprehensive ML models for manufacturing and energy including:
//! - Discrete & Process Manufacturing
//! - Industrial Automation & Robotics
//! - Oil & Gas, Mining
//! - Utilities & Power Grids
//! - Clean Energy & Climate Tech

pub mod manufacturing;
pub mod automotive;
pub mod aerospace;
pub mod robotics;
pub mod physical_ai;
pub mod oil_gas_exploration;
pub mod offshore_drilling;
pub mod mining;
pub mod energy;
pub mod solar_installations;

// Re-export commonly used types
pub use manufacturing::*;
pub use automotive::*;
pub use aerospace::*;
pub use robotics::*;
pub use physical_ai::*;
pub use oil_gas_exploration::*;
pub use offshore_drilling::*;
pub use mining::*;
pub use energy::*;
pub use solar_installations::*;
