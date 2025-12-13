//! Python UDF Support for Orbit-RS
//!
//! This module provides Python User-Defined Function (UDF) support via subprocess execution.
//! Unlike PyO3 which has cross-platform and version compatibility issues, this implementation:
//!
//! - Uses long-running Python subprocesses
//! - Communicates via MessagePack-RPC (or JSON fallback)
//! - Supports any Python version (3.8+)
//! - Works identically across Windows, macOS, Linux
//! - Provides connection pooling for performance
//! - Supports batch execution
//! - Pre-loads common libraries (numpy, pandas, etc.)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │  SQL: CREATE FUNCTION ... PYTHON   │
//! └──────────────┬──────────────────────┘
//!                │
//!                ▼
//! ┌─────────────────────────────────────┐
//! │     Python UDF Registry (Rust)     │
//! │  • Manages function metadata       │
//! │  • Routes to runtime pool          │
//! └──────────────┬──────────────────────┘
//!                │
//!                ▼
//! ┌─────────────────────────────────────┐
//! │   Python Runtime Pool (Rust)       │
//! │  • N worker processes              │
//! │  • Load balancing                  │
//! │  • Health checking                 │
//! │  • Auto-restart                    │
//! └──────────────┬──────────────────────┘
//!                │
//!                ▼
//! ┌─────────────────────────────────────┐
//! │    Python Worker (Python)          │
//! │  • Executes UDFs                   │
//! │  • MessagePack communication       │
//! │  • Security restrictions           │
//! └─────────────────────────────────────┘
//! ```

pub mod config;
pub mod runtime;
pub mod types;
pub mod udf_registry;

#[cfg(feature = "protocol-postgres")]
pub mod udf_handler;

pub use config::{PythonConfig, PythonWorkerConfig};
pub use runtime::{PythonRuntime, PythonRuntimePool};
pub use types::{PythonError, PythonResult, PythonValue};
pub use udf_registry::{PythonUdfMetadata, PythonUdfRegistry};

#[cfg(test)]
mod tests;
