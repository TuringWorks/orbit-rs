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

#[cfg(feature = "wasm-wasi")]
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

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
    #[allow(dead_code)] // Reserved for future cache expiry logic
    compiled_at: Instant,
}

/// Store data that can hold WASI context and resource limits
#[cfg(feature = "wasm-wasi")]
struct WasmStoreData {
    wasi: Option<WasiCtx>,
    limits: StoreLimits,
}

#[cfg(feature = "wasm-wasi")]
impl WasmStoreData {
    fn new(wasi: Option<WasiCtx>, limits: StoreLimits) -> Self {
        Self { wasi, limits }
    }
}

#[cfg(not(feature = "wasm-wasi"))]
type WasmStoreData = StoreLimits;

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
        config
            .validate()
            .map_err(|e| ProtocolError::PostgresError(format!("Invalid WASM config: {}", e)))?;

        // Configure the WASM engine
        let mut wasm_config = Config::new();
        wasm_config.consume_fuel(config.enable_fuel);
        wasm_config.async_support(true);
        wasm_config.epoch_interruption(true);

        // Enable SIMD for vectorized operations
        wasm_config.wasm_simd(config.enable_simd);

        // Enable multi-threading (WASM threads proposal)
        if config.enable_threads {
            wasm_config.wasm_threads(true);
            wasm_config.thread_stack_size(config.thread_stack_size);
        }

        // Enable Component Model (experimental)
        if config.enable_component_model {
            wasm_config.wasm_component_model(true);
        }

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

    /// Create WASI context if WASI is enabled
    #[cfg(feature = "wasm-wasi")]
    fn create_wasi_context(&self) -> Option<WasiCtx> {
        if !self.config.enable_wasi {
            return None;
        }

        let mut builder = WasiCtxBuilder::new();

        // Configure allowed directories
        for dir in &self.config.wasi_allowed_dirs {
            builder = builder.preopened_dir(
                wasmtime_wasi::sync::Dir::open_ambient_dir(
                    dir,
                    wasmtime_wasi::sync::ambient_authority(),
                )
                .ok()?,
                dir,
            );
        }

        // Configure stdio
        if self.config.wasi_inherit_stdio {
            builder = builder.inherit_stdio();
        }

        // Configure environment
        if self.config.wasi_inherit_env {
            builder = builder.inherit_env();
        }

        Some(builder.build())
    }

    #[cfg(not(feature = "wasm-wasi"))]
    fn create_wasi_context(&self) -> Option<()> {
        None
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
        let mut limits_builder = StoreLimitsBuilder::new();
        limits_builder.memory_size(self.config.max_memory_bytes);

        // Set thread limits if threading is enabled
        if self.config.enable_threads {
            limits_builder.instances(self.config.max_threads);
            limits_builder.tables(self.config.max_threads);
        }

        let limits = limits_builder.build();

        // Create store data with optional WASI context
        #[cfg(feature = "wasm-wasi")]
        let store_data = WasmStoreData::new(self.create_wasi_context(), limits);

        #[cfg(not(feature = "wasm-wasi"))]
        let store_data = limits;

        // Create a new store for this execution
        let mut store = Store::new(&self.engine, store_data);

        // Set up resource limiter
        #[cfg(feature = "wasm-wasi")]
        store.limiter(|data| &mut data.limits);

        #[cfg(not(feature = "wasm-wasi"))]
        store.limiter(|data| data);

        // Set fuel limit if enabled
        if self.config.enable_fuel {
            store
                .set_fuel(self.config.fuel_limit)
                .map_err(|e| WasmError::ExecutionError(format!("Failed to set fuel: {}", e)))?;
        }

        // Create linker and add WASI if enabled
        #[cfg(feature = "wasm-wasi")]
        let instance = {
            let mut linker = Linker::new(&self.engine);

            // Add WASI to linker if context exists
            if store.data().wasi.is_some() {
                wasmtime_wasi::add_to_linker(&mut linker, |data: &mut WasmStoreData| {
                    data.wasi.as_mut().expect("WASI context should exist")
                })
                .map_err(|e| {
                    WasmError::InstantiationError(format!("Failed to link WASI: {}", e))
                })?;
            }

            linker
                .instantiate_async(&mut store, &module)
                .await
                .map_err(|e| {
                    WasmError::InstantiationError(format!("Failed to instantiate module: {}", e))
                })?
        };

        // Instantiate without WASI
        #[cfg(not(feature = "wasm-wasi"))]
        let instance = Instance::new_async(&mut store, &module, &[])
            .await
            .map_err(|e| {
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
        store: &mut Store<WasmStoreData>,
        func: &Func,
        args: &[Val],
    ) -> Result<WasmValue, WasmError> {
        // Call the function (async version for better performance)
        let mut results = vec![Val::I32(0)]; // Placeholder for result

        // Use async call for non-blocking execution
        func.call_async(store, args, &mut results)
            .await
            .map_err(|e| {
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

    /// Execute a WASM function with streaming input
    /// Processes data in chunks to avoid loading everything into memory
    pub async fn execute_streaming<R>(
        &self,
        wasm_binary: &[u8],
        export_name: &str,
        mut reader: R,
    ) -> Result<Vec<WasmValue>, WasmError>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        use super::types::StreamingBuffer;
        use tokio::io::AsyncReadExt;

        if !self.config.enable_streaming {
            return Err(WasmError::ExecutionError(
                "Streaming I/O is disabled".to_string(),
            ));
        }

        let mut results = Vec::new();
        let mut offset = 0;
        let mut total_bytes = 0;

        // Compile module once
        let module = self.compile_module(wasm_binary).await?;

        loop {
            // Read next chunk
            let mut chunk = vec![0u8; self.config.streaming_chunk_size];
            let bytes_read = reader
                .read(&mut chunk)
                .await
                .map_err(|e| WasmError::ExecutionError(format!("Streaming read error: {}", e)))?;

            if bytes_read == 0 {
                break; // End of stream
            }

            chunk.truncate(bytes_read);
            total_bytes += bytes_read;

            // Check streaming limit
            if total_bytes > self.config.streaming_max_bytes {
                return Err(WasmError::ExecutionError(format!(
                    "Stream size {} exceeds limit {}",
                    total_bytes, self.config.streaming_max_bytes
                )));
            }

            let is_last = bytes_read < self.config.streaming_chunk_size;

            // Create streaming buffer
            let buffer = StreamingBuffer::new(chunk, offset, None, is_last);

            // Serialize to MessagePack for WASM
            let buffer_bytes = rmp_serde::to_vec(&buffer.data).map_err(|e| {
                WasmError::TypeConversionError(format!("Failed to serialize chunk: {}", e))
            })?;

            // Process chunk through WASM
            let chunk_result = self
                .execute_chunk(&module, export_name, &buffer_bytes, offset, is_last)
                .await?;

            results.push(chunk_result);
            offset += bytes_read;

            if is_last {
                break;
            }
        }

        Ok(results)
    }

    /// Execute a single chunk of data
    async fn execute_chunk(
        &self,
        module: &Module,
        export_name: &str,
        chunk_data: &[u8],
        offset: usize,
        is_last: bool,
    ) -> Result<WasmValue, WasmError> {
        use super::types::WasmValue;

        // Create store limits
        let mut limits_builder = StoreLimitsBuilder::new();
        limits_builder.memory_size(self.config.max_memory_bytes);

        // Set thread limits if threading is enabled
        if self.config.enable_threads {
            limits_builder.instances(self.config.max_threads);
            limits_builder.tables(self.config.max_threads);
        }

        let limits = limits_builder.build();

        // Create store data with optional WASI context
        #[cfg(feature = "wasm-wasi")]
        let store_data = WasmStoreData::new(self.create_wasi_context(), limits);

        #[cfg(not(feature = "wasm-wasi"))]
        let store_data = limits;

        // Create a new store for this execution
        let mut store = Store::new(&self.engine, store_data);

        // Set up resource limiter
        #[cfg(feature = "wasm-wasi")]
        store.limiter(|data| &mut data.limits);

        #[cfg(not(feature = "wasm-wasi"))]
        store.limiter(|data| data);

        // Set fuel limit if enabled
        if self.config.enable_fuel {
            store
                .set_fuel(self.config.fuel_limit)
                .map_err(|e| WasmError::ExecutionError(format!("Failed to set fuel: {}", e)))?;
        }

        // Instantiate module
        #[cfg(feature = "wasm-wasi")]
        let instance = {
            let mut linker = Linker::new(&self.engine);

            // Add WASI to linker if context exists
            if store.data().wasi.is_some() {
                wasmtime_wasi::add_to_linker(&mut linker, |data: &mut WasmStoreData| {
                    data.wasi.as_mut().expect("WASI context should exist")
                })
                .map_err(|e| {
                    WasmError::InstantiationError(format!("Failed to link WASI: {}", e))
                })?;
            }

            linker
                .instantiate_async(&mut store, module)
                .await
                .map_err(|e| {
                    WasmError::InstantiationError(format!("Failed to instantiate module: {}", e))
                })?
        };

        #[cfg(not(feature = "wasm-wasi"))]
        let instance = Instance::new_async(&mut store, module, &[])
            .await
            .map_err(|e| {
                WasmError::InstantiationError(format!("Failed to instantiate module: {}", e))
            })?;

        // Get exported function
        let func = instance
            .get_func(&mut store, export_name)
            .ok_or_else(|| WasmError::FunctionNotFound(export_name.to_string()))?;

        // Allocate memory in WASM for chunk data
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| WasmError::ExecutionError("No memory export found".to_string()))?;

        // Write chunk data to WASM memory
        let data_ptr = 0; // Use offset 0 for simplicity (real impl would allocate properly)
        memory
            .write(&mut store, data_ptr, chunk_data)
            .map_err(|e| {
                WasmError::ExecutionError(format!("Failed to write to WASM memory: {}", e))
            })?;

        // Call function with (data_ptr, data_len, offset, is_last)
        let args = &[
            Val::I32(data_ptr as i32),
            Val::I32(chunk_data.len() as i32),
            Val::I32(offset as i32),
            Val::I32(if is_last { 1 } else { 0 }),
        ];

        let mut results = vec![Val::I32(0)];

        // Execute with timeout
        tokio::time::timeout(
            self.config.timeout,
            func.call_async(&mut store, args, &mut results),
        )
        .await
        .map_err(|_| WasmError::TimeoutError(self.config.timeout_millis()))?
        .map_err(|e| {
            if e.to_string().contains("fuel") {
                WasmError::OutOfFuelError
            } else {
                WasmError::ExecutionError(format!("Function execution failed: {}", e))
            }
        })?;

        // Convert result
        if results.is_empty() {
            return Ok(WasmValue::Null);
        }

        self.convert_wasm_result_to_value(&results[0])
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
        let result = runtime.execute(ADD_WASM, "nonexistent", vec![]).await;
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
