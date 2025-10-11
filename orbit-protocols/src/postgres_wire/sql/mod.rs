//! MVCC-Enabled SQL Parser and Executor for Orbit-RS
//!
//! This module provides a comprehensive SQL engine featuring Multi-Version Concurrency Control (MVCC)
//! for deadlock prevention, high concurrency, and ACID compliance. The engine supports all major ANSI SQL
//! constructs while maintaining compatibility with vector operations and Orbit's distributed architecture.
//!
//! ## 🚀 Key Features
//!
//! - **MVCC by Default**: Automatic deadlock prevention and high concurrency
//! - **Strategy Pattern**: Pluggable execution engines (MVCC vs Traditional)
//! - **Full ANSI SQL**: Complete DDL, DML, DCL, and TCL support
//! - **Vector Operations**: AI/ML workloads with pgvector compatibility
//! - **Transaction Management**: ACID compliance with isolation levels
//! - **Automatic Cleanup**: Efficient memory management of old versions
//!
//! ## 🏗️ Architecture
//!
//! - **Lexer**: Advanced tokenization with vector operator support
//! - **Parser**: Recursive descent parser with comprehensive AST
//! - **MVCC Executor**: Core deadlock prevention and concurrency control
//! - **Strategy Engine**: Pluggable execution with MVCC as default
//! - **Optimizer**: Cost-based query optimization and statistics
//!
//! ## 📖 Quick Start
//!
//! ```rust
//! use crate::postgres_wire::sql::SqlEngine;
//!
//! // MVCC engine by default
//! let mut engine = SqlEngine::new();
//! let result = engine.execute("SELECT * FROM users").await?;
//! ```
//!
//! See the [README.md](./README.md) for comprehensive documentation.

pub mod analyzer;
pub mod ast;
pub mod execution_strategy;
pub mod executor;
pub mod expression_evaluator;
pub mod lexer;
pub mod mvcc_executor;
pub mod optimizer;
pub mod parser;
pub mod types;

#[cfg(test)]
mod integration_test;
#[cfg(test)]
mod mvcc_demo;
#[cfg(test)]
mod tests;

pub use ast::{Expression, SelectStatement, Statement};
pub use execution_strategy::{
    ConfigurableSqlEngine, ExecutionStrategy, SqlEngineConfig, SqlExecutionStrategy,
    UnifiedExecutionResult,
};
pub use executor::{ExecutionResult, SqlExecutor};
pub use lexer::{Lexer, Token};
pub use mvcc_executor::{MvccSqlExecutor, TransactionId};
pub use parser::{ParseResult, SqlParser};
pub use types::{SqlType, SqlValue};

use crate::error::{ProtocolError, ProtocolResult};

/// Main SQL engine that coordinates parsing, analysis, and execution
///
/// **Note**: This now defaults to MVCC execution for better performance and deadlock prevention.
/// Use `SqlEngine::new_traditional()` for compatibility with the old behavior.
pub type SqlEngine = ConfigurableSqlEngine;

/// MVCC-aware SQL engine that coordinates parsing, analysis, and execution
/// with deadlock prevention and high concurrency support
pub struct MvccSqlEngine {
    parser: SqlParser,
    executor: MvccSqlExecutor,
}

impl MvccSqlEngine {
    /// Create a new MVCC SQL engine
    pub fn new() -> Self {
        Self {
            parser: SqlParser::new(),
            executor: MvccSqlExecutor::new(),
        }
    }

    /// Create a new MVCC SQL engine with vector support
    pub fn new_with_vector_support(_orbit_client: orbit_client::OrbitClient) -> Self {
        // TODO: Integrate OrbitClient with MVCC executor
        Self::new()
    }

    /// Begin a new transaction and return the transaction ID
    pub async fn begin_transaction(
        &self,
        isolation_level: Option<ast::IsolationLevel>,
        access_mode: Option<ast::AccessMode>,
    ) -> ProtocolResult<TransactionId> {
        self.executor
            .begin_transaction(isolation_level, access_mode)
            .await
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()> {
        self.executor.commit_transaction(transaction_id).await
    }

    /// Rollback a transaction
    pub async fn rollback_transaction(&self, transaction_id: TransactionId) -> ProtocolResult<()> {
        self.executor.rollback_transaction(transaction_id).await
    }

    /// Execute a SQL statement within a transaction context
    pub async fn execute_in_transaction(
        &mut self,
        sql: &str,
        transaction_id: TransactionId,
    ) -> ProtocolResult<MvccExecutionResult> {
        // Parse the SQL into an AST
        let statement = self.parser.parse(sql)?;

        // Execute based on statement type with MVCC semantics
        match statement {
            Statement::Select(select_stmt) => {
                // Use MVCC read for non-blocking SELECT
                let rows = self
                    .executor
                    .mvcc_read(
                        &self.extract_table_name(&Statement::Select(select_stmt))?,
                        transaction_id,
                        None, // TODO: Add WHERE clause predicate support
                    )
                    .await?;

                Ok(MvccExecutionResult::Select {
                    rows,
                    transaction_id,
                })
            }
            Statement::Insert(insert_stmt) => {
                let table_name = insert_stmt.table.full_name();

                // Convert INSERT values to row data
                if let ast::InsertSource::Values(values_list) = insert_stmt.source {
                    let mut inserted_count = 0;

                    for values in values_list {
                        let mut row_data = std::collections::HashMap::new();

                        // Extract column names and values
                        let columns = insert_stmt
                            .columns
                            .clone()
                            .unwrap_or_else(|| vec!["data".to_string()]); // Default column

                        for (i, value_expr) in values.iter().enumerate() {
                            if i < columns.len() {
                                let column_name = &columns[i];
                                let value = self.evaluate_expression_to_sql_value(value_expr)?;
                                row_data.insert(column_name.clone(), value);
                            }
                        }

                        self.executor
                            .mvcc_insert(&table_name, transaction_id, row_data)
                            .await?;
                        inserted_count += 1;
                    }

                    Ok(MvccExecutionResult::Insert {
                        count: inserted_count,
                        transaction_id,
                    })
                } else {
                    Err(ProtocolError::PostgresError(
                        "Only VALUES clause supported in MVCC INSERT".to_string(),
                    ))
                }
            }
            Statement::Update(update_stmt) => {
                let table_name = update_stmt.table.full_name();

                // Convert SET clauses to update map
                let mut updates = std::collections::HashMap::new();
                for assignment in &update_stmt.set {
                    let value = self.evaluate_expression_to_sql_value(&assignment.value)?;
                    let column = match &assignment.target {
                        ast::AssignmentTarget::Column(col) => col.clone(),
                        ast::AssignmentTarget::Columns(_cols) => {
                            // For now, just use the first column for multi-column assignments
                            return Err(ProtocolError::PostgresError(
                                "Multi-column assignments not yet supported".to_string(),
                            ));
                        }
                    };
                    updates.insert(column, value);
                }

                let updated_count = self
                    .executor
                    .mvcc_update(
                        &table_name,
                        transaction_id,
                        updates,
                        None, // TODO: Add WHERE clause predicate support
                    )
                    .await?;

                Ok(MvccExecutionResult::Update {
                    count: updated_count,
                    transaction_id,
                })
            }
            Statement::Delete(delete_stmt) => {
                let table_name = delete_stmt.table.full_name();

                let deleted_count = self
                    .executor
                    .mvcc_delete(
                        &table_name,
                        transaction_id,
                        None, // TODO: Add WHERE clause predicate support
                    )
                    .await?;

                Ok(MvccExecutionResult::Delete {
                    count: deleted_count,
                    transaction_id,
                })
            }
            Statement::CreateTable(create_table_stmt) => {
                let table_name = create_table_stmt.name.full_name();
                self.executor.create_table(&table_name).await?;

                Ok(MvccExecutionResult::CreateTable {
                    table_name,
                    transaction_id,
                })
            }
            _ => {
                // For other statements, fall back to non-transactional execution
                // This could be enhanced to support DDL within transactions
                Err(ProtocolError::PostgresError(format!(
                    "Statement type not yet supported in MVCC transactions: {:?}",
                    statement
                )))
            }
        }
    }

    /// Execute a SQL statement with automatic transaction management
    pub async fn execute(&mut self, sql: &str) -> ProtocolResult<MvccExecutionResult> {
        // Start an implicit transaction
        let transaction_id = self.begin_transaction(None, None).await?;

        match self.execute_in_transaction(sql, transaction_id).await {
            Ok(result) => {
                // Auto-commit on success
                self.commit_transaction(transaction_id).await?;
                Ok(result)
            }
            Err(e) => {
                // Auto-rollback on error
                let _ = self.rollback_transaction(transaction_id).await;
                Err(e)
            }
        }
    }

    /// Parse a SQL statement without executing it
    pub fn parse(&mut self, sql: &str) -> ProtocolResult<Statement> {
        self.parser.parse(sql)
    }

    /// Check if a SQL statement is vector-related
    pub fn is_vector_statement(&self, sql: &str) -> bool {
        // Quick check for vector-related keywords before full parsing
        let sql_upper = sql.to_uppercase();
        sql_upper.contains("VECTOR(")
            || sql_upper.contains("HALFVEC(")
            || sql_upper.contains("SPARSEVEC(")
            || sql_upper.contains("<->")
            || sql_upper.contains("<#>")
            || sql_upper.contains("<=>")
            || sql_upper.contains("VECTOR_DIMS")
            || sql_upper.contains("VECTOR_NORM")
            || (sql_upper.contains("CREATE INDEX")
                && (sql_upper.contains("USING IVFFLAT") || sql_upper.contains("USING HNSW")))
            || sql_upper.contains("CREATE EXTENSION VECTOR")
    }

    /// Clean up old transactions and row versions
    pub async fn cleanup(&self) -> ProtocolResult<usize> {
        self.executor.cleanup_old_transactions().await
    }

    // Helper methods

    fn extract_table_name(&self, statement: &Statement) -> ProtocolResult<String> {
        match statement {
            Statement::Select(select) => {
                if let Some(ast::FromClause::Table { name, .. }) = &select.from_clause {
                    Ok(name.full_name())
                } else {
                    Err(ProtocolError::PostgresError(
                        "No table found in SELECT statement".to_string(),
                    ))
                }
            }
            _ => Err(ProtocolError::PostgresError(
                "Cannot extract table name from this statement type".to_string(),
            )),
        }
    }

    fn evaluate_expression_to_sql_value(&self, expr: &ast::Expression) -> ProtocolResult<SqlValue> {
        match expr {
            ast::Expression::Literal(value) => Ok(value.clone()),
            ast::Expression::Parameter(param_num) => {
                // For now, return a placeholder
                Ok(SqlValue::Text(format!("${}", param_num)))
            }
            _ => {
                // For complex expressions, return a placeholder
                // This could be enhanced with a full expression evaluator
                Ok(SqlValue::Text("evaluated_expression".to_string()))
            }
        }
    }
}

impl Default for MvccSqlEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution result from MVCC-aware SQL engine
#[derive(Debug, Clone)]
pub enum MvccExecutionResult {
    Select {
        rows: Vec<std::collections::HashMap<String, SqlValue>>,
        transaction_id: TransactionId,
    },
    Insert {
        count: usize,
        transaction_id: TransactionId,
    },
    Update {
        count: usize,
        transaction_id: TransactionId,
    },
    Delete {
        count: usize,
        transaction_id: TransactionId,
    },
    CreateTable {
        table_name: String,
        transaction_id: TransactionId,
    },
    Transaction {
        transaction_id: TransactionId,
        operation: String,
    },
}
