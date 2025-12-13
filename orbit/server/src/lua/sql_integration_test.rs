//! End-to-End SQL Integration Tests for Lua UDFs
//!
//! Tests the full pipeline: SQL -> Parser -> QueryEngine -> UdfHandler -> LuaRuntime

#[cfg(all(test, feature = "lua-mlua"))]
mod tests {
    use crate::protocols::postgres_wire::sql::query_engine::OptimizedQueryEngine;
    use crate::protocols::postgres_wire::sql::executor::ExecutionResult;


    async fn create_engine() -> OptimizedQueryEngine {
        OptimizedQueryEngine::new_default().await.expect("Failed to create query engine")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_create_select_drop_udf_flow() {
        let engine = create_engine().await;

        // 1. Create a simple Lua UDF
        let create_sql = r#"
            CREATE FUNCTION add_integers(a INTEGER, b INTEGER)
            RETURNS INTEGER
            LANGUAGE lua
            AS $$
                return a + b
            $$;
        "#;
        
        let result = engine.execute(create_sql).await.expect("Failed to execute CREATE FUNCTION");
        if let ExecutionResult::Show { variable, value } = result.result {
             assert_eq!(variable, "CREATE FUNCTION add_integers");
             assert_eq!(value, "OK");
        } else {
             panic!("Expected Show result for CREATE FUNCTION");
        }

        // 2. Call the UDF using SELECT (Evaluation via ExpressionEvaluator would happen here)
        // Note: Currently, simple SELECT evaluation of scalar functions might not be fully routed 
        // through the UDF engine if generic expression evaluation isn't modifying the plan. 
        // However, we verify the function persists in the system.
        // To verify EXECUTION, we need to ensure the expression evaluator calls the registry.
        // Assuming expression evaluation is wired up (based on user summary saying "Expression evaluator integration ✅").

        // Let's rely on `udf_handler.list_functions()` or similar verification if direct SELECT isn't fully implemented 
        // for scalar expressions without a table. 
        // But let's try a SELECT with a dummy table if needed, or just scalar SELECT.
        
        // For now, let's verify specific execution if possible, or at least the registry state if we can access it.
        // But `OptimizedQueryEngine` doesn't expose the registry directly easily.
        // Let's assume `SELECT add_integers(5, 10)` should work if the evaluator is ready.
        
        // TODO: The summary says "Expression Evaluator Integration: Fully Functional".
        // "SELECT add_numbers(5, 10); -- Returns 15"
        
        let select_sql = "SELECT add_integers(5, 10)";
        // If the parser supports function calls in projection, this should work.
        // Warning: if `SELECT 1` works, `SELECT func()` might work too.
        
        // Note: The `execute` method in `query_engine.rs` falls back to `execute_standard` for non-SIMD.
        // `execute_standard` uses `SqlExecutor`. Does `SqlExecutor` support UDFs?
        // IF `SqlExecutor` uses `ExpressionEvaluator`, and `ExpressionEvaluator` uses `UdfRegistry`, then yes.
        
        match engine.execute(select_sql).await {
            Ok(execution_result) => {
                 match execution_result.result {
                     ExecutionResult::Select { rows, .. } => {
                         // Expect 1 row, 1 column, value 15
                         assert_eq!(rows.len(), 1);
                         assert_eq!(rows[0].len(), 1);
                         match &rows[0][0] {
                             Some(val) => assert_eq!(val, "15"), // Results usually stringified in some paths or SqlValue
                             None => panic!("Expected value 15, got None"),
                         }
                     },
                     _ => panic!("Expected Select result"),
                 }
            },
            Err(e) => {
                panic!("SELECT execution failed: {}", e);
            }
        }

        // 3. Drop the function
        let drop_sql = "DROP FUNCTION add_integers";
        let result = engine.execute(drop_sql).await.expect("Failed to execute DROP FUNCTION");
        
        if let ExecutionResult::Show { variable, value } = result.result {
             assert_eq!(variable, "DROP FUNCTION add_integers");
             assert_eq!(value, "OK");
        } else {
             panic!("Expected Show result for DROP FUNCTION");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_udf_replacement() {
        let engine = create_engine().await;

        let create_v1 = r#"
            CREATE FUNCTION my_calc(x INTEGER) RETURNS INTEGER LANGUAGE lua AS $$ return x * 2 $$;
        "#;
        engine.execute(create_v1).await.expect("Create failed");

        let create_v2 = r#"
            CREATE OR REPLACE FUNCTION my_calc(x INTEGER) RETURNS INTEGER LANGUAGE lua AS $$ return x * 10 $$;
        "#;
        engine.execute(create_v2).await.expect("Replace failed");

        // Verify result (assuming SELECT works)
        let result = engine.execute("SELECT my_calc(5)").await.expect("Select failed");
        if let ExecutionResult::Select { rows, .. } = result.result {
             // Expect 50, not 10
             // Assuming rows contain stringified values for now based on standard Postgres wire behavior
             assert_eq!(rows[0][0].as_deref(), Some("50"));
        }
    }
}
