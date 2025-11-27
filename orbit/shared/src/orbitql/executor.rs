//! Query execution engine for OrbitQL
//!
//! This module provides the query executor that runs parsed OrbitQL queries
//! against the distributed database system.

use crate::orbitql::ast::{Expression, GraphPattern, TimeRange, TimeSeriesAggregation};
use crate::orbitql::lexer::LexError;
use crate::orbitql::optimizer::OptimizationError;
use crate::orbitql::parser::ParseError;
use crate::orbitql::planner::{DataModel, ExecutionPlan, PlanNode, PlanningError};
use crate::orbitql::QueryValue;
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
    /// Create a new empty QueryExecutor
    pub fn new() -> Self {
        Self {
            document_store: HashMap::new(),
            graph_store: HashMap::new(),
            timeseries_store: HashMap::new(),
        }
    }

    /// Create a QueryExecutor with sample data for testing
    #[cfg(test)]
    pub fn with_sample_data() -> Self {
        let mut executor = Self::new();
        executor.initialize_sample_data();
        executor
    }

    /// Initialize with sample data (for testing only)
    #[cfg(test)]
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

        // Sample metrics table (document-based representation of time series)
        let metrics_docs = vec![
            json!({
                "timestamp": Utc::now() - chrono::Duration::hours(2),
                "value": 45.2,
                "tags": {"device_id": "device1", "metric": "cpu_usage"}
            })
            .as_object()
            .unwrap()
            .clone(),
            json!({
                "timestamp": Utc::now() - chrono::Duration::hours(1),
                "value": 52.8,
                "tags": {"device_id": "device1", "metric": "cpu_usage"}
            })
            .as_object()
            .unwrap()
            .clone(),
            json!({
                "timestamp": Utc::now(),
                "value": 38.1,
                "tags": {"device_id": "device1", "metric": "cpu_usage"}
            })
            .as_object()
            .unwrap()
            .clone(),
        ];

        self.document_store.insert(
            "metrics".to_string(),
            metrics_docs
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

        self.graph_store
            .insert("follows".to_string(), follows_data.clone());

        // Also add follows as a document table for JOIN queries
        let follows_docs: Vec<HashMap<String, serde_json::Value>> = follows_data
            .iter()
            .map(|edge| {
                let mut doc = HashMap::new();
                doc.insert("from".to_string(), json!(edge.from.clone()));
                doc.insert("to".to_string(), json!(edge.to.clone()));
                doc.insert("relationship".to_string(), json!(edge.relationship.clone()));
                for (k, v) in &edge.properties {
                    doc.insert(k.clone(), v.clone());
                }
                doc
            })
            .collect();
        self.document_store
            .insert("follows".to_string(), follows_docs);

        // Add _relate table for RELATE statement execution
        self.document_store
            .insert("_relate".to_string(), Vec::new());

        // Add _transaction table for transaction statements
        self.document_store
            .insert("_transaction".to_string(), Vec::new());

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
                PlanNode::CrossModelJoin {
                    left,
                    right,
                    left_model,
                    right_model,
                    left_key,
                    right_key,
                    join_type,
                } => {
                    let left_rows = self.execute_node(left, _params, _context).await?;
                    let right_rows = self.execute_node(right, _params, _context).await?;
                    self.execute_cross_model_join(
                        left_rows,
                        right_rows,
                        left_model,
                        right_model,
                        left_key,
                        right_key,
                        join_type,
                    )
                }
            }
        })
    }

    /// Execute table scan
    fn execute_table_scan(
        &self,
        table: &str,
        columns: &[String],
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        // Handle virtual _dual table for queries without FROM clause
        if table == "_dual" {
            // Return a single empty row for _dual table
            return Ok(vec![HashMap::new()]);
        }

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

    /// Execute aggregation with full support for COUNT, SUM, AVG, MIN, MAX
    fn execute_aggregation(
        &self,
        input_rows: Vec<HashMap<String, serde_json::Value>>,
        group_by: &[Expression],
        aggregates: &[crate::orbitql::planner::AggregateExpression],
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        use serde_json::json;

        // If no GROUP BY, treat all rows as a single group
        if group_by.is_empty() {
            let mut result_row = HashMap::new();

            for agg in aggregates {
                let alias = agg
                    .alias
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", agg.function).to_lowercase());

                let value = self.compute_aggregate(&input_rows, agg)?;
                result_row.insert(alias, value);
            }

            // If no aggregates specified, return COUNT(*) as default
            if aggregates.is_empty() {
                result_row.insert("count".to_string(), json!(input_rows.len()));
            }

            return Ok(vec![result_row]);
        }

        // GROUP BY: partition rows by group key
        let mut groups: HashMap<String, Vec<&HashMap<String, serde_json::Value>>> = HashMap::new();

        for row in &input_rows {
            let group_key = self.compute_group_key(row, group_by);
            groups.entry(group_key).or_default().push(row);
        }

        // Compute aggregates for each group
        let mut results = Vec::new();
        for (_group_key, group_rows) in groups {
            let mut result_row = HashMap::new();

            // Add group by column values
            if let Some(first_row) = group_rows.first() {
                for expr in group_by {
                    if let Expression::Identifier(name) = expr {
                        if let Some(value) = first_row.get(name) {
                            result_row.insert(name.clone(), value.clone());
                        }
                    }
                }
            }

            // Compute each aggregate
            let owned_rows: Vec<HashMap<String, serde_json::Value>> =
                group_rows.into_iter().cloned().collect();

            for agg in aggregates {
                let alias = agg
                    .alias
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", agg.function).to_lowercase());

                let value = self.compute_aggregate(&owned_rows, agg)?;
                result_row.insert(alias, value);
            }

            results.push(result_row);
        }

        Ok(results)
    }

    /// Compute group key from row based on GROUP BY expressions
    fn compute_group_key(
        &self,
        row: &HashMap<String, serde_json::Value>,
        group_by: &[Expression],
    ) -> String {
        let mut key_parts = Vec::new();
        for expr in group_by {
            if let Expression::Identifier(name) = expr {
                let value = row.get(name).map(|v| v.to_string()).unwrap_or_default();
                key_parts.push(value);
            }
        }
        key_parts.join("|")
    }

    /// Compute a single aggregate over rows
    fn compute_aggregate(
        &self,
        rows: &[HashMap<String, serde_json::Value>],
        agg: &crate::orbitql::planner::AggregateExpression,
    ) -> Result<serde_json::Value, ExecutionError> {
        use crate::orbitql::ast::AggregateFunction;
        use serde_json::json;

        // Extract values for the aggregate expression
        let values: Vec<Option<serde_json::Value>> = if let Some(expr) = &agg.expression {
            rows.iter()
                .map(|row| self.extract_value_for_expression(row, expr))
                .collect()
        } else {
            // COUNT(*) - all rows count
            rows.iter().map(|_| Some(json!(1))).collect()
        };

        // Handle DISTINCT
        let values = if agg.distinct {
            let mut seen = std::collections::HashSet::new();
            values
                .into_iter()
                .filter(|v| {
                    let key = v.as_ref().map(|x| x.to_string()).unwrap_or_default();
                    seen.insert(key)
                })
                .collect()
        } else {
            values
        };

        match agg.function {
            AggregateFunction::Count => {
                // COUNT counts non-null values (or all rows for COUNT(*))
                let count = if agg.expression.is_none() {
                    rows.len() // COUNT(*)
                } else {
                    values.iter().filter(|v| v.is_some()).count()
                };
                Ok(json!(count))
            }
            AggregateFunction::Sum => {
                let sum: f64 = values
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .filter_map(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .sum();
                Ok(json!(sum))
            }
            AggregateFunction::Avg => {
                let nums: Vec<f64> = values
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .filter_map(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .collect();
                if nums.is_empty() {
                    Ok(json!(null))
                } else {
                    let avg = nums.iter().sum::<f64>() / nums.len() as f64;
                    Ok(json!(avg))
                }
            }
            AggregateFunction::Min => {
                let min = values
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .filter_map(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .fold(f64::INFINITY, f64::min);
                if min == f64::INFINITY {
                    Ok(json!(null))
                } else {
                    Ok(json!(min))
                }
            }
            AggregateFunction::Max => {
                let max = values
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .filter_map(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .fold(f64::NEG_INFINITY, f64::max);
                if max == f64::NEG_INFINITY {
                    Ok(json!(null))
                } else {
                    Ok(json!(max))
                }
            }
            AggregateFunction::First => {
                Ok(values.into_iter().find_map(|v| v).unwrap_or(json!(null)))
            }
            AggregateFunction::Last => Ok(values
                .into_iter()
                .rev()
                .find_map(|v| v)
                .unwrap_or(json!(null))),
            AggregateFunction::StdDev => {
                let nums: Vec<f64> = values
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .filter_map(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .collect();
                if nums.len() < 2 {
                    Ok(json!(null))
                } else {
                    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
                    let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                        / (nums.len() - 1) as f64;
                    Ok(json!(variance.sqrt()))
                }
            }
            AggregateFunction::Variance => {
                let nums: Vec<f64> = values
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .filter_map(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .collect();
                if nums.len() < 2 {
                    Ok(json!(null))
                } else {
                    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
                    let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                        / (nums.len() - 1) as f64;
                    Ok(json!(variance))
                }
            }
        }
    }

    /// Extract a value from a row based on an expression
    fn extract_value_for_expression(
        &self,
        row: &HashMap<String, serde_json::Value>,
        expr: &Expression,
    ) -> Option<serde_json::Value> {
        match expr {
            Expression::Identifier(name) => row.get(name).cloned(),
            Expression::Literal(lit) => Some(self.query_value_to_json(lit)),
            _ => None, // Complex expressions not yet supported
        }
    }

    /// Convert a QueryValue to JSON value
    fn query_value_to_json(&self, val: &QueryValue) -> serde_json::Value {
        use serde_json::json;
        match val {
            QueryValue::Integer(i) => json!(i),
            QueryValue::Float(f) => json!(f),
            QueryValue::String(s) => json!(s),
            QueryValue::Boolean(b) => json!(b),
            QueryValue::Null => json!(null),
            QueryValue::Array(arr) => {
                let values: Vec<serde_json::Value> =
                    arr.iter().map(|v| self.query_value_to_json(v)).collect();
                json!(values)
            }
            QueryValue::Object(map) => {
                let obj: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), self.query_value_to_json(v)))
                    .collect();
                serde_json::Value::Object(obj)
            }
            QueryValue::DateTime(dt) => json!(dt.to_rfc3339()),
            QueryValue::Duration(d) => json!(d.as_secs_f64()),
            QueryValue::Uuid(u) => json!(u.to_string()),
        }
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

    /// Execute graph traversal with BFS algorithm
    fn execute_graph_traversal(
        &self,
        start_nodes: &[Expression],
        pattern: &GraphPattern,
        max_depth: u32,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        use crate::orbitql::ast::EdgeDirection;
        use serde_json::json;
        use std::collections::{HashSet, VecDeque};

        // Extract edge label and direction from pattern
        let (edge_label, direction) = self.extract_edge_info_from_pattern(pattern);

        // Extract start node IDs from expressions
        let start_node_ids: Vec<String> = start_nodes
            .iter()
            .filter_map(|expr| self.extract_node_id(expr))
            .collect();

        // If no explicit start nodes, use all nodes as starting points
        let start_ids: Vec<String> = if start_node_ids.is_empty() {
            // Collect all unique node IDs from graph store
            let mut all_nodes = HashSet::new();
            for edges in self.graph_store.values() {
                for edge in edges {
                    all_nodes.insert(edge.from.clone());
                    all_nodes.insert(edge.to.clone());
                }
            }
            all_nodes.into_iter().collect()
        } else {
            start_node_ids
        };

        let mut results = Vec::new();
        let mut visited = HashSet::new();

        // BFS traversal from each start node
        for start_id in &start_ids {
            let mut queue: VecDeque<(String, u32, Vec<String>)> = VecDeque::new();
            queue.push_back((start_id.clone(), 0, vec![start_id.clone()]));

            while let Some((current_node, depth, path)) = queue.pop_front() {
                if depth >= max_depth {
                    continue;
                }

                // Find matching edges based on direction
                for (rel_name, edges) in &self.graph_store {
                    // Filter by edge label if specified
                    if let Some(ref label) = edge_label {
                        if rel_name != label {
                            continue;
                        }
                    }

                    for edge in edges {
                        let (neighbor, is_match) = match direction {
                            EdgeDirection::Outgoing => {
                                if edge.from == current_node {
                                    (edge.to.clone(), true)
                                } else {
                                    (String::new(), false)
                                }
                            }
                            EdgeDirection::Incoming => {
                                if edge.to == current_node {
                                    (edge.from.clone(), true)
                                } else {
                                    (String::new(), false)
                                }
                            }
                            EdgeDirection::Both => {
                                if edge.from == current_node {
                                    (edge.to.clone(), true)
                                } else if edge.to == current_node {
                                    (edge.from.clone(), true)
                                } else {
                                    (String::new(), false)
                                }
                            }
                        };

                        if is_match && !visited.contains(&(current_node.clone(), neighbor.clone()))
                        {
                            visited.insert((current_node.clone(), neighbor.clone()));

                            let mut new_path = path.clone();
                            new_path.push(neighbor.clone());

                            // Add result for this edge
                            let mut row = HashMap::new();
                            row.insert("from".to_string(), json!(edge.from));
                            row.insert("to".to_string(), json!(edge.to));
                            row.insert("relationship".to_string(), json!(edge.relationship));
                            row.insert("depth".to_string(), json!(depth + 1));
                            row.insert("path".to_string(), json!(new_path.clone()));

                            // Add edge properties
                            for (key, value) in &edge.properties {
                                row.insert(key.clone(), value.clone());
                            }

                            results.push(row);

                            // Continue traversal
                            if depth + 1 < max_depth {
                                queue.push_back((neighbor, depth + 1, new_path));
                            }
                        }
                    }
                }
            }
        }

        // Apply pattern's where clause filter if present
        if let Some(ref where_clause) = pattern.where_clause {
            results = self.apply_expression_filter(results, where_clause)?;
        }

        Ok(results)
    }

    /// Extract edge label and direction from graph pattern
    fn extract_edge_info_from_pattern(
        &self,
        pattern: &GraphPattern,
    ) -> (Option<String>, crate::orbitql::ast::EdgeDirection) {
        use crate::orbitql::ast::{EdgeDirection, GraphStep};

        let mut edge_label = None;
        let mut direction = EdgeDirection::Outgoing; // Default direction

        for step in &pattern.path.steps {
            if let GraphStep::Edge {
                direction: dir,
                label,
                ..
            } = step
            {
                direction = dir.clone();
                edge_label = label.clone();
                break;
            }
        }

        (edge_label, direction)
    }

    /// Extract node ID from expression
    fn extract_node_id(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Literal(crate::orbitql::QueryValue::String(s)) => Some(s.clone()),
            Expression::Identifier(id) => Some(id.clone()),
            _ => None,
        }
    }

    /// Apply expression filter to rows
    fn apply_expression_filter(
        &self,
        rows: Vec<HashMap<String, serde_json::Value>>,
        _condition: &Expression,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        // TODO: Implement full expression evaluation
        // For now, return all rows (filter evaluation would be implemented here)
        Ok(rows)
    }

    /// Execute cross-model join between different data models
    #[allow(clippy::too_many_arguments)]
    fn execute_cross_model_join(
        &self,
        left_rows: Vec<HashMap<String, serde_json::Value>>,
        right_rows: Vec<HashMap<String, serde_json::Value>>,
        left_model: &DataModel,
        right_model: &DataModel,
        left_key: &Expression,
        right_key: &Expression,
        join_type: &crate::orbitql::ast::JoinType,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        use crate::orbitql::ast::JoinType;

        // Extract key field names from expressions
        let left_key_field = self.extract_key_field(left_key);
        let right_key_field = self.extract_key_field(right_key);

        // Build a hash index on the right side for efficient lookups
        let mut right_index: HashMap<String, Vec<&HashMap<String, serde_json::Value>>> =
            HashMap::new();
        for row in &right_rows {
            if let Some(key_value) = row.get(&right_key_field) {
                let key_str = self.value_to_join_key(key_value);
                right_index.entry(key_str).or_default().push(row);
            }
        }

        let mut results = Vec::new();
        let mut left_matched = vec![false; left_rows.len()];

        // Perform the join
        for (left_idx, left_row) in left_rows.iter().enumerate() {
            if let Some(left_key_value) = left_row.get(&left_key_field) {
                let left_key_str = self.value_to_join_key(left_key_value);

                if let Some(matching_right_rows) = right_index.get(&left_key_str) {
                    left_matched[left_idx] = true;

                    for right_row in matching_right_rows {
                        let merged = self.merge_cross_model_rows(
                            left_row,
                            right_row,
                            left_model,
                            right_model,
                        );
                        results.push(merged);
                    }
                } else if matches!(join_type, JoinType::Left | JoinType::Full) {
                    // Left outer join: include left row with nulls for right side
                    let merged = self.merge_cross_model_rows_with_nulls(
                        left_row,
                        right_model,
                        true, // left has data
                    );
                    results.push(merged);
                }
            }
        }

        // For right/full outer joins, include unmatched right rows
        if matches!(join_type, JoinType::Right | JoinType::Full) {
            let matched_right_keys: std::collections::HashSet<String> = left_rows
                .iter()
                .filter_map(|row| row.get(&left_key_field))
                .map(|v| self.value_to_join_key(v))
                .collect();

            for right_row in &right_rows {
                if let Some(right_key_value) = right_row.get(&right_key_field) {
                    let right_key_str = self.value_to_join_key(right_key_value);
                    if !matched_right_keys.contains(&right_key_str) {
                        let merged = self.merge_cross_model_rows_with_nulls(
                            right_row, left_model, false, // right has data, left is null
                        );
                        results.push(merged);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Extract key field name from expression
    fn extract_key_field(&self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(name) => name.clone(),
            Expression::FieldAccess { field, .. } => field.clone(),
            _ => "id".to_string(), // Default key field
        }
    }

    /// Convert a JSON value to a string for join key comparison
    fn value_to_join_key(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => "null".to_string(),
            _ => value.to_string(),
        }
    }

    /// Merge rows from different data models with schema unification
    fn merge_cross_model_rows(
        &self,
        left_row: &HashMap<String, serde_json::Value>,
        right_row: &HashMap<String, serde_json::Value>,
        left_model: &DataModel,
        right_model: &DataModel,
    ) -> HashMap<String, serde_json::Value> {
        use serde_json::json;

        let mut merged = HashMap::new();

        // Add left row fields with model prefix for disambiguation
        let left_prefix = self.get_model_prefix(left_model);
        for (key, value) in left_row {
            merged.insert(format!("{}_{}", left_prefix, key), value.clone());
            // Also add without prefix for convenience
            if !merged.contains_key(key) {
                merged.insert(key.clone(), value.clone());
            }
        }

        // Add right row fields with model prefix
        let right_prefix = self.get_model_prefix(right_model);
        for (key, value) in right_row {
            merged.insert(format!("{}_{}", right_prefix, key), value.clone());
            // Add without prefix if not already present
            let prefixed_key = format!("right_{}", key);
            merged.insert(prefixed_key, value.clone());
        }

        // Add metadata about the join
        merged.insert(
            "_left_model".to_string(),
            json!(format!("{:?}", left_model)),
        );
        merged.insert(
            "_right_model".to_string(),
            json!(format!("{:?}", right_model)),
        );

        merged
    }

    /// Merge row with nulls for outer joins
    fn merge_cross_model_rows_with_nulls(
        &self,
        row: &HashMap<String, serde_json::Value>,
        null_model: &DataModel,
        row_is_left: bool,
    ) -> HashMap<String, serde_json::Value> {
        use serde_json::json;

        let mut merged = HashMap::new();
        let null_prefix = self.get_model_prefix(null_model);

        if row_is_left {
            // Row is from left side
            for (key, value) in row {
                merged.insert(key.clone(), value.clone());
            }
            // Add null placeholders for right side
            merged.insert(format!("{}_id", null_prefix), json!(null));
        } else {
            // Row is from right side
            for (key, value) in row {
                merged.insert(format!("right_{}", key), value.clone());
            }
        }

        merged
    }

    /// Get a prefix string for a data model
    fn get_model_prefix(&self, model: &DataModel) -> &'static str {
        match model {
            DataModel::Document => "doc",
            DataModel::Graph => "graph",
            DataModel::TimeSeries => "ts",
            DataModel::Vector => "vec",
        }
    }

    /// Execute time series query with range filtering and aggregation
    fn execute_timeseries_query(
        &self,
        metric: &str,
        range: &TimeRange,
        aggregation: &Option<TimeSeriesAggregation>,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, ExecutionError> {
        use serde_json::json;

        let Some(points) = self.timeseries_store.get(metric) else {
            return Ok(vec![]);
        };

        // Resolve time range to actual timestamps
        let (start_time, end_time) = self.resolve_time_range(range)?;

        // Filter points by time range
        let filtered_points: Vec<&TimeSeriesPoint> = points
            .iter()
            .filter(|point| point.timestamp >= start_time && point.timestamp <= end_time)
            .collect();

        // Apply aggregation if specified
        if let Some(agg) = aggregation {
            let aggregated_value = self.compute_timeseries_aggregation(&filtered_points, agg)?;
            let mut result_row = HashMap::new();
            result_row.insert("metric".to_string(), json!(metric));
            result_row.insert("start_time".to_string(), json!(start_time.to_rfc3339()));
            result_row.insert("end_time".to_string(), json!(end_time.to_rfc3339()));
            result_row.insert("aggregation".to_string(), json!(format!("{:?}", agg)));
            result_row.insert("value".to_string(), aggregated_value);
            result_row.insert("point_count".to_string(), json!(filtered_points.len()));
            return Ok(vec![result_row]);
        }

        // Return raw filtered points
        let result: Vec<HashMap<String, serde_json::Value>> = filtered_points
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
    }

    /// Resolve time range to actual DateTime values
    fn resolve_time_range(
        &self,
        range: &TimeRange,
    ) -> Result<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>), ExecutionError>
    {
        use crate::orbitql::ast::{TimeDirection, TimeExpression, TimeUnit};
        use chrono::{Duration, Utc};

        fn resolve_time_expr(expr: &TimeExpression) -> chrono::DateTime<chrono::Utc> {
            match expr {
                TimeExpression::Now => Utc::now(),
                TimeExpression::Literal(dt) => *dt,
                TimeExpression::Relative {
                    base,
                    offset,
                    direction,
                } => {
                    let base_time = resolve_time_expr(base);
                    let duration = match offset.unit {
                        TimeUnit::Millisecond => Duration::milliseconds(offset.value as i64),
                        TimeUnit::Second => Duration::seconds(offset.value as i64),
                        TimeUnit::Minute => Duration::minutes(offset.value as i64),
                        TimeUnit::Hour => Duration::hours(offset.value as i64),
                        TimeUnit::Day => Duration::days(offset.value as i64),
                        TimeUnit::Week => Duration::weeks(offset.value as i64),
                        TimeUnit::Month => Duration::days(offset.value as i64 * 30), // Approximation
                        TimeUnit::Year => Duration::days(offset.value as i64 * 365), // Approximation
                    };
                    match direction {
                        TimeDirection::Plus => base_time + duration,
                        TimeDirection::Minus => base_time - duration,
                    }
                }
            }
        }

        let start = resolve_time_expr(&range.start);
        let end = resolve_time_expr(&range.end);
        Ok((start, end))
    }

    /// Compute time-series aggregation over filtered points
    fn compute_timeseries_aggregation(
        &self,
        points: &[&TimeSeriesPoint],
        aggregation: &TimeSeriesAggregation,
    ) -> Result<serde_json::Value, ExecutionError> {
        use serde_json::json;

        if points.is_empty() {
            return Ok(json!(null));
        }

        let values: Vec<f64> = points.iter().map(|p| p.value).collect();

        match aggregation {
            TimeSeriesAggregation::Count => Ok(json!(values.len())),
            TimeSeriesAggregation::Sum => Ok(json!(values.iter().sum::<f64>())),
            TimeSeriesAggregation::Avg => {
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                Ok(json!(avg))
            }
            TimeSeriesAggregation::Min => {
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                Ok(json!(min))
            }
            TimeSeriesAggregation::Max => {
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                Ok(json!(max))
            }
            TimeSeriesAggregation::First => Ok(json!(values.first().unwrap_or(&0.0))),
            TimeSeriesAggregation::Last => Ok(json!(values.last().unwrap_or(&0.0))),
            TimeSeriesAggregation::StdDev => {
                if values.len() < 2 {
                    return Ok(json!(0.0));
                }
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                    / (values.len() - 1) as f64;
                Ok(json!(variance.sqrt()))
            }
            TimeSeriesAggregation::Percentile(p) => {
                let mut sorted = values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let index = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
                let index = index.min(sorted.len() - 1);
                Ok(json!(sorted[index]))
            }
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
        let executor = QueryExecutor::with_sample_data();

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
