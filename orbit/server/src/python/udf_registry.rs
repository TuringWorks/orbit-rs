//! Python UDF Registry
//!
//! Manages Python User-Defined Functions (UDFs) with metadata and execution routing.

use super::config::PythonConfig;
use super::runtime::PythonRuntimePool;
use super::types::{PythonError, PythonResult, PythonValue};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Metadata for a Python UDF
#[derive(Debug, Clone)]
pub struct PythonUdfMetadata {
    /// Function name
    pub name: String,

    /// Python function source code
    pub source: String,

    /// Python function name (may differ from SQL name)
    pub python_function_name: String,

    /// Parameter types (SQL types)
    pub param_types: Vec<String>,

    /// Return type (SQL type)
    pub return_type: String,

    /// Whether the function is volatile (non-deterministic)
    pub is_volatile: bool,

    /// Optional schema/namespace
    pub schema: Option<String>,
}

impl PythonUdfMetadata {
    /// Create new UDF metadata
    pub fn new(
        name: String,
        source: String,
        python_function_name: String,
        param_types: Vec<String>,
        return_type: String,
    ) -> Self {
        Self {
            name,
            source,
            python_function_name,
            param_types,
            return_type,
            is_volatile: true, // Default to volatile for safety
            schema: None,
        }
    }

    /// Get fully qualified name (schema.name or just name)
    pub fn qualified_name(&self) -> String {
        if let Some(ref schema) = self.schema {
            format!("{}.{}", schema, self.name)
        } else {
            self.name.clone()
        }
    }

    /// Validate parameter count
    pub fn validate_param_count(&self, arg_count: usize) -> PythonResult<()> {
        if arg_count != self.param_types.len() {
            return Err(PythonError::RuntimeError(format!(
                "Function {} expects {} arguments, got {}",
                self.name,
                self.param_types.len(),
                arg_count
            )));
        }
        Ok(())
    }
}

/// Registry for Python UDFs
pub struct PythonUdfRegistry {
    /// Registered functions (name -> metadata)
    functions: Arc<RwLock<HashMap<String, PythonUdfMetadata>>>,

    /// Python runtime pool for execution
    runtime_pool: Arc<PythonRuntimePool>,
}

impl PythonUdfRegistry {
    /// Create a new UDF registry with runtime pool
    pub async fn new(config: PythonConfig) -> PythonResult<Self> {
        let runtime_pool = PythonRuntimePool::new(config).await?;

        Ok(Self {
            functions: Arc::new(RwLock::new(HashMap::new())),
            runtime_pool: Arc::new(runtime_pool),
        })
    }

    /// Register a Python UDF
    pub async fn register(&self, metadata: PythonUdfMetadata) -> PythonResult<()> {
        let qualified_name = metadata.qualified_name();

        // Validate function by attempting to load it
        self.runtime_pool
            .execute(&metadata.source, &metadata.python_function_name, vec![])
            .await
            .map_err(|e| {
                PythonError::RuntimeError(format!(
                    "Failed to validate function {}: {}",
                    qualified_name, e
                ))
            })?;

        // Store metadata
        let mut functions = self.functions.write().await;
        functions.insert(qualified_name.clone(), metadata);

        Ok(())
    }

    /// Unregister a Python UDF
    pub async fn unregister(&self, name: &str) -> PythonResult<()> {
        let mut functions = self.functions.write().await;

        if functions.remove(name).is_none() {
            return Err(PythonError::FunctionNotFound(name.to_string()));
        }

        Ok(())
    }

    /// Get UDF metadata
    pub async fn get_metadata(&self, name: &str) -> Option<PythonUdfMetadata> {
        let functions = self.functions.read().await;
        functions.get(name).cloned()
    }

    /// Check if function exists
    pub async fn exists(&self, name: &str) -> bool {
        let functions = self.functions.read().await;
        functions.contains_key(name)
    }

    /// List all registered functions
    pub async fn list_functions(&self) -> Vec<String> {
        let functions = self.functions.read().await;
        functions.keys().cloned().collect()
    }

    /// Execute a Python UDF by name
    pub async fn execute(&self, name: &str, args: Vec<SqlValue>) -> PythonResult<SqlValue> {
        // Get function metadata
        let metadata = self
            .get_metadata(name)
            .await
            .ok_or_else(|| PythonError::FunctionNotFound(name.to_string()))?;

        // Validate argument count
        metadata.validate_param_count(args.len())?;

        // Convert SQL arguments to Python values
        let python_args: Vec<PythonValue> = args.iter().map(PythonValue::from_sql_value).collect();

        // Execute function
        let result = self
            .runtime_pool
            .execute(
                &metadata.source,
                &metadata.python_function_name,
                python_args,
            )
            .await?;

        // Convert Python result back to SQL value
        Ok(result.to_sql_value())
    }

    /// Execute multiple UDFs in a batch
    pub async fn execute_batch(
        &self,
        calls: Vec<(String, Vec<SqlValue>)>,
    ) -> PythonResult<Vec<PythonResult<SqlValue>>> {
        // Prepare batch requests
        let mut batch_requests = Vec::new();

        for (name, args) in calls {
            // Get function metadata
            let metadata = match self.get_metadata(&name).await {
                Some(m) => m,
                None => {
                    return Err(PythonError::FunctionNotFound(name.clone()));
                }
            };

            // Validate argument count
            metadata.validate_param_count(args.len())?;

            // Convert SQL arguments to Python values
            let python_args: Vec<PythonValue> =
                args.iter().map(PythonValue::from_sql_value).collect();

            batch_requests.push((
                metadata.source.clone(),
                metadata.python_function_name.clone(),
                python_args,
            ));
        }

        // Execute batch
        let results = self.runtime_pool.execute_batch(batch_requests).await?;

        // Convert Python results back to SQL values
        let sql_results: Vec<PythonResult<SqlValue>> = results
            .into_iter()
            .map(|r| r.map(|v| v.to_sql_value()))
            .collect();

        Ok(sql_results)
    }

    /// Execute a Python UDF with direct Python values (for internal use)
    pub async fn execute_raw(
        &self,
        name: &str,
        args: Vec<PythonValue>,
    ) -> PythonResult<PythonValue> {
        // Get function metadata
        let metadata = self
            .get_metadata(name)
            .await
            .ok_or_else(|| PythonError::FunctionNotFound(name.to_string()))?;

        // Validate argument count
        metadata.validate_param_count(args.len())?;

        // Execute function
        self.runtime_pool
            .execute(&metadata.source, &metadata.python_function_name, args)
            .await
    }

    /// Health check all worker processes
    pub async fn health_check(&self) -> Vec<bool> {
        self.runtime_pool.health_check().await
    }

    /// Get number of registered functions
    pub async fn function_count(&self) -> usize {
        let functions = self.functions.read().await;
        functions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_udf_registration() {
        let config = PythonConfig::default();
        let registry = PythonUdfRegistry::new(config).await.unwrap();

        let metadata = PythonUdfMetadata::new(
            "add_two".to_string(),
            "def add_two(x, y):\n    return x + y".to_string(),
            "add_two".to_string(),
            vec!["INTEGER".to_string(), "INTEGER".to_string()],
            "INTEGER".to_string(),
        );

        // Register function
        registry.register(metadata).await.unwrap();

        // Check existence
        assert!(registry.exists("add_two").await);
        assert_eq!(registry.function_count().await, 1);

        // Unregister
        registry.unregister("add_two").await.unwrap();
        assert!(!registry.exists("add_two").await);
        assert_eq!(registry.function_count().await, 0);
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_udf_execution() {
        let config = PythonConfig::default();
        let registry = PythonUdfRegistry::new(config).await.unwrap();

        let metadata = PythonUdfMetadata::new(
            "multiply".to_string(),
            "def multiply(x, y):\n    return x * y".to_string(),
            "multiply".to_string(),
            vec!["INTEGER".to_string(), "INTEGER".to_string()],
            "INTEGER".to_string(),
        );

        registry.register(metadata).await.unwrap();

        // Execute function
        let result = registry
            .execute("multiply", vec![SqlValue::Integer(6), SqlValue::Integer(7)])
            .await
            .unwrap();

        assert_eq!(result, SqlValue::Integer(42));
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_batch_execution() {
        let config = PythonConfig::default();
        let registry = PythonUdfRegistry::new(config).await.unwrap();

        // Register multiple functions
        let add_metadata = PythonUdfMetadata::new(
            "add".to_string(),
            "def add(x, y):\n    return x + y".to_string(),
            "add".to_string(),
            vec!["INTEGER".to_string(), "INTEGER".to_string()],
            "INTEGER".to_string(),
        );

        let sub_metadata = PythonUdfMetadata::new(
            "sub".to_string(),
            "def sub(x, y):\n    return x - y".to_string(),
            "sub".to_string(),
            vec!["INTEGER".to_string(), "INTEGER".to_string()],
            "INTEGER".to_string(),
        );

        registry.register(add_metadata).await.unwrap();
        registry.register(sub_metadata).await.unwrap();

        // Execute batch
        let batch = vec![
            (
                "add".to_string(),
                vec![SqlValue::Integer(10), SqlValue::Integer(5)],
            ),
            (
                "sub".to_string(),
                vec![SqlValue::Integer(10), SqlValue::Integer(5)],
            ),
        ];

        let results = registry.execute_batch(batch).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].as_ref().unwrap(), &SqlValue::Integer(15));
        assert_eq!(results[1].as_ref().unwrap(), &SqlValue::Integer(5));
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_qualified_names() {
        let mut metadata = PythonUdfMetadata::new(
            "my_func".to_string(),
            "def my_func():\n    return 42".to_string(),
            "my_func".to_string(),
            vec![],
            "INTEGER".to_string(),
        );

        // Without schema
        assert_eq!(metadata.qualified_name(), "my_func");

        // With schema
        metadata.schema = Some("my_schema".to_string());
        assert_eq!(metadata.qualified_name(), "my_schema.my_func");
    }

    #[tokio::test]
    #[ignore] // Requires Python runtime - run with --ignored
    async fn test_param_validation() {
        let metadata = PythonUdfMetadata::new(
            "test_func".to_string(),
            "def test_func(a, b, c):\n    return a + b + c".to_string(),
            "test_func".to_string(),
            vec![
                "INTEGER".to_string(),
                "INTEGER".to_string(),
                "INTEGER".to_string(),
            ],
            "INTEGER".to_string(),
        );

        // Correct count
        assert!(metadata.validate_param_count(3).is_ok());

        // Incorrect count
        assert!(metadata.validate_param_count(2).is_err());
        assert!(metadata.validate_param_count(4).is_err());
    }
}
