//! CQL Integration Tests with Infrastructure Setup
//!
//! These tests require proper setup and teardown of test infrastructure.
//! They are marked as slow tests and include embedded setup/teardown.

#[path = "cql_test_helpers.rs"]
mod helpers;
use helpers::*;

use orbit_protocols::cql::{CqlParser, CqlStatement};
use orbit_protocols::postgres_wire::sql::executor::ExecutionResult;

// ============================================================================
// Query Execution Tests with Setup/Teardown
// ============================================================================

#[tokio::test]
async fn test_cql_select_with_where_integration() {
    let ctx = CqlTestContext::new().await;
    
    // Setup
    create_test_users_table(&ctx).await;
    
    // Verify table was created
    match ctx.verify_data("SELECT COUNT(*) FROM test_users").await {
        Ok(_) => println!("Table created successfully"),
        Err(e) => panic!("Table creation failed or table not found: {:?}", e),
    }
    
    ctx.insert_data_sql("INSERT INTO test_users (id, name) VALUES (1, 'Alice')").await;
    
    // Verify data was inserted
    match ctx.verify_data("SELECT * FROM test_users WHERE id = 1").await {
        Ok(ExecutionResult::Select { row_count, .. }) => {
            assert_eq!(row_count, 1, "Data should be inserted");
        }
        Ok(other) => panic!("Expected Select result after insert, got: {:?}", other),
        Err(e) => panic!("Failed to verify data: {:?}", e),
    }
    
    // Test
    let parser = CqlParser::new();
    let stmt = parser.parse("SELECT * FROM test_users WHERE id = 1").unwrap();
    
    let result = ctx.adapter.execute_statement(&stmt, 0).await;
    match result {
        Ok(frame) => {
            if frame.opcode == orbit_protocols::cql::protocol::CqlOpcode::Error {
                // Decode error to see what went wrong
                use bytes::Buf;
                let mut body = frame.body.clone();
                let error_code = body.get_i32();
                let error_msg_len = body.get_i32();
                let error_msg_bytes = body.copy_to_bytes(error_msg_len as usize);
                let error_msg = String::from_utf8_lossy(&error_msg_bytes);
                panic!("CQL query returned Error opcode: code={}, message={}", error_code, error_msg);
            }
            // Should be Result opcode for successful query
            assert_eq!(frame.opcode, orbit_protocols::cql::protocol::CqlOpcode::Result, 
                "Expected Result opcode, got {:?}", frame.opcode);
        }
        Err(e) => {
            panic!("Query execution failed: {:?}", e);
        }
    }
    
    // Verify
    let verify_result = ctx.verify_data("SELECT * FROM test_users WHERE id = 1").await;
    match verify_result {
        Ok(ExecutionResult::Select { row_count, .. }) => {
            assert_eq!(row_count, 1);
        }
        Ok(other) => panic!("Expected Select result, got: {:?}", other),
        Err(e) => panic!("Failed to verify data: {:?}", e),
    }
    
    // Teardown
    ctx.cleanup_table("test_users").await;
}

#[tokio::test]
async fn test_cql_insert_execution_integration() {
    let ctx = CqlTestContext::new().await;
    
    // Setup
    create_test_users_table(&ctx).await;
    
    // Test
    let parser = CqlParser::new();
    let stmt = parser.parse("INSERT INTO test_users (id, name, age) VALUES (1, 'Alice', 30)").unwrap();
    
    let result = ctx.adapter.execute_statement(&stmt, 0).await.unwrap();
    assert_eq!(result.opcode, orbit_protocols::cql::protocol::CqlOpcode::Result);
    
    // Verify data was inserted
    let verify_result = ctx.verify_data("SELECT * FROM test_users WHERE id = 1").await;
    match verify_result {
        Ok(ExecutionResult::Select { rows, .. }) => {
            assert_eq!(rows.len(), 1);
        }
        Ok(other) => panic!("Expected Select result, got: {:?}", other),
        Err(e) => panic!("Failed to verify data: {:?}", e),
    }
    
    // Teardown
    ctx.cleanup_table("test_users").await;
}

#[tokio::test]
async fn test_cql_update_execution_integration() {
    let ctx = CqlTestContext::new().await;
    
    // Setup
    create_test_users_with_email_table(&ctx).await;
    ctx.insert_data_sql("INSERT INTO test_users (id, name, email) VALUES (1, 'Alice', 'old@example.com')").await;
    
    // Test
    let parser = CqlParser::new();
    let stmt = parser.parse("UPDATE test_users SET email = 'new@example.com' WHERE id = 1").unwrap();
    
    let result = ctx.adapter.execute_statement(&stmt, 0).await.unwrap();
    assert_eq!(result.opcode, orbit_protocols::cql::protocol::CqlOpcode::Result);
    
    // Verify data was updated
    let verify_result = ctx.verify_data("SELECT email FROM test_users WHERE id = 1").await;
    match verify_result {
        Ok(ExecutionResult::Select { columns, rows, .. }) => {
            assert_eq!(rows.len(), 1);
            // Find email column index
            let email_idx = columns.iter().position(|c| c.eq_ignore_ascii_case("email"));
            if let Some(email_pos) = email_idx {
                if let Some(row) = rows.first() {
                    if row.len() > email_pos {
                        if let Some(email_opt) = row.get(email_pos) {
                            if let Some(email) = email_opt {
                                assert_eq!(email, "new@example.com", "Email should be updated");
                            } else {
                                panic!("Email value is None");
                            }
                        } else {
                            panic!("Could not get email from row");
                        }
                    } else {
                        panic!("Row too short: {:?}, columns: {:?}", row, columns);
                    }
                } else {
                    panic!("No rows returned");
                }
            } else {
                panic!("Could not find email column in: {:?}", columns);
            }
        }
        Ok(other) => panic!("Expected Select result, got: {:?}", other),
        Err(e) => panic!("Failed to verify data: {:?}", e),
    }
    
    // Teardown
    ctx.cleanup_table("test_users").await;
}

#[tokio::test]
async fn test_cql_delete_execution_integration() {
    let ctx = CqlTestContext::new().await;
    
    // Setup
    create_test_users_table(&ctx).await;
    ctx.insert_data_sql("INSERT INTO test_users (id, name) VALUES (1, 'Alice')").await;
    ctx.insert_data_sql("INSERT INTO test_users (id, name) VALUES (2, 'Bob')").await;
    
    // Verify initial state - should have 2 rows
    let initial_result = ctx.verify_data("SELECT COUNT(*) FROM test_users").await;
    match initial_result {
        Ok(ExecutionResult::Select { rows, .. }) => {
            assert_eq!(rows.len(), 2, "Should have 2 rows initially");
        }
        _ => panic!("Failed to verify initial state"),
    }
    
    // Test DELETE with WHERE clause (now supported!)
    let parser = CqlParser::new();
    let stmt = parser.parse("DELETE FROM test_users WHERE id = 1").unwrap();
    
    let result = ctx.adapter.execute_statement(&stmt, 0).await.unwrap();
    assert_eq!(result.opcode, orbit_protocols::cql::protocol::CqlOpcode::Result);
    
    // Verify data was deleted - only Bob (id=2) should remain
    let verify_result = ctx.verify_data("SELECT * FROM test_users").await;
    match verify_result {
        Ok(ExecutionResult::Select { columns, rows, .. }) => {
            // DELETE WHERE id = 1 should remove Alice, leaving only Bob
            assert_eq!(rows.len(), 1, "Only Bob (id=2) should remain after deleting id=1");
            
            // Find id and name column indices
            let id_idx = columns.iter().position(|c| c.eq_ignore_ascii_case("id"));
            let name_idx = columns.iter().position(|c| c.eq_ignore_ascii_case("name"));
            
            if let (Some(id_pos), Some(name_pos)) = (id_idx, name_idx) {
                if let Some(row) = rows.first() {
                    if row.len() > id_pos.max(name_pos) {
                        let id = row.get(id_pos).and_then(|v| v.as_ref()).map(|s| s.as_str());
                        let name = row.get(name_pos).and_then(|v| v.as_ref()).map(|s| s.as_str());
                        assert_eq!(id, Some("2"), "Remaining row should have id=2, got: {:?}", id);
                        assert_eq!(name, Some("Bob"), "Remaining row should be Bob, got: {:?}", name);
                    } else {
                        panic!("Row too short: {:?}, columns: {:?}", row, columns);
                    }
                } else {
                    panic!("Expected 1 row but got none");
                }
            } else {
                panic!("Could not find id or name columns in: {:?}", columns);
            }
        }
        Ok(other) => panic!("Expected Select result, got: {:?}", other),
        Err(e) => panic!("Failed to verify data: {:?}", e),
    }
    
    // Teardown
    ctx.cleanup_table("test_users").await;
}

// ============================================================================
// Result Set Tests with Setup/Teardown
// ============================================================================

#[tokio::test]
async fn test_result_set_empty_integration() {
    let ctx = CqlTestContext::new().await;
    
    // Setup
    create_test_users_table(&ctx).await;
    
    // Test
    let stmt = CqlStatement::Select {
        columns: vec!["*".to_string()],
        table: "test_users".to_string(),
        where_clause: None,
        limit: None,
        allow_filtering: false,
    };
    
    let result = ctx.adapter.execute_statement(&stmt, 0).await.unwrap();
    assert_eq!(result.opcode, orbit_protocols::cql::protocol::CqlOpcode::Result);
    
    // Verify it's a ROWS result (0x0002)
    use bytes::Buf;
    let mut body = result.body.clone();
    let result_kind = body.get_i32();
    assert_eq!(result_kind, 0x0002); // RESULT::Rows
    
    // Teardown
    ctx.cleanup_table("test_users").await;
}

#[tokio::test]
async fn test_result_set_with_data_integration() {
    let ctx = CqlTestContext::new().await;
    
    // Setup
    create_test_users_table(&ctx).await;
    ctx.insert_data_sql("INSERT INTO test_users (id, name) VALUES (1, 'Alice')").await;
    
    // Test
    let stmt = CqlStatement::Select {
        columns: vec!["*".to_string()],
        table: "test_users".to_string(),
        where_clause: None,
        limit: None,
        allow_filtering: false,
    };
    
    let result = ctx.adapter.execute_statement(&stmt, 0).await.unwrap();
    assert_eq!(result.opcode, orbit_protocols::cql::protocol::CqlOpcode::Result);
    
    // Verify it's a ROWS result with data
    use bytes::Buf;
    let mut body = result.body.clone();
    let result_kind = body.get_i32();
    assert_eq!(result_kind, 0x0002); // RESULT::Rows
    
    // Check metadata flags
    let flags = body.get_i32();
    assert_eq!(flags, 0x0001); // Global tables spec
    
    // Check column count (should be 3: id, name, age)
    let column_count = body.get_i32();
    assert!(column_count >= 2); // At least id and name
    
    // Teardown
    ctx.cleanup_table("test_users").await;
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_error_invalid_table_integration() {
    let ctx = CqlTestContext::new().await;
    
    // Test with non-existent table
    let stmt = CqlStatement::Select {
        columns: vec!["*".to_string()],
        table: "nonexistent_table".to_string(),
        where_clause: None,
        limit: None,
        allow_filtering: false,
    };
    
    let result = ctx.adapter.execute_statement(&stmt, 0).await.unwrap();
    // Should return either Error opcode or Result with empty rows
    // The comprehensive engine returns Result with empty rows for non-existent tables
    if result.opcode == orbit_protocols::cql::protocol::CqlOpcode::Error {
        // Error opcode is acceptable
        return;
    }
    // If Result opcode, verify it's empty
    assert_eq!(result.opcode, orbit_protocols::cql::protocol::CqlOpcode::Result);
    // Verify it's an empty result set
    use bytes::Buf;
    let mut body = result.body.clone();
    let result_kind = body.get_i32();
    if result_kind == 0x0002 { // RESULT::Rows
        let _flags = body.get_i32();
        let column_count = body.get_i32();
        let row_count = body.get_i32();
        assert_eq!(row_count, 0, "Non-existent table should return empty result");
        assert!(column_count >= 0);
    }
}

