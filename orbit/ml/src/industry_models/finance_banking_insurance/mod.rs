//! Finance, Banking & Insurance industry models
//!
//! Comprehensive ML models for financial services including:
//! - Retail & Corporate Banking
//! - Capital Markets & Trading
//! - Insurance (P&C, Life, Health, Reinsurance)
//! - Fintech & Payments

pub mod fintech;
pub mod hedge_fund;
pub mod insurance;
pub mod retail_banking;
pub mod trading;

// Re-export commonly used types
pub use fintech::*;
pub use hedge_fund::*;
pub use insurance::*;
pub use retail_banking::*;
pub use trading::*;
