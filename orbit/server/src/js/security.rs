//! JavaScript Security Sandbox
//!
//! Provides security controls for JavaScript execution including:
//! - Execution timeouts
//! - Memory limits
//! - API restrictions
//! - Context isolation

use super::types::JsError;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Execution limits for JavaScript runtime
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    /// Maximum execution time
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
            timeout: Duration::from_millis(super::DEFAULT_TIMEOUT_MS),
            memory_limit: super::DEFAULT_MEMORY_LIMIT,
            max_stack_depth: 1000,
            max_operations: 10_000_000,
        }
    }
}

impl ExecutionLimits {
    /// Create new limits with specified timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Create new limits with specified memory limit
    pub fn with_memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = limit;
        self
    }

    /// Create strict limits for untrusted code
    pub fn strict() -> Self {
        Self {
            timeout: Duration::from_millis(1000),
            memory_limit: 4 * 1024 * 1024, // 4 MB
            max_stack_depth: 100,
            max_operations: 1_000_000,
        }
    }

    /// Create relaxed limits for trusted code
    pub fn relaxed() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            memory_limit: 64 * 1024 * 1024, // 64 MB
            max_stack_depth: 10000,
            max_operations: 100_000_000,
        }
    }
}

/// Security configuration for JavaScript runtime
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Execution limits
    pub limits: ExecutionLimits,
    /// Allow eval() function
    pub allow_eval: bool,
    /// Allow Function constructor
    pub allow_function_constructor: bool,
    /// Allow access to global object properties
    pub allow_global_access: bool,
    /// Allowed global functions (whitelist)
    pub allowed_globals: Vec<String>,
    /// Blocked global functions (blacklist)
    pub blocked_globals: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            limits: ExecutionLimits::default(),
            allow_eval: false,
            allow_function_constructor: false,
            allow_global_access: false,
            allowed_globals: vec![
                // Safe built-ins
                "JSON".to_string(),
                "Math".to_string(),
                "Date".to_string(),
                "Array".to_string(),
                "Object".to_string(),
                "String".to_string(),
                "Number".to_string(),
                "Boolean".to_string(),
                "RegExp".to_string(),
                "Map".to_string(),
                "Set".to_string(),
                "WeakMap".to_string(),
                "WeakSet".to_string(),
                "Promise".to_string(),
                "Symbol".to_string(),
                "BigInt".to_string(),
                "Intl".to_string(),
                // Safe functions
                "parseInt".to_string(),
                "parseFloat".to_string(),
                "isNaN".to_string(),
                "isFinite".to_string(),
                "encodeURI".to_string(),
                "decodeURI".to_string(),
                "encodeURIComponent".to_string(),
                "decodeURIComponent".to_string(),
            ],
            blocked_globals: vec![
                // Dangerous functions
                "eval".to_string(),
                "Function".to_string(),
                // Environment access
                "process".to_string(),
                "require".to_string(),
                "module".to_string(),
                "exports".to_string(),
                "__dirname".to_string(),
                "__filename".to_string(),
                // Browser/Node APIs
                "fetch".to_string(),
                "XMLHttpRequest".to_string(),
                "WebSocket".to_string(),
                "Worker".to_string(),
                "SharedArrayBuffer".to_string(),
                "Atomics".to_string(),
            ],
        }
    }
}

impl SecurityConfig {
    /// Create a minimal security config (most restrictive)
    pub fn minimal() -> Self {
        Self {
            limits: ExecutionLimits::strict(),
            allow_eval: false,
            allow_function_constructor: false,
            allow_global_access: false,
            allowed_globals: vec![
                "JSON".to_string(),
                "Math".to_string(),
                "Array".to_string(),
                "Object".to_string(),
                "String".to_string(),
                "Number".to_string(),
                "Boolean".to_string(),
            ],
            blocked_globals: vec![],
        }
    }

    /// Create a permissive config for trusted code
    pub fn permissive() -> Self {
        Self {
            limits: ExecutionLimits::relaxed(),
            allow_eval: true,
            allow_function_constructor: true,
            allow_global_access: true,
            allowed_globals: vec![],
            blocked_globals: vec![
                // Still block dangerous system access
                "process".to_string(),
                "require".to_string(),
            ],
        }
    }

    /// Check if a global name is allowed
    pub fn is_global_allowed(&self, name: &str) -> bool {
        // If in blocked list, deny
        if self.blocked_globals.iter().any(|b| b == name) {
            return false;
        }
        // If whitelist is empty, allow all (except blocked)
        if self.allowed_globals.is_empty() {
            return true;
        }
        // Otherwise, must be in whitelist
        self.allowed_globals.iter().any(|a| a == name)
    }
}

/// Execution guard that tracks resource usage and enforces limits
pub struct ExecutionGuard {
    /// Start time of execution
    start_time: Instant,
    /// Execution limits
    limits: ExecutionLimits,
    /// Whether execution should be interrupted
    interrupted: Arc<AtomicBool>,
    /// Current memory usage
    memory_used: Arc<AtomicUsize>,
    /// Operation count
    operation_count: Arc<AtomicUsize>,
}

impl ExecutionGuard {
    /// Create a new execution guard
    pub fn new(limits: ExecutionLimits) -> Self {
        Self {
            start_time: Instant::now(),
            limits,
            interrupted: Arc::new(AtomicBool::new(false)),
            memory_used: Arc::new(AtomicUsize::new(0)),
            operation_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Check if execution should continue
    pub fn should_continue(&self) -> Result<(), JsError> {
        // Check interrupt flag
        if self.interrupted.load(Ordering::Relaxed) {
            return Err(JsError::RuntimeError("Execution interrupted".to_string()));
        }

        // Check timeout
        let elapsed = self.start_time.elapsed();
        if elapsed > self.limits.timeout {
            return Err(JsError::Timeout(self.limits.timeout.as_millis() as u64));
        }

        // Check memory
        let memory = self.memory_used.load(Ordering::Relaxed);
        if memory > self.limits.memory_limit {
            return Err(JsError::MemoryLimitExceeded {
                used: memory,
                limit: self.limits.memory_limit,
            });
        }

        Ok(())
    }

    /// Record memory allocation
    pub fn record_allocation(&self, size: usize) -> Result<(), JsError> {
        let current = self.memory_used.fetch_add(size, Ordering::Relaxed);
        let new_total = current + size;
        if new_total > self.limits.memory_limit {
            self.memory_used.fetch_sub(size, Ordering::Relaxed);
            return Err(JsError::MemoryLimitExceeded {
                used: new_total,
                limit: self.limits.memory_limit,
            });
        }
        Ok(())
    }

    /// Record memory deallocation
    pub fn record_deallocation(&self, size: usize) {
        self.memory_used.fetch_sub(size, Ordering::Relaxed);
    }

    /// Record an operation
    pub fn record_operation(&self) -> Result<(), JsError> {
        let count = self.operation_count.fetch_add(1, Ordering::Relaxed);
        if count >= self.limits.max_operations as usize {
            return Err(JsError::RuntimeError(format!(
                "Operation limit exceeded: {} operations",
                self.limits.max_operations
            )));
        }
        Ok(())
    }

    /// Request interruption of execution
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    /// Get the interrupt flag for use in callbacks
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        self.interrupted.clone()
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get memory usage
    pub fn memory_usage(&self) -> usize {
        self.memory_used.load(Ordering::Relaxed)
    }

    /// Get operation count
    pub fn operations(&self) -> usize {
        self.operation_count.load(Ordering::Relaxed)
    }
}

/// Script validator for security checks before execution
pub struct ScriptValidator {
    config: SecurityConfig,
}

impl ScriptValidator {
    /// Create a new script validator
    pub fn new(config: SecurityConfig) -> Self {
        Self { config }
    }

    /// Validate a script before execution
    pub fn validate(&self, script: &str) -> Result<(), JsError> {
        // Check script size
        if script.len() > super::MAX_SCRIPT_SIZE {
            return Err(JsError::ScriptTooLarge {
                size: script.len(),
                max: super::MAX_SCRIPT_SIZE,
            });
        }

        // Check for eval if not allowed
        if !self.config.allow_eval && script.contains("eval(") {
            return Err(JsError::SecurityViolation(
                "Use of eval() is not allowed".to_string(),
            ));
        }

        // Check for Function constructor if not allowed
        if !self.config.allow_function_constructor
            && (script.contains("new Function(") || script.contains("Function("))
        {
            return Err(JsError::SecurityViolation(
                "Use of Function constructor is not allowed".to_string(),
            ));
        }

        // Check for blocked globals
        for blocked in &self.config.blocked_globals {
            if script.contains(blocked) {
                return Err(JsError::SecurityViolation(format!(
                    "Access to '{}' is not allowed",
                    blocked
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_limits_default() {
        let limits = ExecutionLimits::default();
        assert_eq!(limits.timeout.as_millis(), 5000);
        assert_eq!(limits.memory_limit, 16 * 1024 * 1024);
    }

    #[test]
    fn test_security_config_global_check() {
        let config = SecurityConfig::default();
        assert!(config.is_global_allowed("JSON"));
        assert!(config.is_global_allowed("Math"));
        assert!(!config.is_global_allowed("eval"));
        assert!(!config.is_global_allowed("process"));
    }

    #[test]
    fn test_execution_guard_timeout() {
        let limits = ExecutionLimits {
            timeout: Duration::from_millis(1),
            ..Default::default()
        };
        let guard = ExecutionGuard::new(limits);

        // Sleep to trigger timeout
        std::thread::sleep(Duration::from_millis(5));

        let result = guard.should_continue();
        assert!(matches!(result, Err(JsError::Timeout(_))));
    }

    #[test]
    fn test_script_validator() {
        let config = SecurityConfig::default();
        let validator = ScriptValidator::new(config);

        // Valid script
        assert!(validator.validate("const x = 1 + 2;").is_ok());

        // Invalid: contains eval
        assert!(validator.validate("eval('code')").is_err());

        // Invalid: contains blocked global
        assert!(validator.validate("process.exit()").is_err());
    }
}
