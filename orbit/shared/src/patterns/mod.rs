//! Advanced Rust design patterns and idioms
//!
//! This module demonstrates advanced Rust patterns for building robust,
//! type-safe, and efficient systems.

pub mod conversions;
pub mod interior_mutability;
pub mod iterators;
pub mod phantom_types;
pub mod raii_guards;
pub mod sealed_traits;
pub mod strategy;
pub mod typestate;
pub mod visitors;

pub use conversions::*;
pub use interior_mutability::*;
pub use iterators::*;
pub use phantom_types::*;
pub use raii_guards::*;
pub use sealed_traits::*;
pub use strategy::*;
pub use typestate::*;
pub use visitors::*;
