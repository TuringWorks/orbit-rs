//! CQL Graph Extensions
//!
//! This module provides graph traversal and algorithm support for CQL protocol,
//! using the shared graph algorithms from protocols/common/graph_algorithms.
//!
//! Inspired by DataStax Graph functionality, this module adds graph operations
//! to CQL queries including traversal, shortest path, and centrality algorithms.

use crate::protocols::common::graph_algorithms::{self as graph_algo, Graph};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;

/// CQL Graph query types
#[derive(Debug, Clone)]
pub enum GraphQuery {
    /// Traverse from a vertex
    Traverse {
        start_vertex: String,
        direction: GraphDirection,
        edge_label: Option<String>,
        max_depth: Option<usize>,
    },
    /// Find shortest path between vertices
    ShortestPath {
        from_vertex: String,
        to_vertex: String,
        weighted: bool,
    },
    /// Find all shortest paths
    AllShortestPaths {
        from_vertex: String,
        to_vertex: String,
    },
    /// Compute PageRank
    PageRank {
        damping: f64,
        max_iterations: usize,
        tolerance: f64,
    },
    /// Find neighbors of a vertex
    Neighbors {
        vertex: String,
        direction: GraphDirection,
        edge_label: Option<String>,
    },
    /// Connected components analysis
    ConnectedComponents,
    /// Strongly connected components
    StronglyConnectedComponents,
}

/// Direction for graph traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphDirection {
    Out,
    In,
    Both,
}

/// Result of a graph query
#[derive(Debug, Clone, Default)]
pub struct GraphQueryResult {
    /// Result vertices
    pub vertices: Vec<GraphVertex>,
    /// Result edges
    pub edges: Vec<GraphEdge>,
    /// Paths (for path queries)
    pub paths: Vec<GraphPath>,
    /// Scores (for ranking algorithms)
    pub scores: HashMap<String, f64>,
    /// Component assignments (for component analysis)
    pub components: HashMap<String, usize>,
}

/// A vertex in the graph result
#[derive(Debug, Clone)]
pub struct GraphVertex {
    pub id: String,
    pub label: String,
    pub properties: HashMap<String, SqlValue>,
}

/// An edge in the graph result
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub id: String,
    pub label: String,
    pub from_vertex: String,
    pub to_vertex: String,
    pub properties: HashMap<String, SqlValue>,
}

/// A path result
#[derive(Debug, Clone)]
pub struct GraphPath {
    pub vertices: Vec<String>,
    pub edges: Vec<String>,
    pub cost: f64,
}

/// CQL Graph Engine - executes graph queries using shared algorithms
pub struct CqlGraphEngine {
    /// The underlying graph structure
    graph: Graph,
    /// Vertex labels mapping
    vertex_labels: HashMap<String, String>,
    /// Edge labels mapping
    edge_labels: HashMap<String, String>,
}

impl CqlGraphEngine {
    /// Create a new CQL graph engine
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            vertex_labels: HashMap::new(),
            edge_labels: HashMap::new(),
        }
    }

    /// Build graph from vertex and edge tables
    pub fn build_from_tables(
        &mut self,
        vertices: &[HashMap<String, SqlValue>],
        edges: &[HashMap<String, SqlValue>],
        vertex_id_col: &str,
        vertex_label_col: Option<&str>,
        edge_from_col: &str,
        edge_to_col: &str,
        edge_label_col: Option<&str>,
        edge_weight_col: Option<&str>,
    ) -> ProtocolResult<()> {
        // Clear existing graph
        self.graph = Graph::new();
        self.vertex_labels.clear();
        self.edge_labels.clear();

        // Add vertices
        for vertex in vertices {
            let id = extract_string(vertex, vertex_id_col)?;
            let label = vertex_label_col
                .and_then(|col| extract_string(vertex, col).ok())
                .unwrap_or_else(|| "vertex".to_string());

            // Convert properties
            let properties: HashMap<String, serde_json::Value> = vertex
                .iter()
                .map(|(k, v)| (k.clone(), sql_value_to_json(v)))
                .collect();

            self.graph.add_node(id.clone(), properties);
            self.vertex_labels.insert(id, label);
        }

        // Add edges
        for (idx, edge) in edges.iter().enumerate() {
            let from = extract_string(edge, edge_from_col)?;
            let to = extract_string(edge, edge_to_col)?;
            let label = edge_label_col
                .and_then(|col| extract_string(edge, col).ok())
                .unwrap_or_else(|| "edge".to_string());
            let weight = edge_weight_col
                .and_then(|col| extract_float(edge, col))
                .unwrap_or(1.0);

            self.graph.add_edge(from, to, weight, Some(label.clone()));
            self.edge_labels.insert(format!("e{}", idx), label);
        }

        Ok(())
    }

    /// Execute a graph query
    pub fn execute(&self, query: &GraphQuery) -> ProtocolResult<GraphQueryResult> {
        match query {
            GraphQuery::Traverse {
                start_vertex,
                direction,
                edge_label,
                max_depth,
            } => self.execute_traverse(start_vertex, *direction, edge_label.as_deref(), *max_depth),

            GraphQuery::ShortestPath {
                from_vertex,
                to_vertex,
                weighted,
            } => self.execute_shortest_path(from_vertex, to_vertex, *weighted),

            GraphQuery::AllShortestPaths {
                from_vertex,
                to_vertex,
            } => self.execute_all_shortest_paths(from_vertex, to_vertex),

            GraphQuery::PageRank {
                damping,
                max_iterations,
                tolerance,
            } => self.execute_pagerank(*damping, *max_iterations, *tolerance),

            GraphQuery::Neighbors {
                vertex,
                direction,
                edge_label,
            } => self.execute_neighbors(vertex, *direction, edge_label.as_deref()),

            GraphQuery::ConnectedComponents => self.execute_connected_components(),

            GraphQuery::StronglyConnectedComponents => self.execute_strongly_connected_components(),
        }
    }

    /// Execute a traverse query
    fn execute_traverse(
        &self,
        start: &str,
        direction: GraphDirection,
        edge_label: Option<&str>,
        max_depth: Option<usize>,
    ) -> ProtocolResult<GraphQueryResult> {
        let mut result = GraphQueryResult::default();

        // Use BFS traversal from shared algorithms
        let traversal = graph_algo::bfs_traversal(&self.graph, start, max_depth);

        // Filter by direction and edge label
        for node_id in &traversal.visited {
            // Check direction constraint
            let neighbors = match direction {
                GraphDirection::Out => self.graph.get_outgoing_neighbors(node_id),
                GraphDirection::In => self.graph.get_incoming_neighbors(node_id),
                GraphDirection::Both => self.graph.get_all_neighbors(node_id),
            };

            // Filter by edge label if specified
            let valid = if let Some(label) = edge_label {
                neighbors
                    .iter()
                    .any(|(_, _, etype)| etype.as_deref() == Some(label))
            } else {
                true
            };

            if valid || node_id == start {
                if let Some(node) = self.graph.nodes.get(node_id) {
                    let vertex = GraphVertex {
                        id: node_id.clone(),
                        label: self.vertex_labels.get(node_id).cloned().unwrap_or_default(),
                        properties: node
                            .properties
                            .iter()
                            .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                            .collect(),
                    };
                    result.vertices.push(vertex);
                }
            }
        }

        Ok(result)
    }

    /// Execute shortest path query
    fn execute_shortest_path(
        &self,
        from: &str,
        to: &str,
        weighted: bool,
    ) -> ProtocolResult<GraphQueryResult> {
        let mut result = GraphQueryResult::default();

        let path_result = if weighted {
            graph_algo::dijkstra(&self.graph, from, to)
        } else {
            graph_algo::bfs_shortest_path(&self.graph, from, to)
        };

        if path_result.found {
            let path = GraphPath {
                vertices: path_result.path.clone(),
                edges: Vec::new(), // Could add edge IDs here
                cost: path_result.cost,
            };
            result.paths.push(path);

            // Add vertices along the path
            for node_id in &path_result.path {
                if let Some(node) = self.graph.nodes.get(node_id) {
                    let vertex = GraphVertex {
                        id: node_id.clone(),
                        label: self.vertex_labels.get(node_id).cloned().unwrap_or_default(),
                        properties: node
                            .properties
                            .iter()
                            .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                            .collect(),
                    };
                    result.vertices.push(vertex);
                }
            }
        }

        Ok(result)
    }

    /// Execute all shortest paths query
    fn execute_all_shortest_paths(&self, from: &str, to: &str) -> ProtocolResult<GraphQueryResult> {
        let mut result = GraphQueryResult::default();

        let all_paths = graph_algo::all_shortest_paths(&self.graph, from, to);
        let mut seen_vertices: std::collections::HashSet<String> = std::collections::HashSet::new();

        for path in all_paths.paths {
            let graph_path = GraphPath {
                vertices: path.clone(),
                edges: Vec::new(),
                cost: path.len() as f64 - 1.0,
            };
            result.paths.push(graph_path);

            // Add vertices (avoid duplicates)
            for node_id in &path {
                if !seen_vertices.contains(node_id) {
                    seen_vertices.insert(node_id.clone());
                    if let Some(node) = self.graph.nodes.get(node_id) {
                        let vertex = GraphVertex {
                            id: node_id.clone(),
                            label: self.vertex_labels.get(node_id).cloned().unwrap_or_default(),
                            properties: node
                                .properties
                                .iter()
                                .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                                .collect(),
                        };
                        result.vertices.push(vertex);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Execute PageRank algorithm
    fn execute_pagerank(
        &self,
        damping: f64,
        max_iterations: usize,
        tolerance: f64,
    ) -> ProtocolResult<GraphQueryResult> {
        let mut result = GraphQueryResult::default();

        let pr_result = graph_algo::pagerank(&self.graph, damping, max_iterations, tolerance);
        result.scores = pr_result.scores;

        // Add all vertices with their scores
        for (node_id, score) in &result.scores {
            if let Some(node) = self.graph.nodes.get(node_id) {
                let mut properties: HashMap<String, SqlValue> = node
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                    .collect();
                properties.insert("pagerank".to_string(), SqlValue::DoublePrecision(*score));

                let vertex = GraphVertex {
                    id: node_id.clone(),
                    label: self.vertex_labels.get(node_id).cloned().unwrap_or_default(),
                    properties,
                };
                result.vertices.push(vertex);
            }
        }

        Ok(result)
    }

    /// Execute neighbors query
    fn execute_neighbors(
        &self,
        vertex: &str,
        direction: GraphDirection,
        edge_label: Option<&str>,
    ) -> ProtocolResult<GraphQueryResult> {
        let mut result = GraphQueryResult::default();

        let neighbors: Vec<(String, f64, Option<String>)> = match direction {
            GraphDirection::Out => self.graph.get_outgoing_neighbors(vertex),
            GraphDirection::In => self.graph.get_incoming_neighbors(vertex),
            GraphDirection::Both => self.graph.get_all_neighbors(vertex),
        };

        for (neighbor_id, _weight, etype) in neighbors {
            // Filter by edge label if specified
            if let Some(label) = edge_label {
                if etype.as_deref() != Some(label) {
                    continue;
                }
            }

            if let Some(node) = self.graph.nodes.get(&neighbor_id) {
                let vertex = GraphVertex {
                    id: neighbor_id.clone(),
                    label: self
                        .vertex_labels
                        .get(&neighbor_id)
                        .cloned()
                        .unwrap_or_default(),
                    properties: node
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                        .collect(),
                };
                result.vertices.push(vertex);
            }
        }

        Ok(result)
    }

    /// Execute connected components analysis
    fn execute_connected_components(&self) -> ProtocolResult<GraphQueryResult> {
        let mut result = GraphQueryResult::default();

        let cc_result = graph_algo::connected_components(&self.graph);
        result.components = cc_result.component_ids;

        // Add all vertices with their component IDs
        for (node_id, component_id) in &result.components {
            if let Some(node) = self.graph.nodes.get(node_id) {
                let mut properties: HashMap<String, SqlValue> = node
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                    .collect();
                properties.insert(
                    "component_id".to_string(),
                    SqlValue::Integer(*component_id as i32),
                );

                let vertex = GraphVertex {
                    id: node_id.clone(),
                    label: self.vertex_labels.get(node_id).cloned().unwrap_or_default(),
                    properties,
                };
                result.vertices.push(vertex);
            }
        }

        Ok(result)
    }

    /// Execute strongly connected components analysis
    fn execute_strongly_connected_components(&self) -> ProtocolResult<GraphQueryResult> {
        let mut result = GraphQueryResult::default();

        let scc_result = graph_algo::strongly_connected_components(&self.graph);
        result.components = scc_result.component_ids;

        // Add all vertices with their component IDs
        for (node_id, component_id) in &result.components {
            if let Some(node) = self.graph.nodes.get(node_id) {
                let mut properties: HashMap<String, SqlValue> = node
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), json_to_sql_value(v)))
                    .collect();
                properties.insert(
                    "scc_id".to_string(),
                    SqlValue::Integer(*component_id as i32),
                );

                let vertex = GraphVertex {
                    id: node_id.clone(),
                    label: self.vertex_labels.get(node_id).cloned().unwrap_or_default(),
                    properties,
                };
                result.vertices.push(vertex);
            }
        }

        Ok(result)
    }
}

impl Default for CqlGraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions

fn extract_string(row: &HashMap<String, SqlValue>, column: &str) -> ProtocolResult<String> {
    row.get(column)
        .map(|v| v.to_postgres_string())
        .ok_or_else(|| ProtocolError::PostgresError(format!("Column '{}' not found", column)))
}

fn extract_float(row: &HashMap<String, SqlValue>, column: &str) -> Option<f64> {
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
        SqlValue::Json(v) | SqlValue::Jsonb(v) => v.clone(),
        SqlValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sql_value_to_json).collect())
        }
        SqlValue::Uuid(u) => serde_json::Value::String(u.to_string()),
        _ => serde_json::Value::String(value.to_postgres_string()),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_vertices() -> Vec<HashMap<String, SqlValue>> {
        vec![
            {
                let mut m = HashMap::new();
                m.insert("id".to_string(), SqlValue::Text("v1".to_string()));
                m.insert("name".to_string(), SqlValue::Text("Alice".to_string()));
                m.insert("label".to_string(), SqlValue::Text("person".to_string()));
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("id".to_string(), SqlValue::Text("v2".to_string()));
                m.insert("name".to_string(), SqlValue::Text("Bob".to_string()));
                m.insert("label".to_string(), SqlValue::Text("person".to_string()));
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("id".to_string(), SqlValue::Text("v3".to_string()));
                m.insert("name".to_string(), SqlValue::Text("Charlie".to_string()));
                m.insert("label".to_string(), SqlValue::Text("person".to_string()));
                m
            },
        ]
    }

    fn create_test_edges() -> Vec<HashMap<String, SqlValue>> {
        vec![
            {
                let mut m = HashMap::new();
                m.insert("from".to_string(), SqlValue::Text("v1".to_string()));
                m.insert("to".to_string(), SqlValue::Text("v2".to_string()));
                m.insert("label".to_string(), SqlValue::Text("knows".to_string()));
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("from".to_string(), SqlValue::Text("v2".to_string()));
                m.insert("to".to_string(), SqlValue::Text("v3".to_string()));
                m.insert("label".to_string(), SqlValue::Text("knows".to_string()));
                m
            },
        ]
    }

    #[test]
    fn test_graph_engine_creation() {
        let mut engine = CqlGraphEngine::new();
        let vertices = create_test_vertices();
        let edges = create_test_edges();

        engine
            .build_from_tables(
                &vertices,
                &edges,
                "id",
                Some("label"),
                "from",
                "to",
                Some("label"),
                None,
            )
            .unwrap();

        assert_eq!(engine.graph.nodes.len(), 3);
    }

    #[test]
    fn test_traverse() {
        let mut engine = CqlGraphEngine::new();
        let vertices = create_test_vertices();
        let edges = create_test_edges();

        engine
            .build_from_tables(
                &vertices,
                &edges,
                "id",
                Some("label"),
                "from",
                "to",
                Some("label"),
                None,
            )
            .unwrap();

        let query = GraphQuery::Traverse {
            start_vertex: "v1".to_string(),
            direction: GraphDirection::Out,
            edge_label: None,
            max_depth: Some(2),
        };

        let result = engine.execute(&query).unwrap();
        assert!(!result.vertices.is_empty());
    }

    #[test]
    fn test_shortest_path() {
        let mut engine = CqlGraphEngine::new();
        let vertices = create_test_vertices();
        let edges = create_test_edges();

        engine
            .build_from_tables(
                &vertices,
                &edges,
                "id",
                Some("label"),
                "from",
                "to",
                Some("label"),
                None,
            )
            .unwrap();

        let query = GraphQuery::ShortestPath {
            from_vertex: "v1".to_string(),
            to_vertex: "v3".to_string(),
            weighted: false,
        };

        let result = engine.execute(&query).unwrap();
        assert_eq!(result.paths.len(), 1);
        assert_eq!(result.paths[0].vertices, vec!["v1", "v2", "v3"]);
    }
}
