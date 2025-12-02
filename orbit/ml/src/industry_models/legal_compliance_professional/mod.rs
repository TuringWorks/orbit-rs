//! Legal, Compliance & Professional Services industry models
//!
//! Comprehensive ML models for legal and professional services including:
//! - Legal Services
//! - Consulting & Advisory
//! - Audit, Tax & Accounting

pub mod audit_tax_accounting;
pub mod legal_services;

// Re-export commonly used types
pub use audit_tax_accounting::*;
pub use legal_services::*;
