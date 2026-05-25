//! MongoDB Graph Support
//!
//! This module provides graph traversal capabilities for MongoDB's $graphLookup stage,
//! using the shared graph algorithms from protocols/common/graph_algorithms.

use crate::protocols::common::graph_algorithms::{self as graph_algo, Graph};
use bson::{Bson, Document};
use std::collections::HashMap;

/// MongoDB Graph Lookup configuration
#[derive(Debug, Clone)]
pub struct GraphLookupConfig {
    /// Collection to search
    pub from: String,
    /// Field in current document to start with
    pub start_with_field: Option<String>,
    /// Field in 'from' collection to match against the connectToField
    pub connect_from_field: String,
    /// Field in 'from' collection to match against startWith/connectFromField values
    pub connect_to_field: String,
    /// Name for the result array field
    pub as_field: String,
    /// Maximum recursion depth
    pub max_depth: Option<usize>,
    /// Field name to store depth information
    pub depth_field: Option<String>,
    /// Optional filter for restricting the search
    pub restrict_search_with_match: Option<Document>,
}

/// Result of a graph lookup operation
#[derive(Debug)]
pub struct GraphLookupResult {
    /// Documents found through traversal
    pub documents: Vec<Document>,
    /// Depth of each document from the start
    pub depths: HashMap<String, usize>,
}

/// MongoDB Graph Engine - builds and queries graphs from MongoDB collections
pub struct MongoGraphEngine {
    /// The underlying graph structure
    graph: Graph,
    /// Document ID to full document mapping
    documents: HashMap<String, Document>,
}

impl MongoGraphEngine {
    /// Create a new MongoDB graph engine
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            documents: HashMap::new(),
        }
    }

    /// Build a graph from a collection of documents
    ///
    /// # Arguments
    /// * `collection_docs` - Documents from the collection to build the graph from
    /// * `connect_from_field` - Field that contains the outgoing link value
    /// * `connect_to_field` - Field that contains the node identifier
    pub fn build_from_documents(
        &mut self,
        collection_docs: &[Document],
        connect_from_field: &str,
        connect_to_field: &str,
    ) {
        self.graph = Graph::new();
        self.documents.clear();

        // First pass: create nodes
        for doc in collection_docs {
            if let Some(node_id) = get_field_as_string(doc, connect_to_field) {
                // Convert document to properties
                let properties: HashMap<String, serde_json::Value> = doc
                    .iter()
                    .map(|(k, v)| (k.clone(), bson_to_json(v)))
                    .collect();

                self.graph.add_node(node_id.clone(), properties);
                self.documents.insert(node_id, doc.clone());
            }
        }

        // Second pass: create edges
        for doc in collection_docs {
            if let Some(from_val) = get_field_as_string(doc, connect_from_field) {
                if let Some(to_val) = get_field_as_string(doc, connect_to_field) {
                    // Edge goes from: node identified by connect_from_field value
                    //            to: node identified by connect_to_field value in this doc
                    // This matches MongoDB's $graphLookup semantics:
                    // - We traverse FROM the connectFromField value TO matching connectToField docs
                    self.graph
                        .add_edge(to_val.clone(), from_val.clone(), 1.0, None);
                }
            }
        }
    }

    /// Execute a graph lookup using BFS traversal
    ///
    /// This implements MongoDB's $graphLookup semantics using the shared
    /// graph algorithms module.
    pub fn graph_lookup(
        &self,
        start_value: &Bson,
        config: &GraphLookupConfig,
    ) -> GraphLookupResult {
        let start_key = bson_to_string(start_value);

        // Check if starting node exists
        if !self.graph.nodes.contains_key(&start_key) {
            // Node doesn't exist, return empty result
            return GraphLookupResult {
                documents: Vec::new(),
                depths: HashMap::new(),
            };
        }

        // Use BFS traversal from shared algorithms
        let max_depth = config.max_depth.map(|d| d + 1); // +1 because we want depth inclusive
        let traversal = graph_algo::bfs_traversal(&self.graph, &start_key, max_depth);

        // Build result
        let mut result = GraphLookupResult {
            documents: Vec::new(),
            depths: HashMap::new(),
        };

        for node_id in &traversal.visited {
            // Skip the start node itself (MongoDB behavior)
            if node_id == &start_key && config.max_depth.is_some_and(|_| true) {
                // Only skip if we have maxDepth set and it's the actual start
                // Actually, in MongoDB $graphLookup, the starting value matches are included
            }

            if let Some(doc) = self.documents.get(node_id) {
                let mut result_doc = doc.clone();

                // Add depth field if requested
                if let Some(ref depth_field) = config.depth_field {
                    let depth = traversal.depths.get(node_id).copied().unwrap_or(0);
                    result_doc.insert(depth_field.clone(), depth as i64);
                }

                // Apply restrictSearchWithMatch filter if present
                if let Some(ref filter) = config.restrict_search_with_match {
                    if !matches_filter(&result_doc, filter) {
                        continue;
                    }
                }

                result.documents.push(result_doc);
                if let Some(&depth) = traversal.depths.get(node_id) {
                    result.depths.insert(node_id.clone(), depth);
                }
            }
        }

        result
    }

    /// Execute a graph lookup for multiple start values
    pub fn graph_lookup_multi(
        &self,
        start_values: &[Bson],
        config: &GraphLookupConfig,
    ) -> GraphLookupResult {
        let mut combined_result = GraphLookupResult {
            documents: Vec::new(),
            depths: HashMap::new(),
        };
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for start_value in start_values {
            let result = self.graph_lookup(start_value, config);

            for doc in result.documents {
                // Get document ID to avoid duplicates
                let doc_id = get_field_as_string(&doc, &config.connect_to_field)
                    .unwrap_or_else(|| format!("{:?}", doc));

                if !seen.contains(&doc_id) {
                    seen.insert(doc_id.clone());
                    combined_result.documents.push(doc);

                    if let Some(depth) = result.depths.get(&doc_id) {
                        combined_result.depths.insert(doc_id, *depth);
                    }
                }
            }
        }

        combined_result
    }

    /// Find shortest path between two documents
    pub fn shortest_path(
        &self,
        from_value: &str,
        to_value: &str,
        weighted: bool,
    ) -> Option<Vec<Document>> {
        let path_result = if weighted {
            graph_algo::dijkstra(&self.graph, from_value, to_value)
        } else {
            graph_algo::bfs_shortest_path(&self.graph, from_value, to_value)
        };

        if path_result.found {
            Some(
                path_result
                    .path
                    .iter()
                    .filter_map(|id| self.documents.get(id).cloned())
                    .collect(),
            )
        } else {
            None
        }
    }

    /// Get neighbors of a document
    pub fn get_neighbors(&self, node_value: &str, direction: NeighborDirection) -> Vec<Document> {
        let neighbors = match direction {
            NeighborDirection::Out => self.graph.get_outgoing_neighbors(node_value),
            NeighborDirection::In => self.graph.get_incoming_neighbors(node_value),
            NeighborDirection::Both => self.graph.get_all_neighbors(node_value),
        };

        neighbors
            .iter()
            .filter_map(|(id, _, _)| self.documents.get(id).cloned())
            .collect()
    }
}

impl Default for MongoGraphEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Direction for neighbor queries
#[derive(Debug, Clone, Copy)]
pub enum NeighborDirection {
    Out,
    In,
    Both,
}

// Helper functions

fn get_field_as_string(doc: &Document, field: &str) -> Option<String> {
    doc.get(field).map(bson_to_string)
}

fn bson_to_string(value: &Bson) -> String {
    match value {
        Bson::String(s) => s.clone(),
        Bson::ObjectId(oid) => oid.to_hex(),
        Bson::Int32(i) => i.to_string(),
        Bson::Int64(i) => i.to_string(),
        Bson::Double(f) => f.to_string(),
        Bson::Boolean(b) => b.to_string(),
        _ => format!("{:?}", value),
    }
}

fn bson_to_json(value: &Bson) -> serde_json::Value {
    match value {
        Bson::Null => serde_json::Value::Null,
        Bson::Boolean(b) => serde_json::Value::Bool(*b),
        Bson::Int32(i) => serde_json::Value::Number((*i).into()),
        Bson::Int64(i) => serde_json::Value::Number((*i).into()),
        Bson::Double(f) => serde_json::json!(*f),
        Bson::String(s) => serde_json::Value::String(s.clone()),
        Bson::Array(arr) => serde_json::Value::Array(arr.iter().map(bson_to_json).collect()),
        Bson::Document(doc) => {
            let obj: serde_json::Map<String, serde_json::Value> = doc
                .iter()
                .map(|(k, v)| (k.clone(), bson_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        Bson::ObjectId(oid) => serde_json::Value::String(oid.to_hex()),
        _ => serde_json::Value::String(format!("{:?}", value)),
    }
}

fn matches_filter(doc: &Document, filter: &Document) -> bool {
    for (key, filter_val) in filter.iter() {
        if let Some(doc_val) = doc.get(key) {
            // Simple equality check
            if doc_val != filter_val {
                // Check for operator expressions
                if let Bson::Document(op_doc) = filter_val {
                    if !matches_operator(doc_val, op_doc) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        } else {
            return false;
        }
    }
    true
}

fn matches_operator(doc_val: &Bson, op_doc: &Document) -> bool {
    for (op, expected) in op_doc.iter() {
        match op.as_str() {
            "$eq" if doc_val != expected => {
                return false;
            }
            "$ne" if doc_val == expected => {
                return false;
            }
            "$gt" if !compare_bson(doc_val, expected, |a, b| a > b) => {
                return false;
            }
            "$gte" if !compare_bson(doc_val, expected, |a, b| a >= b) => {
                return false;
            }
            "$lt" if !compare_bson(doc_val, expected, |a, b| a < b) => {
                return false;
            }
            "$lte" if !compare_bson(doc_val, expected, |a, b| a <= b) => {
                return false;
            }
            "$in" => {
                if let Bson::Array(arr) = expected {
                    if !arr.contains(doc_val) {
                        return false;
                    }
                }
            }
            "$nin" => {
                if let Bson::Array(arr) = expected {
                    if arr.contains(doc_val) {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    true
}

fn compare_bson<F>(a: &Bson, b: &Bson, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    let a_num = bson_to_f64(a);
    let b_num = bson_to_f64(b);

    match (a_num, b_num) {
        (Some(a), Some(b)) => cmp(a, b),
        _ => false,
    }
}

fn bson_to_f64(value: &Bson) -> Option<f64> {
    match value {
        Bson::Int32(i) => Some(*i as f64),
        Bson::Int64(i) => Some(*i as f64),
        Bson::Double(f) => Some(*f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    fn create_test_docs() -> Vec<Document> {
        vec![
            doc! { "_id": "1", "name": "Alice", "reports_to": "2" },
            doc! { "_id": "2", "name": "Bob", "reports_to": "3" },
            doc! { "_id": "3", "name": "Charlie", "reports_to": null },
        ]
    }

    #[test]
    fn test_build_graph() {
        let mut engine = MongoGraphEngine::new();
        let docs = create_test_docs();

        engine.build_from_documents(&docs, "reports_to", "_id");

        assert_eq!(engine.graph.nodes.len(), 3);
    }

    #[test]
    fn test_graph_lookup() {
        let mut engine = MongoGraphEngine::new();
        let docs = create_test_docs();

        engine.build_from_documents(&docs, "reports_to", "_id");

        let config = GraphLookupConfig {
            from: "employees".to_string(),
            start_with_field: None,
            connect_from_field: "reports_to".to_string(),
            connect_to_field: "_id".to_string(),
            as_field: "hierarchy".to_string(),
            max_depth: Some(5),
            depth_field: Some("depth".to_string()),
            restrict_search_with_match: None,
        };

        let result = engine.graph_lookup(&Bson::String("1".to_string()), &config);
        assert!(!result.documents.is_empty());
    }

    #[test]
    fn test_shortest_path() {
        let mut engine = MongoGraphEngine::new();
        let docs = create_test_docs();

        engine.build_from_documents(&docs, "reports_to", "_id");

        let path = engine.shortest_path("1", "3", false);
        assert!(path.is_some());
    }
}
