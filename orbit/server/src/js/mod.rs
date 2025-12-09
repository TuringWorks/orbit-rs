//! JavaScript Engine Integration for Orbit-RS
//!
//! This module provides JavaScript execution capabilities for multiple protocols:
//!
//! - **PostgreSQL**: PL/JavaScript stored procedures via Boa (pure Rust, security-first)
//! - **MongoDB**: $where and $function operators via QuickJS (performance-critical)
//! - **Redis**: EVAL/EVALSHA commands via QuickJS (performance-critical)
//!
//! ## Engine Selection
//!
//! The multi-engine approach optimizes for different use cases:
//!
//! | Engine   | Use Case              | Priority    | Performance |
//! |----------|-----------------------|-------------|-------------|
//! | Boa      | PostgreSQL UDFs       | Security    | 2-5x slower than V8 |
//! | QuickJS  | MongoDB/Redis scripts | Performance | Near-V8 speed |
//!
//! ## Security Features
//!
//! - Execution timeouts (prevent infinite loops)
//! - Memory limits (prevent memory exhaustion)
//! - API restrictions (no dangerous built-ins)
//! - Context isolation (separate contexts per execution)
//!
//! ## Feature Flags
//!
//! - `js-boa`: Enable Boa engine for PostgreSQL
//! - `js-quickjs`: Enable QuickJS for MongoDB/Redis
//! - `js-postgres`: Full PostgreSQL PL/JavaScript support
//! - `js-mongodb`: MongoDB $where/$function support
//! - `js-redis`: Redis EVAL/EVALSHA support
//! - `javascript`: All JavaScript features

#[cfg(feature = "js-boa")]
pub mod boa_runtime;

#[cfg(feature = "js-quickjs")]
pub mod quickjs_runtime;

pub mod security;
pub mod types;

#[cfg(feature = "js-boa")]
pub use boa_runtime::BoaRuntime;

#[cfg(feature = "js-quickjs")]
pub use quickjs_runtime::QuickJsRuntime;

pub use security::{ExecutionLimits, SecurityConfig};
pub use types::{JsError, JsResult, JsValue};

/// Default execution timeout in milliseconds
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Default memory limit in bytes (16 MB)
pub const DEFAULT_MEMORY_LIMIT: usize = 16 * 1024 * 1024;

/// Maximum script size in bytes (1 MB)
pub const MAX_SCRIPT_SIZE: usize = 1024 * 1024;
