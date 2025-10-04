//! Comprehensive SQL Parser and Executor for ANSI SQL Compliance
//! 
//! This module provides a complete SQL parser that supports all major ANSI SQL
//! constructs while maintaining compatibility with existing vector operations
//! and Orbit's actor-based storage system.
//!
//! ## Architecture
//! 
//! - **Lexer**: Tokenizes SQL input into structured tokens
//! - **Parser**: Recursive descent parser with AST generation
//! - **Analyzer**: Semantic analysis, type checking, and optimization
//! - **Executor**: Query execution engine with actor/vector integration
//!
//! ## Features
//! 
//! - Full ANSI SQL DDL, DML, DCL, and TCL support
//! - Complex expressions, JOINs, and subqueries
//! - Vector operations and pgvector compatibility
//! - Comprehensive error handling and recovery
//! - Performance optimization and caching

pub mod lexer;
pub mod ast;
pub mod parser;
pub mod analyzer;
pub mod executor;
pub mod types;

#[cfg(test)]
mod tests;

pub use ast::{Statement, SelectStatement, Expression};
pub use lexer::{Lexer, Token};
pub use parser::{SqlParser, ParseResult};
pub use executor::{SqlExecutor, ExecutionResult};
pub use types::{SqlType, SqlValue};

use crate::error::{ProtocolError, ProtocolResult};

/// Main SQL engine that coordinates parsing, analysis, and execution
pub struct SqlEngine {
    parser: SqlParser,
    executor: SqlExecutor,
}

impl SqlEngine {
    /// Create a new SQL engine
    pub fn new() -> Self {
        Self {
            parser: SqlParser::new(),
            executor: SqlExecutor::new(),
        }
    }

    /// Create a new SQL engine with vector support
    pub fn new_with_vector_support(orbit_client: orbit_client::OrbitClient) -> Self {
        Self {
            parser: SqlParser::new(),
            executor: SqlExecutor::new_with_vector_support(orbit_client),
        }
    }

    /// Parse and execute a SQL statement
    pub async fn execute(&mut self, sql: &str) -> ProtocolResult<ExecutionResult> {
        // Parse the SQL into an AST
        let statement = self.parser.parse(sql)?;
        
        // Execute the statement
        self.executor.execute(statement).await
    }

    /// Parse a SQL statement without executing it (for validation/analysis)
    pub fn parse(&mut self, sql: &str) -> ProtocolResult<Statement> {
        self.parser.parse(sql)
    }

    /// Check if a SQL statement is vector-related
    pub fn is_vector_statement(&self, sql: &str) -> bool {
        // Quick check for vector-related keywords before full parsing
        let sql_upper = sql.to_uppercase();
        sql_upper.contains("VECTOR(") ||
        sql_upper.contains("HALFVEC(") || 
        sql_upper.contains("SPARSEVEC(") ||
        sql_upper.contains("<->") || 
        sql_upper.contains("<#>") || 
        sql_upper.contains("<=>") ||
        sql_upper.contains("VECTOR_DIMS") ||
        sql_upper.contains("VECTOR_NORM") ||
        (sql_upper.contains("CREATE INDEX") && 
         (sql_upper.contains("USING IVFFLAT") || sql_upper.contains("USING HNSW"))) ||
        sql_upper.contains("CREATE EXTENSION VECTOR")
    }
}

impl Default for SqlEngine {
    fn default() -> Self {
        Self::new()
    }
}