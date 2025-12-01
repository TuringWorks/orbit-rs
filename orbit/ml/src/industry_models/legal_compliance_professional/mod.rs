//! Legal, Compliance & Professional Services industry models
//!
//! Comprehensive ML models for legal and professional services including:
//! - Legal Services
//! - Consulting & Advisory
//! - Audit, Tax & Accounting

pub mod legal_services;
pub mod audit_tax_accounting;

// Re-export commonly used types
pub use legal_services::*;
pub use audit_tax_accounting::*;
