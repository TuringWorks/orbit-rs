//! Lua Engine Integration for Orbit-RS
//!
//! This module provides comprehensive Lua execution capabilities for multiple protocols:
//! - **Redis**: FUNCTION, EVAL, EVALSHA, SCRIPT * commands
//! - **PostgreSQL**: PL/Lua stored procedures
//! - **MySQL**: Lua-based stored procedures
//! - **Custom**: UDFs, triggers, ETL pipelines
//!
//! ## Engine: mlua (LuaJIT/Lua 5.4)
//!
//! ## Features
//! - **Multi-layer security**: Pre-validation, sandboxing, runtime monitoring, API restrictions
//! - **Resource limits**: Timeouts, memory limits, operation counting
//! - **Script caching**: SHA1-based caching for EVALSHA support
//! - **Function registry**: Register and manage Lua functions
//! - **Redis API**: Full redis.call(), redis.pcall(), redis.register_function() support
//! - **Hybrid state**: Ephemeral by default, opt-in persistent actors
//!
//! ## Usage
//!
//! ```rust,no_run
//! use orbit_server::lua::{MluaRuntime, SecurityConfig, LuaValue};
//!
//! #[tokio::main]
//! async fn main() {
//!     let runtime = MluaRuntime::new();
//!
//!     // Simple evaluation
//!     let result = runtime.eval("return 1 + 2").await.unwrap();
//!     assert_eq!(result.as_i64(), Some(3));
//!
//!     // Redis-style evaluation with KEYS and ARGV
//!     let result = runtime.eval_with_keys_args(
//!         "return KEYS[1] .. ARGV[1]",
//!         &["key1".to_string()],
//!         &[LuaValue::String("value1".to_string())]
//!     ).await.unwrap();
//! }
//! ```

// Core modules
pub mod security;
pub mod types;

// Runtime
#[cfg(feature = "lua-mlua")]
pub mod mlua_runtime;

// API modules
#[cfg(feature = "lua-mlua")]
pub mod redis_api;

#[cfg(feature = "lua-mlua")]
pub mod database_api;

#[cfg(feature = "lua-mlua")]
pub mod udf_registry;

// Re-exports for convenience
pub use security::{
    ExecutionGuard, ExecutionLimits, ScriptValidator, SecurityConfig, DEFAULT_MEMORY_LIMIT,
    DEFAULT_TIMEOUT_MS, MAX_SCRIPT_SIZE,
};
pub use types::{LuaError, LuaFunction, LuaParameter, LuaResult, LuaValue};

#[cfg(feature = "lua-mlua")]
pub use mlua_runtime::MluaRuntime;

#[cfg(feature = "lua-mlua")]
pub use redis_api::{setup_redis_api, RedisApi};

#[cfg(feature = "lua-mlua")]
pub use database_api::{setup_database_api, DatabaseApi};

#[cfg(feature = "lua-mlua")]
pub use udf_registry::{
    lua_to_sql, sql_to_lua, SqlValue, UdfMetadata, UdfParameter, UdfRegistry, UdfRuntime,
};

// Constants
pub const LUA_ENGINE_VERSION: &str = "0.1.0";
pub const SUPPORTED_LUA_VERSION: &str = "5.4";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all exports are accessible
        let _config = SecurityConfig::default();
        let _limits = ExecutionLimits::default();
        let _validator = ScriptValidator::default();
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_runtime_creation() {
        let runtime = MluaRuntime::new();
        let result = runtime.eval("return 42").await.unwrap();
        assert_eq!(result.as_i64(), Some(42));
    }

    #[cfg(feature = "lua-mlua")]
    #[tokio::test]
    async fn test_redis_style_execution() {
        let runtime = MluaRuntime::new();
        let result = runtime
            .eval_with_keys_args(
                "return #KEYS + #ARGV",
                &["k1".to_string(), "k2".to_string()],
                &[LuaValue::Integer(1), LuaValue::Integer(2)],
            )
            .await
            .unwrap();

        assert_eq!(result.as_i64(), Some(4)); // 2 keys + 2 args
    }
}

#[cfg(test)]
mod sql_integration_test;
#[cfg(test)]
mod udf_integration_test;

#[cfg(test)]
mod sql_syntax_e2e_test;
