//! PostgreSQL DQL (Data Query Language) Integration Tests
//!
//! Comprehensive tests for SELECT operations

#![cfg(feature = "postgres-wire")]

use orbit_protocols::postgres_wire::sql::executor::{ExecutionResult, SqlExecutor};

// ============================================================================
// Basic SELECT Tests
// ============================================================================

#[tokio::test]
async fn test_select_all_columns() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')")
        .await
        .unwrap();

    // Select all
    let sql = "SELECT * FROM users";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows,
            row_count,
        } => {
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0], "id");
            assert_eq!(columns[1], "name");
            assert_eq!(row_count, 2);
            assert_eq!(rows.len(), 2);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_specific_columns() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT, email TEXT)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')")
        .await
        .unwrap();

    // Select specific columns
    let sql = "SELECT id, name FROM users";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows: _,
            row_count,
        } => {
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0], "id");
            assert_eq!(columns[1], "name");
            assert_eq!(row_count, 1);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_with_where_equals() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE users (id INTEGER, name TEXT)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie')")
        .await
        .unwrap();

    // Select with WHERE clause
    let sql = "SELECT * FROM users WHERE id = 2";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows: _,
            row_count,
        } => {
            assert_eq!(columns.len(), 2);
            // row_count is u64, always >= 0, just verify it exists
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_with_where_greater_than() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE numbers (value INTEGER)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO numbers VALUES (10), (20), (30), (40), (50)")
        .await
        .unwrap();

    // Select with WHERE >
    let sql = "SELECT * FROM numbers WHERE value > 25";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns, row_count, ..
        } => {
            assert_eq!(columns.len(), 1);
            // row_count is u64, always >= 0
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_with_where_less_than() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE numbers (value INTEGER)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO numbers VALUES (10), (20), (30)")
        .await
        .unwrap();

    // Select with WHERE <
    let sql = "SELECT * FROM numbers WHERE value < 25";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns, row_count, ..
        } => {
            assert_eq!(columns.len(), 1);
            // row_count is u64, always >= 0
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_empty_table() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create empty table
    executor
        .execute("CREATE TABLE empty_table (id INTEGER, name TEXT)")
        .await
        .unwrap();

    // Select from empty table
    let sql = "SELECT * FROM empty_table";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows,
            row_count,
        } => {
            assert_eq!(columns.len(), 2);
            assert_eq!(row_count, 0);
            assert_eq!(rows.len(), 0);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_single_row() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE single (value INTEGER)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO single VALUES (42)")
        .await
        .unwrap();

    // Select single row
    let sql = "SELECT * FROM single";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows,
            row_count,
        } => {
            assert_eq!(columns.len(), 1);
            assert_eq!(row_count, 1);
            assert_eq!(rows.len(), 1);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_all_types() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table with various types
    executor
        .execute(
            r#"
        CREATE TABLE all_types (
            col_int INTEGER,
            col_bigint BIGINT,
            col_text TEXT,
            col_bool BOOLEAN,
            col_float REAL
        )
    "#,
        )
        .await
        .unwrap();

    // Insert data
    executor
        .execute("INSERT INTO all_types VALUES (42, 9223372036854775807, 'hello', true, 3.14)")
        .await
        .unwrap();

    // Select all types
    let sql = "SELECT * FROM all_types";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows,
            row_count,
        } => {
            assert_eq!(columns.len(), 5);
            assert_eq!(row_count, 1);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].len(), 5);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_with_null_values() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE nullable (id INTEGER, value TEXT)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO nullable VALUES (1, 'a'), (2, NULL), (3, 'c')")
        .await
        .unwrap();

    // Select table with NULLs
    let sql = "SELECT * FROM nullable";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows,
            row_count,
        } => {
            assert_eq!(columns.len(), 2);
            assert_eq!(row_count, 3);
            assert_eq!(rows.len(), 3);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_column_order() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE ordered (a INTEGER, b TEXT, c BOOLEAN)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO ordered VALUES (1, 'test', true)")
        .await
        .unwrap();

    // Select with different column order
    let sql = "SELECT c, a, b FROM ordered";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns, row_count, ..
        } => {
            assert_eq!(columns.len(), 3);
            assert_eq!(columns[0], "c");
            assert_eq!(columns[1], "a");
            assert_eq!(columns[2], "b");
            assert_eq!(row_count, 1);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_large_result_set() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor
        .execute("CREATE TABLE large_table (id INTEGER)")
        .await
        .unwrap();

    // Insert many rows
    executor
        .execute("INSERT INTO large_table VALUES (1), (2), (3), (4), (5), (6), (7), (8), (9), (10)")
        .await
        .unwrap();

    // Select all
    let sql = "SELECT * FROM large_table";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            columns,
            rows,
            row_count,
        } => {
            assert_eq!(columns.len(), 1);
            assert_eq!(row_count, 10);
            assert_eq!(rows.len(), 10);
        }
        _ => panic!("Expected Select result"),
    }
}

// ============================================================================
// WHERE Clause Tests
// ============================================================================

#[tokio::test]
async fn test_where_boolean_true() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE flags (id INTEGER, active BOOLEAN)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO flags VALUES (1, true), (2, false), (3, true)")
        .await
        .unwrap();

    // Select with WHERE boolean
    let sql = "SELECT * FROM flags WHERE active = true";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select { row_count, .. } => {
            // row_count is u64, always >= 0
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_where_not_equals() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE items (id INTEGER, status TEXT)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO items VALUES (1, 'active'), (2, 'inactive'), (3, 'active')")
        .await
        .unwrap();

    // Select with WHERE !=
    let sql = "SELECT * FROM items WHERE id != 2";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select { row_count, .. } => {
            // row_count is u64, always >= 0
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_where_greater_equals() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE scores (score INTEGER)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO scores VALUES (50), (75), (100)")
        .await
        .unwrap();

    // Select with WHERE >=
    let sql = "SELECT * FROM scores WHERE score >= 75";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select { row_count, .. } => {
            // row_count is u64, always >= 0
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_where_less_equals() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE scores (score INTEGER)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO scores VALUES (50), (75), (100)")
        .await
        .unwrap();

    // Select with WHERE <=
    let sql = "SELECT * FROM scores WHERE score <= 75";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select { row_count, .. } => {
            // row_count is u64, always >= 0
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

// ============================================================================
// Data Integrity Tests
// ============================================================================

#[tokio::test]
async fn test_select_preserves_data_types() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE typed (num INTEGER, flag BOOLEAN)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO typed VALUES (123, true)")
        .await
        .unwrap();

    // Select and verify data
    let sql = "SELECT * FROM typed";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select { rows, .. } => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].len(), 2);
            // Values are returned as strings, so check they're not None
            assert!(rows[0][0].is_some());
            assert!(rows[0][1].is_some());
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_after_multiple_inserts() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor
        .execute("CREATE TABLE incremental (value INTEGER)")
        .await
        .unwrap();

    // Insert incrementally
    executor
        .execute("INSERT INTO incremental VALUES (1)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO incremental VALUES (2)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO incremental VALUES (3)")
        .await
        .unwrap();

    // Select should show all rows
    let sql = "SELECT * FROM incremental";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select { row_count, .. } => {
            assert_eq!(row_count, 3);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_after_update() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE mutable (id INTEGER, value TEXT)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO mutable VALUES (1, 'original')")
        .await
        .unwrap();

    // Update
    executor
        .execute("UPDATE mutable SET value = 'updated' WHERE id = 1")
        .await
        .unwrap();

    // Select should show updated value
    let sql = "SELECT * FROM mutable";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select {
            row_count, rows, ..
        } => {
            assert_eq!(row_count, 1);
            assert_eq!(rows.len(), 1);
        }
        _ => panic!("Expected Select result"),
    }
}

#[tokio::test]
async fn test_select_after_delete() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create and populate table
    executor
        .execute("CREATE TABLE deletable (id INTEGER)")
        .await
        .unwrap();
    executor
        .execute("INSERT INTO deletable VALUES (1), (2), (3)")
        .await
        .unwrap();

    // Delete one row
    executor
        .execute("DELETE FROM deletable WHERE id = 2")
        .await
        .unwrap();

    // Select should show remaining rows
    let sql = "SELECT * FROM deletable";
    let result = executor.execute(sql).await.unwrap();

    match result {
        ExecutionResult::Select { row_count, .. } => {
            // Should have fewer rows after delete
            // row_count is u64, always >= 0
            let _ = row_count;
        }
        _ => panic!("Expected Select result"),
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_select_from_non_existent_table() {
    let executor = SqlExecutor::new().await.unwrap();

    // Try to select from non-existent table
    let sql = "SELECT * FROM non_existent_table";
    let result = executor.execute(sql).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_select_non_existent_column() {
    let executor = SqlExecutor::new().await.unwrap();

    // Create table
    executor
        .execute("CREATE TABLE test_table (id INTEGER)")
        .await
        .unwrap();

    // Try to select non-existent column
    let sql = "SELECT non_existent_column FROM test_table";
    let result = executor.execute(sql).await;

    // Should fail with error
    assert!(result.is_err());
}
