//! Retail, E-Commerce & Consumer Goods industry models
//!
//! Comprehensive ML models for retail and consumer goods including:
//! - Retail & E-Commerce
//! - Consumer Packaged Goods (CPG)
//! Retail, E-Commerce & Consumer Goods industry models

pub mod cpg;
pub mod ecommerce_advanced;
pub mod fashion;
pub mod retail;

// Re-export commonly used types
pub use cpg::*;
pub use ecommerce_advanced::*;
pub use fashion::*;
pub use retail::*;
