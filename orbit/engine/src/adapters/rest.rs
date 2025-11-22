//! REST API Adapter
//!
//! Provides a REST API layer over the unified orbit-engine.
//!
//! ## Features
//!
//! - RESTful resource-based API
//! - JSON request/response format
//! - CRUD operations on tables
//! - Transaction support via session tokens
//! - Query filtering with JSON query language
//!
//! ## API Endpoints
//!
//! - `POST /tables` - Create table
//! - `GET /tables/{name}` - Get table schema
//! - `DELETE /tables/{name}` - Drop table
//! - `GET /tables/{name}/rows` - Query rows (with filters)
//! - `POST /tables/{name}/rows` - Insert rows
//! - `PUT /tables/{name}/rows` - Update rows
//! - `DELETE /tables/{name}/rows` - Delete rows
//! - `POST /transactions` - Begin transaction
//! - `POST /transactions/{id}/commit` - Commit transaction
//! - `POST /transactions/{id}/rollback` - Rollback transaction
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use orbit_engine::adapters::{AdapterContext, RestAdapter};
//! use orbit_engine::storage::{HybridStorageManager, HybridStorageConfig, ColumnSchema};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create storage engine with proper configuration
//! // let table_name = "api_data".to_string();
//! // let schema = vec![]; // Define your schema
//! // let config = HybridStorageConfig::default();
//! // let storage = Arc::new(HybridStorageManager::new(table_name, schema, config));
//! // let context = AdapterContext::new(storage);
//! // let adapter = RestAdapter::new(context);
//! //
//! // Adapter is ready to handle REST API requests
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{EngineError, EngineResult};
use crate::storage::{ColumnDef, DataType, Row, SqlValue, TableSchema};
use crate::transaction::IsolationLevel;

use super::{AdapterContext, ProtocolAdapter, TransactionAdapter};

/// REST API adapter
pub struct RestAdapter {
    /// Adapter context with engine components
    context: AdapterContext,
    /// Transaction adapter
    transaction_adapter: TransactionAdapter,
}

impl RestAdapter {
    /// Create a new REST adapter
    pub fn new(context: AdapterContext) -> Self {
        let transaction_adapter = TransactionAdapter::new(context.clone());
        Self {
            context,
            transaction_adapter,
        }
    }

    /// Handle CREATE TABLE request
    pub async fn create_table_request(
        &self,
        request: CreateTableRequest,
    ) -> EngineResult<RestResponse> {
        let schema = request.to_table_schema();
        self.context.storage.create_table(schema).await?;
        Ok(RestResponse::success("Table created"))
    }

    /// Handle GET TABLE SCHEMA request
    pub async fn get_table_schema(&self, table_name: &str) -> EngineResult<RestResponse> {
        let schema = self.context.storage.get_schema(table_name).await?;
        Ok(RestResponse::data(serde_json::to_value(schema)?))
    }

    /// Handle QUERY ROWS request
    pub async fn query_rows(
        &self,
        table_name: &str,
        request: QueryRequest,
    ) -> EngineResult<RestResponse> {
        let pattern = request.to_access_pattern();
        let result = self.context.storage.query(table_name, pattern).await?;

        match result {
            crate::storage::QueryResult::Rows(rows) => {
                let json_rows: Vec<_> = rows
                    .into_iter()
                    .map(|row| row_to_json(row))
                    .collect::<Result<_, _>>()?;
                Ok(RestResponse::data(serde_json::json!({ "rows": json_rows })))
            }
            crate::storage::QueryResult::Empty => {
                Ok(RestResponse::data(serde_json::json!({ "rows": [] })))
            }
            _ => Err(EngineError::not_implemented("Column batch conversion")),
        }
    }

    /// Handle INSERT ROWS request
    pub async fn insert_rows(
        &self,
        table_name: &str,
        request: InsertRequest,
    ) -> EngineResult<RestResponse> {
        let rows = request
            .rows
            .into_iter()
            .map(|json_row| json_to_row(json_row))
            .collect::<Result<Vec<_>, _>>()?;

        self.context.storage.insert_rows(table_name, rows).await?;
        Ok(RestResponse::success("Rows inserted"))
    }

    /// Handle UPDATE ROWS request
    pub async fn update_rows(
        &self,
        table_name: &str,
        request: UpdateRequest,
    ) -> EngineResult<RestResponse> {
        let filter = request.filter.to_filter_predicate();
        let updates = json_to_row(request.updates)?;

        let count = self.context.storage.update(table_name, filter, updates).await?;
        Ok(RestResponse::data(serde_json::json!({ "rows_affected": count })))
    }

    /// Handle DELETE ROWS request
    pub async fn delete_rows(
        &self,
        table_name: &str,
        request: DeleteRequest,
    ) -> EngineResult<RestResponse> {
        let filter = request.filter.to_filter_predicate();
        let count = self.context.storage.delete(table_name, filter).await?;
        Ok(RestResponse::data(serde_json::json!({ "rows_affected": count })))
    }

    /// Begin transaction
    pub async fn begin_transaction_request(
        &mut self,
        request: BeginTransactionRequest,
    ) -> EngineResult<RestResponse> {
        let tx_id = uuid::Uuid::new_v4().to_string();
        self.transaction_adapter
            .begin(tx_id.clone(), request.isolation_level.to_engine_isolation())
            .await?;
        Ok(RestResponse::data(serde_json::json!({ "transaction_id": tx_id })))
    }

    /// Commit transaction
    pub async fn commit_transaction(&mut self, tx_id: &str) -> EngineResult<RestResponse> {
        self.transaction_adapter.commit(tx_id).await?;
        Ok(RestResponse::success("Transaction committed"))
    }

    /// Rollback transaction
    pub async fn rollback_transaction(&mut self, tx_id: &str) -> EngineResult<RestResponse> {
        self.transaction_adapter.rollback(tx_id).await?;
        Ok(RestResponse::success("Transaction rolled back"))
    }
}

#[async_trait]
impl ProtocolAdapter for RestAdapter {
    fn protocol_name(&self) -> &'static str {
        "REST API"
    }

    async fn initialize(&mut self) -> EngineResult<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> EngineResult<()> {
        Ok(())
    }
}

// REST API Types

/// REST response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestResponse {
    /// Success status
    pub success: bool,
    /// Response data (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Error message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RestResponse {
    /// Create success response with message
    pub fn success(message: &str) -> Self {
        Self {
            success: true,
            data: Some(serde_json::json!({ "message": message })),
            error: None,
        }
    }

    /// Create success response with data
    pub fn data(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create error response
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Create table request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTableRequest {
    /// Table name
    pub name: String,
    /// Column definitions
    pub columns: Vec<RestColumnDef>,
    /// Primary key columns
    pub primary_key: Vec<String>,
}

impl CreateTableRequest {
    /// Convert to TableSchema
    pub fn to_table_schema(&self) -> TableSchema {
        TableSchema {
            name: self.name.clone(),
            columns: self
                .columns
                .iter()
                .map(|col| col.to_column_def())
                .collect(),
            primary_key: self.primary_key.clone(),
        }
    }
}

/// REST column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestColumnDef {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: String,
    /// Nullable
    pub nullable: bool,
}

impl RestColumnDef {
    /// Convert to ColumnDef
    pub fn to_column_def(&self) -> ColumnDef {
        ColumnDef {
            name: self.name.clone(),
            data_type: self.parse_data_type(),
            nullable: self.nullable,
        }
    }

    fn parse_data_type(&self) -> DataType {
        match self.data_type.to_lowercase().as_str() {
            "int32" | "integer" => DataType::Int32,
            "int64" | "bigint" => DataType::Int64,
            "float32" | "real" => DataType::Float32,
            "float64" | "double" => DataType::Float64,
            "string" | "text" | "varchar" => DataType::String,
            "binary" | "bytea" => DataType::Binary,
            "timestamp" => DataType::Timestamp,
            "boolean" | "bool" => DataType::Boolean,
            _ => DataType::String, // Default
        }
    }
}

/// Query request with filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    /// Optional filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<RestFilter>,
    /// Optional limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl QueryRequest {
    /// Convert to AccessPattern
    pub fn to_access_pattern(&self) -> crate::storage::AccessPattern {
        crate::storage::AccessPattern::Scan {
            time_range: None,
            filter: self.filter.as_ref().map(|f| f.to_filter_predicate()),
        }
    }
}

/// REST filter (JSON-based query language)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum RestFilter {
    /// Equals
    #[serde(rename = "eq")]
    Eq {
        /// Field name
        field: String,
        /// Value to compare
        value: serde_json::Value
    },
    /// Not equals
    #[serde(rename = "ne")]
    Ne {
        /// Field name
        field: String,
        /// Value to compare
        value: serde_json::Value
    },
    /// Less than
    #[serde(rename = "lt")]
    Lt {
        /// Field name
        field: String,
        /// Value to compare
        value: serde_json::Value
    },
    /// Less than or equal
    #[serde(rename = "lte")]
    Lte {
        /// Field name
        field: String,
        /// Value to compare
        value: serde_json::Value
    },
    /// Greater than
    #[serde(rename = "gt")]
    Gt {
        /// Field name
        field: String,
        /// Value to compare
        value: serde_json::Value
    },
    /// Greater than or equal
    #[serde(rename = "gte")]
    Gte {
        /// Field name
        field: String,
        /// Value to compare
        value: serde_json::Value
    },
    /// AND
    #[serde(rename = "and")]
    And {
        /// Filters to combine with AND
        filters: Vec<RestFilter>
    },
    /// OR
    #[serde(rename = "or")]
    Or {
        /// Filters to combine with OR
        filters: Vec<RestFilter>
    },
}

impl RestFilter {
    /// Convert to FilterPredicate
    pub fn to_filter_predicate(&self) -> crate::storage::FilterPredicate {
        match self {
            RestFilter::Eq { field, value } => crate::storage::FilterPredicate::Eq(
                field.clone(),
                json_to_sql_value(value).unwrap_or(SqlValue::Null),
            ),
            RestFilter::Ne { field, value } => crate::storage::FilterPredicate::Ne(
                field.clone(),
                json_to_sql_value(value).unwrap_or(SqlValue::Null),
            ),
            RestFilter::Lt { field, value } => crate::storage::FilterPredicate::Lt(
                field.clone(),
                json_to_sql_value(value).unwrap_or(SqlValue::Null),
            ),
            RestFilter::Lte { field, value } => crate::storage::FilterPredicate::Le(
                field.clone(),
                json_to_sql_value(value).unwrap_or(SqlValue::Null),
            ),
            RestFilter::Gt { field, value } => crate::storage::FilterPredicate::Gt(
                field.clone(),
                json_to_sql_value(value).unwrap_or(SqlValue::Null),
            ),
            RestFilter::Gte { field, value } => crate::storage::FilterPredicate::Ge(
                field.clone(),
                json_to_sql_value(value).unwrap_or(SqlValue::Null),
            ),
            RestFilter::And { filters } => crate::storage::FilterPredicate::And(
                filters.iter().map(|f| f.to_filter_predicate()).collect(),
            ),
            RestFilter::Or { filters } => crate::storage::FilterPredicate::Or(
                filters.iter().map(|f| f.to_filter_predicate()).collect(),
            ),
        }
    }
}

/// Insert request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertRequest {
    /// Rows to insert (JSON objects)
    pub rows: Vec<serde_json::Map<String, serde_json::Value>>,
}

/// Update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRequest {
    /// Filter for rows to update
    pub filter: RestFilter,
    /// Updates to apply
    pub updates: serde_json::Map<String, serde_json::Value>,
}

/// Delete request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    /// Filter for rows to delete
    pub filter: RestFilter,
}

/// Begin transaction request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeginTransactionRequest {
    /// Isolation level
    #[serde(default)]
    pub isolation_level: RestIsolationLevel,
}

/// REST isolation level
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RestIsolationLevel {
    /// Read uncommitted (dirty reads allowed)
    #[serde(rename = "read_uncommitted")]
    ReadUncommitted,
    /// Read committed (no dirty reads)
    #[serde(rename = "read_committed")]
    ReadCommitted,
    /// Repeatable read (consistent snapshots)
    #[serde(rename = "repeatable_read")]
    RepeatableRead,
    /// Serializable (full isolation)
    #[serde(rename = "serializable")]
    Serializable,
}

impl Default for RestIsolationLevel {
    fn default() -> Self {
        RestIsolationLevel::ReadCommitted
    }
}

impl RestIsolationLevel {
    /// Convert to engine IsolationLevel
    pub fn to_engine_isolation(&self) -> IsolationLevel {
        match self {
            RestIsolationLevel::ReadUncommitted => IsolationLevel::ReadUncommitted,
            RestIsolationLevel::ReadCommitted => IsolationLevel::ReadCommitted,
            RestIsolationLevel::RepeatableRead => IsolationLevel::RepeatableRead,
            RestIsolationLevel::Serializable => IsolationLevel::Serializable,
        }
    }
}

// Helper functions for JSON <-> SQL conversion

fn json_to_sql_value(value: &serde_json::Value) -> EngineResult<SqlValue> {
    match value {
        serde_json::Value::Null => Ok(SqlValue::Null),
        serde_json::Value::Bool(b) => Ok(SqlValue::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(SqlValue::Int64(i))
            } else if let Some(f) = n.as_f64() {
                Ok(SqlValue::Float64(f))
            } else {
                Err(EngineError::invalid_input("Invalid number"))
            }
        }
        serde_json::Value::String(s) => Ok(SqlValue::String(s.clone())),
        _ => Err(EngineError::invalid_input("Unsupported JSON type")),
    }
}

fn sql_value_to_json(value: &SqlValue) -> EngineResult<serde_json::Value> {
    match value {
        SqlValue::Null => Ok(serde_json::Value::Null),
        SqlValue::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        SqlValue::Int16(i) => Ok(serde_json::json!(i)),
        SqlValue::Int32(i) => Ok(serde_json::json!(i)),
        SqlValue::Int64(i) => Ok(serde_json::json!(i)),
        SqlValue::Float32(f) => Ok(serde_json::json!(f)),
        SqlValue::Float64(f) => Ok(serde_json::json!(f)),
        SqlValue::String(s) | SqlValue::Varchar(s) | SqlValue::Char(s) | SqlValue::Decimal(s) => {
            Ok(serde_json::Value::String(s.clone()))
        }
        SqlValue::Binary(b) => {
            use base64::{Engine as _, engine::general_purpose};
            Ok(serde_json::Value::String(general_purpose::STANDARD.encode(b)))
        }
        SqlValue::Timestamp(ts) => {
            let duration = ts.duration_since(std::time::UNIX_EPOCH).map_err(|e| {
                EngineError::internal(format!("Invalid timestamp: {}", e))
            })?;
            Ok(serde_json::json!(duration.as_secs()))
        }
    }
}

fn row_to_json(row: Row) -> EngineResult<serde_json::Map<String, serde_json::Value>> {
    let mut map = serde_json::Map::new();
    for (key, value) in row {
        map.insert(key, sql_value_to_json(&value)?);
    }
    Ok(map)
}

fn json_to_row(map: serde_json::Map<String, serde_json::Value>) -> EngineResult<Row> {
    let mut row = HashMap::new();
    for (key, value) in map {
        row.insert(key, json_to_sql_value(&value)?);
    }
    Ok(row)
}
