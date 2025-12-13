//! WASM Runtime Management
//!
//! This module provides a runtime for executing WASM modules with sandboxing,
//! resource limits, and caching for performance.

use super::config::WasmConfig;
use super::types::WasmValue;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use wasmtime::*;

/// WASM runtime error
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    #[error("Compilation error: {0}")]
    CompilationError(String),

    #[error("Instantiation error: {0}")]
    InstantiationError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Timeout error: function exceeded {0}ms")]
    TimeoutError(u64),

    #[error("Out of fuel: function exceeded instruction limit")]
    OutOfFuelError,

    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,

    #[error("Type conversion error: {0}")]
    TypeConversionError(String),

    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Invalid WASM module: {0}")]
    InvalidModule(String),
}

impl From<WasmError> for ProtocolError {
    fn from(err: WasmError) -> Self {
        ProtocolError::PostgresError(err.to_string())
    }
}

/// Compiled WASM module cache entry
struct CachedModule {
    module: Module,
    compiled_at: Instant,
}

/// WASM runtime for executing user-defined functions
pub struct WasmRuntime {
    config: WasmConfig,
    engine: Engine,
    /// LRU cache of compiled modules (keyed by WASM binary hash)
    module_cache: Arc<RwLock<LruCache<u64, CachedModule>>>,
}

impl WasmRuntime {
    /// Create a new WASM runtime with the given configuration
    pub fn new(config: WasmConfig) -> ProtocolResult<Self> {
        config.validate().map_err(|e| {
            ProtocolError::PostgresError(format!("Invalid WASM config: {}", e))
        })?;

        // Configure the WASM engine
        let mut wasm_config = Config::new();
        wasm_config.consume_fuel(config.enable_fuel);
        wasm_config.async_support(true);
        wasm_config.epoch_interruption(true);

        // Enable SIMD for vectorized operations
        wasm_config.wasm_simd(config.enable_simd);

        // Set memory limits
        wasm_config.max_wasm_stack(1024 * 1024); // 1MB stack

        let engine = Engine::new(&wasm_config).map_err(|e| {
            ProtocolError::PostgresError(format!("Failed to create WASM engine: {}", e))
        })?;

        let cache_size = NonZeroUsize::new(config.cache_size)
            .ok_or_else(|| ProtocolError::PostgresError("Invalid cache size".to_string()))?;

        Ok(Self {
            config,
            engine,
            module_cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
        })
    }

    /// Create runtime with default configuration
    pub fn new_default() -> ProtocolResult<Self> {
        Self::new(WasmConfig::default())
    }

    /// Compile a WASM module (with caching)
    pub async fn compile_module(&self, wasm_binary: &[u8]) -> Result<Module, WasmError> {
        // Validate module size
        if wasm_binary.len() > self.config.max_module_size {
            return Err(WasmError::InvalidModule(format!(
                "Module size {} exceeds limit {}",
                wasm_binary.len(),
                self.config.max_module_size
            )));
        }

        // Calculate hash for caching
        let hash = calculate_hash(wasm_binary);

        // Check cache
        if self.config.enable_cache {
            let cache = self.module_cache.read().await;
            if let Some(cached) = cache.peek(&hash) {
                return Ok(cached.module.clone());
            }
        }

        // Compile the module
        let module = Module::new(&self.engine, wasm_binary).map_err(|e| {
            WasmError::CompilationError(format!("Failed to compile WASM module: {}", e))
        })?;

        // Cache the compiled module
        if self.config.enable_cache {
            let mut cache = self.module_cache.write().await;
            cache.put(
                hash,
                CachedModule {
                    module: module.clone(),
                    compiled_at: Instant::now(),
                },
            );
        }

        Ok(module)
    }

    /// Execute a WASM function with the given arguments
    pub async fn execute(
        &self,
        wasm_binary: &[u8],
        export_name: &str,
        args: Vec<WasmValue>,
    ) -> Result<WasmValue, WasmError> {
        // Compile the module
        let module = self.compile_module(wasm_binary).await?;

        // Create store limits
        let limits = StoreLimitsBuilder::new()
            .memory_size(self.config.max_memory_bytes)
            .build();

        // Create a new store for this execution with limits
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|data| data);

        // Set fuel limit if enabled
        if self.config.enable_fuel {
            store
                .set_fuel(self.config.fuel_limit)
                .map_err(|e| WasmError::ExecutionError(format!("Failed to set fuel: {}", e)))?;
        }

        // Instantiate the module (async for better performance)
        let instance = Instance::new_async(&mut store, &module, &[]).await.map_err(|e| {
            WasmError::InstantiationError(format!("Failed to instantiate module: {}", e))
        })?;

        // Get the exported function
        let func = instance
            .get_func(&mut store, export_name)
            .ok_or_else(|| WasmError::FunctionNotFound(export_name.to_string()))?;

        // Convert arguments to WASM values
        let wasm_args = self.convert_args_to_wasm(&args)?;

        // Execute with timeout
        let result = tokio::time::timeout(
            self.config.timeout,
            self.execute_func(&mut store, &func, &wasm_args),
        )
        .await
        .map_err(|_| WasmError::TimeoutError(self.config.timeout_millis()))?;

        result
    }

    /// Execute a WASM function (internal)
    async fn execute_func(
        &self,
        store: &mut Store<StoreLimits>,
        func: &Func,
        args: &[Val],
    ) -> Result<WasmValue, WasmError> {
        // Call the function (async version for better performance)
        let mut results = vec![Val::I32(0)]; // Placeholder for result

        // Use async call for non-blocking execution
        func.call_async(store, args, &mut results).await.map_err(|e| {
            // Check if it's an out-of-fuel error
            if e.to_string().contains("fuel") {
                WasmError::OutOfFuelError
            } else {
                WasmError::ExecutionError(format!("Function execution failed: {}", e))
            }
        })?;

        // Convert result back to WasmValue
        if results.is_empty() {
            return Ok(WasmValue::Null);
        }

        self.convert_wasm_result_to_value(&results[0])
    }

    /// Convert WasmValue arguments to wasmtime Val types
    fn convert_args_to_wasm(&self, args: &[WasmValue]) -> Result<Vec<Val>, WasmError> {
        args.iter()
            .map(|arg| match arg {
                WasmValue::Null => Ok(Val::I32(0)), // Represent null as 0
                WasmValue::Bool(b) => Ok(Val::I32(if *b { 1 } else { 0 })),
                WasmValue::I32(i) => Ok(Val::I32(*i)),
                WasmValue::I64(i) => Ok(Val::I64(*i)),
                WasmValue::F32(f) => Ok(Val::F32((*f).to_bits())),
                WasmValue::F64(f) => Ok(Val::F64((*f).to_bits())),
                _ => Err(WasmError::TypeConversionError(format!(
                    "Complex types must be serialized as bytes"
                ))),
            })
            .collect()
    }

    /// Convert wasmtime Val result to WasmValue
    fn convert_wasm_result_to_value(&self, val: &Val) -> Result<WasmValue, WasmError> {
        match val {
            Val::I32(i) => Ok(WasmValue::I32(*i)),
            Val::I64(i) => Ok(WasmValue::I64(*i)),
            Val::F32(bits) => Ok(WasmValue::F32(f32::from_bits(*bits))),
            Val::F64(bits) => Ok(WasmValue::F64(f64::from_bits(*bits))),
            _ => Err(WasmError::TypeConversionError(
                "Unsupported return type".to_string(),
            )),
        }
    }

    /// Clear the module cache
    pub async fn clear_cache(&self) {
        let mut cache = self.module_cache.write().await;
        cache.clear();
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.module_cache.read().await;
        (cache.len(), cache.cap().get())
    }
}

/// Calculate a simple hash for the WASM binary
fn calculate_hash(data: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple WASM module that adds two i32 numbers
    // Compiled from: fn add(a: i32, b: i32) -> i32 { a + b }
    const ADD_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f,
        0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
    ];

    #[tokio::test]
    async fn test_runtime_creation() {
        let runtime = WasmRuntime::new_default();
        assert!(runtime.is_ok());
    }

    #[tokio::test]
    async fn test_module_compilation() {
        let runtime = WasmRuntime::new_default().unwrap();
        let module = runtime.compile_module(ADD_WASM).await;
        assert!(module.is_ok());
    }

    #[tokio::test]
    async fn test_simple_execution() {
        let runtime = WasmRuntime::new_default().unwrap();
        let result = runtime
            .execute(ADD_WASM, "add", vec![WasmValue::I32(5), WasmValue::I32(3)])
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WasmValue::I32(8));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let runtime = WasmRuntime::new_default().unwrap();
        let (used, cap) = runtime.cache_stats().await;
        assert_eq!(used, 0);
        assert!(cap > 0);

        // Compile a module to populate cache
        let _ = runtime.compile_module(ADD_WASM).await;
        let (used, _) = runtime.cache_stats().await;
        assert_eq!(used, 1);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let runtime = WasmRuntime::new_default().unwrap();
        let _ = runtime.compile_module(ADD_WASM).await;

        runtime.clear_cache().await;
        let (used, _) = runtime.cache_stats().await;
        assert_eq!(used, 0);
    }

    #[tokio::test]
    async fn test_invalid_module() {
        let runtime = WasmRuntime::new_default().unwrap();
        let invalid_wasm = &[0x00, 0x01, 0x02, 0x03];
        let result = runtime.compile_module(invalid_wasm).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_function_not_found() {
        let runtime = WasmRuntime::new_default().unwrap();
        let result = runtime
            .execute(ADD_WASM, "nonexistent", vec![])
            .await;
        assert!(matches!(result, Err(WasmError::FunctionNotFound(_))));
    }

    #[tokio::test]
    async fn test_timeout() {
        let config = WasmConfig {
            timeout: std::time::Duration::from_millis(1),
            ..Default::default()
        };
        let runtime = WasmRuntime::new(config).unwrap();

        // This should timeout (if we had an infinite loop WASM module)
        // For now, just verify timeout configuration works
        assert!(runtime.config.timeout.as_millis() == 1);
    }
}
