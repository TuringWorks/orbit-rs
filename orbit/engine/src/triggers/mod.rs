//! Database Triggers module
//!
//! This module implements database triggers that execute stored procedures
//! in response to data modification events (INSERT, UPDATE, DELETE).

#![allow(missing_docs)]

pub mod executor;
pub mod manager;

pub use executor::*;
pub use manager::*;
