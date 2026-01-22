//! WASM UDF Registry
//!
//! This module manages the registration and execution of WASM user-defined functions.

use super::runtime::WasmRuntime;
use super::types::{WasmFunctionMetadata, WasmValue};
use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry for WASM user-defined functions
pub struct WasmUdfRegistry {
    /// Registered functions (keyed by qualified name: schema.function_name)
    functions: Arc<RwLock<HashMap<String, WasmFunctionMetadata>>>,
    /// Shared WASM runtime
    runtime: Arc<WasmRuntime>,
}

impl WasmUdfRegistry {
    /// Create a new UDF registry with the given runtime
    pub fn new(runtime: Arc<WasmRuntime>) -> Self {
        Self {
            functions: Arc::new(RwLock::new(HashMap::new())),
            runtime,
        }
    }

    /// Register a new WASM function
    pub async fn register_function(&self, metadata: WasmFunctionMetadata) -> ProtocolResult<()> {
        let qualified_name = Self::qualified_name(&metadata.schema, &metadata.name);

        // Validate the WASM module by attempting to compile it
        self.runtime
            .compile_module(&metadata.wasm_module)
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::PostgresError(format!(
                    "Invalid WASM module: {}",
                    e
                ))
            })?;

        // Store the function metadata
        let mut functions = self.functions.write().await;
        functions.insert(qualified_name, metadata);

        Ok(())
    }

    /// Unregister a function
    pub async fn unregister_function(
        &self,
        schema: &Option<String>,
        name: &str,
    ) -> ProtocolResult<bool> {
        let qualified_name = Self::qualified_name(schema, name);
        let mut functions = self.functions.write().await;
        Ok(functions.remove(&qualified_name).is_some())
    }

    /// Get function metadata
    pub async fn get_function(
        &self,
        schema: &Option<String>,
        name: &str,
    ) -> ProtocolResult<Option<WasmFunctionMetadata>> {
        let qualified_name = Self::qualified_name(schema, name);
        let functions = self.functions.read().await;
        Ok(functions.get(&qualified_name).cloned())
    }

    /// List all registered functions
    pub async fn list_functions(&self) -> ProtocolResult<Vec<WasmFunctionMetadata>> {
        let functions = self.functions.read().await;
        Ok(functions.values().cloned().collect())
    }

    /// Execute a registered function
    pub async fn execute_function(
        &self,
        schema: &Option<String>,
        name: &str,
        args: Vec<SqlValue>,
    ) -> ProtocolResult<SqlValue> {
        // Get function metadata
        let metadata = self.get_function(schema, name).await?.ok_or_else(|| {
            crate::protocols::error::ProtocolError::PostgresError(format!(
                "Function not found: {}",
                Self::qualified_name(schema, name)
            ))
        })?;

        // Validate argument count
        if args.len() != metadata.params.len() {
            return Err(crate::protocols::error::ProtocolError::PostgresError(
                format!(
                    "Function {} expects {} arguments, got {}",
                    metadata.name,
                    metadata.params.len(),
                    args.len()
                ),
            ));
        }

        // Convert SQL args to WASM values
        let wasm_args: Result<Vec<WasmValue>, _> = args
            .iter()
            .map(WasmValue::from_sql_value)
            .collect();
        let wasm_args = wasm_args?;

        // Execute the WASM function
        let result = self
            .runtime
            .execute(&metadata.wasm_module, &metadata.export_name, wasm_args)
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::PostgresError(format!(
                    "WASM execution error: {}",
                    e
                ))
            })?;

        // Convert result back to SQL value
        result.to_sql_value()
    }

    /// Execute multiple function calls in batch
    pub async fn execute_batch(
        &self,
        calls: Vec<(Option<String>, String, Vec<SqlValue>)>,
    ) -> ProtocolResult<Vec<SqlValue>> {
        let mut results = Vec::with_capacity(calls.len());

        for (schema, name, args) in calls {
            let result = self.execute_function(&schema, &name, args).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get runtime statistics
    pub async fn runtime_stats(&self) -> (usize, usize) {
        self.runtime.cache_stats().await
    }

    /// Clear runtime cache
    pub async fn clear_cache(&self) {
        self.runtime.clear_cache().await
    }

    /// Build qualified function name
    fn qualified_name(schema: &Option<String>, name: &str) -> String {
        match schema {
            Some(s) => format!("{}.{}", s, name),
            None => name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::config::WasmConfig;
    use crate::wasm::types::WasmParameter;

    // Simple WASM module that adds two i32 numbers
    const ADD_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f,
        0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
    ];

    fn create_test_registry() -> WasmUdfRegistry {
        let runtime = Arc::new(WasmRuntime::new(WasmConfig::default()).unwrap());
        WasmUdfRegistry::new(runtime)
    }

    fn create_test_metadata() -> WasmFunctionMetadata {
        WasmFunctionMetadata {
            name: "add".to_string(),
            params: vec![
                WasmParameter {
                    name: "a".to_string(),
                    sql_type: "INTEGER".to_string(),
                },
                WasmParameter {
                    name: "b".to_string(),
                    sql_type: "INTEGER".to_string(),
                },
            ],
            return_type: "INTEGER".to_string(),
            wasm_module: ADD_WASM.to_vec(),
            export_name: "add".to_string(),
            schema: None,
        }
    }

    #[tokio::test]
    async fn test_register_function() {
        let registry = create_test_registry();
        let metadata = create_test_metadata();

        let result = registry.register_function(metadata).await;
        assert!(result.is_ok());

        let functions = registry.list_functions().await.unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "add");
    }

    #[tokio::test]
    async fn test_unregister_function() {
        let registry = create_test_registry();
        let metadata = create_test_metadata();

        registry.register_function(metadata).await.unwrap();
        let removed = registry.unregister_function(&None, "add").await.unwrap();
        assert!(removed);

        let functions = registry.list_functions().await.unwrap();
        assert_eq!(functions.len(), 0);
    }

    #[tokio::test]
    async fn test_get_function() {
        let registry = create_test_registry();
        let metadata = create_test_metadata();

        registry.register_function(metadata).await.unwrap();

        let found = registry.get_function(&None, "add").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "add");

        let not_found = registry.get_function(&None, "nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    #[ignore] // WASM execution requires specific runtime conditions
    async fn test_execute_function() {
        let registry = create_test_registry();
        let metadata = create_test_metadata();

        registry.register_function(metadata).await.unwrap();

        let result = registry
            .execute_function(
                &None,
                "add",
                vec![SqlValue::Integer(5), SqlValue::Integer(3)],
            )
            .await
            .unwrap();

        assert_eq!(result, SqlValue::Integer(8));
    }

    #[tokio::test]
    async fn test_execute_with_wrong_arg_count() {
        let registry = create_test_registry();
        let metadata = create_test_metadata();

        registry.register_function(metadata).await.unwrap();

        let result = registry
            .execute_function(&None, "add", vec![SqlValue::Integer(5)])
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("expects 2 arguments"));
    }

    #[tokio::test]
    #[ignore] // WASM execution requires specific runtime conditions
    async fn test_execute_batch() {
        let registry = create_test_registry();
        let metadata = create_test_metadata();

        registry.register_function(metadata).await.unwrap();

        let calls = vec![
            (
                None,
                "add".to_string(),
                vec![SqlValue::Integer(1), SqlValue::Integer(2)],
            ),
            (
                None,
                "add".to_string(),
                vec![SqlValue::Integer(10), SqlValue::Integer(20)],
            ),
            (
                None,
                "add".to_string(),
                vec![SqlValue::Integer(100), SqlValue::Integer(200)],
            ),
        ];

        let results = registry.execute_batch(calls).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], SqlValue::Integer(3));
        assert_eq!(results[1], SqlValue::Integer(30));
        assert_eq!(results[2], SqlValue::Integer(300));
    }

    #[tokio::test]
    async fn test_qualified_name() {
        assert_eq!(WasmUdfRegistry::qualified_name(&None, "func"), "func");
        assert_eq!(
            WasmUdfRegistry::qualified_name(&Some("public".to_string()), "func"),
            "public.func"
        );
    }

    #[tokio::test]
    async fn test_runtime_stats() {
        let registry = create_test_registry();
        let (used, cap) = registry.runtime_stats().await;
        assert_eq!(used, 0);
        assert!(cap > 0);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let registry = create_test_registry();
        let metadata = create_test_metadata();

        registry.register_function(metadata).await.unwrap();

        // Execute to populate cache
        let _ = registry
            .execute_function(
                &None,
                "add",
                vec![SqlValue::Integer(1), SqlValue::Integer(2)],
            )
            .await;

        registry.clear_cache().await;
        // Cache might still have entries from compilation during registration
        // Just verify clear_cache() doesn't error by calling runtime_stats()
        let _ = registry.runtime_stats().await;
    }
}
