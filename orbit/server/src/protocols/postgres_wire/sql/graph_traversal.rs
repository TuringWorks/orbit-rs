//! Graph Traversal Support for OrbitQL
//!
//! This module provides graph traversal capabilities for OrbitQL's TRAVERSE clause,
//! using the shared graph algorithms from protocols/common/graph_algorithms.

use crate::protocols::common::graph_algorithms::{self as graph_algo, Graph, TraversalResult};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::ast::{TraverseClause, TraverseDirection};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;

/// Result of a graph traversal operation
#[derive(Debug, Clone, Default)]
pub struct GraphTraversalResult {
    /// Visited nodes with their data
    pub nodes: Vec<HashMap<String, SqlValue>>,
    /// Edges traversed (from_id, to_id, edge_type)
    pub edges: Vec<(String, String, Option<String>)>,
    /// Depth of each node from the start
    pub depths: HashMap<String, usize>,
    /// Path taken (for path queries)
    pub paths: Vec<Vec<String>>,
}

/// Graph builder for constructing a graph from table data
pub struct OrbitQLGraphBuilder {
    /// Node table name
    #[allow(dead_code)]
    node_table: String,
    /// Edge table name (edge collection)
    #[allow(dead_code)]
    edge_table: String,
    /// Column containing the from node ID in edge table
    from_column: String,
    /// Column containing the to node ID in edge table
    to_column: String,
    /// Column containing the node ID in node table
    node_id_column: String,
    /// Optional weight column
    weight_column: Option<String>,
    /// Optional edge type column
    edge_type_column: Option<String>,
}

impl OrbitQLGraphBuilder {
    /// Create a new graph builder with default column names
    pub fn new(node_table: &str, edge_table: &str) -> Self {
        Self {
            node_table: node_table.to_string(),
            edge_table: edge_table.to_string(),
            from_column: "_from".to_string(),
            to_column: "_to".to_string(),
            node_id_column: "_key".to_string(),
            weight_column: None,
            edge_type_column: None,
        }
    }

    /// Set the from column name for edges
    pub fn with_from_column(mut self, column: &str) -> Self {
        self.from_column = column.to_string();
        self
    }

    /// Set the to column name for edges
    pub fn with_to_column(mut self, column: &str) -> Self {
        self.to_column = column.to_string();
        self
    }

    /// Set the node ID column name
    pub fn with_node_id_column(mut self, column: &str) -> Self {
        self.node_id_column = column.to_string();
        self
    }

    /// Set the weight column for weighted traversal
    pub fn with_weight_column(mut self, column: &str) -> Self {
        self.weight_column = Some(column.to_string());
        self
    }

    /// Set the edge type column
    pub fn with_edge_type_column(mut self, column: &str) -> Self {
        self.edge_type_column = Some(column.to_string());
        self
    }

    /// Build a graph from node and edge data
    pub fn build_graph(
        &self,
        node_data: &[HashMap<String, SqlValue>],
        edge_data: &[HashMap<String, SqlValue>],
    ) -> ProtocolResult<Graph> {
        let mut graph = Graph::new();

        // Add nodes
        for node_row in node_data {
            let node_id = self.extract_string_value(node_row, &self.node_id_column)?;

            // Convert SqlValue properties to serde_json::Value
            let properties: HashMap<String, serde_json::Value> = node_row
                .iter()
                .map(|(k, v)| (k.clone(), sql_value_to_json(v)))
                .collect();

            graph.add_node(node_id, properties);
        }

        // Add edges
        for edge_row in edge_data {
            let from_id = self.extract_string_value(edge_row, &self.from_column)?;
            let to_id = self.extract_string_value(edge_row, &self.to_column)?;

            let weight = if let Some(ref weight_col) = self.weight_column {
                self.extract_float_value(edge_row, weight_col)
                    .unwrap_or(1.0)
            } else {
                1.0
            };

            let edge_type = if let Some(ref type_col) = self.edge_type_column {
                self.extract_string_value(edge_row, type_col).ok()
            } else {
                None
            };

            graph.add_edge(from_id, to_id, weight, edge_type);
        }

        Ok(graph)
    }

    /// Extract a string value from a row
    fn extract_string_value(
        &self,
        row: &HashMap<String, SqlValue>,
        column: &str,
    ) -> ProtocolResult<String> {
        row.get(column)
            .map(|v| v.to_postgres_string())
            .ok_or_else(|| {
                ProtocolError::PostgresError(format!("Column '{}' not found in row", column))
            })
    }

    /// Extract a float value from a row
    fn extract_float_value(&self, row: &HashMap<String, SqlValue>, column: &str) -> Option<f64> {
        row.get(column).and_then(|v| match v {
            SqlValue::Integer(i) => Some(*i as f64),
            SqlValue::BigInt(i) => Some(*i as f64),
            SqlValue::Real(f) => Some(*f as f64),
            SqlValue::DoublePrecision(f) => Some(*f),
            SqlValue::Decimal(d) => d.to_string().parse().ok(),
            SqlValue::Text(s) => s.parse().ok(),
            _ => None,
        })
    }
}

/// Execute a TRAVERSE clause using the shared graph algorithms
pub fn execute_traverse(
    graph: &Graph,
    start_nodes: &[String],
    traverse_clause: &TraverseClause,
) -> ProtocolResult<GraphTraversalResult> {
    let mut result = GraphTraversalResult::default();

    for start_node in start_nodes {
        // Check if start node exists
        if !graph.nodes.contains_key(start_node) {
            continue;
        }

        // Execute BFS traversal with depth limits
        let traversal = execute_bounded_traversal(
            graph,
            start_node,
            &traverse_clause.direction,
            traverse_clause.min_steps,
            traverse_clause.max_steps,
            Some(traverse_clause.edge_collection.as_str()),
        )?;

        // Merge results
        result.depths.extend(traversal.depths.clone());

        // Collect nodes at valid depths
        for node_id in &traversal.visited {
            if let Some(&depth) = traversal.depths.get(node_id) {
                if depth >= traverse_clause.min_steps as usize
                    && depth <= traverse_clause.max_steps as usize
                {
                    if let Some(node) = graph.nodes.get(node_id) {
                        let row: HashMap<String, SqlValue> = node
                            .properties
                            .iter()
                            .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                            .collect();
                        result.nodes.push(row);
                    }
                }
            }
        }

        // Build path from start to each visited node
        for node_id in &traversal.visited {
            if let Some(&depth) = traversal.depths.get(node_id) {
                if depth >= traverse_clause.min_steps as usize
                    && depth <= traverse_clause.max_steps as usize
                {
                    let path = reconstruct_path(&traversal.parents, start_node, node_id);
                    if !path.is_empty() {
                        result.paths.push(path);
                    }
                }
            }
        }
    }

    Ok(result)
}

/// Execute a bounded traversal respecting depth limits
fn execute_bounded_traversal(
    graph: &Graph,
    start: &str,
    direction: &TraverseDirection,
    _min_depth: u32,
    max_depth: u32,
    edge_filter: Option<&str>,
) -> ProtocolResult<TraversalResult> {
    // Use BFS from shared algorithms for bounded traversal
    let traversal = graph_algo::bfs_traversal(graph, start, Some(max_depth as usize));

    // Filter by direction and edge type if needed
    let filtered = if matches!(direction, TraverseDirection::Any) && edge_filter.is_none() {
        traversal
    } else {
        filter_traversal_by_direction(graph, &traversal, start, direction, edge_filter)
    };

    Ok(filtered)
}

/// Filter traversal results based on direction
fn filter_traversal_by_direction(
    graph: &Graph,
    _traversal: &TraversalResult,
    start: &str,
    direction: &TraverseDirection,
    edge_filter: Option<&str>,
) -> TraversalResult {
    use std::collections::{HashSet, VecDeque};

    let mut result = TraversalResult {
        visited: vec![start.to_string()],
        depths: HashMap::new(),
        parents: HashMap::new(),
    };
    result.depths.insert(start.to_string(), 0);

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(start.to_string());

    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((start.to_string(), 0));

    while let Some((node, depth)) = queue.pop_front() {
        // Get neighbors based on direction
        let neighbors: Vec<(String, f64, Option<String>)> = match direction {
            TraverseDirection::Outbound => graph.get_outgoing_neighbors(&node),
            TraverseDirection::Inbound => graph.get_incoming_neighbors(&node),
            TraverseDirection::Any => graph.get_all_neighbors(&node),
        };

        for (neighbor, _weight, edge_type) in neighbors {
            // Apply edge type filter if specified
            if let Some(filter) = edge_filter {
                if let Some(ref etype) = edge_type {
                    if etype != filter {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if !visited.contains(&neighbor) {
                visited.insert(neighbor.clone());
                result.visited.push(neighbor.clone());
                result.depths.insert(neighbor.clone(), depth + 1);
                result.parents.insert(neighbor.clone(), node.clone());
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    result
}

/// Reconstruct the path from start to target using parent pointers
fn reconstruct_path(parents: &HashMap<String, String>, start: &str, target: &str) -> Vec<String> {
    let mut path = Vec::new();
    let mut current = target.to_string();

    path.push(current.clone());

    while current != start {
        if let Some(parent) = parents.get(&current) {
            path.push(parent.clone());
            current = parent.clone();
        } else {
            break;
        }
    }

    path.reverse();
    path
}

/// Convert SqlValue to serde_json::Value
fn sql_value_to_json(value: &SqlValue) -> serde_json::Value {
    match value {
        SqlValue::Null => serde_json::Value::Null,
        SqlValue::Boolean(b) => serde_json::Value::Bool(*b),
        SqlValue::Integer(i) => serde_json::Value::Number((*i).into()),
        SqlValue::BigInt(i) => serde_json::Value::Number((*i).into()),
        SqlValue::SmallInt(i) => serde_json::Value::Number((*i as i64).into()),
        SqlValue::Real(f) => serde_json::json!(*f),
        SqlValue::DoublePrecision(f) => serde_json::json!(*f),
        SqlValue::Text(s) => serde_json::Value::String(s.clone()),
        SqlValue::Decimal(d) => serde_json::Value::String(d.to_string()),
        SqlValue::Bytea(b) => serde_json::Value::String(format!("\\x{}", hex::encode(b))),
        SqlValue::Json(v) | SqlValue::Jsonb(v) => v.clone(),
        SqlValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sql_value_to_json).collect())
        }
        SqlValue::Uuid(u) => serde_json::Value::String(u.to_string()),
        _ => serde_json::Value::String(value.to_postgres_string()),
    }
}

/// Convert serde_json::Value to SqlValue
fn json_to_sql_value(value: &serde_json::Value) -> SqlValue {
    match value {
        serde_json::Value::Null => SqlValue::Null,
        serde_json::Value::Bool(b) => SqlValue::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    SqlValue::Integer(i as i32)
                } else {
                    SqlValue::BigInt(i)
                }
            } else if let Some(f) = n.as_f64() {
                SqlValue::DoublePrecision(f)
            } else {
                SqlValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => SqlValue::Text(s.clone()),
        serde_json::Value::Array(arr) => {
            SqlValue::Array(arr.iter().map(json_to_sql_value).collect())
        }
        serde_json::Value::Object(_) => SqlValue::Json(value.clone()),
    }
}

/// Execute shortest path between two nodes
pub fn execute_shortest_path(
    graph: &Graph,
    from_node: &str,
    to_node: &str,
    weighted: bool,
) -> ProtocolResult<GraphTraversalResult> {
    let path_result = if weighted {
        graph_algo::dijkstra(graph, from_node, to_node)
    } else {
        graph_algo::bfs_shortest_path(graph, from_node, to_node)
    };

    let mut result = GraphTraversalResult::default();

    if path_result.found {
        // Add nodes along the path
        for (i, node_id) in path_result.path.iter().enumerate() {
            if let Some(node) = graph.nodes.get(node_id) {
                let row: HashMap<String, SqlValue> = node
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                    .collect();
                result.nodes.push(row);
                result.depths.insert(node_id.clone(), i);
            }
        }

        // Add edges between consecutive nodes in path
        for window in path_result.path.windows(2) {
            result
                .edges
                .push((window[0].clone(), window[1].clone(), None));
        }

        result.paths.push(path_result.path);
    }

    Ok(result)
}

/// Execute all shortest paths between two nodes
pub fn execute_all_shortest_paths(
    graph: &Graph,
    from_node: &str,
    to_node: &str,
) -> ProtocolResult<GraphTraversalResult> {
    let all_paths = graph_algo::all_shortest_paths(graph, from_node, to_node);

    let mut result = GraphTraversalResult::default();
    let mut seen_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();

    for path in all_paths.paths {
        result.paths.push(path.clone());

        // Add nodes (avoid duplicates)
        for node_id in &path {
            if !seen_nodes.contains(node_id) {
                seen_nodes.insert(node_id.clone());
                if let Some(node) = graph.nodes.get(node_id) {
                    let row: HashMap<String, SqlValue> = node
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                        .collect();
                    result.nodes.push(row);
                }
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> Graph {
        let mut graph = Graph::new();

        // Add nodes
        let mut props = HashMap::new();
        props.insert("name".to_string(), serde_json::json!("Alice"));
        graph.add_node("n1".to_string(), props);

        let mut props = HashMap::new();
        props.insert("name".to_string(), serde_json::json!("Bob"));
        graph.add_node("n2".to_string(), props);

        let mut props = HashMap::new();
        props.insert("name".to_string(), serde_json::json!("Charlie"));
        graph.add_node("n3".to_string(), props);

        // Add edges: n1 -> n2 -> n3
        graph.add_edge(
            "n1".to_string(),
            "n2".to_string(),
            1.0,
            Some("KNOWS".to_string()),
        );
        graph.add_edge(
            "n2".to_string(),
            "n3".to_string(),
            1.0,
            Some("KNOWS".to_string()),
        );

        graph
    }

    #[test]
    fn test_traverse_outbound() {
        let graph = create_test_graph();
        let clause = TraverseClause {
            direction: TraverseDirection::Outbound,
            min_steps: 1,
            max_steps: 2,
            edge_collection: "KNOWS".to_string(),
            target_alias: None,
        };

        let result = execute_traverse(&graph, &["n1".to_string()], &clause).unwrap();
        assert!(!result.nodes.is_empty());
    }

    #[test]
    fn test_shortest_path() {
        let graph = create_test_graph();
        let result = execute_shortest_path(&graph, "n1", "n3", false).unwrap();

        assert_eq!(result.paths.len(), 1);
        assert_eq!(result.paths[0], vec!["n1", "n2", "n3"]);
    }
}
