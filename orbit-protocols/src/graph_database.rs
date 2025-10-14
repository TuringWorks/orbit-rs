//! Graph database implementation for RedisGraph compatibility
//!
//! This module provides a distributed graph database compatible with RedisGraph commands,
//! built on top of Orbit's actor system and leveraging the existing Cypher implementation.

use crate::cypher::graph_engine::QueryResult;
use crate::cypher::{CypherParser, GraphEngine};
use crate::error::{ProtocolError, ProtocolResult};
use orbit_shared::graph::InMemoryGraphStorage;
use orbit_shared::Addressable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Graph database actor that manages a single named graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphActor {
    /// Name of the graph
    pub graph_name: String,
    /// Graph configuration
    pub config: GraphConfig,
    /// Query execution statistics
    pub stats: GraphStats,
    /// Slow query log
    pub slow_queries: Vec<SlowQuery>,
    /// Graph creation timestamp
    pub created_at: i64,
    /// Last update timestamp
    pub updated_at: i64,
}

/// Configuration for a graph database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Maximum number of nodes (0 for unlimited)
    pub max_nodes: u64,
    /// Maximum number of relationships (0 for unlimited)
    pub max_relationships: u64,
    /// Query timeout in milliseconds
    pub query_timeout_ms: u64,
    /// Maximum query result size
    pub max_result_size: usize,
    /// Enable query profiling
    pub profiling_enabled: bool,
    /// Slow query threshold in milliseconds
    pub slow_query_threshold_ms: u64,
    /// Maximum slow queries to keep
    pub max_slow_queries: usize,
    /// Memory limit for graph operations in bytes
    pub memory_limit_bytes: u64,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            max_nodes: 1_000_000,
            max_relationships: 10_000_000,
            query_timeout_ms: 30_000,
            max_result_size: 10_000,
            profiling_enabled: false,
            slow_query_threshold_ms: 100,
            max_slow_queries: 10,
            memory_limit_bytes: 512 * 1024 * 1024, // 512MB
        }
    }
}

/// Graph execution statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    /// Total queries executed
    pub queries_executed: u64,
    /// Read-only queries executed
    pub read_queries_executed: u64,
    /// Write queries executed
    pub write_queries_executed: u64,
    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,
    /// Average query execution time
    pub avg_execution_time_ms: f64,
    /// Number of nodes in the graph
    pub node_count: u64,
    /// Number of relationships in the graph
    pub relationship_count: u64,
    /// Number of labels used
    pub label_count: u64,
    /// Number of relationship types used
    pub relationship_type_count: u64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Last statistics update timestamp
    pub last_updated: i64,
}

/// Slow query record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQuery {
    /// Query string that was slow
    pub query: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Timestamp when query was executed
    pub timestamp: i64,
    /// Query parameters (if any)
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

/// Query execution plan step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Plan steps in execution order
    pub steps: Vec<PlanStep>,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Query string
    pub query: String,
}

/// Individual execution plan step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step operation name
    pub operation: String,
    /// Step description
    pub description: String,
    /// Estimated rows
    pub estimated_rows: u64,
    /// Estimated cost for this step
    pub estimated_cost: f64,
    /// Child steps (if any)
    pub children: Vec<PlanStep>,
}

/// Query execution profile with performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryProfile {
    /// Execution plan
    pub plan: ExecutionPlan,
    /// Actual execution metrics
    pub metrics: ProfileMetrics,
    /// Total execution time
    pub total_time_ms: u64,
}

/// Performance metrics for query profiling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetrics {
    /// Actual rows processed per step
    pub rows_processed: Vec<u64>,
    /// Time spent per step in milliseconds
    pub step_times_ms: Vec<u64>,
    /// Memory used per step in bytes
    pub memory_used: Vec<u64>,
    /// Cache hits/misses
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl GraphActor {
    /// Create a new graph actor with the given name
    pub fn new(graph_name: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            graph_name,
            config: GraphConfig::default(),
            stats: GraphStats::default(),
            slow_queries: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new graph actor with custom configuration
    pub fn with_config(graph_name: String, config: GraphConfig) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            graph_name,
            config,
            stats: GraphStats::default(),
            slow_queries: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Execute a Cypher query with write capabilities
    pub async fn execute_query(&mut self, query: &str) -> ProtocolResult<QueryResult> {
        self.execute_query_internal(query, false).await
    }

    /// Execute a read-only Cypher query
    pub async fn execute_read_only_query(&mut self, query: &str) -> ProtocolResult<QueryResult> {
        self.execute_query_internal(query, true).await
    }

    /// Internal query execution with read-only flag
    async fn execute_query_internal(
        &mut self,
        query: &str,
        read_only: bool,
    ) -> ProtocolResult<QueryResult> {
        let start_time = Instant::now();

        debug!(
            graph = %self.graph_name,
            query = query,
            read_only = read_only,
            "Executing Cypher query"
        );

        // Create storage and engine (in production this would be persistent)
        let storage = Arc::new(InMemoryGraphStorage::new());
        let engine = GraphEngine::new(storage);

        // Validate read-only constraint
        if read_only && self.is_write_query(query) {
            return Err(ProtocolError::CypherError(
                "Write operations not allowed in read-only query".to_string(),
            ));
        }

        // Execute the query
        let result = engine.execute_query(query).await;
        let execution_time = start_time.elapsed();

        // Update statistics
        self.update_stats(execution_time, read_only, result.is_ok());

        // Record slow query if needed
        if execution_time.as_millis() as u64 >= self.config.slow_query_threshold_ms {
            self.record_slow_query(query, execution_time.as_millis() as u64);
        }

        self.updated_at = chrono::Utc::now().timestamp_millis();

        match result {
            Ok(query_result) => {
                info!(
                    graph = %self.graph_name,
                    nodes = query_result.nodes.len(),
                    relationships = query_result.relationships.len(),
                    time_ms = execution_time.as_millis(),
                    "Query executed successfully"
                );
                Ok(query_result)
            }
            Err(e) => {
                warn!(
                    graph = %self.graph_name,
                    error = %e,
                    time_ms = execution_time.as_millis(),
                    "Query execution failed"
                );
                Err(e)
            }
        }
    }

    /// Generate execution plan for a query without executing it
    pub async fn explain_query(&self, query: &str) -> ProtocolResult<ExecutionPlan> {
        debug!(
            graph = %self.graph_name,
            query = query,
            "Generating execution plan"
        );

        // Parse the query to create a basic execution plan
        let parser = CypherParser::new();
        let parsed_query = parser.parse(query)?;

        let mut steps = Vec::new();
        let mut estimated_cost = 0.0;

        for clause in &parsed_query.clauses {
            match clause {
                crate::cypher::cypher_parser::CypherClause::Match { pattern } => {
                    let step = PlanStep {
                        operation: "NodeScan".to_string(),
                        description: format!("Scan nodes matching pattern: {pattern:?}"),
                        estimated_rows: 1000, // Simplified estimation
                        estimated_cost: 10.0,
                        children: vec![],
                    };
                    estimated_cost += step.estimated_cost;
                    steps.push(step);
                }
                crate::cypher::cypher_parser::CypherClause::Create { pattern } => {
                    let step = PlanStep {
                        operation: "Create".to_string(),
                        description: format!("Create nodes/relationships: {pattern:?}"),
                        estimated_rows: 1,
                        estimated_cost: 5.0,
                        children: vec![],
                    };
                    estimated_cost += step.estimated_cost;
                    steps.push(step);
                }
                crate::cypher::cypher_parser::CypherClause::Return { items } => {
                    let step = PlanStep {
                        operation: "Projection".to_string(),
                        description: format!("Return {} items", items.len()),
                        estimated_rows: 100,
                        estimated_cost: 2.0,
                        children: vec![],
                    };
                    estimated_cost += step.estimated_cost;
                    steps.push(step);
                }
                crate::cypher::cypher_parser::CypherClause::Where { .. } => {
                    let step = PlanStep {
                        operation: "Filter".to_string(),
                        description: "Apply WHERE clause filter".to_string(),
                        estimated_rows: 50,
                        estimated_cost: 3.0,
                        children: vec![],
                    };
                    estimated_cost += step.estimated_cost;
                    steps.push(step);
                }
            }
        }

        Ok(ExecutionPlan {
            steps,
            estimated_cost,
            query: query.to_string(),
        })
    }

    /// Execute a query and return both results and profiling information
    pub async fn profile_query(&mut self, query: &str) -> ProtocolResult<QueryProfile> {
        let start_time = Instant::now();

        // Generate execution plan
        let plan = self.explain_query(query).await?;

        // Execute the query (this would collect actual metrics in production)
        let _result = self.execute_query_internal(query, false).await?;

        let total_time = start_time.elapsed();

        // Create mock profiling metrics (in production, these would be collected during execution)
        let metrics = ProfileMetrics {
            rows_processed: plan.steps.iter().map(|s| s.estimated_rows).collect(),
            step_times_ms: plan
                .steps
                .iter()
                .map(|_| total_time.as_millis() as u64 / plan.steps.len() as u64)
                .collect(),
            memory_used: plan.steps.iter().map(|_| 1024u64).collect(), // Mock memory usage
            cache_hits: 0,
            cache_misses: 0,
        };

        Ok(QueryProfile {
            plan,
            metrics,
            total_time_ms: total_time.as_millis() as u64,
        })
    }

    /// Get the slow query log for this graph
    pub fn get_slow_queries(&self) -> Vec<SlowQuery> {
        self.slow_queries.clone()
    }

    /// Clear the slow query log
    pub fn clear_slow_queries(&mut self) {
        self.slow_queries.clear();
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Get graph statistics
    pub fn get_stats(&self) -> GraphStats {
        self.stats.clone()
    }

    /// Update graph configuration
    pub fn update_config(&mut self, new_config: GraphConfig) {
        self.config = new_config;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Get a specific configuration parameter
    pub fn get_config_parameter(&self, parameter: &str) -> Option<serde_json::Value> {
        match parameter.to_uppercase().as_str() {
            "MAX_NODES" => Some(serde_json::Value::Number(self.config.max_nodes.into())),
            "MAX_RELATIONSHIPS" => Some(serde_json::Value::Number(
                self.config.max_relationships.into(),
            )),
            "QUERY_TIMEOUT" => Some(serde_json::Value::Number(
                self.config.query_timeout_ms.into(),
            )),
            "MAX_RESULT_SIZE" => Some(serde_json::Value::Number(
                self.config.max_result_size.into(),
            )),
            "PROFILING_ENABLED" => Some(serde_json::Value::Bool(self.config.profiling_enabled)),
            "SLOW_QUERY_THRESHOLD" => Some(serde_json::Value::Number(
                self.config.slow_query_threshold_ms.into(),
            )),
            "MEMORY_LIMIT" => Some(serde_json::Value::Number(
                self.config.memory_limit_bytes.into(),
            )),
            _ => None,
        }
    }

    /// Set a specific configuration parameter
    pub fn set_config_parameter(
        &mut self,
        parameter: &str,
        value: serde_json::Value,
    ) -> ProtocolResult<()> {
        match parameter.to_uppercase().as_str() {
            "MAX_NODES" => {
                if let Some(num) = value.as_u64() {
                    self.config.max_nodes = num;
                } else {
                    return Err(ProtocolError::CypherError(
                        "Invalid value for MAX_NODES".to_string(),
                    ));
                }
            }
            "MAX_RELATIONSHIPS" => {
                if let Some(num) = value.as_u64() {
                    self.config.max_relationships = num;
                } else {
                    return Err(ProtocolError::CypherError(
                        "Invalid value for MAX_RELATIONSHIPS".to_string(),
                    ));
                }
            }
            "QUERY_TIMEOUT" => {
                if let Some(num) = value.as_u64() {
                    self.config.query_timeout_ms = num;
                } else {
                    return Err(ProtocolError::CypherError(
                        "Invalid value for QUERY_TIMEOUT".to_string(),
                    ));
                }
            }
            "PROFILING_ENABLED" => {
                if let Some(bool_val) = value.as_bool() {
                    self.config.profiling_enabled = bool_val;
                } else {
                    return Err(ProtocolError::CypherError(
                        "Invalid value for PROFILING_ENABLED".to_string(),
                    ));
                }
            }
            "SLOW_QUERY_THRESHOLD" => {
                if let Some(num) = value.as_u64() {
                    self.config.slow_query_threshold_ms = num;
                } else {
                    return Err(ProtocolError::CypherError(
                        "Invalid value for SLOW_QUERY_THRESHOLD".to_string(),
                    ));
                }
            }
            _ => {
                return Err(ProtocolError::CypherError(format!(
                    "Unknown configuration parameter: {parameter}"
                )));
            }
        }

        self.updated_at = chrono::Utc::now().timestamp_millis();
        Ok(())
    }

    /// Check if a query contains write operations
    fn is_write_query(&self, query: &str) -> bool {
        let upper_query = query.to_uppercase();
        upper_query.contains("CREATE")
            || upper_query.contains("DELETE")
            || upper_query.contains("SET")
            || upper_query.contains("REMOVE")
            || upper_query.contains("MERGE")
    }

    /// Update execution statistics
    fn update_stats(&mut self, execution_time: Duration, read_only: bool, success: bool) {
        if success {
            self.stats.queries_executed += 1;
            if read_only {
                self.stats.read_queries_executed += 1;
            } else {
                self.stats.write_queries_executed += 1;
            }

            let time_ms = execution_time.as_millis() as u64;
            self.stats.total_execution_time_ms += time_ms;

            // Update average execution time
            if self.stats.queries_executed > 0 {
                self.stats.avg_execution_time_ms =
                    self.stats.total_execution_time_ms as f64 / self.stats.queries_executed as f64;
            }
        }

        self.stats.last_updated = chrono::Utc::now().timestamp_millis();
    }

    /// Record a slow query
    fn record_slow_query(&mut self, query: &str, execution_time_ms: u64) {
        let slow_query = SlowQuery {
            query: query.to_string(),
            execution_time_ms,
            timestamp: chrono::Utc::now().timestamp_millis(),
            parameters: None, // TODO: Add parameter support
        };

        self.slow_queries.push(slow_query);

        // Keep only the configured number of slow queries
        if self.slow_queries.len() > self.config.max_slow_queries {
            self.slow_queries.remove(0);
        }
    }
}

impl Addressable for GraphActor {
    fn addressable_type() -> &'static str {
        "GraphActor"
    }
}

/// Graph management service for handling multiple graphs
#[derive(Debug, Default)]
pub struct GraphManager {
    /// All managed graphs
    graphs: HashMap<String, GraphActor>,
}

impl GraphManager {
    /// Create a new graph manager
    pub fn new() -> Self {
        Self {
            graphs: HashMap::new(),
        }
    }

    /// Create a new graph
    pub fn create_graph(&mut self, name: String) -> ProtocolResult<()> {
        if self.graphs.contains_key(&name) {
            return Err(ProtocolError::CypherError(format!(
                "Graph '{name}' already exists"
            )));
        }

        let graph = GraphActor::new(name.clone());
        self.graphs.insert(name.clone(), graph);

        info!("Created graph: {}", name);
        Ok(())
    }

    /// Delete a graph
    pub fn delete_graph(&mut self, name: &str) -> ProtocolResult<bool> {
        if self.graphs.remove(name).is_some() {
            info!("Deleted graph: {}", name);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get a mutable reference to a graph
    pub fn get_graph_mut(&mut self, name: &str) -> Option<&mut GraphActor> {
        self.graphs.get_mut(name)
    }

    /// Get a reference to a graph
    pub fn get_graph(&self, name: &str) -> Option<&GraphActor> {
        self.graphs.get(name)
    }

    /// List all graph names
    pub fn list_graphs(&self) -> Vec<String> {
        self.graphs.keys().cloned().collect()
    }

    /// Get the number of graphs managed
    pub fn graph_count(&self) -> usize {
        self.graphs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_actor_creation() {
        let graph = GraphActor::new("test_graph".to_string());
        assert_eq!(graph.graph_name, "test_graph");
        assert_eq!(graph.stats.queries_executed, 0);
    }

    #[test]
    fn test_config_parameter_operations() {
        let mut graph = GraphActor::new("test_graph".to_string());

        // Test getting a parameter
        let max_nodes = graph.get_config_parameter("MAX_NODES");
        assert!(max_nodes.is_some());

        // Test setting a parameter
        let result =
            graph.set_config_parameter("MAX_NODES", serde_json::Value::Number(500000.into()));
        assert!(result.is_ok());
        assert_eq!(graph.config.max_nodes, 500000);
    }

    #[test]
    fn test_write_query_detection() {
        let graph = GraphActor::new("test_graph".to_string());

        assert!(graph.is_write_query("CREATE (n:Person) RETURN n"));
        assert!(graph.is_write_query("MATCH (n) DELETE n"));
        assert!(graph.is_write_query("MATCH (n) SET n.name = 'Alice'"));
        assert!(!graph.is_write_query("MATCH (n:Person) RETURN n"));
        assert!(!graph.is_write_query("MATCH (n)-[r]->(m) RETURN n, r, m"));
    }

    #[tokio::test]
    async fn test_query_explanation() {
        let graph = GraphActor::new("test_graph".to_string());
        let query = "MATCH (n:Person) RETURN n";

        let plan = graph.explain_query(query).await;
        if let Err(e) = &plan {
            eprintln!("Plan error: {:?}", e);
        }
        assert!(plan.is_ok());

        let plan = plan.unwrap();
        assert_eq!(plan.query, query);
        assert!(!plan.steps.is_empty());
    }

    #[test]
    fn test_graph_manager() {
        let mut manager = GraphManager::new();

        // Create graph
        let result = manager.create_graph("graph1".to_string());
        assert!(result.is_ok());
        assert_eq!(manager.graph_count(), 1);

        // Try to create duplicate
        let result = manager.create_graph("graph1".to_string());
        assert!(result.is_err());

        // List graphs
        let graphs = manager.list_graphs();
        assert_eq!(graphs, vec!["graph1"]);

        // Delete graph
        let result = manager.delete_graph("graph1");
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(manager.graph_count(), 0);
    }
}
