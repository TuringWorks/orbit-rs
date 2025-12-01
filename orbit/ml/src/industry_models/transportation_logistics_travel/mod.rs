//! Transportation, Logistics & Travel industry models
//!
//! Comprehensive ML models for transportation and logistics including:
//! - Logistics, Shipping, 3PL
//! - Airlines, Rail, Public Transit
//! - Ride-sharing, Delivery Platforms
//! - Travel & Hospitality
//! - Ticketing Systems

pub mod fleet_logistics;
pub mod rail_systems;
pub mod autonomous_fleet;
pub mod venue_management;
pub mod ticketing;

// Re-export commonly used types
pub use fleet_logistics::*;
pub use rail_systems::*;
pub use autonomous_fleet::*;
pub use venue_management::*;
pub use ticketing::*;
