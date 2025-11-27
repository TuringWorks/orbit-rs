//! OrbitQL Documentation Verification Tests
//!
//! These tests verify that the examples from the OrbitQL documentation
//! (docs/ORBITQL_COMPLETE_DOCUMENTATION.md) work correctly.
//!
//! Each test corresponds to an example from the documentation and ensures
//! the SQL parser and executor handle the documented syntax.

#![cfg(feature = "postgres-wire")]

use orbit_protocols::postgres_wire::sql::executor::{ExecutionResult, SqlExecutor};

// =============================================================================
// Basic SQL Features (from docs: Language Reference)
// =============================================================================

mod basic_sql_tests {
    use super::*;

    #[tokio::test]
    async fn test_doc_example_scalar_types() {
        let executor = SqlExecutor::new().await.unwrap();

        // From docs: Data Types - Scalar types
        let sql = "SELECT 42 as integer_val, 3.14159 as float_val, 'Hello World' as string_val, true as boolean_val";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"integer_val".to_string()));
                assert!(columns.contains(&"float_val".to_string()));
                assert!(columns.contains(&"string_val".to_string()));
                assert!(columns.contains(&"boolean_val".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_select_all() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE users (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')")
            .await
            .unwrap();

        // From docs: Basic SELECT *
        let result = executor.execute("SELECT * FROM users").await.unwrap();

        match result {
            ExecutionResult::Select { columns, row_count, .. } => {
                assert_eq!(columns.len(), 2);
                assert!(row_count >= 2);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_select_with_alias() {
        let executor = SqlExecutor::new().await.unwrap();

        // Create table and test column alias on actual table column
        executor
            .execute("CREATE TABLE aliased_test (value INTEGER)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO aliased_test VALUES (42)")
            .await
            .unwrap();

        // Column aliasing on table columns
        let sql = "SELECT value as val FROM aliased_test";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, row_count, .. } => {
                // Verify we get 1 column and 1 row
                assert_eq!(columns.len(), 1);
                assert_eq!(row_count, 1);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_select_distinct() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE colors (name TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO colors VALUES ('red'), ('blue'), ('red'), ('green'), ('blue')")
            .await
            .unwrap();

        // From docs: SELECT [DISTINCT]
        let result = executor
            .execute("SELECT DISTINCT name FROM colors")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0], "name");
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// WHERE Clause Tests
// =============================================================================

mod where_clause_tests {
    use super::*;

    #[tokio::test]
    async fn test_doc_example_where_equals() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE users (id INTEGER, name TEXT, location TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, 'Alice', 'San Francisco'), (2, 'Bob', 'New York')")
            .await
            .unwrap();

        // From docs: WHERE conditions
        let result = executor
            .execute("SELECT * FROM users WHERE location = 'San Francisco'")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 3);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_where_and() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE employees (id INTEGER, department TEXT, salary INTEGER)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO employees VALUES (1, 'Engineering', 100000), (2, 'Engineering', 80000), (3, 'Sales', 90000)")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM employees WHERE department = 'Engineering' AND salary > 90000")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 3);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_where_or() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE products (id INTEGER, category TEXT, price REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO products VALUES (1, 'electronics', 999.99), (2, 'books', 29.99), (3, 'clothing', 49.99)")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM products WHERE category = 'electronics' OR category = 'books'")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 3);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_where_in() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE items (id INTEGER, status TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO items VALUES (1, 'active'), (2, 'pending'), (3, 'inactive'), (4, 'active')")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM items WHERE status IN ('active', 'pending')")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_where_between() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE scores (id INTEGER, value INTEGER)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO scores VALUES (1, 50), (2, 75), (3, 90), (4, 100)")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM scores WHERE value BETWEEN 60 AND 95")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_where_like() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE files (id INTEGER, filename TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO files VALUES (1, 'report.pdf'), (2, 'data.csv'), (3, 'report_v2.pdf')")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM files WHERE filename LIKE 'report%'")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_where_is_null() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE contacts (id INTEGER, email TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO contacts VALUES (1, 'alice@example.com'), (2, NULL)")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM contacts WHERE email IS NULL")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// CASE Expression Tests (from docs: Advanced SQL Features)
// =============================================================================

mod case_expression_tests {
    use super::*;

    #[tokio::test]
    // Note: Simple CASE works, but CASE with COUNT(*) in aggregation doesn't
    async fn test_doc_example_simple_case() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, 'Alice', 15), (2, 'Bob', 35), (3, 'Charlie', 70)")
            .await
            .unwrap();

        // From docs: Simple CASE
        let sql = r#"
            SELECT
                name,
                age,
                CASE
                    WHEN age < 18 THEN 'Minor'
                    WHEN age < 65 THEN 'Adult'
                    ELSE 'Senior'
                END AS age_category
            FROM users
        "#;
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"name".to_string()));
                assert!(columns.contains(&"age".to_string()));
                assert!(columns.contains(&"age_category".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "CASE with COUNT(*) in aggregation not yet supported"]
    async fn test_doc_example_case_in_aggregation() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE employees (id INTEGER, department TEXT, salary INTEGER, performance_rating TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO employees VALUES (1, 'Engineering', 120000, 'excellent'), (2, 'Engineering', 80000, 'good'), (3, 'Sales', 90000, 'excellent')")
            .await
            .unwrap();

        // From docs: CASE in aggregations
        let sql = r#"
            SELECT
                department,
                COUNT(*) AS total_employees,
                COUNT(CASE WHEN salary > 100000 THEN 1 END) AS high_earners
            FROM employees
            GROUP BY department
        "#;
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"department".to_string()));
                assert!(columns.contains(&"total_employees".to_string()));
                assert!(columns.contains(&"high_earners".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// Aggregate Function Tests (from docs: Enhanced Aggregates)
// =============================================================================

mod aggregate_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "COUNT(*) syntax not yet implemented - TODO: implement aggregate with star"]
    async fn test_doc_example_count() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE orders (id INTEGER, customer_id INTEGER)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO orders VALUES (1, 100), (2, 100), (3, 101), (4, 102)")
            .await
            .unwrap();

        let result = executor.execute("SELECT COUNT(*) as total FROM orders").await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"total".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "COUNT(DISTINCT) not yet implemented"]
    async fn test_doc_example_count_distinct() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE ad_clicks (id INTEGER, campaign_id INTEGER, user_id INTEGER, session_id INTEGER)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO ad_clicks VALUES (1, 1, 100, 1000), (2, 1, 100, 1001), (3, 1, 101, 1002)")
            .await
            .unwrap();

        // From docs: COUNT(DISTINCT)
        let sql = r#"
            SELECT
                campaign_id,
                COUNT(*) AS total_clicks,
                COUNT(DISTINCT user_id) AS unique_users,
                COUNT(DISTINCT session_id) AS unique_sessions
            FROM ad_clicks
            GROUP BY campaign_id
        "#;
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"campaign_id".to_string()));
                assert!(columns.contains(&"total_clicks".to_string()));
                assert!(columns.contains(&"unique_users".to_string()));
                assert!(columns.contains(&"unique_sessions".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_sum_avg_min_max() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE sales (id INTEGER, amount REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO sales VALUES (1, 100.50), (2, 250.75), (3, 75.25), (4, 400.00)")
            .await
            .unwrap();

        let sql = "SELECT SUM(amount) as total, AVG(amount) as average, MIN(amount) as minimum, MAX(amount) as maximum FROM sales";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"total".to_string()));
                assert!(columns.contains(&"average".to_string()));
                assert!(columns.contains(&"minimum".to_string()));
                assert!(columns.contains(&"maximum".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_group_by() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE sales (id INTEGER, category TEXT, amount REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO sales VALUES (1, 'electronics', 500), (2, 'books', 50), (3, 'electronics', 300), (4, 'books', 25)")
            .await
            .unwrap();

        let sql = "SELECT category, SUM(amount) as total FROM sales GROUP BY category";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"category".to_string()));
                assert!(columns.contains(&"total".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_having() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE orders (id INTEGER, customer_id INTEGER, amount REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO orders VALUES (1, 1, 100), (2, 1, 200), (3, 2, 50), (4, 1, 150)")
            .await
            .unwrap();

        let sql = "SELECT customer_id, SUM(amount) as total FROM orders GROUP BY customer_id HAVING SUM(amount) > 100";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"customer_id".to_string()));
                assert!(columns.contains(&"total".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// JOIN Tests
// =============================================================================

mod join_tests {
    use super::*;

    #[tokio::test]
    async fn test_doc_example_inner_join() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE users (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("CREATE TABLE orders (id INTEGER, user_id INTEGER, amount REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO orders VALUES (1, 1, 100), (2, 1, 200), (3, 2, 150)")
            .await
            .unwrap();

        let sql = "SELECT u.name, o.amount FROM users u JOIN orders o ON u.id = o.user_id";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_left_join() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE users (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("CREATE TABLE orders (id INTEGER, user_id INTEGER, amount REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie')")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO orders VALUES (1, 1, 100), (2, 1, 200)")
            .await
            .unwrap();

        // From docs: LEFT JOIN
        let sql = "SELECT u.name, o.amount FROM users u LEFT JOIN orders o ON u.id = o.user_id";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// ORDER BY and LIMIT Tests
// =============================================================================

mod ordering_tests {
    use super::*;

    #[tokio::test]
    async fn test_doc_example_order_by_asc() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE products (id INTEGER, name TEXT, price REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO products VALUES (1, 'Widget', 29.99), (2, 'Gadget', 99.99), (3, 'Thing', 9.99)")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM products ORDER BY price ASC")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 3);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_order_by_desc() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE products (id INTEGER, name TEXT, price REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO products VALUES (1, 'Widget', 29.99), (2, 'Gadget', 99.99), (3, 'Thing', 9.99)")
            .await
            .unwrap();

        let result = executor
            .execute("SELECT * FROM products ORDER BY price DESC")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 3);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "LIMIT clause not yet implemented"]
    async fn test_doc_example_limit() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE items (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO items VALUES (1, 'A'), (2, 'B'), (3, 'C'), (4, 'D'), (5, 'E')")
            .await
            .unwrap();

        // From docs: LIMIT count
        let result = executor.execute("SELECT * FROM items LIMIT 3").await.unwrap();

        match result {
            ExecutionResult::Select { row_count, .. } => {
                assert!(row_count <= 3);
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "LIMIT OFFSET clause not yet implemented"]
    async fn test_doc_example_limit_offset() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE items (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO items VALUES (1, 'A'), (2, 'B'), (3, 'C'), (4, 'D'), (5, 'E')")
            .await
            .unwrap();

        // From docs: LIMIT count OFFSET start
        let result = executor
            .execute("SELECT * FROM items LIMIT 2 OFFSET 2")
            .await
            .unwrap();

        match result {
            ExecutionResult::Select { row_count, .. } => {
                assert!(row_count <= 2);
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// Subquery Tests
// =============================================================================

mod subquery_tests {
    use super::*;

    #[tokio::test]
    async fn test_doc_example_subquery_in_where() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE orders (id INTEGER, product_id INTEGER, quantity INTEGER)")
            .await
            .unwrap();
        executor
            .execute("CREATE TABLE products (id INTEGER, name TEXT, category TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO products VALUES (1, 'Laptop', 'electronics'), (2, 'Book', 'books')")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO orders VALUES (1, 1, 5), (2, 2, 10)")
            .await
            .unwrap();

        let sql = "SELECT * FROM orders WHERE product_id IN (SELECT id FROM products WHERE category = 'electronics')";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert_eq!(columns.len(), 3);
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// CTE (Common Table Expression) Tests (from docs: Advanced SQL Features)
// =============================================================================

mod cte_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "WITH (CTE) clause not yet implemented"]
    async fn test_doc_example_basic_cte() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE posts (id INTEGER, user_id INTEGER, created_at TEXT)")
            .await
            .unwrap();
        executor
            .execute("CREATE TABLE users (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO posts VALUES (1, 1, '2024-01-01'), (2, 1, '2024-01-02'), (3, 2, '2024-01-01')")
            .await
            .unwrap();

        // From docs: Basic CTE
        let sql = r#"
            WITH user_stats AS (
                SELECT user_id, COUNT(*) AS post_count
                FROM posts
                GROUP BY user_id
            )
            SELECT u.name, COALESCE(us.post_count, 0) AS recent_posts
            FROM users u
            LEFT JOIN user_stats us ON u.id = us.user_id
        "#;
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"name".to_string()));
                assert!(columns.contains(&"recent_posts".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "WITH (CTE) clause not yet implemented"]
    async fn test_doc_example_multiple_ctes() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE sessions (id INTEGER, user_id INTEGER, last_seen TEXT)")
            .await
            .unwrap();
        executor
            .execute("CREATE TABLE likes (id INTEGER, post_id INTEGER)")
            .await
            .unwrap();
        executor
            .execute("CREATE TABLE users (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("CREATE TABLE posts (id INTEGER, author_id INTEGER, title TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, 'Alice')")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO sessions VALUES (1, 1, '2024-01-15')")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO posts VALUES (1, 1, 'My Post')")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO likes VALUES (1, 1), (2, 1), (3, 1)")
            .await
            .unwrap();

        // From docs: Multiple CTEs (simplified version)
        let sql = r#"
            WITH
                active_users AS (
                    SELECT user_id FROM sessions
                ),
                popular_posts AS (
                    SELECT post_id, COUNT(*) AS like_count
                    FROM likes
                    GROUP BY post_id
                )
            SELECT u.name, p.title, pp.like_count
            FROM users u
            JOIN active_users au ON u.id = au.user_id
            JOIN posts p ON u.id = p.author_id
            JOIN popular_posts pp ON p.id = pp.post_id
        "#;
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"name".to_string()));
                assert!(columns.contains(&"title".to_string()));
                assert!(columns.contains(&"like_count".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// NULL Handling Tests
// =============================================================================

mod null_handling_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "COALESCE function not yet implemented"]
    async fn test_doc_example_coalesce() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE users (id INTEGER, nickname TEXT, username TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES (1, NULL, 'alice123'), (2, 'Bobby', 'bob456')")
            .await
            .unwrap();

        let sql = "SELECT id, COALESCE(nickname, username) as display_name FROM users";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"id".to_string()));
                assert!(columns.contains(&"display_name".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "NULLIF function not yet implemented"]
    async fn test_doc_example_nullif() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE values_table (id INTEGER, value INTEGER)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO values_table VALUES (1, 0), (2, 5), (3, 10)")
            .await
            .unwrap();

        // NULLIF returns NULL if both arguments are equal
        let sql = "SELECT id, NULLIF(value, 0) as non_zero_value FROM values_table";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"id".to_string()));
                assert!(columns.contains(&"non_zero_value".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// String Function Tests
// =============================================================================

mod string_function_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "UPPER/LOWER string functions not yet implemented"]
    async fn test_doc_example_upper_lower() {
        let executor = SqlExecutor::new().await.unwrap();

        let sql = "SELECT UPPER('hello') as upper_val, LOWER('WORLD') as lower_val";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"upper_val".to_string()));
                assert!(columns.contains(&"lower_val".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "String concatenation with || not yet implemented"]
    async fn test_doc_example_concat() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE users (first_name TEXT, last_name TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO users VALUES ('John', 'Doe'), ('Jane', 'Smith')")
            .await
            .unwrap();

        let sql = "SELECT first_name || ' ' || last_name as full_name FROM users";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"full_name".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "LENGTH string function not yet implemented"]
    async fn test_doc_example_length() {
        let executor = SqlExecutor::new().await.unwrap();

        let sql = "SELECT LENGTH('Hello World') as str_length";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"str_length".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "SUBSTR string function not yet implemented"]
    async fn test_doc_example_substr() {
        let executor = SqlExecutor::new().await.unwrap();

        let sql = "SELECT SUBSTR('Hello World', 1, 5) as substring_val";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"substring_val".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "TRIM string function not yet implemented"]
    async fn test_doc_example_trim() {
        let executor = SqlExecutor::new().await.unwrap();

        let sql = "SELECT TRIM('  hello  ') as trimmed";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"trimmed".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// Mathematical Function Tests
// =============================================================================

mod math_function_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "ABS function not yet implemented"]
    async fn test_doc_example_abs() {
        let executor = SqlExecutor::new().await.unwrap();

        let sql = "SELECT ABS(-42) as absolute_value";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"absolute_value".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    #[ignore = "ROUND function not yet implemented"]
    async fn test_doc_example_round() {
        let executor = SqlExecutor::new().await.unwrap();

        let sql = "SELECT ROUND(3.14159, 2) as rounded";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"rounded".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_arithmetic() {
        let executor = SqlExecutor::new().await.unwrap();

        let sql = "SELECT 10 + 5 as sum_val, 10 - 5 as diff_val, 10 * 5 as prod_val, 10 / 5 as quot_val, 10 % 3 as mod_val";
        let result = executor.execute(sql).await.unwrap();

        match result {
            ExecutionResult::Select { columns, .. } => {
                assert!(columns.contains(&"sum_val".to_string()));
                assert!(columns.contains(&"diff_val".to_string()));
                assert!(columns.contains(&"prod_val".to_string()));
                assert!(columns.contains(&"quot_val".to_string()));
                assert!(columns.contains(&"mod_val".to_string()));
            }
            _ => panic!("Expected Select result"),
        }
    }
}

// =============================================================================
// DDL Tests
// =============================================================================

mod ddl_tests {
    use super::*;

    #[tokio::test]
    async fn test_doc_example_create_table() {
        let executor = SqlExecutor::new().await.unwrap();

        let result = executor
            .execute("CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT UNIQUE)")
            .await
            .unwrap();

        match result {
            ExecutionResult::CreateTable { table_name } => {
                assert_eq!(table_name, "customers");
            }
            _ => panic!("Expected CreateTable result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_drop_table() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE temp_table (id INTEGER)")
            .await
            .unwrap();

        let result = executor.execute("DROP TABLE temp_table").await.unwrap();

        match result {
            ExecutionResult::DropTable { table_names } => {
                assert!(table_names.contains(&"temp_table".to_string()));
            }
            _ => panic!("Expected DropTable result"),
        }
    }
}

// =============================================================================
// DML Tests
// =============================================================================

mod dml_tests {
    use super::*;

    #[tokio::test]
    async fn test_doc_example_insert() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE products (id INTEGER, name TEXT, price REAL)")
            .await
            .unwrap();

        let result = executor
            .execute("INSERT INTO products VALUES (1, 'Widget', 29.99)")
            .await
            .unwrap();

        match result {
            ExecutionResult::Insert { count } => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected Insert result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_insert_multiple() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE products (id INTEGER, name TEXT, price REAL)")
            .await
            .unwrap();

        let result = executor
            .execute("INSERT INTO products VALUES (1, 'Widget', 29.99), (2, 'Gadget', 49.99), (3, 'Thing', 9.99)")
            .await
            .unwrap();

        match result {
            ExecutionResult::Insert { count } => {
                assert_eq!(count, 3);
            }
            _ => panic!("Expected Insert result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_update() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE products (id INTEGER, name TEXT, price REAL)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO products VALUES (1, 'Widget', 29.99)")
            .await
            .unwrap();

        let result = executor
            .execute("UPDATE products SET price = 24.99 WHERE id = 1")
            .await
            .unwrap();

        match result {
            ExecutionResult::Update { count } => {
                assert!(count >= 1);
            }
            _ => panic!("Expected Update result"),
        }
    }

    #[tokio::test]
    async fn test_doc_example_delete() {
        let executor = SqlExecutor::new().await.unwrap();

        executor
            .execute("CREATE TABLE products (id INTEGER, name TEXT)")
            .await
            .unwrap();
        executor
            .execute("INSERT INTO products VALUES (1, 'Widget'), (2, 'Gadget')")
            .await
            .unwrap();

        let result = executor
            .execute("DELETE FROM products WHERE id = 1")
            .await
            .unwrap();

        match result {
            ExecutionResult::Delete { count } => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected Delete result"),
        }
    }
}
