//! Retail, E-Commerce & Consumer Goods industry models
//!
//! Comprehensive ML models for retail and consumer goods including:
//! - Retail & E-Commerce
//! - Consumer Packaged Goods (CPG)
//! Retail, E-Commerce & Consumer Goods industry models

pub mod retail;
pub mod fashion;
pub mod cpg;
pub mod ecommerce_advanced;

// Re-export commonly used types
pub use retail::*;
pub use fashion::*;
pub use cpg::*;
pub use ecommerce_advanced::*;
