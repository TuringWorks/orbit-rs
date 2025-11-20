//! PostgreSQL Protocol Adapter
//!
//! Bridges PostgreSQL wire protocol requests to the unified orbit-engine.
//!
//! ## Features
//!
//! - Full SQL type system mapping
//! - Transaction isolation level mapping
//! - Error code translation (PostgreSQL SQLSTATE codes)
//! - Schema and DDL operation support
//!
//! ## Type Mapping
//!
//! | PostgreSQL Type    | Engine Type       |
//! |-------------------|-------------------|
//! | SMALLINT          | SqlValue::Int16   |
//! | INTEGER           | SqlValue::Int32   |
//! | BIGINT            | SqlValue::Int64   |
//! | REAL              | SqlValue::Float32 |
//! | DOUBLE PRECISION  | SqlValue::Float64 |
//! | TEXT/VARCHAR      | SqlValue::String  |
//! | BYTEA             | SqlValue::Binary  |
//! | TIMESTAMP         | SqlValue::Timestamp |
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use orbit_engine::adapters::{AdapterContext, PostgresAdapter};
//! use orbit_engine::storage::{HybridStorageManager, HybridStorageConfig, ColumnSchema};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create storage engine with proper configuration
//! let table_name = "test_table".to_string();
//! let schema = vec![]; // Define your schema
//! let config = HybridStorageConfig::default();
//! let storage = Arc::new(HybridStorageManager::new(table_name, schema, config));
//!
//! // Create adapter context
//! let context = AdapterContext::new(storage);
//!
//! // Create PostgreSQL adapter
//! let adapter = PostgresAdapter::new(context);
//!
//! // Adapter is now ready to handle PostgreSQL protocol requests
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::SystemTime;

use crate::error::{EngineError, EngineResult};
use crate::storage::{ColumnDef, DataType, Row, SqlValue, TableSchema};
use crate::transaction::IsolationLevel;

use super::{AdapterContext, CommandResult, ProtocolAdapter, TransactionAdapter};

/// Time travel specification for querying historical data
#[derive(Debug, Clone)]
pub enum TimeTravelSpec {
    /// Query as of specific timestamp (TIMESTAMP AS OF)
    Timestamp(SystemTime),
    /// Query as of specific snapshot/version ID (VERSION AS OF)
    SnapshotId(i64),
    /// Query as of named tag or branch (VERSION AS OF 'tag_name')
    NamedReference(String),
}

/// PostgreSQL protocol adapter
pub struct PostgresAdapter {
    /// Adapter context with engine components
    context: AdapterContext,
    /// Transaction adapter
    transaction_adapter: TransactionAdapter,
}

impl PostgresAdapter {
    /// Create a new PostgreSQL adapter
    pub fn new(context: AdapterContext) -> Self {
        let transaction_adapter = TransactionAdapter::new(context.clone());
        Self {
            context,
            transaction_adapter,
        }
    }

    /// Execute a CREATE TABLE statement
    pub async fn create_table(
        &self,
        table_name: &str,
        columns: Vec<PostgresColumnDef>,
        primary_key: Vec<String>,
    ) -> EngineResult<CommandResult> {
        let schema = TableSchema {
            name: table_name.to_string(),
            columns: columns
                .into_iter()
                .map(|col| col.to_engine_column_def())
                .collect(),
            primary_key,
        };

        self.context.storage.create_table(schema).await?;
        Ok(CommandResult::Ok)
    }

    /// Execute a SELECT statement with time travel support
    ///
    /// Supports PostgreSQL time travel syntax:
    /// - `SELECT * FROM table TIMESTAMP AS OF '2023-04-11T18:06:36.289+00:00'`
    /// - `SELECT * FROM table TIMESTAMP AS OF 1681236397` (UNIX timestamp)
    /// - `SELECT * FROM table VERSION AS OF <snapshot_id>`
    /// - `SELECT * FROM table VERSION AS OF 'tag_name'`
    pub async fn select(
        &self,
        table_name: &str,
        columns: Option<Vec<String>>,
        filter: Option<PostgresFilter>,
    ) -> EngineResult<CommandResult> {
        self.select_with_time_travel(table_name, columns, filter, None).await
    }

    /// Execute a SELECT statement with explicit time travel
    ///
    /// When time_travel is specified, queries historical data from Iceberg snapshots.
    /// Falls back to current data if time travel is not supported or not specified.
    pub async fn select_with_time_travel(
        &self,
        table_name: &str,
        columns: Option<Vec<String>>,
        filter: Option<PostgresFilter>,
        time_travel: Option<TimeTravelSpec>,
    ) -> EngineResult<CommandResult> {
        // If time travel is requested, use specialized query
        if let Some(spec) = time_travel {
            return self.execute_time_travel_query(table_name, columns, filter, spec).await;
        }

        // Regular query without time travel
        // Convert PostgreSQL filter to engine access pattern
        let pattern = if let Some(pg_filter) = filter {
            pg_filter.to_engine_access_pattern()
        } else {
            crate::storage::AccessPattern::Scan {
                time_range: None,
                filter: None,
            }
        };

        let result = self.context.storage.query(table_name, pattern).await?;

        match result {
            crate::storage::QueryResult::Rows(rows) => {
                // Apply column projection if specified
                let projected_rows = if let Some(cols) = columns {
                    rows.into_iter()
                        .map(|mut row| {
                            row.retain(|k, _| cols.contains(k));
                            row
                        })
                        .collect()
                } else {
                    rows
                };
                Ok(CommandResult::Rows(projected_rows))
            }
            crate::storage::QueryResult::ColumnBatch(batch) => {
                // Convert column batch to rows
                let mut rows = Vec::new();

                // Get column names
                let col_names = batch.column_names.clone().unwrap_or_else(|| {
                    // Generate default column names if not provided
                    (0..batch.columns.len())
                        .map(|i| format!("column_{}", i))
                        .collect()
                });

                // Transpose columnar data to row format
                for row_idx in 0..batch.row_count {
                    let mut row = HashMap::new();

                    for (col_idx, column) in batch.columns.iter().enumerate() {
                        let col_name = &col_names[col_idx];
                        let null_bitmap = &batch.null_bitmaps[col_idx];

                        let value = if null_bitmap.is_null(row_idx) {
                            SqlValue::Null
                        } else {
                            match column {
                                crate::storage::Column::Bool(vals) => SqlValue::Boolean(vals[row_idx]),
                                crate::storage::Column::Int16(vals) => SqlValue::Int16(vals[row_idx]),
                                crate::storage::Column::Int32(vals) => SqlValue::Int32(vals[row_idx]),
                                crate::storage::Column::Int64(vals) => SqlValue::Int64(vals[row_idx]),
                                crate::storage::Column::Float32(vals) => SqlValue::Float32(vals[row_idx]),
                                crate::storage::Column::Float64(vals) => SqlValue::Float64(vals[row_idx]),
                                crate::storage::Column::String(vals) => SqlValue::String(vals[row_idx].clone()),
                                crate::storage::Column::Binary(vals) => SqlValue::Binary(vals[row_idx].clone()),
                            }
                        };

                        row.insert(col_name.clone(), value);
                    }

                    rows.push(row);
                }

                Ok(CommandResult::Rows(rows))
            }
            crate::storage::QueryResult::Aggregate(value) => {
                // Return single aggregate value as a row
                let mut row = HashMap::new();
                row.insert("result".to_string(), value);
                Ok(CommandResult::Rows(vec![row]))
            }
            crate::storage::QueryResult::Empty => Ok(CommandResult::Rows(vec![])),
        }
    }

    /// Execute an INSERT statement
    pub async fn insert(
        &self,
        table_name: &str,
        rows: Vec<Row>,
    ) -> EngineResult<CommandResult> {
        let count = rows.len();
        self.context.storage.insert_rows(table_name, rows).await?;
        Ok(CommandResult::RowsAffected(count))
    }

    /// Execute an UPDATE statement
    pub async fn update(
        &self,
        table_name: &str,
        filter: PostgresFilter,
        updates: HashMap<String, SqlValue>,
    ) -> EngineResult<CommandResult> {
        let engine_filter = filter.to_engine_filter_predicate();
        let count = self
            .context
            .storage
            .update(table_name, engine_filter, updates)
            .await?;
        Ok(CommandResult::RowsAffected(count))
    }

    /// Execute a DELETE statement
    pub async fn delete(
        &self,
        table_name: &str,
        filter: PostgresFilter,
    ) -> EngineResult<CommandResult> {
        let engine_filter = filter.to_engine_filter_predicate();
        let count = self.context.storage.delete(table_name, engine_filter).await?;
        Ok(CommandResult::RowsAffected(count))
    }

    // === Time Travel Support ===

    /// Execute time travel query
    ///
    /// NOTE: Currently returns current snapshot data due to iceberg-rust API limitations.
    /// Will be fully implemented when snapshot access API is available.
    async fn execute_time_travel_query(
        &self,
        table_name: &str,
        columns: Option<Vec<String>>,
        filter: Option<PostgresFilter>,
        _spec: TimeTravelSpec,
    ) -> EngineResult<CommandResult> {
        // For now, delegate to regular query (without time travel to avoid recursion)
        // When Iceberg API supports snapshots, this will:
        // 1. Convert spec to timestamp
        // 2. Call IcebergColdStore::query_as_of()
        // 3. Return historical data

        // Regular query without time travel
        let pattern = if let Some(pg_filter) = filter {
            pg_filter.to_engine_access_pattern()
        } else {
            crate::storage::AccessPattern::Scan {
                time_range: None,
                filter: None,
            }
        };

        let result = self.context.storage.query(table_name, pattern).await?;

        match result {
            crate::storage::QueryResult::Rows(rows) => {
                let projected_rows = if let Some(cols) = columns {
                    rows.into_iter()
                        .map(|mut row| {
                            row.retain(|k, _| cols.contains(&k));
                            row
                        })
                        .collect()
                } else {
                    rows
                };
                Ok(CommandResult::Rows(projected_rows))
            }
            crate::storage::QueryResult::ColumnBatch(batch) => {
                // Convert column batch to rows (same logic as main select)
                let mut rows = Vec::new();
                let col_names = batch.column_names.clone().unwrap_or_else(|| {
                    (0..batch.columns.len())
                        .map(|i| format!("column_{}", i))
                        .collect()
                });

                for row_idx in 0..batch.row_count {
                    let mut row = HashMap::new();
                    for (col_idx, column) in batch.columns.iter().enumerate() {
                        let col_name = &col_names[col_idx];
                        let null_bitmap = &batch.null_bitmaps[col_idx];
                        let value = if null_bitmap.is_null(row_idx) {
                            SqlValue::Null
                        } else {
                            match column {
                                crate::storage::Column::Bool(vals) => SqlValue::Boolean(vals[row_idx]),
                                crate::storage::Column::Int16(vals) => SqlValue::Int16(vals[row_idx]),
                                crate::storage::Column::Int32(vals) => SqlValue::Int32(vals[row_idx]),
                                crate::storage::Column::Int64(vals) => SqlValue::Int64(vals[row_idx]),
                                crate::storage::Column::Float32(vals) => SqlValue::Float32(vals[row_idx]),
                                crate::storage::Column::Float64(vals) => SqlValue::Float64(vals[row_idx]),
                                crate::storage::Column::String(vals) => SqlValue::String(vals[row_idx].clone()),
                                crate::storage::Column::Binary(vals) => SqlValue::Binary(vals[row_idx].clone()),
                            }
                        };
                        row.insert(col_name.clone(), value);
                    }
                    rows.push(row);
                }

                // Apply projection if needed
                if let Some(cols) = columns {
                    let projected_rows = rows.into_iter()
                        .map(|mut row| {
                            row.retain(|k, _| cols.contains(k));
                            row
                        })
                        .collect();
                    Ok(CommandResult::Rows(projected_rows))
                } else {
                    Ok(CommandResult::Rows(rows))
                }
            }
            crate::storage::QueryResult::Aggregate(value) => {
                let mut row = HashMap::new();
                row.insert("result".to_string(), value);
                Ok(CommandResult::Rows(vec![row]))
            }
            crate::storage::QueryResult::Empty => Ok(CommandResult::Rows(vec![])),
        }
    }

    /// View table history (snapshots)
    ///
    /// Equivalent to `SELECT * FROM table.history`
    ///
    /// Returns snapshot metadata including snapshot_id, timestamp, operation, etc.
    pub async fn view_history(&self, table_name: &str) -> EngineResult<CommandResult> {
        // This would query the Iceberg metadata table
        // For now, return placeholder
        let history_table = format!("{}.history", table_name);
        Err(EngineError::not_implemented(format!(
            "History table '{}' access not yet implemented. Requires Iceberg metadata access.",
            history_table
        )))
    }

    /// View table snapshots
    ///
    /// Equivalent to `SELECT * FROM table.snapshots`
    ///
    /// Returns detailed snapshot information.
    pub async fn view_snapshots(&self, table_name: &str) -> EngineResult<CommandResult> {
        let snapshots_table = format!("{}.snapshots", table_name);
        Err(EngineError::not_implemented(format!(
            "Snapshots table '{}' access not yet implemented. Requires Iceberg metadata access.",
            snapshots_table
        )))
    }

    /// View table references (branches and tags)
    ///
    /// Equivalent to `SELECT * FROM table.refs`
    pub async fn view_refs(&self, table_name: &str) -> EngineResult<CommandResult> {
        let refs_table = format!("{}.refs", table_name);
        Err(EngineError::not_implemented(format!(
            "Refs table '{}' access not yet implemented. Requires Iceberg metadata access.",
            refs_table
        )))
    }

    /// Create a named tag for current snapshot
    ///
    /// Equivalent to `ALTER TABLE table CREATE TAG tag_name`
    pub async fn create_tag(&self, table_name: &str, tag_name: &str) -> EngineResult<CommandResult> {
        Err(EngineError::not_implemented(format!(
            "CREATE TAG '{}' for table '{}' not yet implemented. Requires Iceberg tag management.",
            tag_name, table_name
        )))
    }

    /// Create a named branch
    ///
    /// Equivalent to `ALTER TABLE table CREATE BRANCH branch_name`
    pub async fn create_branch(&self, table_name: &str, branch_name: &str) -> EngineResult<CommandResult> {
        Err(EngineError::not_implemented(format!(
            "CREATE BRANCH '{}' for table '{}' not yet implemented. Requires Iceberg branch management.",
            branch_name, table_name
        )))
    }

    /// Begin a PostgreSQL transaction
    pub async fn begin_transaction(
        &mut self,
        isolation: PostgresIsolationLevel,
    ) -> EngineResult<String> {
        let tx_id = uuid::Uuid::new_v4().to_string();
        let engine_isolation = isolation.to_engine_isolation();
        self.transaction_adapter
            .begin(tx_id.clone(), engine_isolation)
            .await?;
        Ok(tx_id)
    }

    /// Commit a transaction
    pub async fn commit_transaction(&mut self, tx_id: &str) -> EngineResult<CommandResult> {
        self.transaction_adapter.commit(tx_id).await?;
        Ok(CommandResult::Ok)
    }

    /// Rollback a transaction
    pub async fn rollback_transaction(&mut self, tx_id: &str) -> EngineResult<CommandResult> {
        self.transaction_adapter.rollback(tx_id).await?;
        Ok(CommandResult::Ok)
    }
}

#[async_trait]
impl ProtocolAdapter for PostgresAdapter {
    fn protocol_name(&self) -> &'static str {
        "PostgreSQL"
    }

    async fn initialize(&mut self) -> EngineResult<()> {
        // Adapter initialization if needed
        Ok(())
    }

    async fn shutdown(&mut self) -> EngineResult<()> {
        // Graceful shutdown if needed
        Ok(())
    }
}

/// PostgreSQL column definition
#[derive(Debug, Clone)]
pub struct PostgresColumnDef {
    /// Column name
    pub name: String,
    /// PostgreSQL data type
    pub data_type: PostgresDataType,
    /// Whether column is nullable
    pub nullable: bool,
}

impl PostgresColumnDef {
    /// Convert to engine ColumnDef
    pub fn to_engine_column_def(&self) -> ColumnDef {
        ColumnDef {
            name: self.name.clone(),
            data_type: self.data_type.to_engine_data_type(),
            nullable: self.nullable,
        }
    }
}

/// PostgreSQL data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresDataType {
    /// SMALLINT
    SmallInt,
    /// INTEGER
    Integer,
    /// BIGINT
    BigInt,
    /// REAL
    Real,
    /// DOUBLE PRECISION
    DoublePrecision,
    /// TEXT
    Text,
    /// VARCHAR
    Varchar,
    /// BYTEA
    Bytea,
    /// TIMESTAMP
    Timestamp,
    /// BOOLEAN
    Boolean,
}

impl PostgresDataType {
    /// Convert to engine DataType
    pub fn to_engine_data_type(&self) -> DataType {
        match self {
            PostgresDataType::SmallInt => DataType::Int32, // Promote to Int32
            PostgresDataType::Integer => DataType::Int32,
            PostgresDataType::BigInt => DataType::Int64,
            PostgresDataType::Real => DataType::Float32,
            PostgresDataType::DoublePrecision => DataType::Float64,
            PostgresDataType::Text | PostgresDataType::Varchar => DataType::String,
            PostgresDataType::Bytea => DataType::Binary,
            PostgresDataType::Timestamp => DataType::Timestamp,
            PostgresDataType::Boolean => DataType::Boolean,
        }
    }
}

/// PostgreSQL isolation levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresIsolationLevel {
    /// READ UNCOMMITTED
    ReadUncommitted,
    /// READ COMMITTED
    ReadCommitted,
    /// REPEATABLE READ
    RepeatableRead,
    /// SERIALIZABLE
    Serializable,
}

impl PostgresIsolationLevel {
    /// Convert to engine IsolationLevel
    pub fn to_engine_isolation(&self) -> IsolationLevel {
        match self {
            PostgresIsolationLevel::ReadUncommitted => IsolationLevel::ReadUncommitted,
            PostgresIsolationLevel::ReadCommitted => IsolationLevel::ReadCommitted,
            PostgresIsolationLevel::RepeatableRead => IsolationLevel::RepeatableRead,
            PostgresIsolationLevel::Serializable => IsolationLevel::Serializable,
        }
    }
}

/// PostgreSQL filter (WHERE clause)
#[derive(Debug, Clone)]
pub enum PostgresFilter {
    /// Column = value
    Equals(String, SqlValue),
    /// Column != value
    NotEquals(String, SqlValue),
    /// Column < value
    LessThan(String, SqlValue),
    /// Column <= value
    LessThanOrEqual(String, SqlValue),
    /// Column > value
    GreaterThan(String, SqlValue),
    /// Column >= value
    GreaterThanOrEqual(String, SqlValue),
    /// AND condition
    And(Vec<PostgresFilter>),
    /// OR condition
    Or(Vec<PostgresFilter>),
    /// NOT condition
    Not(Box<PostgresFilter>),
}

impl PostgresFilter {
    /// Convert to engine FilterPredicate
    pub fn to_engine_filter_predicate(&self) -> crate::storage::FilterPredicate {
        match self {
            PostgresFilter::Equals(col, val) => {
                crate::storage::FilterPredicate::Eq(col.clone(), val.clone())
            }
            PostgresFilter::NotEquals(col, val) => {
                crate::storage::FilterPredicate::Ne(col.clone(), val.clone())
            }
            PostgresFilter::LessThan(col, val) => {
                crate::storage::FilterPredicate::Lt(col.clone(), val.clone())
            }
            PostgresFilter::LessThanOrEqual(col, val) => {
                crate::storage::FilterPredicate::Le(col.clone(), val.clone())
            }
            PostgresFilter::GreaterThan(col, val) => {
                crate::storage::FilterPredicate::Gt(col.clone(), val.clone())
            }
            PostgresFilter::GreaterThanOrEqual(col, val) => {
                crate::storage::FilterPredicate::Ge(col.clone(), val.clone())
            }
            PostgresFilter::And(filters) => crate::storage::FilterPredicate::And(
                filters
                    .iter()
                    .map(|f| f.to_engine_filter_predicate())
                    .collect(),
            ),
            PostgresFilter::Or(filters) => crate::storage::FilterPredicate::Or(
                filters
                    .iter()
                    .map(|f| f.to_engine_filter_predicate())
                    .collect(),
            ),
            PostgresFilter::Not(filter) => crate::storage::FilterPredicate::Not(Box::new(
                filter.to_engine_filter_predicate(),
            )),
        }
    }

    /// Convert to engine AccessPattern
    pub fn to_engine_access_pattern(&self) -> crate::storage::AccessPattern {
        crate::storage::AccessPattern::Scan {
            time_range: None,
            filter: Some(self.to_engine_filter_predicate()),
        }
    }
}

/// PostgreSQL error code mapper
pub struct PostgresErrorMapper;

impl super::error_mapping::ErrorMapper for PostgresErrorMapper {
    fn to_error_code(error: &EngineError) -> String {
        match error {
            EngineError::NotFound(_) => "42P01".to_string(), // undefined_table
            EngineError::AlreadyExists(_) => "42P07".to_string(), // duplicate_table
            EngineError::Conflict(_) => "40001".to_string(),   // serialization_failure
            EngineError::Timeout(_) => "57014".to_string(),    // query_canceled
            EngineError::InvalidInput(_) => "22000".to_string(), // data_exception
            EngineError::Transaction(_) => "25000".to_string(), // invalid_transaction_state
            EngineError::NotImplemented(_) => "0A000".to_string(), // feature_not_supported
            _ => "XX000".to_string(),                          // internal_error
        }
    }
}
