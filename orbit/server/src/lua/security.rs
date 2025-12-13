//! Security and sandboxing for Lua execution
//!
//! This module provides multi-layer security for Lua script execution:
//! - Pre-execution validation (script size, forbidden patterns)
//! - Resource limits (timeout, memory, stack depth)
//! - Runtime monitoring (ExecutionGuard)
//! - Sandbox environment setup (removing dangerous globals)

use super::types::{LuaError, LuaResult};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "lua-mlua")]
use mlua::Lua;

/// Constants for security limits
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;
pub const DEFAULT_MEMORY_LIMIT: usize = 16 * 1024 * 1024; // 16MB
pub const MAX_SCRIPT_SIZE: usize = 1024 * 1024; // 1MB
pub const DEFAULT_MAX_STACK_DEPTH: usize = 100;
pub const DEFAULT_MAX_OPERATIONS: u64 = 1_000_000;

/// Execution limits for Lua scripts
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    /// Maximum execution time in milliseconds
    pub timeout: Duration,
    /// Maximum memory usage in bytes
    pub memory_limit: usize,
    /// Maximum stack depth
    pub max_stack_depth: usize,
    /// Maximum number of operations (for instruction counting)
    pub max_operations: u64,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            memory_limit: DEFAULT_MEMORY_LIMIT,
            max_stack_depth: DEFAULT_MAX_STACK_DEPTH,
            max_operations: DEFAULT_MAX_OPERATIONS,
        }
    }
}

impl ExecutionLimits {
    /// Create limits suitable for Redis EVAL/FUNCTION (untrusted scripts)
    pub fn redis_default() -> Self {
        Self {
            timeout: Duration::from_millis(5000),
            memory_limit: 16 * 1024 * 1024,
            max_stack_depth: 100,
            max_operations: 1_000_000,
        }
    }

    /// Create limits suitable for PostgreSQL PL/Lua (semi-trusted)
    pub fn postgres_default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            memory_limit: 64 * 1024 * 1024,
            max_stack_depth: 1000,
            max_operations: 10_000_000,
        }
    }

    /// Create minimal limits for maximum security
    pub fn minimal() -> Self {
        Self {
            timeout: Duration::from_secs(1),
            memory_limit: 4 * 1024 * 1024,
            max_stack_depth: 50,
            max_operations: 100_000,
        }
    }

    /// Create permissive limits for trusted code
    pub fn permissive() -> Self {
        Self {
            timeout: Duration::from_secs(300), // 5 minutes
            memory_limit: 256 * 1024 * 1024,   // 256MB
            max_stack_depth: 5000,
            max_operations: 100_000_000,
        }
    }

    /// Set custom timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set custom memory limit
    pub fn with_memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = limit;
        self
    }
}

/// Security configuration for Lua runtime
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Execution limits
    pub limits: ExecutionLimits,
    /// Allow debug library
    pub allow_debug: bool,
    /// Allow I/O operations (file, network)
    pub allow_io: bool,
    /// Allowed standard library modules
    pub allowed_modules: Vec<String>,
    /// Explicitly blocked functions
    pub blocked_functions: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self::redis_default()
    }
}

impl SecurityConfig {
    /// Create configuration suitable for Redis (most restrictive)
    pub fn redis_default() -> Self {
        Self {
            limits: ExecutionLimits::redis_default(),
            allow_debug: false,
            allow_io: false,
            allowed_modules: vec![
                "math".to_string(),
                "string".to_string(),
                "table".to_string(),
            ],
            blocked_functions: vec![
                "os".to_string(),
                "io".to_string(),
                "debug".to_string(),
                "loadfile".to_string(),
                "dofile".to_string(),
                "require".to_string(),
                "package".to_string(),
            ],
        }
    }

    /// Create configuration suitable for PostgreSQL PL/Lua
    pub fn postgres_default() -> Self {
        Self {
            limits: ExecutionLimits::postgres_default(),
            allow_debug: false,
            allow_io: true,
            allowed_modules: vec![
                "math".to_string(),
                "string".to_string(),
                "table".to_string(),
                "json".to_string(),
            ],
            blocked_functions: vec!["os.execute".to_string(), "io.popen".to_string()],
        }
    }

    /// Create minimal security configuration
    pub fn minimal() -> Self {
        Self {
            limits: ExecutionLimits::minimal(),
            allow_debug: false,
            allow_io: false,
            allowed_modules: vec!["math".to_string(), "string".to_string()],
            blocked_functions: vec![
                "os".to_string(),
                "io".to_string(),
                "debug".to_string(),
                "loadfile".to_string(),
                "dofile".to_string(),
                "require".to_string(),
                "package".to_string(),
                "load".to_string(),
            ],
        }
    }

    /// Create permissive configuration for trusted code
    pub fn permissive() -> Self {
        Self {
            limits: ExecutionLimits::permissive(),
            allow_debug: true,
            allow_io: true,
            allowed_modules: vec![
                "math".to_string(),
                "string".to_string(),
                "table".to_string(),
                "json".to_string(),
                "os".to_string(),
            ],
            blocked_functions: vec![],
        }
    }
}

/// Guards script execution and enforces resource limits
pub struct ExecutionGuard {
    start_time: Instant,
    timeout: Duration,
    operation_count: AtomicU64,
    max_operations: u64,
    interrupted: Arc<AtomicBool>,
}

impl ExecutionGuard {
    /// Create a new execution guard with limits
    pub fn new(limits: &ExecutionLimits) -> Self {
        Self {
            start_time: Instant::now(),
            timeout: limits.timeout,
            operation_count: AtomicU64::new(0),
            max_operations: limits.max_operations,
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if execution should continue
    pub fn should_continue(&self) -> LuaResult<()> {
        // Check timeout
        if self.start_time.elapsed() > self.timeout {
            return Err(LuaError::Timeout {
                timeout_ms: self.timeout.as_millis() as u64,
            });
        }

        // Check operation count
        let ops = self.operation_count.load(Ordering::Relaxed);
        if ops > self.max_operations {
            return Err(LuaError::SecurityViolation(format!(
                "Operation limit exceeded: {} > {}",
                ops, self.max_operations
            )));
        }

        // Check interrupted flag
        if self.interrupted.load(Ordering::Relaxed) {
            return Err(LuaError::RuntimeError("Execution interrupted".to_string()));
        }

        Ok(())
    }

    /// Increment operation counter
    pub fn count_operation(&self) {
        self.operation_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Interrupt execution
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get operation count
    pub fn operations(&self) -> u64 {
        self.operation_count.load(Ordering::Relaxed)
    }

    /// Get interrupt handle for use in Lua interrupt callback
    pub fn interrupt_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupted)
    }
}

/// Validates scripts before execution
pub struct ScriptValidator {
    max_script_size: usize,
    forbidden_patterns: Vec<String>,
}

impl Default for ScriptValidator {
    fn default() -> Self {
        Self {
            max_script_size: MAX_SCRIPT_SIZE,
            forbidden_patterns: vec![
                "io.popen".to_string(),
                "os.execute".to_string(),
                "loadfile".to_string(),
                "dofile".to_string(),
                "require(\"ffi\")".to_string(),
            ],
        }
    }
}

impl ScriptValidator {
    /// Create a new validator
    pub fn new(max_script_size: usize) -> Self {
        Self {
            max_script_size,
            forbidden_patterns: Self::default().forbidden_patterns,
        }
    }

    /// Validate a script before execution
    pub fn validate(&self, script: &str) -> LuaResult<()> {
        // Check script size
        if script.len() > self.max_script_size {
            return Err(LuaError::ScriptTooLarge {
                size: script.len(),
                max_size: self.max_script_size,
            });
        }

        // Check for forbidden patterns
        for pattern in &self.forbidden_patterns {
            if script.contains(pattern) {
                return Err(LuaError::SecurityViolation(format!(
                    "Script contains forbidden pattern: {}",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Add a forbidden pattern
    pub fn add_forbidden_pattern(&mut self, pattern: String) {
        self.forbidden_patterns.push(pattern);
    }
}

/// Setup sandbox environment by removing dangerous globals
#[cfg(feature = "lua-mlua")]
pub fn setup_sandbox(lua: &Lua, config: &SecurityConfig) -> LuaResult<()> {
    let globals = lua.globals();

    // Always remove dangerous functions
    for func_name in &config.blocked_functions {
        // Handle nested paths like "os.execute"
        if func_name.contains('.') {
            let parts: Vec<&str> = func_name.split('.').collect();
            if parts.len() == 2 {
                if let Ok(table) = globals.get::<_, mlua::Table>(parts[0]) {
                    let _ = table.set(parts[1], mlua::Value::Nil);
                }
            }
        } else {
            let _ = globals.set(func_name.as_str(), mlua::Value::Nil);
        }
    }

    // Remove dangerous globals if not explicitly allowed
    if !config.allow_debug {
        let _ = globals.set("debug", mlua::Value::Nil);
    }

    if !config.allow_io {
        let _ = globals.set("io", mlua::Value::Nil);
        let _ = globals.set("os", mlua::Value::Nil);
    } else {
        // Remove dangerous os functions even if io is allowed
        if let Ok(os_table) = globals.get::<_, mlua::Table>("os") {
            let _ = os_table.set("execute", mlua::Value::Nil);
            let _ = os_table.set("exit", mlua::Value::Nil);
            let _ = os_table.set("remove", mlua::Value::Nil);
            let _ = os_table.set("rename", mlua::Value::Nil);
        }
    }

    // Always remove these dangerous functions
    let _ = globals.set("loadfile", mlua::Value::Nil);
    let _ = globals.set("dofile", mlua::Value::Nil);

    // Replace require with whitelist-based version
    let allowed_modules = config.allowed_modules.clone();
    let require_fn =
        lua.create_function(move |lua, module: String| -> mlua::Result<mlua::Value> {
            if allowed_modules.contains(&module) {
                // Load the module normally
                lua.globals()
                    .get::<_, mlua::Function>("_original_require")?
                    .call::<_, mlua::Value>(module)
            } else {
                Err(mlua::Error::RuntimeError(format!(
                    "Module '{}' is not allowed",
                    module
                )))
            }
        })?;

    // Store original require before replacing
    if let Ok(original_require) = globals.get::<_, mlua::Function>("require") {
        globals.set("_original_require", original_require)?;
    }
    globals.set("require", require_fn)?;

    Ok(())
}

/// Configure memory limit for Lua context
#[cfg(feature = "lua-mlua")]
pub fn set_memory_limit(lua: &Lua, limit: usize) -> LuaResult<()> {
    lua.set_memory_limit(limit)
        .map_err(|e| LuaError::InternalError(format!("Failed to set memory limit: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_limits_defaults() {
        let limits = ExecutionLimits::default();
        assert_eq!(limits.timeout.as_millis(), 5000);
        assert_eq!(limits.memory_limit, 16 * 1024 * 1024);
        assert_eq!(limits.max_stack_depth, 100);
    }

    #[test]
    fn test_execution_guard() {
        let limits = ExecutionLimits::minimal();
        let guard = ExecutionGuard::new(&limits);

        assert!(guard.should_continue().is_ok());
        assert_eq!(guard.operations(), 0);

        guard.count_operation();
        assert_eq!(guard.operations(), 1);
    }

    #[test]
    fn test_execution_guard_interrupt() {
        let limits = ExecutionLimits::default();
        let guard = ExecutionGuard::new(&limits);

        guard.interrupt();
        assert!(guard.should_continue().is_err());
    }

    #[test]
    fn test_script_validator() {
        let validator = ScriptValidator::default();

        // Valid script
        assert!(validator.validate("return 1 + 2").is_ok());

        // Script with forbidden pattern
        assert!(validator.validate("os.execute('rm -rf /')").is_err());

        // Script too large
        let large_script = "x".repeat(MAX_SCRIPT_SIZE + 1);
        assert!(validator.validate(&large_script).is_err());
    }

    #[test]
    fn test_security_config_redis() {
        let config = SecurityConfig::redis_default();
        assert!(!config.allow_debug);
        assert!(!config.allow_io);
        assert!(config.allowed_modules.contains(&"math".to_string()));
        assert!(config.blocked_functions.contains(&"os".to_string()));
    }

    #[test]
    fn test_security_config_postgres() {
        let config = SecurityConfig::postgres_default();
        assert!(!config.allow_debug);
        assert!(config.allow_io); // PL/Lua allows I/O with restrictions
        assert_eq!(config.limits.timeout.as_secs(), 30);
    }
}
