//! QuickJS JavaScript Runtime for MongoDB and Redis
//!
//! This module provides a high-performance JavaScript runtime using QuickJS
//! via rquickjs bindings, optimized for:
//!
//! - MongoDB $where and $function operators
//! - Redis EVAL/EVALSHA commands
//!
//! ## Features
//!
//! - Near-V8 performance for hot paths
//! - Small memory footprint (~500KB per context)
//! - Fast script compilation (<1ms)
//! - Interrupt handlers for timeouts
//! - ES2020+ support
//!
//! ## Performance
//!
//! QuickJS is chosen for MongoDB/Redis because these are performance-critical
//! paths where scripts execute frequently in query evaluation.

use super::security::{ExecutionGuard, ScriptValidator, SecurityConfig};
use super::types::{JsError, JsResult, JsValue};
use rquickjs::{function::Func, Array, Context, Object, Runtime, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, warn};

/// QuickJS runtime for MongoDB and Redis
pub struct QuickJsRuntime {
    /// QuickJS runtime instance
    runtime: Runtime,
    /// Security configuration
    config: SecurityConfig,
    /// Compiled script cache (SHA -> compiled bytecode reference)
    script_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl QuickJsRuntime {
    /// Create a new QuickJS runtime with default config
    pub fn new() -> JsResult<Self> {
        Self::with_config(SecurityConfig::default())
    }

    /// Create a new QuickJS runtime with custom security config
    pub fn with_config(config: SecurityConfig) -> JsResult<Self> {
        let runtime = Runtime::new()
            .map_err(|e| JsError::InternalError(format!("Failed to create runtime: {e}")))?;

        // Set memory limit
        runtime.set_memory_limit(config.limits.memory_limit);

        // Set max stack size
        runtime.set_max_stack_size(config.limits.max_stack_depth * 1024); // Approximate bytes

        Ok(Self {
            runtime,
            config,
            script_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new execution context
    fn create_context(&self) -> JsResult<Context> {
        Context::full(&self.runtime)
            .map_err(|e| JsError::InternalError(format!("Failed to create context: {e}")))
    }

    /// Execute a JavaScript expression and return the result
    pub fn eval(&self, script: &str) -> JsResult<JsValue> {
        // Validate script
        let validator = ScriptValidator::new(self.config.clone());
        validator.validate(script)?;

        let context = self.create_context()?;
        let guard = ExecutionGuard::new(self.config.limits.clone());

        // Set up interrupt handler
        let interrupt_flag = guard.interrupt_flag();
        self.runtime.set_interrupt_handler(Some(Box::new(move || {
            interrupt_flag.load(std::sync::atomic::Ordering::Relaxed)
        })));

        context.with(|ctx| {
            // Apply security restrictions
            self.apply_security_restrictions(&ctx)?;

            // Register helpers
            self.register_helpers(&ctx)?;

            // Evaluate script
            let result: Value = ctx
                .eval(script)
                .map_err(|e| JsError::RuntimeError(e.to_string()))?;

            // Check limits
            guard.should_continue()?;

            // Convert result
            self.quickjs_to_js_value(result, &ctx)
        })
    }

    /// Execute a function with arguments (MongoDB $function style)
    pub fn call_function(&self, function_body: &str, args: &[JsValue]) -> JsResult<JsValue> {
        // Build a complete script that calls the function with JSON-serialized args
        let args_json: Vec<String> = args
            .iter()
            .map(|arg| arg.to_json().unwrap_or_else(|_| "null".to_string()))
            .collect();

        let script = format!("(({})({}))", function_body, args_json.join(", "));

        self.eval(&script)
    }

    /// Evaluate a MongoDB $where expression
    pub fn eval_where(&self, expression: &str, document: &JsValue) -> JsResult<bool> {
        // Wrap the expression in a function that we call with the document as 'this'
        // This mimics MongoDB's $where behavior where 'this' refers to the document
        let doc_json = document.to_json()?;

        // Create a script that:
        // 1. Creates a function from the expression
        // 2. Calls it with the document as 'this'
        // Also expose 'obj' as a global for compatibility
        let script = format!(
            r#"
            var __doc = {};
            var obj = __doc;
            (function() {{ return {}; }}).call(__doc)
            "#,
            doc_json, expression
        );

        let result = self.eval(&script)?;

        // Convert to boolean
        Ok(result
            .as_bool()
            .unwrap_or_else(|| !matches!(result, JsValue::Null | JsValue::Undefined)))
    }

    /// Cache a script for Redis EVALSHA
    pub fn cache_script(&self, script: &str) -> JsResult<String> {
        // Validate script
        let validator = ScriptValidator::new(self.config.clone());
        validator.validate(script)?;

        // Generate SHA1 hash
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(script.as_bytes());
        let sha = format!("{:x}", hasher.finalize());

        // Store in cache
        let mut cache = self
            .script_cache
            .write()
            .map_err(|_| JsError::InternalError("Failed to acquire cache lock".to_string()))?;
        cache.insert(sha.clone(), script.to_string());

        Ok(sha)
    }

    /// Execute a cached script by SHA (Redis EVALSHA)
    pub fn eval_sha(&self, sha: &str, keys: &[String], args: &[JsValue]) -> JsResult<JsValue> {
        let cache = self
            .script_cache
            .read()
            .map_err(|_| JsError::InternalError("Failed to acquire cache lock".to_string()))?;

        let script = cache
            .get(sha)
            .ok_or_else(|| JsError::RuntimeError(format!("Script not found: {}", sha)))?
            .clone();

        drop(cache);

        self.eval_redis_script(&script, keys, args)
    }

    /// Execute a Redis-style script with KEYS and ARGV
    pub fn eval_redis_script(
        &self,
        script: &str,
        keys: &[String],
        args: &[JsValue],
    ) -> JsResult<JsValue> {
        // Build KEYS and ARGV as JSON arrays
        let keys_json: Vec<String> = keys.iter().map(|k| format!("\"{}\"", k)).collect();
        let argv_json: Vec<String> = args
            .iter()
            .map(|v| v.to_json().unwrap_or_else(|_| "null".to_string()))
            .collect();

        // Wrap script in a function if it contains 'return' at the start
        // This handles Redis Lua-style scripts that use 'return' at top level
        let trimmed = script.trim();
        let wrapped_script = if trimmed.starts_with("return ") || trimmed.starts_with("return\n") {
            format!("(function() {{ {} }})()", script)
        } else {
            script.to_string()
        };

        // Build the complete script with KEYS and ARGV globals
        let full_script = format!(
            "var KEYS = [{}];\nvar ARGV = [{}];\n{}",
            keys_json.join(", "),
            argv_json.join(", "),
            wrapped_script
        );

        self.eval(&full_script)
    }

    /// Apply security restrictions to context
    fn apply_security_restrictions<'js>(&self, ctx: &rquickjs::Ctx<'js>) -> JsResult<()> {
        let globals = ctx.globals();

        // Remove blocked globals
        for blocked in &self.config.blocked_globals {
            let _ = globals.remove(blocked.as_str());
        }

        Ok(())
    }

    /// Register helper functions
    fn register_helpers<'js>(&self, ctx: &rquickjs::Ctx<'js>) -> JsResult<()> {
        let globals = ctx.globals();

        // Create console object with log/warn
        let console =
            Object::new(ctx.clone()).map_err(|e| JsError::InternalError(e.to_string()))?;

        console
            .set(
                "log",
                Func::from(|msg: String| {
                    debug!(target: "quickjs", "{}", msg);
                }),
            )
            .map_err(|e| JsError::InternalError(e.to_string()))?;

        console
            .set(
                "warn",
                Func::from(|msg: String| {
                    warn!(target: "quickjs", "{}", msg);
                }),
            )
            .map_err(|e| JsError::InternalError(e.to_string()))?;

        globals
            .set("console", console)
            .map_err(|e| JsError::InternalError(e.to_string()))?;

        Ok(())
    }

    /// Convert QuickJS value to JsValue
    fn quickjs_to_js_value<'js>(
        &self,
        value: Value<'js>,
        ctx: &rquickjs::Ctx<'js>,
    ) -> JsResult<JsValue> {
        if value.is_undefined() {
            return Ok(JsValue::Undefined);
        }
        if value.is_null() {
            return Ok(JsValue::Null);
        }
        if let Some(b) = value.as_bool() {
            return Ok(JsValue::Bool(b));
        }
        if let Some(n) = value.as_int() {
            return Ok(JsValue::Integer(n as i64));
        }
        if let Some(f) = value.as_float() {
            return Ok(JsValue::Float(f));
        }
        if let Some(s) = value.as_string() {
            return Ok(JsValue::String(
                s.to_string()
                    .map_err(|e| JsError::TypeError(e.to_string()))?,
            ));
        }

        // Check if it's an array
        if let Some(arr) = value.as_array() {
            let mut result = Vec::new();
            for i in 0..arr.len() {
                let item = arr
                    .get::<Value>(i)
                    .map_err(|e| JsError::TypeError(e.to_string()))?;
                result.push(self.quickjs_to_js_value(item, ctx)?);
            }
            return Ok(JsValue::Array(result));
        }

        // Regular object
        if let Some(obj) = value.as_object() {
            let mut result = HashMap::new();
            for key in obj.keys::<String>() {
                let key = key.map_err(|e| JsError::TypeError(e.to_string()))?;
                let val: Value = obj
                    .get(&key)
                    .map_err(|e| JsError::TypeError(e.to_string()))?;
                result.insert(key, self.quickjs_to_js_value(val, ctx)?);
            }
            return Ok(JsValue::Object(result));
        }

        Ok(JsValue::Null)
    }

    /// Convert JsValue to QuickJS value
    fn js_value_to_quickjs<'js>(
        &self,
        value: &JsValue,
        ctx: &rquickjs::Ctx<'js>,
    ) -> JsResult<Value<'js>> {
        match value {
            JsValue::Undefined => Ok(Value::new_undefined(ctx.clone())),
            JsValue::Null => Ok(Value::new_null(ctx.clone())),
            JsValue::Bool(b) => Ok(Value::new_bool(ctx.clone(), *b)),
            JsValue::Integer(n) => {
                if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 {
                    Ok(Value::new_int(ctx.clone(), *n as i32))
                } else {
                    Ok(Value::new_float(ctx.clone(), *n as f64))
                }
            }
            JsValue::Float(f) => Ok(Value::new_float(ctx.clone(), *f)),
            JsValue::String(s) => {
                let js_str = rquickjs::String::from_str(ctx.clone(), s)
                    .map_err(|e| JsError::TypeError(e.to_string()))?;
                Ok(js_str.into())
            }
            JsValue::Array(arr) => {
                let js_arr =
                    Array::new(ctx.clone()).map_err(|e| JsError::TypeError(e.to_string()))?;
                for (i, item) in arr.iter().enumerate() {
                    let js_item = self.js_value_to_quickjs(item, ctx)?;
                    js_arr
                        .set(i, js_item)
                        .map_err(|e| JsError::TypeError(e.to_string()))?;
                }
                Ok(js_arr.into())
            }
            JsValue::Object(obj) => {
                let js_obj =
                    Object::new(ctx.clone()).map_err(|e| JsError::TypeError(e.to_string()))?;
                for (key, val) in obj {
                    let js_val = self.js_value_to_quickjs(val, ctx)?;
                    js_obj
                        .set(key.as_str(), js_val)
                        .map_err(|e| JsError::TypeError(e.to_string()))?;
                }
                Ok(js_obj.into())
            }
            JsValue::Binary(data) => {
                // Return as base64 string for now
                let encoded =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
                let js_str = rquickjs::String::from_str(ctx.clone(), &encoded)
                    .map_err(|e| JsError::TypeError(e.to_string()))?;
                Ok(js_str.into())
            }
            JsValue::Date(s) | JsValue::BigInt(s) => {
                let js_str = rquickjs::String::from_str(ctx.clone(), s)
                    .map_err(|e| JsError::TypeError(e.to_string()))?;
                Ok(js_str.into())
            }
        }
    }
}

impl Default for QuickJsRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create QuickJS runtime")
    }
}

/// MongoDB JavaScript operator executor
pub struct MongoJsOperator {
    runtime: QuickJsRuntime,
}

impl MongoJsOperator {
    /// Create a new MongoDB JavaScript operator executor
    pub fn new() -> JsResult<Self> {
        Ok(Self {
            runtime: QuickJsRuntime::new()?,
        })
    }

    /// Evaluate $where expression against a document
    pub fn eval_where(&self, expression: &str, document: &JsValue) -> JsResult<bool> {
        self.runtime.eval_where(expression, document)
    }

    /// Execute $function operator
    pub fn eval_function(&self, body: &str, args: &[JsValue]) -> JsResult<JsValue> {
        self.runtime.call_function(body, args)
    }
}

impl Default for MongoJsOperator {
    fn default() -> Self {
        Self::new().expect("Failed to create MongoDB JS operator")
    }
}

/// Redis EVAL/EVALSHA executor
pub struct RedisScriptExecutor {
    runtime: QuickJsRuntime,
}

impl RedisScriptExecutor {
    /// Create a new Redis script executor
    pub fn new() -> JsResult<Self> {
        Ok(Self {
            runtime: QuickJsRuntime::new()?,
        })
    }

    /// Execute EVAL command
    pub fn eval(&self, script: &str, keys: &[String], args: &[JsValue]) -> JsResult<JsValue> {
        self.runtime.eval_redis_script(script, keys, args)
    }

    /// Load script and return SHA
    pub fn script_load(&self, script: &str) -> JsResult<String> {
        self.runtime.cache_script(script)
    }

    /// Execute EVALSHA command
    pub fn evalsha(&self, sha: &str, keys: &[String], args: &[JsValue]) -> JsResult<JsValue> {
        self.runtime.eval_sha(sha, keys, args)
    }
}

impl Default for RedisScriptExecutor {
    fn default() -> Self {
        Self::new().expect("Failed to create Redis script executor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_eval() {
        let runtime = QuickJsRuntime::new().unwrap();
        let result = runtime.eval("1 + 2").unwrap();
        assert_eq!(result.as_i64(), Some(3));
    }

    #[test]
    fn test_string_operations() {
        let runtime = QuickJsRuntime::new().unwrap();
        let result = runtime.eval("'hello' + ' ' + 'world'").unwrap();
        assert_eq!(result.as_str(), Some("hello world"));
    }

    #[test]
    fn test_object_creation() {
        let runtime = QuickJsRuntime::new().unwrap();
        let result = runtime.eval("({name: 'test', count: 42})").unwrap();
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("test"));
        assert_eq!(obj.get("count").and_then(|v| v.as_i64()), Some(42));
    }

    #[test]
    fn test_function_call() {
        let runtime = QuickJsRuntime::new().unwrap();
        let result = runtime
            .call_function(
                "function(a, b) { return a * b; }",
                &[JsValue::Integer(6), JsValue::Integer(7)],
            )
            .unwrap();
        assert_eq!(result.as_i64(), Some(42));
    }

    #[test]
    fn test_where_expression() {
        let runtime = QuickJsRuntime::new().unwrap();

        let doc = JsValue::Object(
            [
                ("age".to_string(), JsValue::Integer(25)),
                ("status".to_string(), JsValue::String("active".to_string())),
            ]
            .into_iter()
            .collect(),
        );

        // Should match
        let result = runtime
            .eval_where("this.age > 21 && this.status == 'active'", &doc)
            .unwrap();
        assert!(result);

        // Should not match
        let result = runtime.eval_where("this.age > 30", &doc).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_redis_script() {
        let runtime = QuickJsRuntime::new().unwrap();

        let result = runtime
            .eval_redis_script(
                "KEYS[0] + ':' + ARGV[0]",
                &["mykey".to_string()],
                &[JsValue::String("value".to_string())],
            )
            .unwrap();

        assert_eq!(result.as_str(), Some("mykey:value"));
    }

    #[test]
    fn test_script_caching() {
        let runtime = QuickJsRuntime::new().unwrap();

        let script = "KEYS[0] + ARGV[0]";
        let sha = runtime.cache_script(script).unwrap();

        let result = runtime
            .eval_sha(
                &sha,
                &["key".to_string()],
                &[JsValue::String("val".to_string())],
            )
            .unwrap();

        assert_eq!(result.as_str(), Some("keyval"));
    }

    #[test]
    fn test_mongo_where_operator() {
        let mongo = MongoJsOperator::new().unwrap();

        let doc = JsValue::Object(
            [("price".to_string(), JsValue::Float(99.99))]
                .into_iter()
                .collect(),
        );

        assert!(mongo.eval_where("this.price < 100", &doc).unwrap());
        assert!(!mongo.eval_where("this.price > 100", &doc).unwrap());
    }

    #[test]
    fn test_redis_executor() {
        let redis = RedisScriptExecutor::new().unwrap();

        let result = redis
            .eval(
                "return KEYS.length + ARGV.length",
                &["k1".to_string(), "k2".to_string()],
                &[
                    JsValue::Integer(1),
                    JsValue::Integer(2),
                    JsValue::Integer(3),
                ],
            )
            .unwrap();

        assert_eq!(result.as_i64(), Some(5)); // 2 keys + 3 args
    }
}
