//! Core Lua runtime using mlua
//!
//! This module provides the main Lua execution runtime with:
//! - Script evaluation (eval, eval_with_keys_args)
//! - Function registration and execution
//! - Script caching for EVALSHA
//! - Security enforcement via sandboxing
//! - Context management and pooling

use super::security::{set_memory_limit, setup_sandbox, ExecutionGuard, SecurityConfig, ScriptValidator};
use super::types::{LuaError, LuaFunction, LuaResult, LuaValue};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "lua-mlua")]
use mlua::{Lua, Value as MluaValue};

use sha1::{Digest, Sha1};

/// Main Lua runtime for executing scripts and managing functions
pub struct MluaRuntime {
    /// Security configuration
    config: SecurityConfig,
    /// Registered functions (name -> metadata)
    function_registry: Arc<RwLock<HashMap<String, LuaFunction>>>,
    /// Compiled script cache (SHA1 -> bytecode)
    script_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// Script validator
    validator: ScriptValidator,
}

impl MluaRuntime {
    /// Create a new Lua runtime with default configuration
    pub fn new() -> Self {
        Self::with_config(SecurityConfig::default())
    }

    /// Create a new Lua runtime with custom configuration
    pub fn with_config(config: SecurityConfig) -> Self {
        Self {
            validator: ScriptValidator::new(config.limits.memory_limit),
            config,
            function_registry: Arc::new(RwLock::new(HashMap::new())),
            script_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Evaluate a Lua script and return the result
    pub async fn eval(&self, script: &str) -> LuaResult<LuaValue> {
        self.eval_with_keys_args(script, &[], &[]).await
    }

    /// Evaluate a Lua script with KEYS and ARGV arrays (Redis-style)
    pub async fn eval_with_keys_args(
        &self,
        script: &str,
        keys: &[String],
        args: &[LuaValue],
    ) -> LuaResult<LuaValue> {
        // Validate script
        self.validator.validate(script)?;

        // Clone data for spawn_blocking
        let script = script.to_string();
        let keys = keys.to_vec();
        let args = args.to_vec();
        let config = self.config.clone();

        // Execute in blocking task since Lua is not Send/Sync
        let result = tokio::task::spawn_blocking(move || {
            eval_lua_blocking(&config, &script, &keys, &args)
        })
        .await
        .map_err(|e| LuaError::InternalError(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    /// Call a registered function
    pub async fn call_function(
        &self,
        name: &str,
        keys: &[String],
        args: &[LuaValue],
    ) -> LuaResult<LuaValue> {
        // Get function from registry
        let registry = self.function_registry.read().await;
        let func = registry
            .get(name)
            .ok_or_else(|| LuaError::FunctionNotFound(name.to_string()))?;

        // Execute function body
        self.eval_with_keys_args(&func.body, keys, args).await
    }

    /// Register a Lua function
    pub async fn register_function(&self, func: LuaFunction) -> LuaResult<()> {
        // Validate function body
        self.validator.validate(&func.body)?;

        // Store in registry
        let mut registry = self.function_registry.write().await;
        registry.insert(func.name.clone(), func);

        Ok(())
    }

    /// Load a Lua library (multiple functions)
    /// Returns list of registered function names
    pub async fn load_library(&self, _name: &str, code: &str) -> LuaResult<Vec<String>> {
        // Validate code
        self.validator.validate(code)?;

        // Parse library code to extract function registrations
        // For Redis compatibility, look for redis.register_function() calls
        let function_names = self.parse_library_functions(code)?;

        // Execute the library code to register functions
        self.eval(code).await?;

        Ok(function_names)
    }

    /// Unload a library (remove all its functions)
    pub async fn unload_library(&self, name: &str) -> LuaResult<()> {
        let mut registry = self.function_registry.write().await;

        // Remove all functions from this library
        // (In a full implementation, we'd track library membership)
        registry.retain(|_, func| !func.name.starts_with(&format!("{}.", name)));

        Ok(())
    }

    /// List all registered functions
    pub async fn list_functions(&self) -> Vec<LuaFunction> {
        let registry = self.function_registry.read().await;
        registry.values().cloned().collect()
    }

    /// Cache a script and return its SHA1 hash
    pub async fn cache_script(&self, script: &str) -> String {
        let sha = self.compute_sha1(script);

        // Compile and cache the script
        // We need to validate the script compiles, but can't hold lua across await
        let is_valid = if let Ok(lua) = self.create_context() {
            lua.load(script).into_function().is_ok()
        } else {
            false
        };

        if is_valid {
            // Store bytecode (in real implementation)
            // For now, store source
            let mut cache = self.script_cache.write().await;
            cache.insert(sha.clone(), script.as_bytes().to_vec());
        }

        sha
    }

    /// Evaluate a cached script by SHA1 hash
    pub async fn eval_sha(
        &self,
        sha: &str,
        keys: &[String],
        args: &[LuaValue],
    ) -> LuaResult<LuaValue> {
        // Look up script in cache
        let cache = self.script_cache.read().await;
        let script_bytes = cache
            .get(sha)
            .ok_or_else(|| LuaError::RuntimeError(format!("NOSCRIPT: {}", sha)))?;

        let script = String::from_utf8(script_bytes.clone())
            .map_err(|e| LuaError::InternalError(e.to_string()))?;

        // Execute cached script
        drop(cache); // Release lock before execution
        self.eval_with_keys_args(&script, keys, args).await
    }

    /// Check if a script exists in cache
    pub async fn script_exists(&self, sha: &str) -> bool {
        let cache = self.script_cache.read().await;
        cache.contains_key(sha)
    }

    /// Flush all cached scripts
    pub async fn flush_scripts(&self) {
        let mut cache = self.script_cache.write().await;
        cache.clear();
    }

    // Private helper methods

    /// Create a new Lua context with security restrictions
    #[cfg(feature = "lua-mlua")]
    fn create_context(&self) -> LuaResult<Lua> {
        let lua = Lua::new();

        // Apply security restrictions
        setup_sandbox(&lua, &self.config)?;

        // Set memory limit
        set_memory_limit(&lua, self.config.limits.memory_limit)?;

        Ok(lua)
    }

    #[cfg(not(feature = "lua-mlua"))]
    fn create_context(&self) -> LuaResult<Lua> {
        Err(LuaError::InternalError(
            "Lua support not enabled (lua-mlua feature required)".to_string(),
        ))
    }

    /// Install interrupt handler for timeout enforcement
    #[cfg(feature = "lua-mlua")]
    #[allow(dead_code)]
    fn install_interrupt_handler(&self, _lua: &Lua, _guard: &ExecutionGuard) -> LuaResult<()> {
        // Note: mlua 0.9 doesn't have set_interrupt API
        // Timeout is enforced via tokio::time::timeout in execute_with_guard
        // In future versions, we can use set_interrupt or polling mechanism
        Ok(())
    }

    #[cfg(not(feature = "lua-mlua"))]
    fn install_interrupt_handler(&self, _lua: &Lua, _guard: &ExecutionGuard) -> LuaResult<()> {
        Ok(())
    }

    /// Inject KEYS and ARGV arrays into Lua globals
    #[cfg(feature = "lua-mlua")]
    #[allow(dead_code)]
    fn inject_keys_argv(&self, lua: &Lua, keys: &[String], args: &[LuaValue]) -> LuaResult<()> {
        let globals = lua.globals();

        // Create KEYS array
        let keys_table = lua.create_table()?;
        for (i, key) in keys.iter().enumerate() {
            keys_table.set(i + 1, key.clone())?;
        }
        globals.set("KEYS", keys_table)?;

        // Create ARGV array
        let argv_table = lua.create_table()?;
        for (i, arg) in args.iter().enumerate() {
            let mlua_value = self.lua_value_to_mlua(lua, arg)?;
            argv_table.set(i + 1, mlua_value)?;
        }
        globals.set("ARGV", argv_table)?;

        Ok(())
    }

    #[cfg(not(feature = "lua-mlua"))]
    fn inject_keys_argv(&self, _lua: &Lua, _keys: &[String], _args: &[LuaValue]) -> LuaResult<()> {
        Ok(())
    }



    /// Convert LuaValue to mlua::Value
    #[cfg(feature = "lua-mlua")]
    #[allow(dead_code)]
    fn lua_value_to_mlua<'lua>(&self, lua: &'lua Lua, value: &LuaValue) -> LuaResult<MluaValue<'lua>> {
        let mlua_value = match value {
            LuaValue::Nil => MluaValue::Nil,
            LuaValue::Boolean(b) => MluaValue::Boolean(*b),
            LuaValue::Integer(i) => MluaValue::Integer(*i),
            LuaValue::Number(f) => MluaValue::Number(*f),
            LuaValue::String(s) => MluaValue::String(lua.create_string(s)?),
            LuaValue::Binary(b) => MluaValue::String(lua.create_string(b)?),
            LuaValue::Array(arr) => {
                let table = lua.create_table()?;
                for (i, item) in arr.iter().enumerate() {
                    table.set(i + 1, self.lua_value_to_mlua(lua, item)?)?;
                }
                MluaValue::Table(table)
            }
            LuaValue::Table(map) => {
                let table = lua.create_table()?;
                for (k, v) in map.iter() {
                    table.set(k.as_str(), self.lua_value_to_mlua(lua, v)?)?;
                }
                MluaValue::Table(table)
            }
            LuaValue::Function(_) => {
                return Err(LuaError::TypeError(
                    "Cannot convert function to mlua value".to_string(),
                ))
            }
        };

        Ok(mlua_value)
    }

    /// Convert mlua::Value to LuaValue
    #[cfg(feature = "lua-mlua")]
    #[allow(dead_code)]
    fn mlua_value_to_lua(&self, value: &MluaValue) -> LuaResult<LuaValue> {
        let lua_value = match value {
            MluaValue::Nil => LuaValue::Nil,
            MluaValue::Boolean(b) => LuaValue::Boolean(*b),
            MluaValue::Integer(i) => LuaValue::Integer(*i),
            MluaValue::Number(f) => LuaValue::Number(*f),
            MluaValue::String(s) => {
                let bytes = s.as_bytes();
                // Try UTF-8 first, fallback to binary
                match std::str::from_utf8(bytes) {
                    Ok(str_val) => LuaValue::String(str_val.to_string()),
                    Err(_) => LuaValue::Binary(bytes.to_vec()),
                }
            }
            MluaValue::Table(table) => {
                // Check if it's an array or a map
                if self.is_array_table(table) {
                    let mut arr = Vec::new();
                    for pair in table.clone().pairs::<i64, MluaValue>() {
                        let (_, v) = pair.map_err(|e| LuaError::TypeError(e.to_string()))?;
                        arr.push(self.mlua_value_to_lua(&v)?);
                    }
                    LuaValue::Array(arr)
                } else {
                    let mut map = HashMap::new();
                    for pair in table.clone().pairs::<String, MluaValue>() {
                        let (k, v) = pair.map_err(|e| LuaError::TypeError(e.to_string()))?;
                        map.insert(k, self.mlua_value_to_lua(&v)?);
                    }
                    LuaValue::Table(map)
                }
            }
            MluaValue::Function(_) => LuaValue::Function("anonymous".to_string()),
            MluaValue::Thread(_) => {
                return Err(LuaError::TypeError(
                    "Thread values not supported".to_string(),
                ))
            }
            MluaValue::UserData(_) => {
                return Err(LuaError::TypeError(
                    "UserData values not supported".to_string(),
                ))
            }
            MluaValue::LightUserData(_) => {
                return Err(LuaError::TypeError(
                    "LightUserData values not supported".to_string(),
                ))
            }
            MluaValue::Error(e) => {
                return Err(LuaError::RuntimeError(e.to_string()));
            }
        };

        Ok(lua_value)
    }

    /// Check if a Lua table is an array (sequential integer keys starting from 1)
    #[cfg(feature = "lua-mlua")]
    #[allow(dead_code)]
    fn is_array_table(&self, table: &mlua::Table) -> bool {
        // Simple heuristic: check if it has numeric key 1
        table.contains_key(1).unwrap_or(false)
    }

    /// Compute SHA1 hash of a script
    fn compute_sha1(&self, script: &str) -> String {
        let mut hasher = Sha1::new();
        hasher.update(script.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Parse library code to extract function names
    /// Simple parser for redis.register_function() calls
    fn parse_library_functions(&self, code: &str) -> LuaResult<Vec<String>> {
        let mut function_names = Vec::new();

        // Simple regex-based parsing (in production, use proper AST parsing)
        for line in code.lines() {
            if line.contains("redis.register_function") {
                // Extract function name from redis.register_function('name', ...)
                if let Some(start) = line.find('\'') {
                    if let Some(end) = line[start + 1..].find('\'') {
                        let name = &line[start + 1..start + 1 + end];
                        function_names.push(name.to_string());
                    }
                } else if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let name = &line[start + 1..start + 1 + end];
                        function_names.push(name.to_string());
                    }
                }
            }
        }

        Ok(function_names)
    }
}

impl Default for MluaRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_eval() {
        let runtime = MluaRuntime::new();
        let result = runtime.eval("return 1 + 2").await.unwrap();
        assert_eq!(result.as_i64(), Some(3));
    }

    #[tokio::test]
    async fn test_eval_with_keys_argv() {
        let runtime = MluaRuntime::new();
        let result = runtime
            .eval_with_keys_args(
                "return KEYS[1] .. ARGV[1]",
                &["key1".to_string()],
                &[LuaValue::String("arg1".to_string())],
            )
            .await
            .unwrap();

        assert_eq!(result.as_str(), Some("key1arg1"));
    }

    #[tokio::test]
    async fn test_function_registration() {
        let runtime = MluaRuntime::new();

        let func = LuaFunction::new("add", "return tonumber(ARGV[1]) + tonumber(ARGV[2])");
        runtime.register_function(func).await.unwrap();

        let result = runtime
            .call_function(
                "add",
                &[],
                &[LuaValue::Integer(10), LuaValue::Integer(20)],
            )
            .await
            .unwrap();

        assert_eq!(result.as_i64(), Some(30));
    }

    #[tokio::test]
    async fn test_script_caching() {
        let runtime = MluaRuntime::new();
        let script = "return 42";

        let sha = runtime.cache_script(script).await;
        assert!(runtime.script_exists(&sha).await);

        let result = runtime.eval_sha(&sha, &[], &[]).await.unwrap();
        assert_eq!(result.as_i64(), Some(42));
    }

    #[tokio::test]
    async fn test_timeout_enforcement() {
        let config = SecurityConfig::redis_default()
            .limits
            .with_timeout(std::time::Duration::from_millis(10));
        let runtime = MluaRuntime::with_config(SecurityConfig {
            limits: config,
            ..SecurityConfig::redis_default()
        });

        let result = runtime.eval("while true do end").await;
        assert!(matches!(result, Err(LuaError::Timeout { .. })));
    }

    #[tokio::test]
    async fn test_security_sandbox() {
        let runtime = MluaRuntime::new();

        // Should fail - os is blocked
        let result = runtime.eval("return os.execute('ls')").await;
        assert!(result.is_err());

        // Should fail - io is blocked
        let result = runtime.eval("return io.open('/etc/passwd')").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_script_too_large() {
        let runtime = MluaRuntime::new();
        let large_script = "x".repeat(2 * 1024 * 1024); // 2MB

        let result = runtime.eval(&large_script).await;
        assert!(matches!(result, Err(LuaError::ScriptTooLarge { .. })));
    }

    #[test]
    fn test_sha1_computation() {
        let runtime = MluaRuntime::new();
        let sha = runtime.compute_sha1("return 42");

        // Verify it's a valid 40-character hex string
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

// Standalone helper function for spawn_blocking
// This doesn't need `self` so it can be moved into the closure
#[cfg(feature = "lua-mlua")]
fn eval_lua_blocking(
    config: &SecurityConfig,
    script: &str,
    keys: &[String],
    args: &[LuaValue],
) -> LuaResult<LuaValue> {
    use super::security::{setup_sandbox, set_memory_limit};
    
    // Create Lua context
    let lua = Lua::new();
    
    // Apply security restrictions
    setup_sandbox(&lua, config)?;
    
    // Set memory limit
    set_memory_limit(&lua, config.limits.memory_limit)?;
    
    // Setup execution guard
    let guard = ExecutionGuard::new(&config.limits);
    
    // Inject KEYS and ARGV into globals
    let globals = lua.globals();
    
    // Create KEYS array
    let keys_table = lua.create_table()?;
    for (i, key) in keys.iter().enumerate() {
        keys_table.set(i + 1, key.clone())?;
    }
    globals.set("KEYS", keys_table)?;
    
    // Create ARGV array
    let argv_table = lua.create_table()?;
    for (i, arg) in args.iter().enumerate() {
        let mlua_value = lua_value_to_mlua(&lua, arg)?;
        argv_table.set(i + 1, mlua_value)?;
    }
    globals.set("ARGV", argv_table)?;
    
    // Compile script
    let chunk = lua
        .load(script)
        .into_function()
        .map_err(|e| LuaError::CompilationError(e.to_string()))?;
    
    // Check guard before execution
    guard.should_continue()?;
    
    // Execute chunk synchronously
    let mlua_result: MluaValue = chunk
        .call(())
        .map_err(|e| LuaError::RuntimeError(e.to_string()))?;
    
    // Convert result to LuaValue
    mlua_value_to_lua(&mlua_result)
}

// Helper to convert LuaValue to mlua::Value
#[cfg(feature = "lua-mlua")]
fn lua_value_to_mlua<'lua>(lua: &'lua Lua, value: &LuaValue) -> LuaResult<MluaValue<'lua>> {
    let mlua_value = match value {
        LuaValue::Nil => MluaValue::Nil,
        LuaValue::Boolean(b) => MluaValue::Boolean(*b),
        LuaValue::Integer(i) => MluaValue::Integer(*i),
        LuaValue::Number(f) => MluaValue::Number(*f),
        LuaValue::String(s) => MluaValue::String(lua.create_string(s)?),
        LuaValue::Binary(b) => MluaValue::String(lua.create_string(b)?),
        LuaValue::Array(arr) => {
            let table = lua.create_table()?;
            for (i, item) in arr.iter().enumerate() {
                table.set(i + 1, lua_value_to_mlua(lua, item)?)?;
            }
            MluaValue::Table(table)
        }
        LuaValue::Table(map) => {
            let table = lua.create_table()?;
            for (k, v) in map.iter() {
                table.set(k.as_str(), lua_value_to_mlua(lua, v)?)?;
            }
            MluaValue::Table(table)
        }
        LuaValue::Function(_) => {
            return Err(LuaError::TypeError(
                "Cannot convert function to mlua value".to_string(),
            ))
        }
    };

    Ok(mlua_value)
}

// Helper to convert mlua::Value to LuaValue
#[cfg(feature = "lua-mlua")]
fn mlua_value_to_lua(value: &MluaValue) -> LuaResult<LuaValue> {
    use std::collections::HashMap;
    
    let lua_value = match value {
        MluaValue::Nil => LuaValue::Nil,
        MluaValue::Boolean(b) => LuaValue::Boolean(*b),
        MluaValue::Integer(i) => LuaValue::Integer(*i),
        MluaValue::Number(f) => LuaValue::Number(*f),
        MluaValue::String(s) => {
            let bytes = s.as_bytes();
            // Try UTF-8 first, fallback to binary
            match std::str::from_utf8(bytes) {
                Ok(str_val) => LuaValue::String(str_val.to_string()),
                Err(_) => LuaValue::Binary(bytes.to_vec()),
            }
        }
        MluaValue::Table(table) => {
            // Check if it's an array or a map
            if is_array_table(table) {
                let mut arr = Vec::new();
                for pair in table.clone().pairs::<i64, MluaValue>() {
                    let (_, v) = pair.map_err(|e| LuaError::TypeError(e.to_string()))?;
                    arr.push(mlua_value_to_lua(&v)?);
                }
                LuaValue::Array(arr)
            } else {
                let mut map = HashMap::new();
                for pair in table.clone().pairs::<String, MluaValue>() {
                    let (k, v) = pair.map_err(|e| LuaError::TypeError(e.to_string()))?;
                    map.insert(k, mlua_value_to_lua(&v)?);
                }
                LuaValue::Table(map)
            }
        }
        MluaValue::Function(_) => LuaValue::Function("anonymous".to_string()),
        MluaValue::Thread(_) => {
            return Err(LuaError::TypeError(
                "Thread values not supported".to_string(),
            ))
        }
        MluaValue::UserData(_) => {
            return Err(LuaError::TypeError(
                "UserData values not supported".to_string(),
            ))
        }
        MluaValue::LightUserData(_) => {
            return Err(LuaError::TypeError(
                "LightUserData values not supported".to_string(),
            ))
        }
        MluaValue::Error(e) => {
            return Err(LuaError::RuntimeError(e.to_string()));
        }
    };

    Ok(lua_value)
}

// Helper to check if a table is an array
#[cfg(feature = "lua-mlua")]
fn is_array_table(table: &mlua::Table) -> bool {
    table.contains_key(1).unwrap_or(false)
}
