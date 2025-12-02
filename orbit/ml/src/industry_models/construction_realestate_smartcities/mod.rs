//! Construction, Real Estate & Smart Cities industry models
//!
//! Comprehensive ML models for construction and smart cities including:
//! - Construction & Engineering
//! - Real Estate & PropTech
//! - Smart Cities & Urban Planning

pub mod building_management;
pub mod construction;
pub mod infrastructure_management;
pub mod real_estate;
pub mod smart_city;

// Re-export commonly used types
pub use building_management::*;
pub use construction::*;
pub use infrastructure_management::*;
pub use real_estate::*;
pub use smart_city::*;
