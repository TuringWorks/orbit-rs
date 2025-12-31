//! WebAssembly (WASM) User-Defined Functions
//!
//! Note on tests:
//! WASM SQL e2e tests live in `sql_syntax_e2e_test.rs` and are compiled only when
//! running tests with the `wasm-udf` feature enabled. We explicitly declare the
//! module here so `cargo test -p orbit-server --features wasm-udf` reliably
//! discovers and compiles the tests.
//!
//! This module provides support for executing WebAssembly modules as user-defined functions (UDFs).
//! WASM UDFs offer several advantages:
//!
//! ## Key Features
//!
//! - **Language Agnostic**: Write UDFs in any language that compiles to WASM (Rust, C, C++, Go, AssemblyScript, etc.)
//! - **Near-Native Performance**: WASM executes at near-native speed with JIT compilation
//! - **Sandboxed Execution**: WASM provides strong isolation and security guarantees
//! - **No External Dependencies**: Unlike Python/Lua, no external runtime required
//! - **Portable**: WASM modules are portable across platforms and architectures
//! - **Resource Limits**: Built-in support for memory and CPU limits
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                WASM UDF Architecture                    │
//! ├─────────────────────────────────────────────────────────┤
//! │                                                         │
//! │  ┌──────────────────────────────────────────────────┐   │
//! │  │          SQL CREATE FUNCTION Statement            │   │
//! │  │  CREATE FUNCTION add(a INT, b INT) RETURNS INT   │   │
//! │  │  LANGUAGE WASM AS '0x00616...' -- hex binary     │   │
//! │  └────────────────────┬─────────────────────────────┘   │
//! │                       │                                 │
//! │                       ▼                                 │
//! │  ┌────────────────────────────────────────────────┐     │
//! │  │           WasmUdfHandler                       │     │
//! │  │  - Parses CREATE/DROP FUNCTION                 │     │
//! │  │  - Decodes hex-encoded WASM binary             │     │
//! │  │  - Validates WASM module                       │     │
//! │  └────────────────────┬───────────────────────────┘     │
//! │                       │                                 │
//! │                       ▼                                 │
//! │  ┌────────────────────────────────────────────────┐     │
//! │  │           WasmUdfRegistry                      │     │
//! │  │  - Function metadata storage                   │     │
//! │  │  - Argument validation                         │     │
//! │  │  - Execution routing                           │     │
//! │  └────────────────────┬───────────────────────────┘     │
//! │                       │                                 │
//! │                       ▼                                 │
//! │  ┌────────────────────────────────────────────────┐     │
//! │  │             WasmRuntime                        │     │
//! │  │  ┌──────────────────────────────────────────┐  │     │
//! │  │  │  wasmtime Engine (JIT Compiler)          │  │     │
//! │  │  └──────────────────────────────────────────┘  │     │
//! │  │  ┌──────────────────────────────────────────┐  │     │
//! │  │  │  Module Cache (LRU)                      │  │     │
//! │  │  │  - Compiled modules                      │  │     │
//! │  │  │  - Fast repeated execution               │  │     │
//! │  │  └──────────────────────────────────────────┘  │     │
//! │  │  ┌──────────────────────────────────────────┐  │     │
//! │  │  │  Resource Limits                         │  │     │
//! │  │  │  - Memory: 64MB default                  │  │     │
//! │  │  │  - CPU: Fuel-based limiting              │  │     │
//! │  │  │  - Timeout: 30s default                  │  │     │
//! │  │  └──────────────────────────────────────────┘  │     │
//! │  └────────────────────────────────────────────────┘     │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage Example
//!
//! ### 1. Compile WASM Module
//!
//! Rust example:
//! ```rust
//! #[no_mangle]
//! pub extern "C" fn add(a: i32, b: i32) -> i32 {
//!     a + b
//! }
//! ```
//!
//! Compile: `rustc --target wasm32-unknown-unknown -O --crate-type=cdylib add.rs`
//!
//! ### 2. Register Function via SQL
//!
//! ```sql
//! CREATE FUNCTION add(a INTEGER, b INTEGER)
//! RETURNS INTEGER
//! LANGUAGE WASM
//! AS '0061736d0100000001070160027f7f017f...';  -- hex-encoded WASM
//! ```
//!
//! ### 3. Use Function in Queries
//!
//! ```sql
//! SELECT add(5, 3);  -- Returns: 8
//! SELECT id, add(score1, score2) as total_score FROM students;
//! ```
//!
//! ## Security
//!
//! WASM UDFs run in a sandboxed environment with:
//! - **Memory isolation**: Cannot access server memory
//! - **CPU limits**: Fuel-based instruction counting
//! - **Time limits**: Execution timeouts
//! - **No system access**: No file I/O, network, or system calls (unless WASI enabled)
//! - **Module validation**: Binary format verification
//!
//! ## Performance
//!
//! - **JIT compilation**: wasmtime compiles WASM to native code
//! - **Module caching**: Compiled modules cached for repeated use
//! - **Near-native speed**: 10-20% overhead vs native code
//! - **No IPC overhead**: Unlike Python subprocess approach
//!
//! ## Implementation Details
//!
//! | Component | Lines | Description |
//! |-----------|-------|-------------|
//! | `types.rs` | ~300 | Type conversions (SQL ↔ WASM) |
//! | `config.rs` | ~180 | Configuration and limits |
//! | `runtime.rs` | ~400 | wasmtime integration |
//! | `udf_registry.rs` | ~280 | Function management |
//! | `udf_handler.rs` | ~250 | SQL integration |
//! | **Total** | ~1,410 | Complete implementation |
//!
//! ## Comparison with Other UDF Systems
//!
//! | Feature | WASM | Python | Lua |
//! |---------|------|--------|-----|
//! | **Performance** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ |
//! | **Language Support** | Multiple | Python only | Lua only |
//! | **Sandbox Security** | Native | Process isolation | Limited |
//! | **Startup Overhead** | Low | High | Low |
//! | **Memory Safety** | Guaranteed | Runtime errors | Runtime errors |
//! | **External Libraries** | Limited | Full (numpy, etc.) | Limited |
//!
//! ## Future Enhancements
//!
//! - WASI support for file/network I/O (opt-in)
//! - SIMD operations for vectorized computation
//! - Streaming I/O for large datasets
//! - Async WASM functions
//! - Component Model support

pub mod config;
pub mod runtime;
pub mod types;
pub mod udf_handler;
pub mod udf_registry;

#[cfg(all(test, feature = "wasm-udf"))]
mod sql_syntax_e2e_test;

pub use config::WasmConfig;
pub use runtime::{WasmError, WasmRuntime};
pub use types::{WasmFunctionMetadata, WasmParameter, WasmValue};
pub use udf_handler::WasmUdfHandler;
pub use udf_registry::WasmUdfRegistry;
