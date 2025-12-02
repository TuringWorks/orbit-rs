//! Healthcare, Pharma & Life Sciences industry models
//!
//! Comprehensive ML models for healthcare and life sciences including:
//! - Hospitals & Clinical Care
//! - Pharmaceuticals & Biotech
//! - Health Insurance & Payers
//! - Medical Devices / Digital Health
//! - Population Health

pub mod genomics;
pub mod health_insurance;
pub mod healthcare;
pub mod hospital_systems;
pub mod medical_devices;
pub mod pharmaceutical_research;
pub mod population_health;

// Re-export commonly used types
pub use genomics::*;
pub use health_insurance::*;
pub use healthcare::*;
pub use hospital_systems::*;
pub use medical_devices::*;
pub use pharmaceutical_research::*;
pub use population_health::*;
