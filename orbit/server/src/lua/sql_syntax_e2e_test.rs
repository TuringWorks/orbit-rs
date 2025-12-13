//! End-to-end SQL syntax test for Lua UDFs
//!
//! Tests CREATE FUNCTION / DROP FUNCTION SQL statements through QueryEngine

#[cfg(all(test, feature = "lua-mlua"))]
mod tests {
    use crate::protocols::postgres_wire::sql::executor::ExecutionResult;
    use crate::protocols::postgres_wire::sql::query_engine::OptimizedQueryEngine;

    #[tokio::test]
    async fn test_create_function_sql_syntax() {
        // Create query engine (which initializes UDF handler automatically)
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        // Test CREATE FUNCTION with Lua
        let sql = r#"
            CREATE FUNCTION add_numbers(a INTEGER, b INTEGER)
            RETURNS INTEGER
            LANGUAGE lua
            AS $$
              return a + b
            $$
        "#;

        let result = engine.execute(sql).await;

        // Should succeed
        assert!(
            result.is_ok(),
            "CREATE FUNCTION should succeed: {:?}",
            result.err()
        );

        let exec_result = result.unwrap();
        match exec_result.result {
            ExecutionResult::Show { variable, value } => {
                assert!(variable.contains("CREATE FUNCTION"));
                assert_eq!(value, "OK");
            }
            other => panic!("Expected Show result, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_and_drop_function() {
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        // Create function
        let create_sql = r#"
            CREATE FUNCTION uppercase_text(input TEXT)
            RETURNS TEXT
            LANGUAGE lua
            AS $$
              return string.upper(input)
            $$
        "#;

        let result = engine.execute(create_sql).await;
        assert!(result.is_ok(), "CREATE should succeed");

        // Drop function
        let drop_sql = "DROP FUNCTION uppercase_text";
        let result = engine.execute(drop_sql).await;
        assert!(result.is_ok(), "DROP should succeed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_create_or_replace_function() {
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        // Create function
        let sql1 = r#"
            CREATE FUNCTION my_func(x INTEGER)
            RETURNS INTEGER
            LANGUAGE lua
            AS $$
              return x * 2
            $$
        "#;

        engine
            .execute(sql1)
            .await
            .expect("First CREATE should succeed");

        // Replace it
        let sql2 = r#"
            CREATE OR REPLACE FUNCTION my_func(x INTEGER)
            RETURNS INTEGER
            LANGUAGE lua
            AS $$
              return x * 3
            $$
        "#;

        let result = engine.execute(sql2).await;
        assert!(result.is_ok(), "CREATE OR REPLACE should succeed");
    }

    #[tokio::test]
    async fn test_drop_function_if_exists() {
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        // Drop non-existent function with IF EXISTS - should not error
        let sql = "DROP FUNCTION IF EXISTS nonexistent_function";
        let result = engine.execute(sql).await;
        assert!(
            result.is_ok(),
            "DROP IF EXISTS should not error for non-existent function"
        );
    }

    #[tokio::test]
    async fn test_create_function_with_multiple_parameters() {
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        let sql = r#"
            CREATE FUNCTION calculate_discount(price REAL, quantity INTEGER, rate REAL)
            RETURNS REAL
            LANGUAGE lua
            AS $$
              local total = price * quantity
              return total * (1 - rate)
            $$
        "#;

        let result = engine.execute(sql).await;
        assert!(
            result.is_ok(),
            "Multi-parameter CREATE should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_create_function_complex_types() {
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        // Function that works with arrays
        let sql = r#"
            CREATE FUNCTION sum_array(numbers INTEGER[])
            RETURNS INTEGER
            LANGUAGE lua
            AS $$
              local sum = 0
              for _, v in ipairs(numbers) do
                sum = sum + v
              end
              return sum
            $$
        "#;

        let result = engine.execute(sql).await;
        assert!(
            result.is_ok(),
            "Array parameter CREATE should succeed: {:?}",
            result.err()
        );
    }
}
