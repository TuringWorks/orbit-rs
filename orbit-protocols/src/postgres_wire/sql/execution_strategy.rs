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

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::{
    ast::*,
    executor::{ExecutionResult, SqlExecutor},
    mvcc_executor::{MvccSqlExecutor, TransactionId},
    parser::SqlParser,
    types::SqlValue,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// Transaction control result
    Transaction {
        transaction_id: TransactionId,
        operation: String,
    },
    /// Other operations
    Other {
        message: String,
        transaction_id: Option<TransactionId>,
    },
}

/// Strategy trait for SQL execution engines
#[async_trait::async_trait]
pub trait SqlExecutionStrategy: Send + Sync {
    /// Execute a SQL statement
    async fn execute(&mut self, sql: &str) -> ProtocolResult<UnifiedExecutionResult>;

    /// Execute within an explicit transaction (MVCC only)
    async fn execute_in_transaction(
        &mut self,
        sql: &str,
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
}

/// MVCC execution strategy
pub struct MvccExecutionStrategy {
    parser: SqlParser,
    executor: MvccSqlExecutor,
    config: SqlEngineConfig,
}

impl MvccExecutionStrategy {
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
        if self.config.auto_transaction {
            // Use automatic transaction management
            let transaction_id = self
                .executor
                .begin_transaction(self.config.default_isolation_level.clone(), None)
                .await?;

            match self.execute_in_transaction(sql, transaction_id).await {
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
            // Execute without transaction management
            self.execute_in_transaction(sql, 1).await // Use dummy transaction ID
        }
    }

    async fn execute_in_transaction(
        &mut self,
        sql: &str,
        transaction_id: TransactionId,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        let statement = self.parser.parse(sql)?;

        match statement {
            Statement::Select(select_stmt) => {
                let table_name = self.extract_table_name(&Statement::Select(select_stmt))?;
                let rows = self
                    .executor
                    .mvcc_read(&table_name, transaction_id, None)
                    .await?;

                // Convert MVCC rows to string format for compatibility
                let mut string_rows = Vec::new();
                let mut columns = Vec::new();

                if let Some(first_row) = rows.first() {
                    columns = first_row.keys().cloned().collect();
                }

                for row in rows {
                    let mut string_row = Vec::new();
                    for col in &columns {
                        let value = row
                            .get(col)
                            .map(|v| v.to_postgres_string())
                            .or(Some("".to_string()));
                        string_row.push(value);
                    }
                    string_rows.push(string_row);
                }

                let row_count = string_rows.len();
                Ok(UnifiedExecutionResult::Select {
                    columns,
                    rows: string_rows,
                    row_count,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::Insert(insert_stmt) => {
                let table_name = insert_stmt.table.full_name();
                let mut count = 0;

                if let InsertSource::Values(values_list) = insert_stmt.source {
                    for values in values_list {
                        let mut row_data = HashMap::new();
                        let columns = insert_stmt
                            .columns
                            .as_ref()
                            .map(|cols| cols.clone())
                            .unwrap_or_else(|| vec!["data".to_string()]);

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

                let count = self
                    .executor
                    .mvcc_update(&table_name, transaction_id, updates, None)
                    .await?;

                Ok(UnifiedExecutionResult::Update {
                    count,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::Delete(delete_stmt) => {
                let table_name = delete_stmt.table.full_name();
                let count = self
                    .executor
                    .mvcc_delete(&table_name, transaction_id, None)
                    .await?;

                Ok(UnifiedExecutionResult::Delete {
                    count,
                    transaction_id: Some(transaction_id),
                })
            }
            Statement::CreateTable(create_stmt) => {
                let table_name = create_stmt.name.full_name();
                self.executor.create_table(&table_name).await?;

                Ok(UnifiedExecutionResult::CreateTable {
                    table_name,
                    transaction_id: Some(transaction_id),
                })
            }
            _ => Ok(UnifiedExecutionResult::Other {
                message: format!("Statement executed: {:?}", statement),
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

    fn evaluate_expression(&self, expr: &Expression) -> ProtocolResult<SqlValue> {
        match expr {
            Expression::Literal(value) => Ok(value.clone()),
            Expression::Parameter(num) => Ok(SqlValue::Text(format!("${}", num))),
            _ => Ok(SqlValue::Text("evaluated_expr".to_string())),
        }
    }
}

/// Traditional (non-MVCC) execution strategy
pub struct TraditionalExecutionStrategy {
    parser: SqlParser,
    executor: SqlExecutor,
    config: SqlEngineConfig,
}

impl TraditionalExecutionStrategy {
    pub fn new(config: SqlEngineConfig) -> Self {
        Self {
            parser: SqlParser::new(),
            executor: SqlExecutor::new(),
            config,
        }
    }
}

#[async_trait::async_trait]
impl SqlExecutionStrategy for TraditionalExecutionStrategy {
    async fn execute(&mut self, sql: &str) -> ProtocolResult<UnifiedExecutionResult> {
        let result = self.executor.execute(sql).await?;
        Ok(self.convert_execution_result(result, None))
    }

    async fn execute_in_transaction(
        &mut self,
        sql: &str,
        _transaction_id: TransactionId,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        // Traditional executor doesn't support explicit transactions
        self.execute(sql).await
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
            ExecutionResult::Insert { count } => UnifiedExecutionResult::Insert {
                count,
                transaction_id,
            },
            ExecutionResult::Update { count } => UnifiedExecutionResult::Update {
                count,
                transaction_id,
            },
            ExecutionResult::Delete { count } => UnifiedExecutionResult::Delete {
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
            _ => UnifiedExecutionResult::Other {
                message: format!("Operation completed: {:?}", result),
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
                Box::new(TraditionalExecutionStrategy::new(config.clone()))
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

    /// Execute within a transaction (MVCC only)
    pub async fn execute_in_transaction(
        &mut self,
        sql: &str,
        transaction_id: TransactionId,
    ) -> ProtocolResult<UnifiedExecutionResult> {
        self.strategy
            .execute_in_transaction(sql, transaction_id)
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
                    Box::new(TraditionalExecutionStrategy::new(self.config.clone()))
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
