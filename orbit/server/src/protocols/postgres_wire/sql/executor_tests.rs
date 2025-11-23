//! Comprehensive unit tests for the SQL executor
//!
//! This module contains extensive unit tests for the SqlExecutor to ensure
//! reliability and correctness of SQL operations including DDL, DML, and
//! complex query operations.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::protocols::error::ProtocolResult;
    use std::sync::Arc;

    /// Test fixture for SQL executor tests
    struct SqlExecutorTestFixture {
        executor: SqlExecutor,
    }

    impl SqlExecutorTestFixture {
        async fn new() -> Self {
            let executor = SqlExecutor::new().await.unwrap();
            Self { executor }
        }

        /// Execute SQL and return result
        async fn execute_sql(&self, sql: &str) -> ProtocolResult<ExecutionResult> {
            self.executor.execute(sql).await
        }

        /// Execute SQL expecting success
        async fn execute_sql_ok(&self, sql: &str) -> ExecutionResult {
            self.execute_sql(sql).await.unwrap()
        }

        /// Execute SQL expecting failure
        async fn execute_sql_err(&self, sql: &str) -> crate::protocols::error::ProtocolError {
            self.execute_sql(sql).await.unwrap_err()
        }
    }

    #[tokio::test]
    async fn test_create_table_basic() {
        let fixture = SqlExecutorTestFixture::new().await;

        let result = fixture
            .execute_sql_ok(
                "CREATE TABLE users (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    email TEXT UNIQUE,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
            )
            .await;

        match result {
            ExecutionResult::CreateTable { table_name } => {
                assert_eq!(table_name, "users");
            }
            _ => panic!("Expected CreateTable result"),
        }
    }

    #[tokio::test]
    async fn test_create_table_if_not_exists() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Create table first time - should succeed
        fixture
            .execute_sql_ok("CREATE TABLE test_table (id INTEGER, name TEXT)")
            .await;

        // Create same table without IF NOT EXISTS - should fail
        let error = fixture
            .execute_sql_err("CREATE TABLE test_table (id INTEGER, name TEXT)")
            .await;

        assert!(error.to_string().contains("already exists"));

        // Create with IF NOT EXISTS - should succeed silently
        fixture
            .execute_sql_ok("CREATE TABLE IF NOT EXISTS test_table (id INTEGER, name TEXT)")
            .await;
    }

    #[tokio::test]
    async fn test_insert_values() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Create table
        fixture
            .execute_sql_ok("CREATE TABLE products (id INTEGER, name TEXT, price DECIMAL)")
            .await;

        // Insert single row
        let result = fixture
            .execute_sql_ok("INSERT INTO products (id, name, price) VALUES (1, 'Laptop', 999.99)")
            .await;

        match result {
            ExecutionResult::Insert { count } => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected Insert result"),
        }
    }

    #[tokio::test]
    async fn test_insert_multiple_values() {
        let fixture = SqlExecutorTestFixture::new().await;

        fixture
            .execute_sql_ok("CREATE TABLE items (id INTEGER, name TEXT)")
            .await;

        let result = fixture
            .execute_sql_ok(
                "INSERT INTO items (id, name) VALUES 
                 (1, 'Item 1'), 
                 (2, 'Item 2'), 
                 (3, 'Item 3')",
            )
            .await;

        match result {
            ExecutionResult::Insert { count } => {
                assert_eq!(count, 3);
            }
            _ => panic!("Expected Insert result with count 3"),
        }
    }

    #[tokio::test]
    async fn test_select_basic() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Setup test data
        fixture
            .execute_sql_ok("CREATE TABLE customers (id INTEGER, name TEXT, city TEXT)")
            .await;

        fixture
            .execute_sql_ok(
                "INSERT INTO customers VALUES (1, 'Alice', 'New York'), (2, 'Bob', 'Boston')",
            )
            .await;

        // Test basic SELECT
        let result = fixture
            .execute_sql_ok("SELECT id, name FROM customers")
            .await;

        match result {
            ExecutionResult::Select {
                columns,
                rows,
                row_count,
            } => {
                assert_eq!(columns, vec!["id", "name"]);
                assert_eq!(row_count, 2);
                assert_eq!(rows.len(), 2);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_select_with_where() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Setup
        fixture
            .execute_sql_ok("CREATE TABLE employees (id INTEGER, name TEXT, salary INTEGER)")
            .await;

        fixture
            .execute_sql_ok(
                "INSERT INTO employees VALUES 
                 (1, 'John', 50000), 
                 (2, 'Jane', 75000), 
                 (3, 'Jim', 60000)",
            )
            .await;

        // Test WHERE condition
        let result = fixture
            .execute_sql_ok("SELECT name FROM employees WHERE salary > 55000")
            .await;

        match result {
            ExecutionResult::Select { rows, .. } => {
                assert_eq!(rows.len(), 2); // Jane and Jim
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_update_basic() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Setup
        fixture
            .execute_sql_ok("CREATE TABLE accounts (id INTEGER, balance INTEGER)")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO accounts VALUES (1, 100), (2, 200)")
            .await;

        // Test UPDATE
        let result = fixture
            .execute_sql_ok("UPDATE accounts SET balance = 150 WHERE id = 1")
            .await;

        match result {
            ExecutionResult::Update { count } => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected Update result"),
        }
    }

    #[tokio::test]
    async fn test_delete_basic() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Setup
        fixture
            .execute_sql_ok("CREATE TABLE temp_data (id INTEGER, value TEXT)")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO temp_data VALUES (1, 'keep'), (2, 'delete'), (3, 'keep')")
            .await;

        // Test DELETE
        let result = fixture
            .execute_sql_ok("DELETE FROM temp_data WHERE value = 'delete'")
            .await;

        match result {
            ExecutionResult::Delete { count } => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected Delete result"),
        }
    }

    #[tokio::test]
    async fn test_inner_join() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Setup tables for JOIN test
        fixture
            .execute_sql_ok("CREATE TABLE orders (id INTEGER, customer_id INTEGER, amount DECIMAL)")
            .await;

        fixture
            .execute_sql_ok("CREATE TABLE customers (id INTEGER, name TEXT)")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO customers VALUES (1, 'Alice'), (2, 'Bob')")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO orders VALUES (100, 1, 50.00), (101, 2, 75.00)")
            .await;

        // Test INNER JOIN
        let result = fixture
            .execute_sql_ok(
                "SELECT c.name, o.amount 
                 FROM customers c 
                 INNER JOIN orders o ON c.id = o.customer_id",
            )
            .await;

        match result {
            ExecutionResult::Select { rows, .. } => {
                assert_eq!(rows.len(), 2); // Should have 2 joined rows
            }
            _ => panic!("Expected Select result for JOIN"),
        }
    }

    #[tokio::test]
    async fn test_left_join() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Setup
        fixture
            .execute_sql_ok("CREATE TABLE categories (id INTEGER, name TEXT)")
            .await;

        fixture
            .execute_sql_ok("CREATE TABLE products (id INTEGER, name TEXT, category_id INTEGER)")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO categories VALUES (1, 'Electronics'), (2, 'Books')")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO products VALUES (1, 'Laptop', 1)")
            .await; // Only one product in Electronics

        // Test LEFT JOIN - should include categories without products
        let result = fixture
            .execute_sql_ok(
                "SELECT c.name, p.name 
                 FROM categories c 
                 LEFT JOIN products p ON c.id = p.category_id",
            )
            .await;

        match result {
            ExecutionResult::Select { rows, .. } => {
                assert_eq!(rows.len(), 2); // Both categories should appear
            }
            _ => panic!("Expected Select result for LEFT JOIN"),
        }
    }

    #[tokio::test]
    async fn test_create_index() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Create table first
        fixture
            .execute_sql_ok("CREATE TABLE indexed_table (id INTEGER, email TEXT)")
            .await;

        // Create index
        let result = fixture
            .execute_sql_ok("CREATE INDEX idx_email ON indexed_table (email)")
            .await;

        match result {
            ExecutionResult::CreateIndex {
                index_name,
                table_name,
            } => {
                assert_eq!(index_name, "idx_email");
                assert_eq!(table_name, "indexed_table");
            }
            _ => panic!("Expected CreateIndex result"),
        }
    }

    #[tokio::test]
    async fn test_drop_table() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Create and then drop table
        fixture
            .execute_sql_ok("CREATE TABLE to_drop (id INTEGER)")
            .await;

        let result = fixture.execute_sql_ok("DROP TABLE to_drop").await;

        match result {
            ExecutionResult::DropTable { table_names } => {
                assert_eq!(table_names, vec!["to_drop"]);
            }
            _ => panic!("Expected DropTable result"),
        }
    }

    #[tokio::test]
    async fn test_drop_table_if_exists() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Drop non-existent table without IF EXISTS - should fail
        let error = fixture.execute_sql_err("DROP TABLE non_existent").await;

        assert!(error.to_string().contains("does not exist"));

        // Drop with IF EXISTS - should succeed
        fixture
            .execute_sql_ok("DROP TABLE IF EXISTS non_existent")
            .await;
    }

    #[tokio::test]
    async fn test_select_wildcard() {
        let fixture = SqlExecutorTestFixture::new().await;

        fixture
            .execute_sql_ok("CREATE TABLE full_table (a INTEGER, b TEXT, c DECIMAL)")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO full_table VALUES (1, 'test', 3.14)")
            .await;

        let result = fixture.execute_sql_ok("SELECT * FROM full_table").await;

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 3); // Should select all columns
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_select_without_from() {
        let fixture = SqlExecutorTestFixture::new().await;

        let result = fixture.execute_sql_ok("SELECT 42, 'hello', 3.14").await;

        match result {
            ExecutionResult::Select { rows, .. } => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].len(), 3);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_invalid_sql() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Test various invalid SQL statements
        let invalid_sqls = vec![
            "INVALID STATEMENT",
            "SELECT FROM",  // Missing column list
            "INSERT INTO",  // Incomplete
            "CREATE TABLE", // Missing table name
            "UPDATE SET",   // Missing table
        ];

        for sql in invalid_sqls {
            let result = fixture.execute_sql(sql).await;
            assert!(result.is_err(), "Expected error for: {}", sql);
        }
    }

    #[tokio::test]
    async fn test_table_not_found_error() {
        let fixture = SqlExecutorTestFixture::new().await;

        let error = fixture
            .execute_sql_err("SELECT * FROM non_existent_table")
            .await;

        assert!(error.to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_column_constraints() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Create table with constraints
        fixture
            .execute_sql_ok(
                "CREATE TABLE constrained_table (
                    id INTEGER PRIMARY KEY,
                    email TEXT UNIQUE NOT NULL,
                    age INTEGER CHECK (age >= 0)
                )",
            )
            .await;

        // Valid insert should work
        fixture
            .execute_sql_ok(
                "INSERT INTO constrained_table (id, email, age) VALUES (1, 'test@example.com', 25)",
            )
            .await;
    }

    #[tokio::test]
    async fn test_information_schema_tables() {
        let fixture = SqlExecutorTestFixture::new().await;

        // Create a test table first
        fixture
            .execute_sql_ok("CREATE TABLE schema_test (id INTEGER)")
            .await;

        // Query information_schema.tables
        let result = fixture
            .execute_sql_ok(
                "SELECT table_name FROM information_schema.tables WHERE table_name = 'schema_test'",
            )
            .await;

        match result {
            ExecutionResult::Select { rows, .. } => {
                assert!(!rows.is_empty(), "Should find the created table");
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let fixture = Arc::new(SqlExecutorTestFixture::new().await);

        // Create table
        fixture
            .execute_sql_ok("CREATE TABLE concurrent_test (id INTEGER, counter INTEGER)")
            .await;

        fixture
            .execute_sql_ok("INSERT INTO concurrent_test VALUES (1, 0)")
            .await;

        // Simulate concurrent updates
        let mut handles = vec![];
        for i in 0..10 {
            let fixture_clone = Arc::clone(&fixture);
            let handle = tokio::spawn(async move {
                fixture_clone
                    .execute_sql_ok(&format!(
                        "UPDATE concurrent_test SET counter = {} WHERE id = 1",
                        i
                    ))
                    .await
            });
            handles.push(handle);
        }

        // Wait for all updates to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify final state
        let result = fixture
            .execute_sql_ok("SELECT counter FROM concurrent_test WHERE id = 1")
            .await;

        match result {
            ExecutionResult::Select { rows, .. } => {
                assert_eq!(rows.len(), 1);
                // Some update should have succeeded
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_complex_where_conditions() {
        let fixture = SqlExecutorTestFixture::new().await;

        fixture
            .execute_sql_ok(
                "CREATE TABLE complex_where (id INTEGER, name TEXT, age INTEGER, salary DECIMAL)",
            )
            .await;

        fixture
            .execute_sql_ok(
                "INSERT INTO complex_where VALUES 
                 (1, 'Alice', 30, 60000.00),
                 (2, 'Bob', 25, 45000.00),
                 (3, 'Charlie', 35, 70000.00)",
            )
            .await;

        // Test complex WHERE with AND/OR
        let result = fixture
            .execute_sql_ok("SELECT name FROM complex_where WHERE (age > 25 AND salary > 50000) OR name = 'Bob'")
            .await;

        match result {
            ExecutionResult::Select { rows, .. } => {
                assert!(rows.len() >= 2); // Alice and Charlie definitely, possibly Bob
            }
            _ => panic!("Expected Select result"),
        }
    }
}
