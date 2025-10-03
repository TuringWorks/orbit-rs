//! SQL Execution Engine
//! 
//! This module provides execution of parsed SQL statements against
//! Orbit's actor-based storage system and vector operations.

pub mod ddl_executor;

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::ast::Statement;
use crate::postgres_wire::query_engine::QueryResult;
use orbit_client::OrbitClient;

/// SQL execution result
pub type ExecutionResult = QueryResult;

/// Main SQL executor that coordinates execution of different statement types
pub struct SqlExecutor {
    /// Optional OrbitClient for actor operations
    orbit_client: Option<OrbitClient>,
}

impl SqlExecutor {
    /// Create a new SQL executor
    pub fn new() -> Self {
        Self {
            orbit_client: None,
        }
    }
    
    /// Create a new SQL executor with OrbitClient support
    pub fn new_with_vector_support(orbit_client: OrbitClient) -> Self {
        Self {
            orbit_client: Some(orbit_client),
        }
    }
    
    /// Execute a parsed SQL statement
    pub async fn execute(&self, statement: Statement) -> ProtocolResult<ExecutionResult> {
        match statement {
            // DDL Statements
            Statement::CreateTable(stmt) => {
                ddl_executor::execute_create_table(self, stmt).await
            }
            Statement::CreateIndex(stmt) => {
                ddl_executor::execute_create_index(self, stmt).await
            }
            Statement::CreateView(stmt) => {
                ddl_executor::execute_create_view(self, stmt).await
            }
            Statement::CreateSchema(stmt) => {
                ddl_executor::execute_create_schema(self, stmt).await
            }
            Statement::CreateExtension(stmt) => {
                ddl_executor::execute_create_extension(self, stmt).await
            }
            Statement::AlterTable(stmt) => {
                ddl_executor::execute_alter_table(self, stmt).await
            }
            Statement::DropTable(stmt) => {
                ddl_executor::execute_drop_table(self, stmt).await
            }
            Statement::DropIndex(stmt) => {
                ddl_executor::execute_drop_index(self, stmt).await
            }
            Statement::DropView(stmt) => {
                ddl_executor::execute_drop_view(self, stmt).await
            }
            Statement::DropSchema(stmt) => {
                ddl_executor::execute_drop_schema(self, stmt).await
            }
            Statement::DropExtension(stmt) => {
                ddl_executor::execute_drop_extension(self, stmt).await
            }
            
            // DML Statements (placeholder)
            Statement::Select(_) => {
                Err(ProtocolError::PostgresError(
                    "SELECT statement execution not yet implemented".to_string()
                ))
            }
            Statement::Insert(_) => {
                Err(ProtocolError::PostgresError(
                    "INSERT statement execution not yet implemented".to_string()
                ))
            }
            Statement::Update(_) => {
                Err(ProtocolError::PostgresError(
                    "UPDATE statement execution not yet implemented".to_string()
                ))
            }
            Statement::Delete(_) => {
                Err(ProtocolError::PostgresError(
                    "DELETE statement execution not yet implemented".to_string()
                ))
            }
            
            // DCL Statements (placeholder)
            Statement::Grant(_) => {
                Err(ProtocolError::PostgresError(
                    "GRANT statement execution not yet implemented".to_string()
                ))
            }
            Statement::Revoke(_) => {
                Err(ProtocolError::PostgresError(
                    "REVOKE statement execution not yet implemented".to_string()
                ))
            }
            
            // TCL Statements (placeholder)
            Statement::Begin(_) => {
                Err(ProtocolError::PostgresError(
                    "BEGIN statement execution not yet implemented".to_string()
                ))
            }
            Statement::Commit(_) => {
                Err(ProtocolError::PostgresError(
                    "COMMIT statement execution not yet implemented".to_string()
                ))
            }
            Statement::Rollback(_) => {
                Err(ProtocolError::PostgresError(
                    "ROLLBACK statement execution not yet implemented".to_string()
                ))
            }
            Statement::Savepoint(_) => {
                Err(ProtocolError::PostgresError(
                    "SAVEPOINT statement execution not yet implemented".to_string()
                ))
            }
            Statement::ReleaseSavepoint(_) => {
                Err(ProtocolError::PostgresError(
                    "RELEASE SAVEPOINT statement execution not yet implemented".to_string()
                ))
            }
            
            // Utility Statements (placeholder)
            Statement::Explain(_) => {
                Err(ProtocolError::PostgresError(
                    "EXPLAIN statement execution not yet implemented".to_string()
                ))
            }
            Statement::Show(_) => {
                Err(ProtocolError::PostgresError(
                    "SHOW statement execution not yet implemented".to_string()
                ))
            }
            Statement::Use(_) => {
                Err(ProtocolError::PostgresError(
                    "USE statement execution not yet implemented".to_string()
                ))
            }
            Statement::Describe(_) => {
                Err(ProtocolError::PostgresError(
                    "DESCRIBE statement execution not yet implemented".to_string()
                ))
            }
        }
    }
    
    /// Get reference to OrbitClient if available
    pub fn orbit_client(&self) -> Option<&OrbitClient> {
        self.orbit_client.as_ref()
    }
}

impl Default for SqlExecutor {
    fn default() -> Self {
        Self::new()
    }
}