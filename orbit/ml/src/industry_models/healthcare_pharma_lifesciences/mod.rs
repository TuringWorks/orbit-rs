//! Healthcare, Pharma & Life Sciences industry models
//!
//! Comprehensive ML models for healthcare and life sciences including:
//! - Hospitals & Clinical Care
//! - Pharmaceuticals & Biotech
//! - Health Insurance & Payers
//! - Medical Devices / Digital Health
//! - Population Health

pub mod healthcare;
pub mod hospital_systems;
pub mod pharmaceutical_research;
pub mod genomics;
pub mod population_health;
pub mod health_insurance;
pub mod medical_devices;

// Re-export commonly used types
pub use healthcare::*;
pub use hospital_systems::*;
pub use pharmaceutical_research::*;
pub use genomics::*;
pub use population_health::*;
pub use health_insurance::*;
pub use medical_devices::*;
