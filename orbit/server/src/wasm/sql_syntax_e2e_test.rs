#![cfg(all(test, feature = "wasm-udf"))]

//! End-to-end SQL syntax tests for WASM UDFs
//!
//! These tests validate that the PostgreSQL query engine accepts
//! CREATE FUNCTION and DROP FUNCTION statements for LANGUAGE WASM,
//! and that registration routes through the WASM UDF handler.

mod tests {
    use crate::protocols::postgres_wire::sql::executor::ExecutionResult;
    use crate::protocols::postgres_wire::sql::query_engine::OptimizedQueryEngine;

    // Minimal WASM module exporting: (func (export "add") (param i32 i32) (result i32) local.get 0 local.get 1 i32.add)
    // Hex encoding used by CREATE FUNCTION ... LANGUAGE WASM AS '<hex>';
    const ADD_WASM_HEX: &str =
        "0x0061736d0100000001070160027f7f017f030201000707010361646400000a09010700200020016a0b";

    #[tokio::test]
    async fn test_create_function_wasm_sql_syntax() {
        // Spin up the optimized query engine (will initialize WASM registry)
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        // CREATE FUNCTION with LANGUAGE WASM using hex-encoded module
        let sql = format!(
            r#"
            CREATE FUNCTION add(a INTEGER, b INTEGER)
            RETURNS INTEGER
            LANGUAGE wasm
            AS '{hex}';
        "#,
            hex = ADD_WASM_HEX
        );

        let result = engine.execute(&sql).await;

        // Should succeed
        assert!(
            result.is_ok(),
            "CREATE FUNCTION should succeed: {:?}",
            result.err()
        );

        let exec_result = result.unwrap();
        match exec_result.result {
            ExecutionResult::Show { variable, value } => {
                assert!(
                    variable.to_uppercase().contains("CREATE FUNCTION"),
                    "Expected 'CREATE FUNCTION' in variable, got: {}",
                    variable
                );
                assert_eq!(value, "OK");
            }
            other => panic!("Expected Show result for CREATE, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_create_and_drop_wasm_function() {
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        // Create function
        let create_sql = format!(
            r#"
            CREATE FUNCTION wasm_add(a INTEGER, b INTEGER)
            RETURNS INTEGER
            LANGUAGE wasm
            AS '{hex}';
        "#,
            hex = ADD_WASM_HEX
        );

        let create_result = engine.execute(&create_sql).await;
        assert!(
            create_result.is_ok(),
            "CREATE (WASM) should succeed: {:?}",
            create_result.err()
        );

        // Drop the function
        let drop_sql = "DROP FUNCTION wasm_add";
        let drop_result = engine.execute(drop_sql).await;
        assert!(
            drop_result.is_ok(),
            "DROP (WASM) should succeed: {:?}",
            drop_result.err()
        );

        // Optional: check the SHOW result for DROP
        match drop_result.unwrap().result {
            ExecutionResult::Show { variable, value } => {
                assert!(
                    variable.to_uppercase().contains("DROP FUNCTION"),
                    "Expected 'DROP FUNCTION' in variable, got: {}",
                    variable
                );
                assert_eq!(value, "OK");
            }
            other => panic!("Expected Show result for DROP, got: {:?}", other),
        }
    }

    // NOTE: This test exercises dynamic WASM UDF execution via the synchronous SQL
    // expression evaluator. The evaluator currently performs async WASM execution
    // by blocking the current thread, which is not permitted when running inside
    // a Tokio runtime (it can trigger "Cannot start a runtime from within a runtime").
    //
    // Once the expression evaluation path is made async end-to-end (or WASM execution
    // is moved out of the sync evaluator), this can be un-ignored.
    #[tokio::test]
    #[ignore]
    async fn test_select_wasm_execution() {
        let engine = OptimizedQueryEngine::new_default()
            .await
            .expect("Failed to create query engine");

        let create_sql = format!(
            r#"
                CREATE FUNCTION wasm_sum(a INTEGER, b INTEGER)
                RETURNS INTEGER
                LANGUAGE wasm
                AS '{hex}';
            "#,
            hex = ADD_WASM_HEX
        );
        engine
            .execute(&create_sql)
            .await
            .expect("CREATE wasm_sum should succeed");

        let select_sql = "SELECT wasm_sum(5, 3)";
        let res = engine
            .execute(select_sql)
            .await
            .expect("SELECT should succeed");
        match res.result {
            ExecutionResult::Select { rows, .. } => {
                assert!(
                    !rows.is_empty() && !rows[0].is_empty(),
                    "Expected at least one row and one column from SELECT"
                );
                let val = rows[0][0].as_deref();
                assert_eq!(
                    val,
                    Some("8"),
                    "Expected wasm_sum(5, 3) = 8, got {:?}",
                    rows[0][0]
                );
            }
            other => panic!("Expected Select result, got {:?}", other),
        }

        // Cleanup
        let _ = engine.execute("DROP FUNCTION wasm_sum").await;
    }
}
