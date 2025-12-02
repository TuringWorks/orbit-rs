//! Manufacturing, Industrial & Energy industry models
//!
//! Comprehensive ML models for manufacturing and energy including:
//! - Discrete & Process Manufacturing
//! - Industrial Automation & Robotics
//! - Oil & Gas, Mining
//! - Utilities & Power Grids
//! - Clean Energy & Climate Tech

pub mod aerospace;
pub mod automotive;
pub mod energy;
pub mod manufacturing;
pub mod mining;
pub mod offshore_drilling;
pub mod oil_gas_exploration;
pub mod physical_ai;
pub mod robotics;
pub mod solar_installations;

// Re-export commonly used types
pub use aerospace::*;
pub use automotive::*;
pub use energy::*;
pub use manufacturing::*;
pub use mining::*;
pub use offshore_drilling::*;
pub use oil_gas_exploration::*;
pub use physical_ai::*;
pub use robotics::*;
pub use solar_installations::*;
