//! Query execution engine for OrbitQL
//!
//! This module provides the query executor that runs parsed OrbitQL queries
//! against the distributed database system.

use crate::orbitql::ast::{Expression, GraphPattern, TimeRange, TimeSeriesAggregation};
use crate::orbitql::lexer::LexError;
use crate::orbitql::optimizer::OptimizationError;
use crate::orbitql::parser::ParseError;
use crate::orbitql::planner::{ExecutionPlan, PlanNode, PlanningError};
use crate::orbitql::{QueryContext, QueryParams, QueryStats};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

// Type alias for complex future type used in execute_node
type ExecutionFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError>,
            >
            + 'a
            + Send,
    >,
>;

/// Query execution errors
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionError {
    PlanningError(String),
    RuntimeError(String),
    AuthorizationError(String),
    ResourceExhausted(String),
    NetworkError(String),
    TimeoutError(String),
    Internal(String),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::PlanningError(msg) => write!(f, "Planning error: {msg}"),
            ExecutionError::RuntimeError(msg) => write!(f, "Runtime error: {msg}"),
            ExecutionError::AuthorizationError(msg) => write!(f, "Authorization error: {msg}"),
            ExecutionError::ResourceExhausted(msg) => write!(f, "Resource exhausted: {msg}"),
            ExecutionError::NetworkError(msg) => write!(f, "Network error: {msg}"),
            ExecutionError::TimeoutError(msg) => write!(f, "Timeout error: {msg}"),
            ExecutionError::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

// Implement From traits for error conversion
impl From<LexError> for ExecutionError {
    fn from(error: LexError) -> Self {
        ExecutionError::RuntimeError(error.to_string())
    }
}

impl From<ParseError> for ExecutionError {
    fn from(error: ParseError) -> Self {
        ExecutionError::RuntimeError(error.to_string())
    }
}

impl From<OptimizationError> for ExecutionError {
    fn from(error: OptimizationError) -> Self {
        ExecutionError::PlanningError(error.to_string())
    }
}

impl From<PlanningError> for ExecutionError {
    fn from(error: PlanningError) -> Self {
        ExecutionError::PlanningError(error.to_string())
    }
}

/// Query execution result
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryResult {
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub stats: QueryStats,
    pub warnings: Vec<String>,
    pub metadata: QueryMetadata,
}

/// Query execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetadata {
    pub query_id: Uuid,
    pub execution_id: Uuid,
    pub node_id: Option<String>,
    pub distributed: bool,
    pub cached: bool,
    pub indices_used: Vec<String>,
}

/// OrbitQL query executor
pub struct QueryExecutor {
    /// In-memory storage for demonstration (replace with real storage engines)
    document_store: HashMap<String, Vec<HashMap<String, serde_json::Value>>>,
    graph_store: HashMap<String, Vec<GraphEdge>>,
    timeseries_store: HashMap<String, Vec<TimeSeriesPoint>>,
}

/// Graph edge representation
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relationship: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Time series data point
#[derive(Debug, Clone)]
pub struct TimeSeriesPoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub value: f64,
    pub tags: HashMap<String, String>,
}

impl QueryExecutor {
    pub fn new() -> Self {
        let mut executor = Self {
            document_store: HashMap::new(),
            graph_store: HashMap::new(),
            timeseries_store: HashMap::new(),
        };

        // Initialize with sample data for testing
        executor.initialize_sample_data();
        executor
    }

    /// Initialize with sample data for demonstration
    fn initialize_sample_data(&mut self) {
        use chrono::Utc;
        use serde_json::json;

        // Sample user documents
        let users_data = vec![
            json!({
                "id": "user1",
                "name": "Alice Johnson",
                "email": "alice@example.com",
                "age": 29,
                "active": true,
                "profile": {
                    "location": "San Francisco",
                    "bio": "Software engineer"
                }
            })
            .as_object()
            .unwrap()
            .clone(),
            json!({
                "id": "user2",
                "name": "Bob Smith",
                "email": "bob@example.com",
                "age": 34,
                "active": true,
                "profile": {
                    "location": "New York",
                    "bio": "Data scientist"
                }
            })
            .as_object()
            .unwrap()
            .clone(),
            json!({
                "id": "user3",
                "name": "Charlie Brown",
                "email": "charlie@example.com",
                "age": 28,
                "active": false,
                "profile": {
                    "location": "Seattle",
                    "bio": "Product manager"
                }
            })
            .as_object()
            .unwrap()
            .clone(),
        ];

        self.document_store.insert(
            "users".to_string(),
            users_data
                .into_iter()
                .map(|map| map.into_iter().collect())
                .collect(),
        );

        // Sample graph relationships
        let follows_data = vec![
            GraphEdge {
                from: "user1".to_string(),
                to: "user2".to_string(),
                relationship: "follows".to_string(),
                properties: HashMap::from([("since".to_string(), json!("2024-01-01"))]),
            },
            GraphEdge {
                from: "user2".to_string(),
                to: "user3".to_string(),
                relationship: "follows".to_string(),
                properties: HashMap::from([("since".to_string(), json!("2024-02-15"))]),
            },
        ];

        self.graph_store.insert("follows".to_string(), follows_data);

        // Sample time series metrics
        let metrics_data = vec![
            TimeSeriesPoint {
                timestamp: Utc::now() - chrono::Duration::hours(2),
                value: 45.2,
                tags: HashMap::from([
                    ("device_id".to_string(), "device1".to_string()),
                    ("metric".to_string(), "cpu_usage".to_string()),
                ]),
            },
            TimeSeriesPoint {
                timestamp: Utc::now() - chrono::Duration::hours(1),
                value: 52.8,
                tags: HashMap::from([
                    ("device_id".to_string(), "device1".to_string()),
                    ("metric".to_string(), "cpu_usage".to_string()),
                ]),
            },
            TimeSeriesPoint {
                timestamp: Utc::now(),
                value: 38.1,
                tags: HashMap::from([
                    ("device_id".to_string(), "device1".to_string()),
                    ("metric".to_string(), "cpu_usage".to_string()),
                ]),
            },
        ];

        self.timeseries_store
            .insert("metrics".to_string(), metrics_data);
    }

    /// Execute a query execution plan
    pub async fn execute(
        &self,
        plan: ExecutionPlan,
        params: QueryParams,
        context: QueryContext,
    ) -> Result<QueryResult, ExecutionError> {
        let start_time = std::time::Instant::now();
        let execution_id = Uuid::new_v4();

        // Execute the root plan node
        let rows = self.execute_node(&plan.root, &params, &context).await?;

        let execution_time = start_time.elapsed();

        Ok(QueryResult {
            rows,
            stats: QueryStats {
                execution_time_ms: execution_time.as_millis() as u64,
                rows_examined: 0, // TODO: Track during execution
                rows_returned: 0, // TODO: Track during execution
                index_hits: 0,
                cache_hits: 0,
                network_calls: 0,
            },
            warnings: vec![],
            metadata: QueryMetadata {
                query_id: Uuid::new_v4(),
                execution_id,
                node_id: Some("node-1".to_string()),
                distributed: false,
                cached: false,
                indices_used: vec![],
            },
        })
    }

    /// Execute a plan node
    fn execute_node<'a>(
        &'a self,
        node: &'a PlanNode,
        _params: &'a QueryParams,
        _context: &'a QueryContext,
    ) -> ExecutionFuture<'a> {
        Box::pin(async move {
            match node {
                PlanNode::TableScan {
                    table,
                    columns,
                    filter: _,
                } => self.execute_table_scan(table, columns),
                PlanNode::Filter { input, condition } => {
                    let input_rows = self.execute_node(input, _params, _context).await?;
                    self.execute_filter(input_rows, condition)
                }
                PlanNode::Join {
                    left,
                    right,
                    join_type: _,
                    condition: _,
                } => {
                    let left_rows = self.execute_node(left, _params, _context).await?;
                    let right_rows = self.execute_node(right, _params, _context).await?;
                    self.execute_join(left_rows, right_rows)
                }
                PlanNode::Aggregation {
                    input,
                    group_by,
                    aggregates,
                } => {
                    let input_rows = self.execute_node(input, _params, _context).await?;
                    self.execute_aggregation(input_rows, group_by, aggregates)
                }
                PlanNode::Sort { input, expressions } => {
                    let mut input_rows = self.execute_node(input, _params, _context).await?;
                    self.execute_sort(&mut input_rows, expressions);
                    Ok(input_rows)
                }
                PlanNode::Limit {
                    input,
                    count,
                    offset,
                } => {
                    let input_rows = self.execute_node(input, _params, _context).await?;
                    self.execute_limit(input_rows, *count, offset.unwrap_or(0))
                }
                PlanNode::GraphTraversal {
                    pattern,
                    start_nodes,
                    max_depth,
                } => self.execute_graph_traversal(start_nodes, pattern, max_depth.unwrap_or(10)),
                PlanNode::TimeSeriesQuery {
                    metric,
                    range,
                    aggregation,
                    window: _,
                } => self.execute_timeseries_query(metric, range, aggregation),
            }
        })
    }

    /// Execute table scan
    fn execute_table_scan(
        &self,
        table: &str,
        columns: &[String],
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        if let Some(table_data) = self.document_store.get(table) {
            let mut result = table_data.clone();

            // Project only requested columns if specified
            if !columns.is_empty() && !columns.contains(&"*".to_string()) {
                for row in &mut result {
                    row.retain(|key, _| columns.contains(key));
                }
            }

            Ok(result)
        } else {
            Err(ExecutionError::RuntimeError(format!(
                "Table {table} not found"
            )))
        }
    }

    /// Execute filter operation
    fn execute_filter(
        &self,
        input_rows: Vec<HashMap<String, serde_json::Value>>,
        _predicate: &crate::orbitql::ast::Expression,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        // Simplified filtering - in real implementation would evaluate the predicate
        // For now, just return all rows
        Ok(input_rows)
    }

    /// Execute join operation  
    fn execute_join(
        &self,
        left_rows: Vec<HashMap<String, serde_json::Value>>,
        right_rows: Vec<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        let mut result = Vec::new();

        // Simple nested loop join (inefficient but functional)
        for left_row in &left_rows {
            for right_row in &right_rows {
                let mut joined_row = left_row.clone();
                for (key, value) in right_row {
                    joined_row.insert(format!("right_{key}"), value.clone());
                }
                result.push(joined_row);
            }
        }

        Ok(result)
    }

    /// Execute aggregation
    fn execute_aggregation(
        &self,
        input_rows: Vec<HashMap<String, serde_json::Value>>,
        _group_by: &[Expression],
        _aggregates: &[crate::orbitql::planner::AggregateExpression],
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        // Simplified aggregation - return count
        use serde_json::json;
        let count = input_rows.len();
        Ok(vec![HashMap::from([("count".to_string(), json!(count))])])
    }

    /// Execute sort operation
    fn execute_sort(
        &self,
        rows: &mut [HashMap<String, serde_json::Value>],
        _expressions: &[crate::orbitql::planner::SortExpression],
    ) {
        // Simplified sort by first key alphabetically
        let empty_key = String::new();
        rows.sort_by(|a, b| {
            let a_key = a.keys().next().unwrap_or(&empty_key);
            let b_key = b.keys().next().unwrap_or(&empty_key);
            a.get(a_key)
                .unwrap_or(&serde_json::Value::Null)
                .to_string()
                .cmp(&b.get(b_key).unwrap_or(&serde_json::Value::Null).to_string())
        });
    }

    /// Execute limit operation
    fn execute_limit(
        &self,
        input_rows: Vec<HashMap<String, serde_json::Value>>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        let start = offset as usize;
        let end = std::cmp::min(start + limit as usize, input_rows.len());

        if start < input_rows.len() {
            Ok(input_rows[start..end].to_vec())
        } else {
            Ok(vec![])
        }
    }

    /// Execute graph traversal
    fn execute_graph_traversal(
        &self,
        _start_nodes: &[Expression],
        _pattern: &GraphPattern,
        _max_depth: u32,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        use serde_json::json;

        // TODO: Extract relationship from pattern
        let relationship_name = "follows"; // Hardcoded for now
        if let Some(edges) = self.graph_store.get(relationship_name) {
            let result: Vec<HashMap<String, serde_json::Value>> = edges
                .iter()
                .map(|edge| {
                    HashMap::from([
                        ("from".to_string(), json!(edge.from)),
                        ("to".to_string(), json!(edge.to)),
                        ("relationship".to_string(), json!(edge.relationship)),
                    ])
                })
                .collect();
            Ok(result)
        } else {
            Ok(vec![])
        }
    }

    /// Execute time series query
    fn execute_timeseries_query(
        &self,
        metric: &str,
        _range: &TimeRange,
        _aggregation: &Option<TimeSeriesAggregation>,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        use serde_json::json;

        if let Some(points) = self.timeseries_store.get(metric) {
            let result: Vec<HashMap<String, serde_json::Value>> = points
                .iter()
                .map(|point| {
                    HashMap::from([
                        ("timestamp".to_string(), json!(point.timestamp.to_rfc3339())),
                        ("value".to_string(), json!(point.value)),
                        ("tags".to_string(), json!(point.tags)),
                    ])
                })
                .collect();
            Ok(result)
        } else {
            Ok(vec![])
        }
    }

    /// Validate execution permissions
    #[allow(dead_code)]
    fn validate_permissions(
        &self,
        _context: &QueryContext,
        _plan: &ExecutionPlan,
    ) -> Result<(), ExecutionError> {
        // TODO: Implement permission checking
        Ok(())
    }

    /// Check resource limits
    #[allow(dead_code)]
    fn check_resource_limits(
        &self,
        _context: &QueryContext,
        _plan: &ExecutionPlan,
    ) -> Result<(), ExecutionError> {
        // TODO: Implement resource limit checking
        Ok(())
    }
}

impl Default for QueryExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for QueryMetadata {
    fn default() -> Self {
        Self {
            query_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            node_id: Some("default".to_string()),
            distributed: false,
            cached: false,
            indices_used: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbitql::planner::ExecutionPlan;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = QueryExecutor::new();

        // Test with users table that exists in sample data
        let plan = ExecutionPlan {
            root: PlanNode::TableScan {
                table: "users".to_string(),
                columns: vec![],
                filter: None,
            },
            estimated_cost: 0.0,
            estimated_rows: 0,
        };

        let result = executor
            .execute(plan, QueryParams::new(), QueryContext::default())
            .await;
        assert!(result.is_ok());
        let result = result.unwrap();
        // Should have 3 users from sample data
        assert_eq!(result.rows.len(), 3);
    }
}
