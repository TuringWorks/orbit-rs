//! SQL Execution Strategy Pattern
//!
//! This module provides a strategy pattern for SQL execution, allowing users to choose
//! between different execution engines (MVCC vs Traditional) based on their needs.
//!
//! ## Default Behavior
//! - **MVCC Execution** is used by default for better concurrency and deadlock prevention
//! - Traditional execution is available for compatibility or specific use cases
//!
//! ## Features
//! - Strategy pattern for pluggable execution engines
//! - Configuration-based engine selection  
//! - Unified interface regardless of underlying engine
//! - Automatic transaction management
//! - Deadlock prevention with MVCC default

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::{
    ast::{
        AssignmentTarget, Expression, FromClause, InsertSource, IsolationLevel, SelectItem,
        Statement,
    },
    executor::{ExecutionResult, SqlExecutor},
    mvcc_executor::{MvccSqlExecutor, TransactionId},
    parser::SqlParser,
    types::SqlValue,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// SQL execution engine configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SqlEngineConfig {
    /// The execution strategy to use
    pub strategy: ExecutionStrategy,
    /// Enable automatic transaction management
    pub auto_transaction: bool,
    /// Transaction isolation level for MVCC
    pub default_isolation_level: Option<IsolationLevel>,
    /// Enable automatic cleanup of old transactions/versions
    pub auto_cleanup: bool,
    /// Cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
    /// Enable query optimization
    pub enable_optimization: bool,
    /// Enable vector operations support
    pub enable_vectors: bool,
}

/// Available execution strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStrategy {
    /// MVCC-based execution (default) - provides deadlock prevention and high concurrency
    Mvcc,
    /// Traditional lock-based execution - for compatibility or specific use cases
    Traditional,
    /// Hybrid mode - automatically chooses based on query type
    Hybrid,
}

impl Default for SqlEngineConfig {
    fn default() -> Self {
        Self {
            strategy: ExecutionStrategy::Mvcc, // Default to MVCC
            auto_transaction: true,
            default_isolation_level: Some(IsolationLevel::ReadCommitted),
            auto_cleanup: true,
            cleanup_interval_seconds: 300, // 5 minutes
            enable_optimization: true,
            enable_vectors: true,
        }
    }
}

/// Unified SQL execution result
#[derive(Debug, Clone)]
pub enum UnifiedExecutionResult {
    /// Query results with row data
    Select {
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
        row_count: usize,
        transaction_id: Option<TransactionId>,
    },
    /// Insert operation result
    Insert {
        count: usize,
        transaction_id: Option<TransactionId>,
    },
    /// Update operation result
    Update {
        count: usize,
        transaction_id: Option<TransactionId>,
    },
    /// Delete operation result
    Delete {
        count: usize,
        transaction_id: Option<TransactionId>,
    },
    /// Merge operation result
    Merge {
        count: usize,
        rows: Vec<Vec<Option<String>>>,
        columns: Vec<String>,
        transaction_id: Option<TransactionId>,
    },
    /// DDL operation result
    CreateTable {
        table_name: String,
        transaction_id: Option<TransactionId>,
    },
    /// DDL operation result
    CreateIndex {
        index_name: String,
        table_name: String,
        transaction_id: Option<TransactionId>,
    },
    /// Create extension result
    CreateExtension {
        extension_name: String,
        transaction_id: Option<TransactionId>,
    },
    /// Create schema result
    CreateSchema {
        schema_name: String,
        transaction_id: Option<TransactionId>,
    },
    /// Create view result
    CreateView {
        view_name: String,
        transaction_id: Option<TransactionId>,
    },
    /// Drop table result
    DropTable {
        table_names: Vec<String>,
        transaction_id: Option<TransactionId>,
    },
    /// Drop index result
    DropIndex {
        index_names: Vec<String>,
        transaction_id: Option<TransactionId>,
    },
    /// Drop extension result
    DropExtension {
        extension_names: Vec<String>,
        transaction_id: Option<TransactionId>,
    },
    /// Drop schema result
    DropSchema {
        schema_names: Vec<String>,
        transaction_id: Option<TransactionId>,
    },
    /// Drop view result
    DropView {
        view_names: Vec<String>,
        transaction_id: Option<TransactionId>,
    },
    /// Transaction control result
    Transaction {
        transaction_id: TransactionId,
        operation: String,
    },
    /// Set session variable result
    Set {
        variable: String,
        value: String,
        transaction_id: Option<TransactionId>,
    },
    /// Other operations
    Other {
        message: String,
        transaction_id: Option<TransactionId>,
    },
}

impl fmt::Display for UnifiedExecutionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnifiedExecutionResult::Select { row_count, .. } => {
                write!(f, "SELECT {}", row_count)
            }
            UnifiedExecutionResult::Insert { count, .. } => {
                write!(f, "INSERT 0 {}", count)
            }
            UnifiedExecutionResult::Update { count, .. } => {
                write!(f, "UPDATE {}", count)
            }
            UnifiedExecutionResult::Delete { count, .. } => {
                write!(f, "DELETE {}", count)
            }
            UnifiedExecutionResult::Merge { count, .. } => {
                write!(f, "MERGE {}", count)
            }
            UnifiedExecutionResult::CreateTable { .. } => {
                write!(f, "CREATE TABLE")
            }
            UnifiedExecutionResult::CreateIndex { .. } => {
                write!(f, "CREATE INDEX")
            }
            UnifiedExecutionResult::CreateExtension { .. } => {
                write!(f, "CREATE EXTENSION")
            }
            UnifiedExecutionResult::CreateSchema { .. } => {
                write!(f, "CREATE SCHEMA")
            }
            UnifiedExecutionResult::CreateView { .. } => {
                write!(f, "CREATE VIEW")
            }
            UnifiedExecutionResult::DropTable { .. } => {
                write!(f, "DROP TABLE")
            }
            UnifiedExecutionResult::DropIndex { .. } => {
                write!(f, "DROP INDEX")
            }
            UnifiedExecutionResult::DropExtension { .. } => {
                write!(f, "DROP EXTENSION")
            }
            UnifiedExecutionResult::DropSchema { .. } => {
                write!(f, "DROP SCHEMA")
            }
            UnifiedExecutionResult::DropView { .. } => {
                write!(f, "DROP VIEW")
            }
            UnifiedExecutionResult::Transaction { operation, .. } => {
                write!(f, "{}", operation.to_uppercase())
            }
            UnifiedExecutionResult::Set { .. } => {
                write!(f, "SET")
            }
            UnifiedExecutionResult::Other { message, .. } => {
                write!(f, "{}", message)
            }
        }
    }
}

/// Strategy trait for SQL execution engines
#[async_trait::async_trait]
pub trait SqlExecutionStrategy: Send + Sync {
    /// Execute a SQL statement
    async fn execute(&mut self, sql: &str) -> ProtocolResult<UnifiedExecutionResult>;

    /// Execute multiple SQL statements
    async fn execute_multiple(&mut self, sql: &str) -> ProtocolResult<Vec<UnifiedExecutionResult>>;

    /// Execute a parsed statement
    async fn execute_statement(
        &mut self,
        statement: Statement,
    ) -> ProtocolResult<UnifiedExecutionResult>;

    /// Execute a parsed statement within an explicit transaction (MVCC only)
    async fn execute_statement_in_transaction(
        &mut self,
        statement: Statement,
        transaction_id: TransactionId,
    ) -> ProtocolResult<UnifiedExecutionResult>;

    /// Begin a new transaction (MVCC only)
    async fn begin_transaction(
        &self,
        isolation_level: Option<IsolationLevel>,
    ) -> ProtocolResult<TransactionId>;

    /// Commit a transaction (MVCC only)
    async fn commit_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()>;

    /// Rollback a transaction (MVCC only)
    async fn rollback_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()>;

    /// Parse SQL without execution
    fn parse(&mut self, sql: &str) -> ProtocolResult<Statement>;

    /// Check if statement is vector-related
    fn is_vector_statement(&self, sql: &str) -> bool;

    /// Cleanup old data/transactions
    async fn cleanup(&self) -> ProtocolResult<usize>;

    /// Get strategy name
    fn strategy_name(&self) -> &'static str;

    /// Set the current database context
    async fn set_current_database(&mut self, database: &str);
}

/// MVCC execution strategy
pub struct MvccExecutionStrategy {
    parser: SqlParser,
    executor: MvccSqlExecutor,
    config: SqlEngineConfig,
}

impl MvccExecutionStrategy {
    /// One-line description of how a statement will be executed.
    fn describe_plan(statement: &Statement) -> String {
        match statement {
            Statement::Select(select) => {
                let table = select
                    .from_clause
                    .as_ref()
                    .map(|_| "table")
                    .unwrap_or("no table");
                let mut plan = format!("Seq Scan on {table}");
                if select.where_clause.is_some() {
                    plan.push_str(" + Filter");
                }
                if select.group_by.is_some() {
                    plan.push_str(" + GroupAggregate");
                }
                if select.distinct.is_some() {
                    plan.push_str(" + Unique");
                }
                if select.order_by.is_some() {
                    plan.push_str(" + Sort");
                }
                if select.limit.is_some() || select.offset.is_some() {
                    plan.push_str(" + Limit");
                }
                plan
            }
            Statement::Insert(_) => "Insert".to_string(),
            Statement::Update(_) => "Update".to_string(),
            Statement::Delete(_) => "Delete".to_string(),
            other => format!("{other:?}")
                .split_whitespace()
                .next()
                .unwrap_or("Statement")
                .to_string(),
        }
    }

    pub fn new(config: SqlEngineConfig) -> Self {
        Self {
            parser: SqlParser::new(),
            executor: MvccSqlExecutor::new(),
            config,
        }
    }
}

#[async_trait::async_trait]
impl SqlExecutionStrategy for MvccExecutionStrategy {
    async fn execute(&mut self, sql: &str) -> ProtocolResult<UnifiedExecutionResult> {
        let statement = self.parser.parse(sql)?;
        self.execute_statement(statement).await
    }

    async fn execute_multiple(&mut self, sql: &str) -> ProtocolResult<Vec<UnifiedExecutionResult>> {
        let statements = self.parser.parse_multiple(sql)?;
        let mut results = Vec::new();

        if self.config.auto_transaction {
            let transaction_id = self
                .executor
                .begin_transaction(self.config.default_isolation_level.clone(), None)
                .await?;

            for stmt in statements {
                match self
                    .execute_statement_in_transaction(stmt, transaction_id)
                    .await
                {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        let _ = self.executor.rollback_transaction(transaction_id).await;
                        return Err(e);
                    }
                }
            }
            self.executor.commit_transaction(transaction_id).await?;
        } else {
            for stmt in statements {
                results.push(self.execute_statement_in_transaction(stmt, 1).await?);
            }
        }
        Ok(results)
    }

    async fn execute_statement(
        &mut self,
        statement: Statement,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        if self.config.auto_transaction {
            let transaction_id = self
                .executor
                .begin_transaction(self.config.default_isolation_level.clone(), None)
                .await?;

            match self
                .execute_statement_in_transaction(statement, transaction_id)
                .await
            {
                Ok(result) => {
                    self.executor.commit_transaction(transaction_id).await?;
                    Ok(result)
                }
                Err(e) => {
                    let _ = self.executor.rollback_transaction(transaction_id).await;
                    Err(e)
                }
            }
        } else {
            self.execute_statement_in_transaction(statement, 1).await
        }
    }

    async fn execute_statement_in_transaction(
        &mut self,
        statement: Statement,
        transaction_id: TransactionId,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        match statement {
            Statement::Select(select_stmt)
                if matches!(select_stmt.from_clause, Some(FromClause::JsonTable(_))) =>
            {
                // Handle JSON_TABLE
                if let Some(FromClause::JsonTable(json_table)) = &select_stmt.from_clause {
                    use crate::protocols::postgres_wire::sql::expression_evaluator::{
                        EvaluationContext, ExpressionEvaluator,
                    };
                    let mut evaluator = ExpressionEvaluator::new();
                    let context = EvaluationContext::empty();

                    // Evaluate context item (JSON document)
                    let json_val = evaluator
                        .evaluate(&json_table.context_item, &context)
                        .map_err(|e| {
                            ProtocolError::PostgresError(format!("Evaluation error: {}", e))
                        })?;

                    let json_str = match json_val {
                        SqlValue::Text(s) => s,
                        SqlValue::Json(s) => s.to_string(),
                        SqlValue::Jsonb(s) => s.to_string(),
                        _ => {
                            return Err(ProtocolError::PostgresError(
                                "JSON_TABLE context item must be a string or JSON".to_string(),
                            ))
                        }
                    };

                    // Parse JSON
                    let parsed_json: serde_json::Value = match serde_json::from_str(&json_str) {
                        Ok(v) => v,
                        Err(_) => {
                            return Ok(UnifiedExecutionResult::Select {
                                columns: json_table
                                    .columns
                                    .iter()
                                    .map(|c| c.name.clone())
                                    .collect(),
                                rows: Vec::new(),
                                row_count: 0,
                                transaction_id: Some(transaction_id),
                            })
                        }
                    };

                    // Handle path expression (simplified: only support $[*] for now which means iterate array)
                    let items = if let Some(arr) = parsed_json.as_array() {
                        arr.iter().collect::<Vec<_>>()
                    } else {
                        vec![&parsed_json]
                    };

                    let mut rows = Vec::new();
                    let mut columns = Vec::new();

                    // Set up columns
                    for col in &json_table.columns {
                        columns.push(col.name.clone());
                    }

                    for item in items {
                        let mut row_values = Vec::new();

                        // Map columns
                        for col_def in &json_table.columns {
                            // Extract value based on path
                            // Default path is $.name
                            let path = col_def
                                .path
                                .clone()
                                .unwrap_or_else(|| format!("$.{}", col_def.name));

                            // Simple path extraction: $.key
                            let key = path.trim_start_matches("$.").to_string();
                            let val = item.get(&key).or_else(|| item.get(&col_def.name));

                            let val_str = match val {
                                Some(serde_json::Value::String(s)) => Some(s.clone()),
                                Some(serde_json::Value::Number(n)) => Some(n.to_string()),
                                Some(serde_json::Value::Bool(b)) => Some(b.to_string()),
                                Some(serde_json::Value::Null) => None,
                                Some(v) => Some(v.to_string()),
                                None => None,
                            };
                            row_values.push(val_str);
                        }
                        rows.push(row_values);
                    }

                    let row_count = rows.len();
                    Ok(UnifiedExecutionResult::Select {
                        columns,
                        rows,
                        row_count,
                        transaction_id: Some(transaction_id),
                    })
                } else {
                    unreachable!()
                }
            }
            Statement::Select(select_stmt) => {
                // Check for time travel clause - this requires Iceberg integration
                if let Some(FromClause::Table {
                    time_travel: Some(tt_clause),
                    name,
                    ..
                }) = &select_stmt.from_clause
                {
                    use crate::protocols::postgres_wire::sql::ast::TimeTravelClause;
                    let query_type = match tt_clause {
                        TimeTravelClause::Timestamp(_) => "TIMESTAMP",
                        TimeTravelClause::Version(_) => "VERSION/SNAPSHOT",
                        TimeTravelClause::SystemTime(_) => "FOR SYSTEM_TIME AS OF",
                    };
                    return Err(ProtocolError::PostgresError(format!(
                        "Time travel query ({}) on table '{}' requires Iceberg cold tier. \
                         Time travel SQL syntax is implemented but Iceberg integration is pending. \
                         See orbit/engine/src/storage/iceberg.rs for query_as_of() and query_by_snapshot_id() methods.",
                        query_type,
                        name.full_name()
                    )));
                }

                // Check if we have a FROM clause
                if select_stmt.from_clause.is_none() {
                    // Evaluate expressions without a table context (e.g. SELECT 1, SELECT func())
                    let mut columns = Vec::new();
                    let mut row_values = Vec::new();

                    use crate::protocols::postgres_wire::sql::expression_evaluator::{
                        EvaluationContext, ExpressionEvaluator,
                    };
                    let mut evaluator = ExpressionEvaluator::new();
                    let context = EvaluationContext::empty(); // Empty context

                    for item in &select_stmt.select_list {
                        match item {
                            SelectItem::Expression { expr, alias } => {
                                let value = evaluator.evaluate(expr, &context).map_err(|e| {
                                    ProtocolError::PostgresError(format!("Evaluation error: {}", e))
                                })?;

                                let col_name = alias.clone().unwrap_or_else(|| "expr".to_string());
                                columns.push(col_name);
                                row_values.push(Some(value.to_postgres_string()));
                            }
                            _ => {
                                return Err(ProtocolError::PostgresError(
                                    "Wildcards not allowed without FROM clause".to_string(),
                                ));
                            }
                        }
                    }

                    return Ok(UnifiedExecutionResult::Select {
                        columns,
                        rows: vec![row_values],
                        row_count: 1,
                        transaction_id: Some(transaction_id),
                    });
                }

                let table_name =
                    self.extract_table_name(&Statement::Select(select_stmt.clone()))?;

                // Determine result columns from SELECT list
                // For SELECT *, get columns from table schema
                let mut columns = Vec::new();
                for item in &select_stmt.select_list {
                    match item {
                        SelectItem::Wildcard => {
                            // Get all columns from table schema using get_table_schema
                            if let Ok(Some(table_schema)) =
                                self.executor.get_table_schema(&table_name).await
                            {
                                for col in &table_schema.columns {
                                    columns.push(col.name.clone());
                                }
                            }
                        }
                        SelectItem::Expression { expr, alias } => {
                            // Try to resolve column name from expression
                            let column_name = if let Some(alias) = alias {
                                alias.clone()
                            } else if let Expression::Column(col_ref) = expr {
                                col_ref.name.clone()
                            } else {
                                "expr".to_string()
                            };
                            columns.push(column_name);
                        }
                        SelectItem::QualifiedWildcard { qualifier: _ } => {
                            // For qualified wildcard, get columns from specified table
                            // For now, treat same as wildcard
                            if let Ok(Some(table_schema)) =
                                self.executor.get_table_schema(&table_name).await
                            {
                                for col in &table_schema.columns {
                                    columns.push(col.name.clone());
                                }
                            }
                        }
                    }
                }

                let rows = self
                    .executor
                    .mvcc_read(&table_name, transaction_id, None)
                    .await?;

                // If columns are still empty (no schema, or a wildcard over an
                // empty table) take them from the first row.
                if columns.is_empty() {
                    if let Some(first_row) = rows.first() {
                        columns = first_row.keys().cloned().collect();
                        columns.sort();
                    }
                }

                // Apply the clauses. Reading the table and projecting by name —
                // which is all this did — silently ignored WHERE, GROUP BY,
                // HAVING, DISTINCT, ORDER BY, LIMIT/OFFSET and every aggregate,
                // so `LIMIT 2` returned the whole table and `COUNT(*)` returned
                // one empty column per row.
                use crate::protocols::postgres_wire::sql::select_pipeline;
                let output = select_pipeline::run_select(&select_stmt, rows)?;

                // A wildcard projection is named by the pipeline as `*`; the
                // real column names are the ones resolved from the schema above.
                let resolved_columns = if output.columns.iter().any(|name| name == "*") {
                    columns
                } else {
                    output.columns
                };

                let row_count = output.rows.len();
                Ok(UnifiedExecutionResult::Select {
                    columns: resolved_columns,
                    rows: output.rows,
                    row_count,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::Insert(insert_stmt) => {
                let table_name = insert_stmt.table.full_name();
                let mut count = 0;

                if let InsertSource::Values(values_list) = insert_stmt.source {
                    // Get column names from table schema if not explicitly specified
                    let columns = if let Some(explicit_columns) = &insert_stmt.columns {
                        explicit_columns.clone()
                    } else {
                        // Get columns from table schema using get_table_schema
                        if let Ok(Some(table_schema)) =
                            self.executor.get_table_schema(&table_name).await
                        {
                            table_schema
                                .columns
                                .iter()
                                .map(|c| c.name.clone())
                                .collect()
                        } else {
                            // Fallback to default if table doesn't exist (shouldn't happen)
                            vec!["data".to_string()]
                        }
                    };

                    for values in values_list {
                        let mut row_data = HashMap::new();

                        for (i, value_expr) in values.iter().enumerate() {
                            if i < columns.len() {
                                let value = self.evaluate_expression(value_expr)?;
                                row_data.insert(columns[i].clone(), value);
                            }
                        }

                        self.executor
                            .mvcc_insert(&table_name, transaction_id, row_data)
                            .await?;
                        count += 1;
                    }
                }

                Ok(UnifiedExecutionResult::Insert {
                    count,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::Update(update_stmt) => {
                let table_name = update_stmt.table.full_name();
                let mut updates = HashMap::new();

                for assignment in &update_stmt.set {
                    let value = self.evaluate_expression(&assignment.value)?;
                    let column = match &assignment.target {
                        AssignmentTarget::Column(col) => col.clone(),
                        AssignmentTarget::Columns(_cols) => {
                            return Err(ProtocolError::PostgresError(
                                "Multi-column assignments not supported".to_string(),
                            ));
                        }
                    };
                    updates.insert(column, value);
                }

                // Convert WHERE clause to predicate
                let predicate = update_stmt
                    .where_clause
                    .as_ref()
                    .and_then(|where_expr| self.create_row_predicate(where_expr.clone()));

                let count = self
                    .executor
                    .mvcc_update(&table_name, transaction_id, updates, predicate)
                    .await?;

                Ok(UnifiedExecutionResult::Update {
                    count,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::Delete(delete_stmt) => {
                let table_name = delete_stmt.table.full_name();

                // Convert WHERE clause to predicate
                let predicate = delete_stmt
                    .where_clause
                    .as_ref()
                    .and_then(|where_expr| self.create_row_predicate(where_expr.clone()));

                let count = self
                    .executor
                    .mvcc_delete(&table_name, transaction_id, predicate)
                    .await?;

                Ok(UnifiedExecutionResult::Delete {
                    count,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::Merge(merge_stmt) => {
                // Delegate to executor for now (placeholder implementation)
                let result = self.executor.execute_merge(merge_stmt).await?;
                match result {
                    ExecutionResult::Merge {
                        count,
                        rows,
                        columns,
                    } => Ok(UnifiedExecutionResult::Merge {
                        count,
                        rows,
                        columns,
                        transaction_id: Some(transaction_id),
                    }),
                    _ => Err(ProtocolError::PostgresError(
                        "Unexpected result from MERGE".to_string(),
                    )),
                }
            }
            Statement::CreateTable(create_stmt) => {
                let table_name = create_stmt.name.full_name();

                // Build table schema from CREATE TABLE statement
                use crate::protocols::postgres_wire::sql::ast::ColumnConstraint;
                use crate::protocols::postgres_wire::sql::executor::{ColumnSchema, TableSchema};
                let mut columns = Vec::new();
                for col_def in &create_stmt.columns {
                    columns.push(ColumnSchema {
                        name: col_def.name.clone(),
                        data_type: col_def.data_type.clone(),
                        nullable: !col_def
                            .constraints
                            .iter()
                            .any(|c| matches!(c, ColumnConstraint::NotNull)),
                        default: None, // TODO: Evaluate default expressions
                        constraints: col_def
                            .constraints
                            .iter()
                            .map(|c| format!("{c:?}"))
                            .collect(),
                        generated: None, // TODO: Support generated columns in execution strategy
                    });
                }

                let table_schema = TableSchema {
                    name: table_name.clone(),
                    columns,
                    constraints: Vec::new(), // TODO: Parse table-level constraints
                    indexes: Vec::new(),
                };

                self.executor
                    .create_table_with_schema(&table_name, Some(table_schema))
                    .await?;

                Ok(UnifiedExecutionResult::CreateTable {
                    table_name,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::CreateExtension(ext_stmt) => Ok(UnifiedExecutionResult::CreateExtension {
                extension_name: ext_stmt.name.clone(),
                transaction_id: Some(transaction_id),
            }),
            Statement::DropExtension(ext_stmt) => {
                let extension_names = ext_stmt.names.clone();
                Ok(UnifiedExecutionResult::DropExtension {
                    extension_names,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::CreateIndex(index_stmt) => {
                let index_name = index_stmt
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("{}_idx", index_stmt.table.name));
                let table_name = index_stmt.table.name.clone();
                Ok(UnifiedExecutionResult::CreateIndex {
                    index_name,
                    table_name,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::CreateSchema(schema_stmt) => Ok(UnifiedExecutionResult::CreateSchema {
                schema_name: schema_stmt.name.clone(),
                transaction_id: Some(transaction_id),
            }),
            Statement::CreateView(view_stmt) => Ok(UnifiedExecutionResult::CreateView {
                view_name: view_stmt.name.name.clone(),
                transaction_id: Some(transaction_id),
            }),
            Statement::DropTable(drop_stmt) => {
                let table_names = drop_stmt.names.iter().map(|n| n.name.clone()).collect();
                Ok(UnifiedExecutionResult::DropTable {
                    table_names,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::DropIndex(drop_stmt) => Ok(UnifiedExecutionResult::DropIndex {
                index_names: drop_stmt.names.clone(),
                transaction_id: Some(transaction_id),
            }),
            Statement::DropSchema(drop_stmt) => Ok(UnifiedExecutionResult::DropSchema {
                schema_names: drop_stmt.names.clone(),
                transaction_id: Some(transaction_id),
            }),
            Statement::DropView(drop_stmt) => {
                let view_names = drop_stmt.names.iter().map(|n| n.name.clone()).collect();
                Ok(UnifiedExecutionResult::DropView {
                    view_names,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::Set(set_stmt) => {
                let variable = set_stmt.variable;
                // Evaluate the first value if present
                let value = if let Some(expr) = set_stmt.value.first() {
                    match self.evaluate_expression(expr) {
                        Ok(val) => val.to_string(),
                        Err(_) => "UNKNOWN".to_string(),
                    }
                } else {
                    "DEFAULT".to_string()
                };

                Ok(UnifiedExecutionResult::Set {
                    variable,
                    value,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::UndropTable(undrop_stmt) => {
                // UNDROP TABLE requires Iceberg snapshot integration
                Err(ProtocolError::PostgresError(format!(
                    "UNDROP TABLE '{}' is not yet implemented. \
                     This feature requires Iceberg snapshot-based restoration. \
                     See orbit/engine/src/storage/iceberg.rs for implementation details.",
                    undrop_stmt.name.full_name()
                )))
            }
            Statement::Explain(explain) => {
                // A description of what will run, not a cost estimate: this
                // engine has no statistics or cost model, and inventing numbers
                // that look like PostgreSQL's would be worse than saying so.
                let mut lines = vec![Self::describe_plan(&explain.statement)];
                if explain.analyze {
                    lines.push(
                        "  (ANALYZE requested; per-node timings are not collected)".to_string(),
                    );
                }
                if explain.verbose {
                    lines.push("  (VERBOSE requested; no extra detail available)".to_string());
                }
                lines.push(
                    "  Cost estimates are not available: no statistics are kept.".to_string(),
                );

                Ok(UnifiedExecutionResult::Select {
                    columns: vec!["QUERY PLAN".to_string()],
                    row_count: lines.len(),
                    rows: lines.into_iter().map(|line| vec![Some(line)]).collect(),
                    transaction_id: Some(transaction_id),
                })
            }
            _ => Ok(UnifiedExecutionResult::Other {
                message: "Command completed successfully".to_string(),
                transaction_id: Some(transaction_id),
            }),
        }
    }

    async fn begin_transaction(
        &self,
        isolation_level: Option<IsolationLevel>,
    ) -> ProtocolResult<TransactionId> {
        self.executor.begin_transaction(isolation_level, None).await
    }

    async fn commit_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()> {
        self.executor.commit_transaction(transaction_id).await
    }

    async fn rollback_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()> {
        self.executor.rollback_transaction(transaction_id).await
    }

    fn parse(&mut self, sql: &str) -> ProtocolResult<Statement> {
        self.parser.parse(sql)
    }

    fn is_vector_statement(&self, sql: &str) -> bool {
        let sql_upper = sql.to_uppercase();
        sql_upper.contains("VECTOR(")
            || sql_upper.contains("<->")
            || sql_upper.contains("<#>")
            || sql_upper.contains("<=>")
    }

    async fn cleanup(&self) -> ProtocolResult<usize> {
        self.executor.cleanup_old_transactions().await
    }

    fn strategy_name(&self) -> &'static str {
        "MVCC"
    }

    async fn set_current_database(&mut self, _database: &str) {
        // MVCC execution doesn't currently support database switching
        // This is a no-op for now, but can be implemented when MVCC supports multiple databases
    }
}

impl MvccExecutionStrategy {
    fn extract_table_name(&self, statement: &Statement) -> ProtocolResult<String> {
        match statement {
            Statement::Select(select) => {
                if let Some(FromClause::Table { name, .. }) = &select.from_clause {
                    Ok(name.full_name())
                } else {
                    Err(ProtocolError::PostgresError(
                        "No table found in SELECT".to_string(),
                    ))
                }
            }
            _ => Err(ProtocolError::PostgresError(
                "Cannot extract table name".to_string(),
            )),
        }
    }

    /// Create a row predicate from a WHERE clause expression
    fn create_row_predicate(
        &self,
        where_expr: Expression,
    ) -> crate::protocols::postgres_wire::sql::mvcc_executor::RowPredicate {
        use crate::protocols::postgres_wire::sql::expression_evaluator::ExpressionEvaluator;

        let expr = where_expr.clone();
        Some(Box::new(move |row: &HashMap<String, SqlValue>| -> bool {
            let context = crate::protocols::postgres_wire::sql::expression_evaluator::EvaluationContext::with_row(row.clone());
            let mut evaluator = ExpressionEvaluator::new();

            match evaluator.evaluate(&expr, &context) {
                Ok(SqlValue::Boolean(b)) => b,
                Ok(SqlValue::Null) => false,
                Ok(_) => false,  // Non-boolean result is false
                Err(_) => false, // Error in evaluation is false
            }
        }))
    }

    fn evaluate_expression(&self, expr: &Expression) -> ProtocolResult<SqlValue> {
        match expr {
            Expression::Literal(value) => Ok(value.clone()),
            Expression::Parameter(num) => Ok(SqlValue::Text(format!("${num}"))),
            _ => Ok(SqlValue::Text("evaluated_expr".to_string())),
        }
    }
}

/// Traditional (non-MVCC) execution strategy
pub struct TraditionalExecutionStrategy {
    pub parser: SqlParser,
    pub executor: SqlExecutor,
    #[allow(dead_code)]
    pub config: SqlEngineConfig,
}

impl TraditionalExecutionStrategy {
    pub async fn new(config: SqlEngineConfig) -> ProtocolResult<Self> {
        Ok(Self {
            parser: SqlParser::new(),
            executor: SqlExecutor::new().await?,
            config,
        })
    }

    pub fn with_executor(executor: SqlExecutor, config: SqlEngineConfig) -> Self {
        Self {
            parser: SqlParser::new(),
            executor,
            config,
        }
    }
}

#[async_trait::async_trait]
impl SqlExecutionStrategy for TraditionalExecutionStrategy {
    async fn execute(&mut self, sql: &str) -> ProtocolResult<UnifiedExecutionResult> {
        let statement = self.parser.parse(sql)?;
        self.execute_statement(statement).await
    }

    async fn execute_multiple(&mut self, sql: &str) -> ProtocolResult<Vec<UnifiedExecutionResult>> {
        let statements = self.parser.parse_multiple(sql)?;
        let mut results = Vec::new();

        for stmt in statements {
            results.push(self.execute_statement(stmt).await?);
        }

        Ok(results)
    }

    async fn execute_statement(
        &mut self,
        statement: Statement,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        let result = self.executor.execute_statement(statement).await?;
        Ok(self.convert_execution_result(result, None))
    }

    async fn execute_statement_in_transaction(
        &mut self,
        statement: Statement,
        _transaction_id: TransactionId,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        // Traditional strategy ignores transaction ID
        self.execute_statement(statement).await
    }

    async fn begin_transaction(
        &self,
        _isolation_level: Option<IsolationLevel>,
    ) -> ProtocolResult<TransactionId> {
        Err(ProtocolError::PostgresError(
            "Traditional executor doesn't support explicit transactions".to_string(),
        ))
    }

    async fn commit_transaction(&self, _transaction_id: TransactionId) -> ProtocolResult<()> {
        Err(ProtocolError::PostgresError(
            "Traditional executor doesn't support explicit transactions".to_string(),
        ))
    }

    async fn rollback_transaction(&self, _transaction_id: TransactionId) -> ProtocolResult<()> {
        Err(ProtocolError::PostgresError(
            "Traditional executor doesn't support explicit transactions".to_string(),
        ))
    }

    fn parse(&mut self, sql: &str) -> ProtocolResult<Statement> {
        self.parser.parse(sql)
    }

    fn is_vector_statement(&self, sql: &str) -> bool {
        let sql_upper = sql.to_uppercase();
        sql_upper.contains("VECTOR(") || sql_upper.contains("<->")
    }

    async fn cleanup(&self) -> ProtocolResult<usize> {
        Ok(0) // Traditional executor doesn't need cleanup
    }

    fn strategy_name(&self) -> &'static str {
        "Traditional"
    }

    async fn set_current_database(&mut self, database: &str) {
        // Set the current database in the underlying SqlExecutor
        self.executor.set_current_database(database).await;
    }
}

impl TraditionalExecutionStrategy {
    fn convert_execution_result(
        &self,
        result: ExecutionResult,
        transaction_id: Option<TransactionId>,
    ) -> UnifiedExecutionResult {
        match result {
            ExecutionResult::Select {
                columns,
                rows,
                row_count,
            } => UnifiedExecutionResult::Select {
                columns,
                rows,
                row_count,
                transaction_id,
            },
            ExecutionResult::Insert { count, .. } => UnifiedExecutionResult::Insert {
                count,
                transaction_id,
            },
            ExecutionResult::Update { count, .. } => UnifiedExecutionResult::Update {
                count,
                transaction_id,
            },
            ExecutionResult::Delete { count, .. } => UnifiedExecutionResult::Delete {
                count,
                transaction_id,
            },
            ExecutionResult::CreateTable { table_name } => UnifiedExecutionResult::CreateTable {
                table_name,
                transaction_id,
            },
            ExecutionResult::CreateIndex {
                index_name,
                table_name,
            } => UnifiedExecutionResult::CreateIndex {
                index_name,
                table_name,
                transaction_id,
            },
            ExecutionResult::Set { variable, value } => UnifiedExecutionResult::Set {
                variable,
                value,
                transaction_id,
            },

            ExecutionResult::CreateExtension { extension_name } => {
                UnifiedExecutionResult::CreateExtension {
                    extension_name,
                    transaction_id,
                }
            }
            ExecutionResult::CreateSchema { schema_name } => UnifiedExecutionResult::CreateSchema {
                schema_name,
                transaction_id,
            },
            ExecutionResult::CreateView { view_name } => UnifiedExecutionResult::CreateView {
                view_name,
                transaction_id,
            },
            ExecutionResult::DropTable { table_names } => UnifiedExecutionResult::DropTable {
                table_names,
                transaction_id,
            },
            ExecutionResult::DropIndex { index_names } => UnifiedExecutionResult::DropIndex {
                index_names,
                transaction_id,
            },
            ExecutionResult::DropExtension { extension_names } => {
                UnifiedExecutionResult::DropExtension {
                    extension_names,
                    transaction_id,
                }
            }
            ExecutionResult::DropSchema { schema_names } => UnifiedExecutionResult::DropSchema {
                schema_names,
                transaction_id,
            },
            ExecutionResult::DropView { view_names } => UnifiedExecutionResult::DropView {
                view_names,
                transaction_id,
            },
            _ => UnifiedExecutionResult::Other {
                message: "Command completed successfully".to_string(),
                transaction_id,
            },
        }
    }
}

/// Configurable SQL engine using strategy pattern
pub struct ConfigurableSqlEngine {
    strategy: Box<dyn SqlExecutionStrategy>,
    config: SqlEngineConfig,
}

impl ConfigurableSqlEngine {
    /// Create a new SQL engine with default configuration (MVCC enabled)
    pub fn new() -> Self {
        let config = SqlEngineConfig::default();
        Self::with_config(config)
    }

    /// Create a new SQL engine with custom configuration
    pub fn with_config(config: SqlEngineConfig) -> Self {
        let strategy: Box<dyn SqlExecutionStrategy> = match config.strategy {
            ExecutionStrategy::Mvcc => Box::new(MvccExecutionStrategy::new(config.clone())),
            ExecutionStrategy::Traditional => {
                // Create executor using tokio runtime or current handle
                use crate::protocols::postgres_wire::sql::executor::SqlExecutor;

                // Use simple memory constructor for testing to avoid runtime issues
                let executor = if cfg!(test) {
                    SqlExecutor::new_simple_memory()
                } else {
                    #[allow(deprecated)]
                    SqlExecutor::new_in_memory()
                };

                Box::new(TraditionalExecutionStrategy::with_executor(
                    executor,
                    config.clone(),
                ))
            }
            ExecutionStrategy::Hybrid => {
                // Default to MVCC for hybrid mode
                Box::new(MvccExecutionStrategy::new(config.clone()))
            }
        };

        Self { strategy, config }
    }

    /// Create an MVCC-enabled engine (explicitly)
    pub fn new_mvcc() -> Self {
        let config = SqlEngineConfig {
            strategy: ExecutionStrategy::Mvcc,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Create a traditional engine (for compatibility)
    pub fn new_traditional() -> Self {
        let config = SqlEngineConfig {
            strategy: ExecutionStrategy::Traditional,
            auto_transaction: false,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Execute a SQL statement
    pub async fn execute(&mut self, sql: &str) -> ProtocolResult<UnifiedExecutionResult> {
        self.strategy.execute(sql).await
    }

    /// Execute multiple SQL statements
    pub async fn execute_multiple(
        &mut self,
        sql: &str,
    ) -> ProtocolResult<Vec<UnifiedExecutionResult>> {
        self.strategy.execute_multiple(sql).await
    }

    /// Execute a parsed statement
    pub async fn execute_statement(
        &mut self,
        statement: Statement,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        self.strategy.execute_statement(statement).await
    }

    /// Execute within a transaction (MVCC only)
    pub async fn execute_in_transaction(
        &mut self,
        sql: &str,
        transaction_id: TransactionId,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        let mut parser = crate::protocols::postgres_wire::sql::parser::SqlParser::new();
        let statement = parser.parse(sql)?;
        self.strategy
            .execute_statement_in_transaction(statement, transaction_id)
            .await
    }

    /// Begin a transaction (MVCC only)
    pub async fn begin_transaction(&self) -> ProtocolResult<TransactionId> {
        self.strategy
            .begin_transaction(self.config.default_isolation_level.clone())
            .await
    }

    /// Commit a transaction (MVCC only)
    pub async fn commit_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()> {
        self.strategy.commit_transaction(transaction_id).await
    }

    /// Rollback a transaction (MVCC only)
    pub async fn rollback_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()> {
        self.strategy.rollback_transaction(transaction_id).await
    }

    /// Parse SQL without execution
    pub fn parse(&mut self, sql: &str) -> ProtocolResult<Statement> {
        self.strategy.parse(sql)
    }

    /// Check if statement uses vector operations
    pub fn is_vector_statement(&self, sql: &str) -> bool {
        self.strategy.is_vector_statement(sql)
    }

    /// Get current configuration
    pub fn config(&self) -> &SqlEngineConfig {
        &self.config
    }

    /// Get current strategy name
    pub fn strategy_name(&self) -> &'static str {
        self.strategy.strategy_name()
    }

    /// Set the current database context
    pub async fn set_current_database(&mut self, database: &str) {
        self.strategy.set_current_database(database).await;
    }

    /// Cleanup old data/transactions
    pub async fn cleanup(&self) -> ProtocolResult<usize> {
        self.strategy.cleanup().await
    }

    /// Switch execution strategy at runtime
    pub fn switch_strategy(&mut self, new_strategy: ExecutionStrategy) {
        if self.config.strategy != new_strategy {
            self.config.strategy = new_strategy.clone();

            self.strategy = match new_strategy {
                ExecutionStrategy::Mvcc => {
                    Box::new(MvccExecutionStrategy::new(self.config.clone()))
                }
                ExecutionStrategy::Traditional => {
                    // Create executor using tokio runtime or current handle
                    use crate::protocols::postgres_wire::sql::executor::SqlExecutor;

                    // Use simple memory constructor for testing to avoid runtime issues
                    let executor = if cfg!(test) {
                        SqlExecutor::new_simple_memory()
                    } else {
                        #[allow(deprecated)]
                        SqlExecutor::new_in_memory()
                    };

                    Box::new(TraditionalExecutionStrategy::with_executor(
                        executor,
                        self.config.clone(),
                    ))
                }
                ExecutionStrategy::Hybrid => {
                    Box::new(MvccExecutionStrategy::new(self.config.clone()))
                }
            };
        }
    }
}

impl Default for ConfigurableSqlEngine {
    /// Default to MVCC for better performance and deadlock prevention
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_uses_mvcc() {
        let engine = ConfigurableSqlEngine::new();
        assert_eq!(engine.strategy_name(), "MVCC");
        assert_eq!(engine.config().strategy, ExecutionStrategy::Mvcc);
    }

    #[tokio::test]
    async fn test_strategy_switching() {
        let mut engine = ConfigurableSqlEngine::new();
        assert_eq!(engine.strategy_name(), "MVCC");

        engine.switch_strategy(ExecutionStrategy::Traditional);
        assert_eq!(engine.strategy_name(), "Traditional");

        engine.switch_strategy(ExecutionStrategy::Mvcc);
        assert_eq!(engine.strategy_name(), "MVCC");
    }

    #[tokio::test]
    async fn test_mvcc_transaction_support() {
        let engine = ConfigurableSqlEngine::new_mvcc();

        let tx_id = engine.begin_transaction().await;
        assert!(tx_id.is_ok());

        if let Ok(tx) = tx_id {
            let commit_result = engine.commit_transaction(tx).await;
            assert!(commit_result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_traditional_no_transactions() {
        let engine = ConfigurableSqlEngine::new_traditional();

        let tx_id = engine.begin_transaction().await;
        assert!(tx_id.is_err());
        assert!(tx_id
            .unwrap_err()
            .to_string()
            .contains("doesn't support explicit transactions"));
    }
}
