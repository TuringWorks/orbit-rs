//! Comprehensive tests for Python UDF system

use super::*;
use crate::protocols::postgres_wire::sql::types::SqlValue;
use config::PythonConfig;
use runtime::PythonRuntimePool;
use std::sync::Arc;
use types::{PythonError, PythonValue};
use udf_handler::PythonUdfHandler;
use udf_registry::{PythonUdfMetadata, PythonUdfRegistry};

// ============================================================================
// Configuration Tests
// ============================================================================

#[test]
fn test_default_config() {
    let config = PythonConfig::default();
    assert_eq!(config.python_path, "python3");
    assert_eq!(config.pool_size, 4);
    assert!(config.use_msgpack);
    assert_eq!(config.worker.timeout_seconds, 30);
    assert_eq!(config.worker.restart_after_executions, 1000);
}

#[test]
fn test_config_builder() {
    let config = PythonConfig::default()
        .with_python_path("/usr/bin/python3.11")
        .with_pool_size(8)
        .with_msgpack(false);

    assert_eq!(config.python_path, "/usr/bin/python3.11");
    assert_eq!(config.pool_size, 8);
    assert!(!config.use_msgpack);
}

// ============================================================================
// Type Conversion Tests
// ============================================================================

#[test]
fn test_python_value_to_sql_value() {
    // Null
    assert_eq!(PythonValue::Null.to_sql_value(), SqlValue::Null);

    // Bool
    assert_eq!(
        PythonValue::Bool(true).to_sql_value(),
        SqlValue::Boolean(true)
    );

    // Int (within i32 range)
    assert_eq!(PythonValue::Int(42).to_sql_value(), SqlValue::Integer(42));

    // Int (outside i32 range)
    assert_eq!(
        PythonValue::Int(i64::MAX).to_sql_value(),
        SqlValue::BigInt(i64::MAX)
    );

    // Float
    assert_eq!(
        PythonValue::Float(3.14).to_sql_value(),
        SqlValue::DoublePrecision(3.14)
    );

    // String
    assert_eq!(
        PythonValue::String("hello".to_string()).to_sql_value(),
        SqlValue::Text("hello".to_string())
    );

    // Bytes
    assert_eq!(
        PythonValue::Bytes(vec![1, 2, 3]).to_sql_value(),
        SqlValue::Bytea(vec![1, 2, 3])
    );

    // List
    let list = PythonValue::List(vec![PythonValue::Int(1), PythonValue::Int(2)]);
    match list.to_sql_value() {
        SqlValue::Array(items) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], SqlValue::Integer(1));
            assert_eq!(items[1], SqlValue::Integer(2));
        }
        _ => panic!("Expected Array"),
    }
}

#[test]
fn test_sql_value_to_python_value() {
    // Null
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::Null),
        PythonValue::Null
    );

    // Boolean
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::Boolean(true)),
        PythonValue::Bool(true)
    );

    // Integer types
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::SmallInt(42)),
        PythonValue::Int(42)
    );
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::Integer(42)),
        PythonValue::Int(42)
    );
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::BigInt(42)),
        PythonValue::Int(42)
    );

    // Float types
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::Real(3.14)),
        PythonValue::Float(3.14 as f64)
    );
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::DoublePrecision(3.14)),
        PythonValue::Float(3.14)
    );

    // String types
    assert_eq!(
        PythonValue::from_sql_value(&SqlValue::Text("hello".to_string())),
        PythonValue::String("hello".to_string())
    );
}

// ============================================================================
// Runtime Pool Tests
// ============================================================================

#[tokio::test]
async fn test_runtime_pool_creation() {
    let config = PythonConfig::default().with_pool_size(2);
    let pool = PythonRuntimePool::new(config).await;
    assert!(pool.is_ok());
}

#[tokio::test]
async fn test_simple_execution() {
    let config = PythonConfig::default();
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let result = pool
        .execute(
            "def add(a, b):\n    return a + b",
            "add",
            vec![PythonValue::Int(5), PythonValue::Int(7)],
        )
        .await
        .unwrap();

    assert_eq!(result, PythonValue::Int(12));
}

#[tokio::test]
async fn test_batch_execution() {
    let config = PythonConfig::default();
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let batch = vec![
        (
            "def double(x):\n    return x * 2".to_string(),
            "double".to_string(),
            vec![PythonValue::Int(5)],
        ),
        (
            "def triple(x):\n    return x * 3".to_string(),
            "triple".to_string(),
            vec![PythonValue::Int(5)],
        ),
    ];

    let results = pool.execute_batch(batch).await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].as_ref().unwrap(), &PythonValue::Int(10));
    assert_eq!(results[1].as_ref().unwrap(), &PythonValue::Int(15));
}

#[tokio::test]
async fn test_health_check() {
    let config = PythonConfig::default().with_pool_size(2);
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let health = pool.health_check().await;
    assert_eq!(health.len(), 2);
    assert!(health.iter().all(|&h| h)); // All workers should be healthy
}

// ============================================================================
// UDF Registry Tests
// ============================================================================

#[tokio::test]
async fn test_registry_registration() {
    let config = PythonConfig::default();
    let registry = PythonUdfRegistry::new(config).await.unwrap();

    let metadata = PythonUdfMetadata::new(
        "test_func".to_string(),
        "def test_func(x):\n    return x * 2".to_string(),
        "test_func".to_string(),
        vec!["INTEGER".to_string()],
        "INTEGER".to_string(),
    );

    // Register
    registry.register(metadata).await.unwrap();
    assert!(registry.exists("test_func").await);
    assert_eq!(registry.function_count().await, 1);

    // Unregister
    registry.unregister("test_func").await.unwrap();
    assert!(!registry.exists("test_func").await);
    assert_eq!(registry.function_count().await, 0);
}

#[tokio::test]
async fn test_registry_execution() {
    let config = PythonConfig::default();
    let registry = PythonUdfRegistry::new(config).await.unwrap();

    let metadata = PythonUdfMetadata::new(
        "multiply".to_string(),
        "def multiply(a, b):\n    return a * b".to_string(),
        "multiply".to_string(),
        vec!["INTEGER".to_string(), "INTEGER".to_string()],
        "INTEGER".to_string(),
    );

    registry.register(metadata).await.unwrap();

    let result = registry
        .execute("multiply", vec![SqlValue::Integer(6), SqlValue::Integer(7)])
        .await
        .unwrap();

    assert_eq!(result, SqlValue::Integer(42));
}

#[tokio::test]
async fn test_registry_batch() {
    let config = PythonConfig::default();
    let registry = PythonUdfRegistry::new(config).await.unwrap();

    // Register functions
    let add = PythonUdfMetadata::new(
        "add".to_string(),
        "def add(a, b):\n    return a + b".to_string(),
        "add".to_string(),
        vec!["INTEGER".to_string(), "INTEGER".to_string()],
        "INTEGER".to_string(),
    );

    let sub = PythonUdfMetadata::new(
        "sub".to_string(),
        "def sub(a, b):\n    return a - b".to_string(),
        "sub".to_string(),
        vec!["INTEGER".to_string(), "INTEGER".to_string()],
        "INTEGER".to_string(),
    );

    registry.register(add).await.unwrap();
    registry.register(sub).await.unwrap();

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

// ============================================================================
// UDF Handler Tests
// ============================================================================

#[tokio::test]
async fn test_handler_create_and_execute() {
    let config = PythonConfig::default();
    let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
    let handler = PythonUdfHandler::new(registry);

    // Create function
    handler
        .handle_create_function(
            "square".to_string(),
            vec![("x".to_string(), "INTEGER".to_string())],
            "INTEGER".to_string(),
            "def square(x):\n    return x * x".to_string(),
            None,
        )
        .await
        .unwrap();

    // Execute
    let result = handler
        .execute_function("square", vec![SqlValue::Integer(8)])
        .await
        .unwrap();

    assert_eq!(result, SqlValue::Integer(64));

    // Drop
    handler.handle_drop_function("square").await.unwrap();
    assert!(!handler.function_exists("square").await);
}

#[tokio::test]
async fn test_handler_with_schema() {
    let config = PythonConfig::default();
    let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
    let handler = PythonUdfHandler::new(registry);

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

    assert!(handler.function_exists("my_schema.my_func").await);

    let result = handler
        .execute_function("my_schema.my_func", vec![])
        .await
        .unwrap();

    assert_eq!(result, SqlValue::Integer(42));
}

#[tokio::test]
async fn test_handler_list_functions() {
    let config = PythonConfig::default();
    let registry = Arc::new(PythonUdfRegistry::new(config).await.unwrap());
    let handler = PythonUdfHandler::new(registry);

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

    let funcs = handler.list_functions().await;
    assert_eq!(funcs.len(), 2);
    assert!(funcs.contains(&"func1".to_string()));
    assert!(funcs.contains(&"func2".to_string()));
}

// ============================================================================
// Advanced Python Features Tests
// ============================================================================

#[tokio::test]
async fn test_python_with_math_library() {
    let config = PythonConfig::default();
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let result = pool
        .execute(
            r#"
def calculate_circle_area(radius):
    import math
    return math.pi * radius * radius
"#,
            "calculate_circle_area",
            vec![PythonValue::Float(5.0)],
        )
        .await
        .unwrap();

    if let PythonValue::Float(area) = result {
        assert!((area - 78.53981633974483).abs() < 0.0001);
    } else {
        panic!("Expected Float result");
    }
}

#[tokio::test]
async fn test_python_with_string_operations() {
    let config = PythonConfig::default();
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let result = pool
        .execute(
            r#"
def process_string(text):
    return text.upper() + "!!!"
"#,
            "process_string",
            vec![PythonValue::String("hello".to_string())],
        )
        .await
        .unwrap();

    assert_eq!(result, PythonValue::String("HELLO!!!".to_string()));
}

#[tokio::test]
async fn test_python_with_lists() {
    let config = PythonConfig::default();
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let result = pool
        .execute(
            r#"
def sum_list(numbers):
    return sum(numbers)
"#,
            "sum_list",
            vec![PythonValue::List(vec![
                PythonValue::Int(1),
                PythonValue::Int(2),
                PythonValue::Int(3),
                PythonValue::Int(4),
                PythonValue::Int(5),
            ])],
        )
        .await
        .unwrap();

    assert_eq!(result, PythonValue::Int(15));
}

#[tokio::test]
async fn test_python_with_dicts() {
    let config = PythonConfig::default();
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let mut input_dict = std::collections::HashMap::new();
    input_dict.insert("name".to_string(), PythonValue::String("Alice".to_string()));
    input_dict.insert("age".to_string(), PythonValue::Int(30));

    let result = pool
        .execute(
            r#"
def get_name(person):
    return person.get('name', 'Unknown')
"#,
            "get_name",
            vec![PythonValue::Dict(input_dict)],
        )
        .await
        .unwrap();

    assert_eq!(result, PythonValue::String("Alice".to_string()));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_invalid_function_name() {
    let config = PythonConfig::default();
    let registry = PythonUdfRegistry::new(config).await.unwrap();

    let result = registry
        .execute("nonexistent", vec![SqlValue::Integer(1)])
        .await;

    assert!(matches!(result, Err(PythonError::FunctionNotFound(_))));
}

#[tokio::test]
async fn test_argument_count_mismatch() {
    let config = PythonConfig::default();
    let registry = PythonUdfRegistry::new(config).await.unwrap();

    let metadata = PythonUdfMetadata::new(
        "two_args".to_string(),
        "def two_args(a, b):\n    return a + b".to_string(),
        "two_args".to_string(),
        vec!["INTEGER".to_string(), "INTEGER".to_string()],
        "INTEGER".to_string(),
    );

    registry.register(metadata).await.unwrap();

    // Try to call with wrong number of arguments
    let result = registry
        .execute("two_args", vec![SqlValue::Integer(1)])
        .await;

    assert!(matches!(result, Err(PythonError::RuntimeError(_))));
}

#[tokio::test]
async fn test_python_runtime_error() {
    let config = PythonConfig::default();
    let pool = PythonRuntimePool::new(config).await.unwrap();

    let result = pool
        .execute(
            r#"
def divide_by_zero():
    return 1 / 0
"#,
            "divide_by_zero",
            vec![],
        )
        .await;

    assert!(matches!(result, Err(PythonError::RuntimeError(_))));
}

// ============================================================================
// Security Tests
// ============================================================================

#[test]
fn test_security_forbidden_eval() {
    let config = PythonConfig::default();
    let registry_future = PythonUdfRegistry::new(config);
    let registry = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(registry_future)
        .unwrap();

    let handler = PythonUdfHandler::new(Arc::new(registry));

    let source = r#"
def bad_func():
    eval('print("hacked")')
    return 1
"#;

    let validation = handler.handle_create_function(
        "bad_func".to_string(),
        vec![],
        "INTEGER".to_string(),
        source.to_string(),
        None,
    );

    let result = tokio::runtime::Runtime::new().unwrap().block_on(validation);
    assert!(matches!(result, Err(PythonError::SecurityViolation(_))));
}

#[test]
fn test_security_forbidden_exec() {
    let config = PythonConfig::default();
    let registry = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(PythonUdfRegistry::new(config))
        .unwrap();

    let handler = PythonUdfHandler::new(Arc::new(registry));

    let source = r#"
def bad_func():
    exec('import os')
    return 1
"#;

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(handler.handle_create_function(
            "bad_func".to_string(),
            vec![],
            "INTEGER".to_string(),
            source.to_string(),
            None,
        ));

    assert!(matches!(result, Err(PythonError::SecurityViolation(_))));
}

// ============================================================================
// Metadata Tests
// ============================================================================

#[test]
fn test_metadata_qualified_name() {
    let mut metadata = PythonUdfMetadata::new(
        "func".to_string(),
        "def func(): pass".to_string(),
        "func".to_string(),
        vec![],
        "VOID".to_string(),
    );

    assert_eq!(metadata.qualified_name(), "func");

    metadata.schema = Some("myschema".to_string());
    assert_eq!(metadata.qualified_name(), "myschema.func");
}

#[test]
fn test_metadata_param_validation() {
    let metadata = PythonUdfMetadata::new(
        "func".to_string(),
        "def func(a, b, c): pass".to_string(),
        "func".to_string(),
        vec![
            "INTEGER".to_string(),
            "TEXT".to_string(),
            "BOOLEAN".to_string(),
        ],
        "VOID".to_string(),
    );

    assert!(metadata.validate_param_count(3).is_ok());
    assert!(metadata.validate_param_count(2).is_err());
    assert!(metadata.validate_param_count(4).is_err());
}
