//! Python UDF Handler for SQL Statements
//!
//! Handles CREATE FUNCTION and DROP FUNCTION statements for Python UDFs.

use super::types::{PythonError, PythonResult};
use super::udf_registry::{PythonUdfMetadata, PythonUdfRegistry};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::sync::Arc;

/// Extract Python function name from source code
fn extract_python_function_name(source: &str, default_name: &str) -> PythonResult<String> {
    // Look for "def function_name(" pattern
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ") {
            if let Some(end) = trimmed.find('(') {
                let func_name = trimmed[4..end].trim();
                if !func_name.is_empty() {
                    return Ok(func_name.to_string());
                }
            }
        }
    }

    // If no "def" found, use the default name
    Ok(default_name.to_string())
}

/// Validate Python source code for basic safety
fn validate_python_source(source: &str) -> PythonResult<()> {
    // Check source size
    if source.len() > 1_000_000 {
        return Err(PythonError::SecurityViolation(
            "Function source too large (max 1MB)".to_string(),
        ));
    }

    // Check for forbidden operations (basic checks)
    let forbidden_patterns = [
        ("eval(", "eval() is not allowed"),
        ("exec(", "exec() is not allowed"),
        ("__import__", "__import__ is not allowed"),
        ("compile(", "compile() is not allowed"),
        ("globals(", "globals() is not allowed"),
        ("locals(", "locals() access is restricted"),
        ("open(", "Direct file access is not allowed"),
    ];

    for (pattern, msg) in &forbidden_patterns {
        if source.contains(pattern) {
            return Err(PythonError::SecurityViolation(format!(
                "Security violation: {}",
                msg
            )));
        }
    }

    // Check for at least one function definition
    if !source.contains("def ") {
        return Err(PythonError::RuntimeError(
            "Source must contain at least one function definition".to_string(),
        ));
    }

    Ok(())
}

/// Handler for Python UDF SQL statements
pub struct PythonUdfHandler {
    registry: Arc<PythonUdfRegistry>,
}

impl PythonUdfHandler {
    /// Create a new Python UDF handler
    pub fn new(registry: Arc<PythonUdfRegistry>) -> Self {
        Self { registry }
    }

    /// Handle CREATE FUNCTION statement
    ///
    /// Expected format:
    /// CREATE FUNCTION function_name(param1 type1, param2 type2, ...)
    /// RETURNS return_type
    /// LANGUAGE PYTHON
    /// AS $$
    ///   Python code here
    /// $$;
    pub async fn handle_create_function(
        &self,
        name: String,
        params: Vec<(String, String)>, // (param_name, param_type)
        return_type: String,
        source: String,
        schema: Option<String>,
    ) -> PythonResult<()> {
        // Extract parameter types
        let param_types: Vec<String> = params.iter().map(|(_, t)| t.clone()).collect();

        // Extract Python function name from source
        let python_function_name = extract_python_function_name(&source, &name)?;

        // Validate source code
        validate_python_source(&source)?;

        // Create metadata
        let mut metadata =
            PythonUdfMetadata::new(name, source, python_function_name, param_types, return_type);

        metadata.schema = schema;

        // Register function
        self.registry.register(metadata).await?;

        Ok(())
    }

    /// Handle DROP FUNCTION statement
    pub async fn handle_drop_function(&self, name: &str) -> PythonResult<()> {
        self.registry.unregister(name).await
    }

    /// Execute a Python UDF
    pub async fn execute_function(
        &self,
        name: &str,
        args: Vec<SqlValue>,
    ) -> PythonResult<SqlValue> {
        self.registry.execute(name, args).await
    }

    /// Execute multiple Python UDFs in a batch
    pub async fn execute_batch(
        &self,
        calls: Vec<(String, Vec<SqlValue>)>,
    ) -> PythonResult<Vec<PythonResult<SqlValue>>> {
        self.registry.execute_batch(calls).await
    }

    /// Check if a function exists
    pub async fn function_exists(&self, name: &str) -> bool {
        self.registry.exists(name).await
    }

    /// List all registered functions
    pub async fn list_functions(&self) -> Vec<String> {
        self.registry.list_functions().await
    }

    /// Get function metadata
    pub async fn get_function_metadata(&self, name: &str) -> Option<PythonUdfMetadata> {
        self.registry.get_metadata(name).await
    }

    /// Health check worker processes
    pub async fn health_check(&self) -> Vec<bool> {
        self.registry.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::python::config::PythonConfig;

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_create_and_execute_function() {
        let config = PythonConfig::default();
        let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
        let handler = PythonUdfHandler::new(registry);

        // Create function
        handler
            .handle_create_function(
                "add_numbers".to_string(),
                vec![
                    ("a".to_string(), "INTEGER".to_string()),
                    ("b".to_string(), "INTEGER".to_string()),
                ],
                "INTEGER".to_string(),
                "def add_numbers(a, b):\n    return a + b".to_string(),
                None,
            )
            .await
            .unwrap();

        // Check existence
        assert!(handler.function_exists("add_numbers").await);

        // Execute function
        let result = handler
            .execute_function(
                "add_numbers",
                vec![SqlValue::Integer(10), SqlValue::Integer(20)],
            )
            .await
            .unwrap();

        assert_eq!(result, SqlValue::Integer(30));

        // Drop function
        handler.handle_drop_function("add_numbers").await.unwrap();
        assert!(!handler.function_exists("add_numbers").await);
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_function_with_schema() {
        let config = PythonConfig::default();
        let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
        let handler = PythonUdfHandler::new(registry);

        // Create function with schema
        handler
            .handle_create_function(
                "my_func".to_string(),
                vec![],
                "INTEGER".to_string(),
                "def my_func():\n    return 42".to_string(),
                Some("my_schema".to_string()),
            )
            .await
            .unwrap();

        // Check with qualified name
        assert!(handler.function_exists("my_schema.my_func").await);

        // Execute
        let result = handler
            .execute_function("my_schema.my_func", vec![])
            .await
            .unwrap();

        assert_eq!(result, SqlValue::Integer(42));
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_batch_execution() {
        let config = PythonConfig::default();
        let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
        let handler = PythonUdfHandler::new(registry);

        // Create multiple functions
        handler
            .handle_create_function(
                "double".to_string(),
                vec![("x".to_string(), "INTEGER".to_string())],
                "INTEGER".to_string(),
                "def double(x):\n    return x * 2".to_string(),
                None,
            )
            .await
            .unwrap();

        handler
            .handle_create_function(
                "triple".to_string(),
                vec![("x".to_string(), "INTEGER".to_string())],
                "INTEGER".to_string(),
                "def triple(x):\n    return x * 3".to_string(),
                None,
            )
            .await
            .unwrap();

        // Execute batch
        let batch = vec![
            ("double".to_string(), vec![SqlValue::Integer(5)]),
            ("triple".to_string(), vec![SqlValue::Integer(5)]),
        ];

        let results = handler.execute_batch(batch).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].as_ref().unwrap(), &SqlValue::Integer(10));
        assert_eq!(results[1].as_ref().unwrap(), &SqlValue::Integer(15));
    }

    #[test]
    fn test_extract_function_name() {
        let source1 = "def my_function(a, b):\n    return a + b";
        assert_eq!(
            extract_python_function_name(source1, "default").unwrap(),
            "my_function"
        );

        let source2 = "  def another_func(x):  \n    pass";
        assert_eq!(
            extract_python_function_name(source2, "default").unwrap(),
            "another_func"
        );

        let source3 = "# No function definition here";
        assert_eq!(
            extract_python_function_name(source3, "fallback").unwrap(),
            "fallback"
        );
    }

    #[test]
    fn test_validate_source() {
        // Valid source
        let valid = "def my_func():\n    return 42";
        assert!(validate_python_source(valid).is_ok());

        // Invalid: eval
        let invalid_eval = "def bad():\n    eval('print(1)')";
        assert!(validate_python_source(invalid_eval).is_err());

        // Invalid: exec
        let invalid_exec = "def bad():\n    exec('x = 1')";
        assert!(validate_python_source(invalid_exec).is_err());

        // Invalid: no function
        let no_func = "x = 1\ny = 2";
        assert!(validate_python_source(no_func).is_err());

        // Invalid: __import__
        let invalid_import = "def bad():\n    os = __import__('os')";
        assert!(validate_python_source(invalid_import).is_err());
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_list_functions() {
        let config = PythonConfig::default();
        let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
        let handler = PythonUdfHandler::new(registry);

        // Initially empty
        assert_eq!(handler.list_functions().await.len(), 0);

        // Create functions
        handler
            .handle_create_function(
                "func1".to_string(),
                vec![],
                "INTEGER".to_string(),
                "def func1():\n    return 1".to_string(),
                None,
            )
            .await
            .unwrap();

        handler
            .handle_create_function(
                "func2".to_string(),
                vec![],
                "INTEGER".to_string(),
                "def func2():\n    return 2".to_string(),
                None,
            )
            .await
            .unwrap();

        // List functions
        let funcs = handler.list_functions().await;
        assert_eq!(funcs.len(), 2);
        assert!(funcs.contains(&"func1".to_string()));
        assert!(funcs.contains(&"func2".to_string()));
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_get_metadata() {
        let config = PythonConfig::default();
        let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
        let handler = PythonUdfHandler::new(registry);

        // Create function
        handler
            .handle_create_function(
                "test_func".to_string(),
                vec![("x".to_string(), "INTEGER".to_string())],
                "TEXT".to_string(),
                "def test_func(x):\n    return str(x)".to_string(),
                None,
            )
            .await
            .unwrap();

        // Get metadata
        let metadata = handler.get_function_metadata("test_func").await.unwrap();
        assert_eq!(metadata.name, "test_func");
        assert_eq!(metadata.param_types, vec!["INTEGER".to_string()]);
        assert_eq!(metadata.return_type, "TEXT");
    }
}
