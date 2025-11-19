//! Query execution and optimization
//!
//! Provides vectorized execution with SIMD optimization for analytical queries.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, EngineResult};
use crate::metrics::QueryMetrics;
use crate::storage::{ColumnBatch, FilterPredicate, QueryResult};

// Module declarations
pub mod execution;
pub mod optimizer;
pub mod simd;

// Re-exports
pub use execution::{
    AggregateFunction, ComparisonOp, VectorizedExecutor, VectorizedExecutorConfig,
    VectorizedExecutorConfigBuilder,
};

/// Query executor trait
#[async_trait]
pub trait QueryExecutor: Send + Sync {
    /// Execute a query and return results
    async fn execute(&self, query: Query) -> EngineResult<QueryResult>;

    /// Explain query execution plan
    async fn explain(&self, query: Query) -> EngineResult<ExecutionPlan>;

    /// Get query metrics
    async fn metrics(&self) -> QueryMetrics;
}

/// Query representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// Table name
    pub table: String,
    /// Projection (columns to return)
    pub projection: Option<Vec<String>>,
    /// Filter predicate
    pub filter: Option<FilterPredicate>,
    /// Limit
    pub limit: Option<usize>,
    /// Offset
    pub offset: Option<usize>,
}

/// Query execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Plan nodes
    pub nodes: Vec<PlanNode>,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Whether SIMD is used
    pub uses_simd: bool,
}

/// Execution plan node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    /// Node type
    pub node_type: PlanNodeType,
    /// Estimated rows
    pub estimated_rows: usize,
    /// Children nodes
    pub children: Vec<PlanNode>,
}

/// Plan node types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanNodeType {
    /// Table scan
    TableScan,
    /// Index scan
    IndexScan,
    /// Filter operation
    Filter,
    /// Projection operation
    Projection,
    /// Aggregation
    Aggregation,
    /// Join operation
    Join,
    /// Sort operation
    Sort,
}
