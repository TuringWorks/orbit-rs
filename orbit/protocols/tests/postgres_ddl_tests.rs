//! PostgreSQL DDL (Data Definition Language) Integration Tests
//!
//! Comprehensive tests for CREATE, ALTER, and DROP operations

#![cfg(feature = "postgres-wire")]

use orbit_protocols::postgres_wire::sql::executor::{ExecutionResult, SqlExecutor};

// ============================================================================
// CREATE TABLE Tests
// ============================================================================

#[tokio::test]
async fn test_create_table_basic() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE TABLE users (id INTEGER, name TEXT)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_all_column_types() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = r#"
        CREATE TABLE all_types (
            col_int INTEGER,
            col_bigint BIGINT,
            col_smallint SMALLINT,
            col_text TEXT,
            col_varchar VARCHAR(255),
            col_bool BOOLEAN,
            col_float REAL,
            col_double DOUBLE PRECISION,
            col_decimal DECIMAL(10, 2),
            col_date DATE,
            col_time TIME,
            col_timestamp TIMESTAMP,
            col_json JSON,
            col_jsonb JSONB,
            col_uuid UUID,
            col_bytea BYTEA
        )
    "#;

    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "all_types");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_with_primary_key() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_with_not_null() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE TABLE users (id INTEGER NOT NULL, name TEXT NOT NULL)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_with_default() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE TABLE users (id INTEGER DEFAULT 0, name TEXT DEFAULT 'unknown')";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_with_unique() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE TABLE users (id INTEGER, email TEXT UNIQUE)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_with_check_constraint() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE TABLE users (id INTEGER, age INTEGER CHECK (age >= 0))";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_with_foreign_key() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create parent table first
    executor
        .execute("CREATE TABLE departments (id INTEGER PRIMARY KEY, name TEXT)")
        .await
        .unwrap();

    // Create child table with foreign key
    let sql = "CREATE TABLE employees (id INTEGER PRIMARY KEY, dept_id INTEGER REFERENCES departments(id))";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "employees");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_if_not_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    let sql = "CREATE TABLE IF NOT EXISTS users (id INTEGER, name TEXT)";
    executor.execute(sql).await.unwrap();

    // Try to create again with IF NOT EXISTS - should succeed
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

#[tokio::test]
async fn test_create_table_with_composite_primary_key() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE TABLE user_roles (user_id INTEGER, role_id INTEGER, PRIMARY KEY (user_id, role_id))";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateTable { table_name } => {
            assert_eq!(table_name, "user_roles");
        }
        _ => panic!("Expected CreateTable result"),
    }
}

// ============================================================================
// DROP TABLE Tests
// ============================================================================

#[tokio::test]
async fn test_drop_table() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table first
    executor
        .execute("CREATE TABLE temp_table (id INTEGER)")
        .await
        .unwrap();

    // Drop table
    let sql = "DROP TABLE temp_table";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropTable { table_names } => {
            assert_eq!(table_names, vec!["temp_table"]);
        }
        _ => panic!("Expected DropTable result"),
    }
}

#[tokio::test]
async fn test_drop_table_if_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    // Drop non-existent table with IF EXISTS - should succeed
    let sql = "DROP TABLE IF EXISTS non_existent_table";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropTable { table_names } => {
            assert_eq!(table_names, vec!["non_existent_table"]);
        }
        _ => panic!("Expected DropTable result"),
    }
}

#[tokio::test]
async fn test_drop_multiple_tables() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create tables first
    executor
        .execute("CREATE TABLE table1 (id INTEGER)")
        .await
        .unwrap();
    executor
        .execute("CREATE TABLE table2 (id INTEGER)")
        .await
        .unwrap();

    // Drop multiple tables
    let sql = "DROP TABLE table1, table2";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropTable { table_names } => {
            assert_eq!(table_names.len(), 2);
            assert!(table_names.contains(&"table1".to_string()));
            assert!(table_names.contains(&"table2".to_string()));
        }
        _ => panic!("Expected DropTable result"),
    }
}

// ============================================================================
// CREATE INDEX Tests
// ============================================================================

#[tokio::test]
async fn test_create_index() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table first
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .unwrap();

    // Create index
    let sql = "CREATE INDEX idx_users_name ON users (name)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateIndex {
            index_name,
            table_name,
        } => {
            assert_eq!(index_name, "idx_users_name");
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateIndex result"),
    }
}

#[tokio::test]
async fn test_create_unique_index() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table first
    executor
        .execute("CREATE TABLE users (id INTEGER, email TEXT)")
        .await
        .unwrap();

    // Create unique index
    let sql = "CREATE UNIQUE INDEX idx_users_email ON users (email)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateIndex {
            index_name,
            table_name,
        } => {
            assert_eq!(index_name, "idx_users_email");
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateIndex result"),
    }
}

#[tokio::test]
async fn test_create_index_if_not_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table first
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .unwrap();

    // Create index
    let sql = "CREATE INDEX IF NOT EXISTS idx_users_name ON users (name)";
    executor.execute(sql).await.unwrap();

    // Try to create again with IF NOT EXISTS - should succeed
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateIndex {
            index_name,
            table_name,
        } => {
            assert_eq!(index_name, "idx_users_name");
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateIndex result"),
    }
}

#[tokio::test]
async fn test_create_composite_index() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table first
    executor
        .execute("CREATE TABLE users (id INTEGER, first_name TEXT, last_name TEXT)")
        .await
        .unwrap();

    // Create composite index
    let sql = "CREATE INDEX idx_users_fullname ON users (first_name, last_name)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateIndex {
            index_name,
            table_name,
        } => {
            assert_eq!(index_name, "idx_users_fullname");
            assert_eq!(table_name, "users");
        }
        _ => panic!("Expected CreateIndex result"),
    }
}

// ============================================================================
// DROP INDEX Tests
// ============================================================================

#[tokio::test]
async fn test_drop_index() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table and index first
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .unwrap();
    executor
        .execute("CREATE INDEX idx_users_name ON users (name)")
        .await
        .unwrap();

    // Drop index
    let sql = "DROP INDEX idx_users_name";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropIndex { index_names } => {
            assert_eq!(index_names, vec!["idx_users_name"]);
        }
        _ => panic!("Expected DropIndex result"),
    }
}

#[tokio::test]
async fn test_drop_index_if_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    // Drop non-existent index with IF EXISTS - should succeed
    let sql = "DROP INDEX IF EXISTS non_existent_index";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropIndex { index_names } => {
            assert_eq!(index_names, vec!["non_existent_index"]);
        }
        _ => panic!("Expected DropIndex result"),
    }
}

// ============================================================================
// CREATE VIEW Tests
// ============================================================================

#[tokio::test]
async fn test_create_view() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table first
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT, active BOOLEAN)")
        .await
        .unwrap();

    // Create view
    let sql = "CREATE VIEW active_users AS SELECT id, name FROM users WHERE active = true";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateView { view_name } => {
            assert_eq!(view_name, "active_users");
        }
        _ => panic!("Expected CreateView result"),
    }
}

#[tokio::test]
async fn test_create_or_replace_view() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table first
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .unwrap();

    // Create view
    let sql = "CREATE VIEW user_names AS SELECT name FROM users";
    executor.execute(sql).await.unwrap();

    // Replace view
    let sql = "CREATE OR REPLACE VIEW user_names AS SELECT id, name FROM users";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateView { view_name } => {
            assert_eq!(view_name, "user_names");
        }
        _ => panic!("Expected CreateView result"),
    }
}

// ============================================================================
// DROP VIEW Tests
// ============================================================================

#[tokio::test]
async fn test_drop_view() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table and view first
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .unwrap();
    executor
        .execute("CREATE VIEW user_names AS SELECT name FROM users")
        .await
        .unwrap();

    // Drop view
    let sql = "DROP VIEW user_names";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropView { view_names } => {
            assert_eq!(view_names, vec!["user_names"]);
        }
        _ => panic!("Expected DropView result"),
    }
}

#[tokio::test]
async fn test_drop_view_if_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    // Drop non-existent view with IF EXISTS - should succeed
    let sql = "DROP VIEW IF EXISTS non_existent_view";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropView { view_names } => {
            assert_eq!(view_names, vec!["non_existent_view"]);
        }
        _ => panic!("Expected DropView result"),
    }
}

// ============================================================================
// CREATE SCHEMA Tests
// ============================================================================

#[tokio::test]
async fn test_create_schema() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE SCHEMA analytics";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateSchema { schema_name } => {
            assert_eq!(schema_name, "analytics");
        }
        _ => panic!("Expected CreateSchema result"),
    }
}

#[tokio::test]
async fn test_create_schema_if_not_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE SCHEMA IF NOT EXISTS analytics";
    executor.execute(sql).await.unwrap();

    // Try to create again with IF NOT EXISTS - should succeed
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateSchema { schema_name } => {
            assert_eq!(schema_name, "analytics");
        }
        _ => panic!("Expected CreateSchema result"),
    }
}

// ============================================================================
// DROP SCHEMA Tests
// ============================================================================

#[tokio::test]
async fn test_drop_schema() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create schema first
    executor.execute("CREATE SCHEMA temp_schema").await.unwrap();

    // Drop schema
    let sql = "DROP SCHEMA temp_schema";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropSchema { schema_names } => {
            assert_eq!(schema_names, vec!["temp_schema"]);
        }
        _ => panic!("Expected DropSchema result"),
    }
}

#[tokio::test]
async fn test_drop_schema_if_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    // Drop non-existent schema with IF EXISTS - should succeed
    let sql = "DROP SCHEMA IF EXISTS non_existent_schema";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropSchema { schema_names } => {
            assert_eq!(schema_names, vec!["non_existent_schema"]);
        }
        _ => panic!("Expected DropSchema result"),
    }
}

// ============================================================================
// Extension Tests
// ============================================================================

#[tokio::test]
async fn test_create_extension() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE EXTENSION vector";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateExtension { extension_name } => {
            assert_eq!(extension_name, "vector");
        }
        _ => panic!("Expected CreateExtension result"),
    }
}

#[tokio::test]
async fn test_create_extension_if_not_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    let sql = "CREATE EXTENSION IF NOT EXISTS vector";
    executor.execute(sql).await.unwrap();

    // Try to create again with IF NOT EXISTS - should succeed
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::CreateExtension { extension_name } => {
            assert_eq!(extension_name, "vector");
        }
        _ => panic!("Expected CreateExtension result"),
    }
}

#[tokio::test]
async fn test_drop_extension() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create extension first
    executor.execute("CREATE EXTENSION vector").await.unwrap();

    // Drop extension
    let sql = "DROP EXTENSION vector";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropExtension { extension_names } => {
            assert_eq!(extension_names, vec!["vector"]);
        }
        _ => panic!("Expected DropExtension result"),
    }
}

#[tokio::test]
async fn test_drop_extension_if_exists() {
    let executor = SqlExecutor::new().await.unwrap();

    // Drop non-existent extension with IF EXISTS - should succeed
    let sql = "DROP EXTENSION IF EXISTS non_existent_ext";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::DropExtension { extension_names } => {
            assert_eq!(extension_names, vec!["non_existent_ext"]);
        }
        _ => panic!("Expected DropExtension result"),
    }
}
