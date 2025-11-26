//! Stored procedures module
//!
//! This module implements the execution engine for stored procedures and triggers.
//! It supports a subset of PL/pgSQL for defining procedural logic.

#![allow(missing_docs)]

pub mod ast;
pub mod catalog;
pub mod interpreter;

pub use ast::*;
pub use catalog::*;
pub use interpreter::*;
