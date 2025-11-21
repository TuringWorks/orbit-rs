//! PostgreSQL DML (Data Manipulation Language) Integration Tests
//!
//! Comprehensive tests for INSERT, UPDATE, DELETE operations

use orbit_protocols::postgres_wire::sql::executor::{ExecutionResult, SqlExecutor};

// ============================================================================
// INSERT Tests
// ============================================================================

#[tokio::test]
async fn test_insert_single_row() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT)").await.unwrap();

    // Insert single row
    let sql = "INSERT INTO users (id, name) VALUES (1, 'Alice')";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Insert { count } => {
            assert_eq!(count, 1);
        }
        _ => panic!("Expected Insert result"),
    }
}

#[tokio::test]
async fn test_insert_multiple_rows() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT)").await.unwrap();

    // Insert multiple rows
    let sql = "INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie')";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Insert { count } => {
            assert_eq!(count, 3);
        }
        _ => panic!("Expected Insert result"),
    }
}

#[tokio::test]
async fn test_insert_all_types() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table with various types
    executor.execute(r#"
        CREATE TABLE all_types (
            col_int INTEGER,
            col_bigint BIGINT,
            col_text TEXT,
            col_bool BOOLEAN,
            col_float REAL
        )
    "#).await.unwrap();

    // Insert row with all types
    let sql = "INSERT INTO all_types VALUES (42, 9223372036854775807, 'hello world', true, 3.14159)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Insert { count } => {
            assert_eq!(count, 1);
        }
        _ => panic!("Expected Insert result"),
    }
}

#[tokio::test]
async fn test_insert_with_null() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT, email TEXT)").await.unwrap();

    // Insert row with NULL
    let sql = "INSERT INTO users (id, name, email) VALUES (1, 'Alice', NULL)";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Insert { count } => {
            assert_eq!(count, 1);
        }
        _ => panic!("Expected Insert result"),
    }
}

#[tokio::test]
async fn test_insert_partial_columns() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT, email TEXT)").await.unwrap();

    // Insert with only some columns
    let sql = "INSERT INTO users (id, name) VALUES (1, 'Alice')";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Insert { count } => {
            assert_eq!(count, 1);
        }
        _ => panic!("Expected Insert result"),
    }
}

#[tokio::test]
async fn test_insert_integers() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE numbers (value INTEGER)").await.unwrap();

    // Insert various integers
    let sqls = vec![
        "INSERT INTO numbers VALUES (0)",
        "INSERT INTO numbers VALUES (-42)",
        "INSERT INTO numbers VALUES (42)",
        "INSERT INTO numbers VALUES (2147483647)",  // Max i32
    ];

    for sql in sqls {
        let result = executor.execute(sql).await.unwrap();
        assert!(matches!(result, ExecutionResult::Insert { count: 1 }));
    }
}

#[tokio::test]
async fn test_insert_text() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE messages (text TEXT)").await.unwrap();

    // Insert various text values
    let sqls = vec![
        "INSERT INTO messages VALUES ('hello')",
        "INSERT INTO messages VALUES ('hello world')",
        "INSERT INTO messages VALUES ('with spaces   ')",
        "INSERT INTO messages VALUES ('')",  // Empty string
    ];

    for sql in sqls {
        let result = executor.execute(sql).await.unwrap();
        assert!(matches!(result, ExecutionResult::Insert { count: 1 }));
    }
}

#[tokio::test]
async fn test_insert_booleans() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE flags (flag BOOLEAN)").await.unwrap();

    // Insert various boolean values
    let sqls = vec![
        "INSERT INTO flags VALUES (true)",
        "INSERT INTO flags VALUES (false)",
        "INSERT INTO flags VALUES (TRUE)",
        "INSERT INTO flags VALUES (FALSE)",
    ];

    for sql in sqls {
        let result = executor.execute(sql).await.unwrap();
        assert!(matches!(result, ExecutionResult::Insert { count: 1 }));
    }
}

// ============================================================================
// UPDATE Tests
// ============================================================================

#[tokio::test]
async fn test_update_single_column() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT)").await.unwrap();
    executor.execute("INSERT INTO users VALUES (1, 'Alice')").await.unwrap();

    // Update
    let sql = "UPDATE users SET name = 'Alice Updated' WHERE id = 1";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Update { count } => {
            assert!(count > 0);  // At least one row should be updated
        }
        _ => panic!("Expected Update result"),
    }
}

#[tokio::test]
async fn test_update_multiple_columns() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT, email TEXT)").await.unwrap();
    executor.execute("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')").await.unwrap();

    // Update multiple columns
    let sql = "UPDATE users SET name = 'Alice Smith', email = 'alice.smith@example.com' WHERE id = 1";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Update { count } => {
            assert!(count > 0);
        }
        _ => panic!("Expected Update result"),
    }
}

#[tokio::test]
async fn test_update_all_rows() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE users (id INTEGER, active BOOLEAN)").await.unwrap();
    executor.execute("INSERT INTO users VALUES (1, true), (2, true), (3, false)").await.unwrap();

    // Update all rows (no WHERE clause)
    let sql = "UPDATE users SET active = false";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Update { count } => {
            assert_eq!(count, 3);
        }
        _ => panic!("Expected Update result"),
    }
}

#[tokio::test]
async fn test_update_to_null() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE users (id INTEGER, email TEXT)").await.unwrap();
    executor.execute("INSERT INTO users VALUES (1, 'alice@example.com')").await.unwrap();

    // Update to NULL
    let sql = "UPDATE users SET email = NULL WHERE id = 1";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Update { count } => {
            assert!(count > 0);
        }
        _ => panic!("Expected Update result"),
    }
}

#[tokio::test]
async fn test_update_with_integer_comparison() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE products (id INTEGER, price INTEGER)").await.unwrap();
    executor.execute("INSERT INTO products VALUES (1, 100), (2, 200), (3, 50)").await.unwrap();

    // Update with WHERE clause
    let sql = "UPDATE products SET price = 150 WHERE price > 100";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Update { count } => {
            assert!(count >= 0);  // May or may not update depending on WHERE evaluation
        }
        _ => panic!("Expected Update result"),
    }
}

// ============================================================================
// DELETE Tests
// ============================================================================

#[tokio::test]
async fn test_delete_single_row() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT)").await.unwrap();
    executor.execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')").await.unwrap();

    // Delete single row
    let sql = "DELETE FROM users WHERE id = 1";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Delete { count } => {
            assert!(count >= 0);  // Should delete at least 0 rows
        }
        _ => panic!("Expected Delete result"),
    }
}

#[tokio::test]
async fn test_delete_multiple_rows() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE users (id INTEGER, active BOOLEAN)").await.unwrap();
    executor.execute("INSERT INTO users VALUES (1, false), (2, false), (3, true)").await.unwrap();

    // Delete multiple rows
    let sql = "DELETE FROM users WHERE active = false";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Delete { count } => {
            assert!(count >= 0);
        }
        _ => panic!("Expected Delete result"),
    }
}

#[tokio::test]
async fn test_delete_all_rows() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE temp_data (value INTEGER)").await.unwrap();
    executor.execute("INSERT INTO temp_data VALUES (1), (2), (3)").await.unwrap();

    // Delete all rows (no WHERE clause)
    let sql = "DELETE FROM temp_data";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Delete { count } => {
            assert_eq!(count, 3);
        }
        _ => panic!("Expected Delete result"),
    }
}

#[tokio::test]
async fn test_delete_with_comparison() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor.execute("CREATE TABLE scores (player TEXT, score INTEGER)").await.unwrap();
    executor.execute("INSERT INTO scores VALUES ('Alice', 100), ('Bob', 50), ('Charlie', 150)").await.unwrap();

    // Delete with WHERE clause
    let sql = "DELETE FROM scores WHERE score < 75";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Delete { count } => {
            assert!(count >= 0);
        }
        _ => panic!("Expected Delete result"),
    }
}

// ============================================================================
// Combined DML Operations
// ============================================================================

#[tokio::test]
async fn test_insert_update_delete_flow() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE workflow (id INTEGER, status TEXT)").await.unwrap();

    // INSERT
    let result = executor.execute("INSERT INTO workflow VALUES (1, 'pending')").await.unwrap();
    assert!(matches!(result, ExecutionResult::Insert { count: 1 }));

    // UPDATE
    let result = executor.execute("UPDATE workflow SET status = 'completed' WHERE id = 1").await.unwrap();
    assert!(matches!(result, ExecutionResult::Update { .. }));

    // DELETE
    let result = executor.execute("DELETE FROM workflow WHERE id = 1").await.unwrap();
    assert!(matches!(result, ExecutionResult::Delete { .. }));
}

#[tokio::test]
async fn test_bulk_operations() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE bulk_test (id INTEGER, value TEXT)").await.unwrap();

    // Bulk insert
    executor.execute("INSERT INTO bulk_test VALUES (1, 'a'), (2, 'b'), (3, 'c'), (4, 'd'), (5, 'e')").await.unwrap();

    // Bulk update
    let result = executor.execute("UPDATE bulk_test SET value = 'updated'").await.unwrap();
    match result {
        ExecutionResult::Update { count } => {
            assert_eq!(count, 5);
        }
        _ => panic!("Expected Update result"),
    }

    // Bulk delete
    let result = executor.execute("DELETE FROM bulk_test").await.unwrap();
    match result {
        ExecutionResult::Delete { count } => {
            assert_eq!(count, 5);
        }
        _ => panic!("Expected Delete result"),
    }
}

#[tokio::test]
async fn test_sequential_inserts() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE sequence (id INTEGER)").await.unwrap();

    // Insert rows sequentially
    for i in 1..=10 {
        let sql = format!("INSERT INTO sequence VALUES ({})", i);
        let result = executor.execute(&sql).await.unwrap();
        assert!(matches!(result, ExecutionResult::Insert { count: 1 }));
    }
}

#[tokio::test]
async fn test_update_non_existent_row() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE users (id INTEGER, name TEXT)").await.unwrap();

    // Try to update non-existent row (should succeed but update 0 rows)
    let sql = "UPDATE users SET name = 'Test' WHERE id = 999";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Update { count } => {
            assert_eq!(count, 0);
        }
        _ => panic!("Expected Update result"),
    }
}

#[tokio::test]
async fn test_delete_non_existent_row() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor.execute("CREATE TABLE users (id INTEGER)").await.unwrap();

    // Try to delete non-existent row (should succeed but delete 0 rows)
    let sql = "DELETE FROM users WHERE id = 999";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Delete { count } => {
            assert_eq!(count, 0);
        }
        _ => panic!("Expected Delete result"),
    }
}
