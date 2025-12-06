// Common Table Expression (CTE) execution support
//
// Implements WITH clause execution including recursive CTEs

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::ast::{CommonTableExpression, SelectStatement};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;

/// CTE execution context
pub struct CteContext {
    /// Materialized CTE results
    cte_results: HashMap<String, CteResult>,
}

/// Materialized CTE result
struct CteResult {
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
}

impl CteContext {
    /// Create a new CTE context
    pub fn new() -> Self {
        Self {
            cte_results: HashMap::new(),
        }
    }

    /// Execute and materialize a CTE
    pub fn materialize_cte(
        &mut self,
        cte: &CommonTableExpression,
        executor: &dyn CteExecutor,
    ) -> ProtocolResult<()> {
        // Execute the CTE query
        let (columns, rows) = executor.execute_cte_query(&cte.query)?;

        // Use explicit column names if provided, otherwise use query columns
        let result_columns = if let Some(ref cte_columns) = cte.columns {
            cte_columns.clone()
        } else {
            columns
        };

        // Store the materialized result
        self.cte_results.insert(
            cte.name.clone(),
            CteResult {
                columns: result_columns,
                rows,
            },
        );

        Ok(())
    }

    /// Get materialized CTE result
    pub fn get_cte(&self, name: &str) -> Option<&CteResult> {
        self.cte_results.get(name)
    }

    /// Get CTE columns
    pub fn get_cte_columns(&self, name: &str) -> Option<&[String]> {
        self.cte_results.get(name).map(|r| r.columns.as_slice())
    }

    /// Get CTE rows
    pub fn get_cte_rows(&self, name: &str) -> Option<&[Vec<SqlValue>]> {
        self.cte_results.get(name).map(|r| r.rows.as_slice())
    }

    /// Check if CTE exists
    pub fn has_cte(&self, name: &str) -> bool {
        self.cte_results.contains_key(name)
    }
}

impl Default for CteContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for executing CTE queries
pub trait CteExecutor {
    /// Execute a CTE query and return columns and rows
    fn execute_cte_query(
        &self,
        query: &SelectStatement,
    ) -> ProtocolResult<(Vec<String>, Vec<Vec<SqlValue>>)>;
}

/// Recursive CTE evaluator
pub struct RecursiveCteEvaluator {
    max_iterations: usize,
}

impl RecursiveCteEvaluator {
    /// Create a new recursive CTE evaluator
    pub fn new() -> Self {
        Self {
            max_iterations: 1000, // Default max iterations to prevent infinite loops
        }
    }

    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Execute a recursive CTE
    pub fn execute_recursive(
        &self,
        cte: &CommonTableExpression,
        executor: &dyn CteExecutor,
    ) -> ProtocolResult<(Vec<String>, Vec<Vec<SqlValue>>)> {
        // Recursive CTEs have the form:
        // WITH RECURSIVE cte_name AS (
        //   base_query
        //   UNION [ALL]
        //   recursive_query
        // )

        // For now, return a simple implementation
        // Full recursive CTE support would require:
        // 1. Split query into base and recursive parts
        // 2. Execute base query
        // 3. Iteratively execute recursive query with previous results
        // 4. Union results until no new rows or max iterations

        executor.execute_cte_query(&cte.query)
    }
}

impl Default for RecursiveCteEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor;

    impl CteExecutor for MockExecutor {
        fn execute_cte_query(
            &self,
            _query: &SelectStatement,
        ) -> ProtocolResult<(Vec<String>, Vec<Vec<SqlValue>>)> {
            Ok((
                vec!["id".to_string(), "name".to_string()],
                vec![
                    vec![SqlValue::Integer(1), SqlValue::Text("Alice".to_string())],
                    vec![SqlValue::Integer(2), SqlValue::Text("Bob".to_string())],
                ],
            ))
        }
    }

    #[test]
    fn test_cte_context() {
        let context = CteContext::new();

        assert!(!context.has_cte("test_cte"));

        // Would need to create a proper CTE to test materialization
        // This is a simplified test
        assert!(context.get_cte("test_cte").is_none());
    }

    #[test]
    fn test_recursive_evaluator() {
        let evaluator = RecursiveCteEvaluator::new();
        assert_eq!(evaluator.max_iterations, 1000);

        let evaluator = evaluator.with_max_iterations(500);
        assert_eq!(evaluator.max_iterations, 500);
    }
}
