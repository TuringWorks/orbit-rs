//! CQL Test Helpers
//!
//! Shared utilities for CQL integration tests with proper setup and teardown

#![cfg(feature = "cql")]

use orbit_protocols::cql::{CqlAdapter, CqlConfig, CqlParser};
use orbit_protocols::postgres_wire::sql::executor::SqlExecutor;
use orbit_protocols::postgres_wire::QueryEngine;
use std::sync::Arc;

/// Test context with shared query engine
pub struct CqlTestContext {
    pub adapter: CqlAdapter,
    pub query_engine: Arc<QueryEngine>,
    #[allow(dead_code)] // Reserved for future use
    pub executor: SqlExecutor,
}

impl CqlTestContext {
    /// Create a new test context with shared query engine
    pub async fn new() -> Self {
        // Create shared query engine
        let query_engine = Arc::new(QueryEngine::new());

        // Create adapter with shared query engine
        let config = CqlConfig::default();
        let adapter = CqlAdapter::with_query_engine(config, query_engine.clone())
            .await
            .unwrap();

        // Create executor for setup (it uses ConfigurableSqlEngine which handles CREATE TABLE)
        let executor = SqlExecutor::new().await.unwrap();

        Self {
            adapter,
            query_engine,
            executor,
        }
    }

    /// Setup: Create a test table using the adapter's SQL engine directly
    /// This ensures shared storage between adapter and setup
    pub async fn setup_table(&self, table_name: &str, schema: &str) {
        let create_sql = format!("CREATE TABLE {} {}", table_name, schema);
        // Use the adapter's execute_sql which uses QueryEngine's comprehensive SQL engine
        // This bypasses QueryEngine's persistent storage requirement
        // and uses ConfigurableSqlEngine which can handle CREATE TABLE
        self.adapter.execute_sql(&create_sql).await.unwrap();
    }

    /// Setup: Insert test data using SQL via adapter's SQL engine
    pub async fn insert_data_sql(&self, sql: &str) {
        // Use adapter's SQL engine to ensure shared storage
        self.adapter.execute_sql(sql).await.unwrap();
    }

    /// Setup: Insert test data using CQL
    #[allow(dead_code)] // Reserved for future use
    pub async fn insert_data_cql(&self, cql: &str) {
        let parser = CqlParser::new();
        let stmt = parser.parse(cql).unwrap();
        self.adapter.execute_statement(&stmt, 0).await.unwrap();
    }

    /// Teardown: Clean up test data
    pub async fn cleanup_table(&self, table_name: &str) {
        let drop_sql = format!("DROP TABLE IF EXISTS {}", table_name);
        let _ = self.query_engine.execute_query(&drop_sql).await;
    }

    /// Verify data using SQL query via adapter's SQL engine (ensures shared storage)
    pub async fn verify_data(
        &self,
        sql: &str,
    ) -> Result<
        orbit_protocols::postgres_wire::sql::executor::ExecutionResult,
        orbit_protocols::error::ProtocolError,
    > {
        // Use adapter's SQL engine to ensure we're querying the same storage
        let result = self.adapter.execute_sql(sql).await?;

        // Convert UnifiedExecutionResult to ExecutionResult for compatibility
        match result {
            orbit_protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                columns,
                rows,
                row_count,
                ..
            } => Ok(
                orbit_protocols::postgres_wire::sql::executor::ExecutionResult::Select {
                    columns,
                    rows,
                    row_count,
                },
            ),
            orbit_protocols::postgres_wire::sql::UnifiedExecutionResult::Insert {
                count, ..
            } => Ok(
                orbit_protocols::postgres_wire::sql::executor::ExecutionResult::Insert { count },
            ),
            orbit_protocols::postgres_wire::sql::UnifiedExecutionResult::Update {
                count, ..
            } => Ok(
                orbit_protocols::postgres_wire::sql::executor::ExecutionResult::Update { count },
            ),
            orbit_protocols::postgres_wire::sql::UnifiedExecutionResult::Delete {
                count, ..
            } => Ok(
                orbit_protocols::postgres_wire::sql::executor::ExecutionResult::Delete { count },
            ),
            orbit_protocols::postgres_wire::sql::UnifiedExecutionResult::CreateTable {
                table_name,
                ..
            } => Ok(
                orbit_protocols::postgres_wire::sql::executor::ExecutionResult::CreateTable {
                    table_name,
                },
            ),
            _ => Err(orbit_protocols::error::ProtocolError::PostgresError(
                format!("Unexpected result type: {:?}", result),
            )),
        }
    }
}

/// Helper to create a test table with common schema
pub async fn create_test_users_table(context: &CqlTestContext) {
    context
        .setup_table("test_users", "(id INTEGER, name TEXT, age INTEGER)")
        .await;
}

/// Helper to create a test table with email
pub async fn create_test_users_with_email_table(context: &CqlTestContext) {
    context
        .setup_table("test_users", "(id INTEGER, name TEXT, email TEXT)")
        .await;
}
