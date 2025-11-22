//! MCP Server Integration with Orbit-RS
//!
//! This module provides integration between the MCP server and Orbit-RS's
//! PostgreSQL wire protocol and query engine.

use crate::mcp::result_processor::{QueryResult as McpQueryResult, Row};
use crate::mcp::sql_generator::GeneratedQuery;
use crate::mcp::types::McpError;
use crate::postgres_wire::query_engine::{QueryEngine, QueryResult as PgQueryResult};
use crate::postgres_wire::persistent_storage::PersistentTableStorage;
use std::sync::Arc;

/// MCP-Orbit Integration Layer
pub struct OrbitMcpIntegration {
    /// Query engine for executing SQL
    query_engine: Arc<QueryEngine>,
    /// Persistent storage for schema operations
    storage: Option<Arc<dyn PersistentTableStorage>>,
}

impl OrbitMcpIntegration {
    /// Create a new integration layer
    pub fn new(query_engine: Arc<QueryEngine>) -> Self {
        Self {
            query_engine,
            storage: None,
        }
    }

    /// Create with persistent storage for schema operations
    pub fn with_storage(
        query_engine: Arc<QueryEngine>,
        storage: Arc<dyn PersistentTableStorage>,
    ) -> Self {
        Self {
            query_engine,
            storage: Some(storage),
        }
    }

    /// Execute a generated SQL query and return MCP-formatted results
    pub async fn execute_generated_query(
        &self,
        generated_query: &GeneratedQuery,
    ) -> Result<McpQueryResult, McpError> {
        // Execute the SQL query using Orbit-RS query engine
        let pg_result = self
            .query_engine
            .execute_query(&generated_query.sql)
            .await
            .map_err(|e| McpError::SqlError(e.to_string()))?;

        // Convert PostgreSQL QueryResult to MCP QueryResult
        self.convert_query_result(pg_result)
    }

    /// Convert PostgreSQL QueryResult to MCP QueryResult
    fn convert_query_result(&self, pg_result: PgQueryResult) -> Result<McpQueryResult, McpError> {
        match pg_result {
            PgQueryResult::Select { columns, rows } => {
                // Convert rows to MCP format
                let mcp_rows: Vec<Row> = rows
                    .into_iter()
                    .map(|row| {
                        let mut mcp_row = Row::new();
                        for (col_idx, col_name) in columns.iter().enumerate() {
                            if let Some(Some(value)) = row.get(col_idx) {
                                // Convert value to JSON
                                let json_value = self.convert_value_to_json(value);
                                mcp_row.insert(col_name.clone(), json_value);
                            } else {
                                // Insert NULL for missing values
                                mcp_row.insert(col_name.clone(), serde_json::Value::Null);
                            }
                        }
                        mcp_row
                    })
                    .collect();

                let row_count = mcp_rows.len();
                Ok(McpQueryResult {
                    columns: columns.clone(),
                    rows: mcp_rows,
                    row_count,
                })
            }
            PgQueryResult::Insert { count } => {
                // For INSERT, return a result indicating success
                Ok(McpQueryResult {
                    columns: vec!["rows_affected".to_string()],
                    rows: vec![{
                        let mut row = Row::new();
                        row.insert(
                            "rows_affected".to_string(),
                            serde_json::Value::Number(count.into()),
                        );
                        row
                    }],
                    row_count: 1,
                })
            }
            PgQueryResult::Update { count } => {
                Ok(McpQueryResult {
                    columns: vec!["rows_affected".to_string()],
                    rows: vec![{
                        let mut row = Row::new();
                        row.insert(
                            "rows_affected".to_string(),
                            serde_json::Value::Number(count.into()),
                        );
                        row
                    }],
                    row_count: 1,
                })
            }
            PgQueryResult::Delete { count } => {
                Ok(McpQueryResult {
                    columns: vec!["rows_affected".to_string()],
                    rows: vec![{
                        let mut row = Row::new();
                        row.insert(
                            "rows_affected".to_string(),
                            serde_json::Value::Number(count.into()),
                        );
                        row
                    }],
                    row_count: 1,
                })
            }
        }
    }

    /// Convert a value to JSON
    fn convert_value_to_json(&self, value: &str) -> serde_json::Value {
        // Try to parse as number first
        if let Ok(num) = value.parse::<i64>() {
            return serde_json::Value::Number(num.into());
        }
        if let Ok(num) = value.parse::<f64>() {
            if let Some(n) = serde_json::Number::from_f64(num) {
                return serde_json::Value::Number(n);
            }
        }

        // Try to parse as boolean
        if value.eq_ignore_ascii_case("true") {
            return serde_json::Value::Bool(true);
        }
        if value.eq_ignore_ascii_case("false") {
            return serde_json::Value::Bool(false);
        }

        // Default to string
        serde_json::Value::String(value.to_string())
    }

    /// Get table schema from Orbit-RS
    pub async fn get_table_schema(
        &self,
        table_name: &str,
    ) -> Result<Option<crate::mcp::schema::TableSchema>, McpError> {
        if let Some(ref storage) = self.storage {
            let pg_schema = storage
                .get_table_schema(table_name)
                .await
                .map_err(|e| McpError::SqlError(e.to_string()))?;

            if let Some(pg_schema) = pg_schema {
                // Convert PostgreSQL TableSchema to MCP TableSchema
                let mcp_schema = self.convert_table_schema(pg_schema);
                Ok(Some(mcp_schema))
            } else {
                Ok(None)
            }
        } else {
            Err(McpError::InternalError(
                "Storage not available for schema operations".to_string(),
            ))
        }
    }

    /// List all tables from Orbit-RS
    pub async fn list_tables(&self) -> Result<Vec<String>, McpError> {
        if let Some(ref storage) = self.storage {
            storage
                .list_tables()
                .await
                .map_err(|e| McpError::SqlError(e.to_string()))
        } else {
            Err(McpError::InternalError(
                "Storage not available for schema operations".to_string(),
            ))
        }
    }

    /// Convert PostgreSQL TableSchema to MCP TableSchema
    fn convert_table_schema(
        &self,
        pg_schema: crate::postgres_wire::persistent_storage::TableSchema,
    ) -> crate::mcp::schema::TableSchema {
        use crate::mcp::schema::{ColumnInfo, TableSchema};
        use crate::postgres_wire::persistent_storage::ColumnType;

        let columns: Vec<ColumnInfo> = pg_schema
            .columns
            .iter()
            .map(|col| {
                let data_type = match &col.data_type {
                    ColumnType::Serial => "SERIAL".to_string(),
                    ColumnType::Integer => "INTEGER".to_string(),
                    ColumnType::BigInt => "BIGINT".to_string(),
                    ColumnType::Text => "TEXT".to_string(),
                    ColumnType::Varchar(n) => format!("VARCHAR({})", n),
                    ColumnType::Boolean => "BOOLEAN".to_string(),
                    ColumnType::Json => "JSON".to_string(),
                    ColumnType::Timestamp => "TIMESTAMP".to_string(),
                };

                ColumnInfo {
                    name: col.name.clone(),
                    data_type,
                    nullable: col.nullable,
                    default_value: col.default_value.as_ref().and_then(|v| {
                        if let Some(s) = v.as_str() {
                            Some(s.to_string())
                        } else {
                            Some(serde_json::to_string(v).unwrap_or_default())
                        }
                    }),
                    is_primary_key: false, // Would need to check constraints
                    is_indexed: false,     // Would need to check indexes
                }
            })
            .collect();

        TableSchema {
            name: pg_schema.name.clone(),
            columns,
            constraints: vec![], // Would need to convert from PostgreSQL constraints
            indexes: vec![],     // Would need to convert from PostgreSQL indexes
            row_estimate: Some(pg_schema.row_count.max(0) as u64),
            data_size_estimate: None,
        }
    }
}

