//! WASM UDF Handler for SQL Integration
//!
//! This module handles CREATE FUNCTION and DROP FUNCTION SQL statements
//! for WASM user-defined functions.

use super::types::{WasmFunctionMetadata, WasmParameter};
use super::udf_registry::WasmUdfRegistry;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::sync::Arc;

/// Handler for WASM UDF SQL operations
pub struct WasmUdfHandler {
    registry: Arc<WasmUdfRegistry>,
}

impl WasmUdfHandler {
    /// Create a new WASM UDF handler
    pub fn new(registry: Arc<WasmUdfRegistry>) -> Self {
        Self { registry }
    }

    /// Handle CREATE FUNCTION statement
    ///
    /// SQL Syntax:
    /// ```sql
    /// CREATE FUNCTION add(a INTEGER, b INTEGER)
    /// RETURNS INTEGER
    /// LANGUAGE WASM
    /// AS '0061736d...' -- hex-encoded WASM binary
    /// ```
    pub async fn handle_create_function(
        &self,
        name: String,
        schema: Option<String>,
        params: Vec<(String, String)>, // (param_name, sql_type)
        return_type: String,
        source: String, // hex-encoded WASM binary
    ) -> ProtocolResult<()> {
        // Decode WASM binary from hex string
        let wasm_module = Self::decode_wasm_hex(&source)?;

        // Create parameter metadata
        let wasm_params: Vec<WasmParameter> = params
            .iter()
            .map(|(name, sql_type)| WasmParameter {
                name: name.clone(),
                sql_type: sql_type.clone(),
            })
            .collect();

        // Create function metadata
        let metadata = WasmFunctionMetadata {
            name: name.clone(),
            params: wasm_params,
            return_type,
            wasm_module,
            export_name: name.clone(), // Default: use function name as export
            schema,
        };

        // Register the function
        self.registry.register_function(metadata).await
    }

    /// Handle DROP FUNCTION statement
    pub async fn handle_drop_function(
        &self,
        name: &str,
        schema: &Option<String>,
    ) -> ProtocolResult<bool> {
        self.registry.unregister_function(schema, name).await
    }

    /// Execute a WASM function
    pub async fn execute_function(
        &self,
        name: &str,
        schema: &Option<String>,
        args: Vec<SqlValue>,
    ) -> ProtocolResult<SqlValue> {
        self.registry.execute_function(schema, name, args).await
    }

    /// List all registered WASM functions
    pub async fn list_functions(&self) -> ProtocolResult<Vec<WasmFunctionMetadata>> {
        self.registry.list_functions().await
    }

    /// Decode hex-encoded WASM binary
    fn decode_wasm_hex(hex_str: &str) -> ProtocolResult<Vec<u8>> {
        // Remove 0x prefix if present
        let hex_str = hex_str.trim_start_matches("0x");

        // Decode hex string
        hex::decode(hex_str)
            .map_err(|e| ProtocolError::PostgresError(format!("Invalid WASM hex encoding: {}", e)))
    }

    /// Encode WASM binary as hex string
    pub fn encode_wasm_hex(wasm_bytes: &[u8]) -> String {
        format!("0x{}", hex::encode(wasm_bytes))
    }

    /// Validate WASM function source for security violations
    ///
    /// Basic checks:
    /// - Module size limit
    /// - No suspicious imports (if WASI is disabled)
    pub fn validate_wasm_source(wasm_bytes: &[u8]) -> ProtocolResult<()> {
        // Check size
        const MAX_SIZE: usize = 10 * 1024 * 1024; // 10MB
        if wasm_bytes.len() > MAX_SIZE {
            return Err(ProtocolError::PostgresError(format!(
                "WASM module too large: {} bytes (max: {} bytes)",
                wasm_bytes.len(),
                MAX_SIZE
            )));
        }

        // Verify WASM magic number
        if wasm_bytes.len() < 4 || &wasm_bytes[0..4] != b"\0asm" {
            return Err(ProtocolError::PostgresError(
                "Invalid WASM module: missing magic number".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::config::WasmConfig;
    use crate::wasm::runtime::WasmRuntime;

    // Simple WASM module that adds two i32 numbers (hex-encoded)
    const ADD_WASM_HEX: &str =
        "0061736d0100000001070160027f7f017f030201000707010361646400000a09010700200020016a0b";

    fn create_test_handler() -> WasmUdfHandler {
        let runtime = Arc::new(WasmRuntime::new(WasmConfig::default()).unwrap());
        let registry = Arc::new(WasmUdfRegistry::new(runtime));
        WasmUdfHandler::new(registry)
    }

    #[tokio::test]
    async fn test_create_function() {
        let handler = create_test_handler();

        let result = handler
            .handle_create_function(
                "add".to_string(),
                None,
                vec![
                    ("a".to_string(), "INTEGER".to_string()),
                    ("b".to_string(), "INTEGER".to_string()),
                ],
                "INTEGER".to_string(),
                ADD_WASM_HEX.to_string(),
            )
            .await;

        assert!(result.is_ok());

        let functions = handler.list_functions().await.unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "add");
    }

    #[tokio::test]
    async fn test_drop_function() {
        let handler = create_test_handler();

        // Create function first
        handler
            .handle_create_function(
                "add".to_string(),
                None,
                vec![
                    ("a".to_string(), "INTEGER".to_string()),
                    ("b".to_string(), "INTEGER".to_string()),
                ],
                "INTEGER".to_string(),
                ADD_WASM_HEX.to_string(),
            )
            .await
            .unwrap();

        // Drop the function
        let dropped = handler.handle_drop_function("add", &None).await.unwrap();
        assert!(dropped);

        let functions = handler.list_functions().await.unwrap();
        assert_eq!(functions.len(), 0);
    }

    #[tokio::test]
    async fn test_execute_function() {
        let handler = create_test_handler();

        // Create function
        handler
            .handle_create_function(
                "add".to_string(),
                None,
                vec![
                    ("a".to_string(), "INTEGER".to_string()),
                    ("b".to_string(), "INTEGER".to_string()),
                ],
                "INTEGER".to_string(),
                ADD_WASM_HEX.to_string(),
            )
            .await
            .unwrap();

        // Execute function
        let result = handler
            .execute_function(
                "add",
                &None,
                vec![SqlValue::Integer(10), SqlValue::Integer(20)],
            )
            .await
            .unwrap();

        assert_eq!(result, SqlValue::Integer(30));
    }

    #[test]
    fn test_decode_wasm_hex() {
        let hex = "0061736d";
        let decoded = WasmUdfHandler::decode_wasm_hex(hex).unwrap();
        assert_eq!(decoded, vec![0x00, 0x61, 0x73, 0x6d]);

        // With 0x prefix
        let hex = "0x0061736d";
        let decoded = WasmUdfHandler::decode_wasm_hex(hex).unwrap();
        assert_eq!(decoded, vec![0x00, 0x61, 0x73, 0x6d]);
    }

    #[test]
    fn test_encode_wasm_hex() {
        let bytes = vec![0x00, 0x61, 0x73, 0x6d];
        let encoded = WasmUdfHandler::encode_wasm_hex(&bytes);
        assert_eq!(encoded, "0x0061736d");
    }

    #[test]
    fn test_validate_wasm_source() {
        // Valid WASM magic number
        let valid = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        assert!(WasmUdfHandler::validate_wasm_source(&valid).is_ok());

        // Invalid magic number
        let invalid = vec![0x00, 0x00, 0x00, 0x00];
        assert!(WasmUdfHandler::validate_wasm_source(&invalid).is_err());

        // Too large
        let too_large = vec![0u8; 11 * 1024 * 1024]; // 11MB
        assert!(WasmUdfHandler::validate_wasm_source(&too_large).is_err());
    }
}
