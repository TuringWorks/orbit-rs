//! Finance, Banking & Insurance industry models
//!
//! Comprehensive ML models for financial services including:
//! - Retail & Corporate Banking
//! - Capital Markets & Trading
//! - Insurance (P&C, Life, Health, Reinsurance)
//! - Fintech & Payments

pub mod retail_banking;
pub mod fintech;
pub mod insurance;
pub mod trading;
pub mod hedge_fund;

// Re-export commonly used types
pub use retail_banking::*;
pub use fintech::*;
pub use insurance::*;
pub use trading::*;
pub use hedge_fund::*;
