//! Graph command handlers for Redis RESP protocol
//!
//! This module implements RedisGraph-compatible commands:
//! - GRAPH.QUERY - Execute a Cypher query
//! - GRAPH.EXPLAIN - Explain query execution plan
//! - GRAPH.PROFILE - Profile query execution
//! - GRAPH.DELETE - Delete a graph
//! - GRAPH.LIST - List all graphs
//! - GRAPH.CONFIG GET/SET - Configure graph settings

#![allow(dead_code)]

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Node in the graph
#[derive(Debug, Clone)]
struct GraphNode {
    id: u64,
    labels: Vec<String>,
    properties: HashMap<String, serde_json::Value>,
}

/// Relationship in the graph
#[derive(Debug, Clone)]
struct GraphRelationship {
    id: u64,
    src_id: u64,
    dest_id: u64,
    rel_type: String,
    properties: HashMap<String, serde_json::Value>,
}

/// Graph storage
struct Graph {
    name: String,
    nodes: HashMap<u64, GraphNode>,
    relationships: HashMap<u64, GraphRelationship>,
    next_node_id: u64,
    next_rel_id: u64,
    /// Node label index: label -> node IDs
    label_index: HashMap<String, Vec<u64>>,
    /// Relationship type index: type -> relationship IDs
    rel_type_index: HashMap<String, Vec<u64>>,
    /// Outgoing relationships: node_id -> relationship IDs
    outgoing_rels: HashMap<u64, Vec<u64>>,
    /// Incoming relationships: node_id -> relationship IDs
    incoming_rels: HashMap<u64, Vec<u64>>,
}

impl Graph {
    fn new(name: String) -> Self {
        Self {
            name,
            nodes: HashMap::new(),
            relationships: HashMap::new(),
            next_node_id: 1,
            next_rel_id: 1,
            label_index: HashMap::new(),
            rel_type_index: HashMap::new(),
            outgoing_rels: HashMap::new(),
            incoming_rels: HashMap::new(),
        }
    }

    /// Create a node with labels and properties
    fn create_node(
        &mut self,
        labels: Vec<String>,
        properties: HashMap<String, serde_json::Value>,
    ) -> u64 {
        let id = self.next_node_id;
        self.next_node_id += 1;

        // Update label index
        for label in &labels {
            self.label_index
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(id);
        }

        self.nodes.insert(
            id,
            GraphNode {
                id,
                labels,
                properties,
            },
        );

        id
    }

    /// Create a relationship between two nodes
    fn create_relationship(
        &mut self,
        src_id: u64,
        dest_id: u64,
        rel_type: String,
        properties: HashMap<String, serde_json::Value>,
    ) -> Option<u64> {
        // Verify both nodes exist
        if !self.nodes.contains_key(&src_id) || !self.nodes.contains_key(&dest_id) {
            return None;
        }

        let id = self.next_rel_id;
        self.next_rel_id += 1;

        // Update relationship type index
        self.rel_type_index
            .entry(rel_type.clone())
            .or_insert_with(Vec::new)
            .push(id);

        // Update adjacency lists
        self.outgoing_rels
            .entry(src_id)
            .or_insert_with(Vec::new)
            .push(id);
        self.incoming_rels
            .entry(dest_id)
            .or_insert_with(Vec::new)
            .push(id);

        self.relationships.insert(
            id,
            GraphRelationship {
                id,
                src_id,
                dest_id,
                rel_type,
                properties,
            },
        );

        Some(id)
    }

    /// Find nodes by label
    fn find_nodes_by_label(&self, label: &str) -> Vec<&GraphNode> {
        self.label_index
            .get(label)
            .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
            .unwrap_or_else(Vec::new)
    }

    /// Get outgoing relationships for a node
    fn get_outgoing_relationships(&self, node_id: u64) -> Vec<&GraphRelationship> {
        self.outgoing_rels
            .get(&node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.relationships.get(id))
                    .collect()
            })
            .unwrap_or_else(Vec::new)
    }

    /// Get incoming relationships for a node
    fn get_incoming_relationships(&self, node_id: u64) -> Vec<&GraphRelationship> {
        self.incoming_rels
            .get(&node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.relationships.get(id))
                    .collect()
            })
            .unwrap_or_else(Vec::new)
    }

    /// Delete a node and its relationships
    fn delete_node(&mut self, node_id: u64) -> bool {
        if let Some(node) = self.nodes.remove(&node_id) {
            // Remove from label index
            for label in &node.labels {
                if let Some(ids) = self.label_index.get_mut(label) {
                    ids.retain(|id| *id != node_id);
                }
            }

            // Remove all relationships involving this node
            let out_rels: Vec<u64> = self.outgoing_rels.remove(&node_id).unwrap_or_default();
            let in_rels: Vec<u64> = self.incoming_rels.remove(&node_id).unwrap_or_default();

            for rel_id in out_rels.into_iter().chain(in_rels.into_iter()) {
                if let Some(rel) = self.relationships.remove(&rel_id) {
                    // Clean up relationship type index
                    if let Some(ids) = self.rel_type_index.get_mut(&rel.rel_type) {
                        ids.retain(|id| *id != rel_id);
                    }
                    // Clean up other node's adjacency lists
                    if rel.src_id != node_id {
                        if let Some(ids) = self.outgoing_rels.get_mut(&rel.src_id) {
                            ids.retain(|id| *id != rel_id);
                        }
                    }
                    if rel.dest_id != node_id {
                        if let Some(ids) = self.incoming_rels.get_mut(&rel.dest_id) {
                            ids.retain(|id| *id != rel_id);
                        }
                    }
                }
            }

            true
        } else {
            false
        }
    }

    /// Get statistics about the graph
    fn stats(&self) -> HashMap<String, i64> {
        let mut stats = HashMap::new();
        stats.insert("node_count".to_string(), self.nodes.len() as i64);
        stats.insert(
            "relationship_count".to_string(),
            self.relationships.len() as i64,
        );
        stats.insert("label_count".to_string(), self.label_index.len() as i64);
        stats.insert(
            "relationship_type_count".to_string(),
            self.rel_type_index.len() as i64,
        );
        stats
    }
}

/// Global graph storage
static GRAPHS: std::sync::OnceLock<Arc<RwLock<HashMap<String, Graph>>>> =
    std::sync::OnceLock::new();

fn get_graphs() -> &'static Arc<RwLock<HashMap<String, Graph>>> {
    GRAPHS.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// Handler for graph commands
pub struct GraphCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
}

impl GraphCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    /// GRAPH.QUERY graph_key query [--compact] [timeout ms]
    async fn cmd_graph_query(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'graph.query' command".to_string(),
            ));
        }

        let graph_key = self.get_string_arg(args, 0, "GRAPH.QUERY")?;
        let query = self.get_string_arg(args, 1, "GRAPH.QUERY")?;

        let graphs = get_graphs();
        let mut graphs_guard = graphs.write().await;

        // Auto-create graph if it doesn't exist
        if !graphs_guard.contains_key(&graph_key) {
            graphs_guard.insert(graph_key.clone(), Graph::new(graph_key.clone()));
        }

        let graph = graphs_guard.get_mut(&graph_key).unwrap();

        // Parse and execute the Cypher query (simplified)
        let result = self.execute_cypher(graph, &query)?;

        debug!("GRAPH.QUERY {} {} -> {:?}", graph_key, query, result);
        Ok(result)
    }

    /// Execute a simplified Cypher query
    fn execute_cypher(&self, graph: &mut Graph, query: &str) -> ProtocolResult<RespValue> {
        let query_upper = query.to_uppercase();

        // Simple CREATE pattern: CREATE (n:Label {prop: value})
        if query_upper.starts_with("CREATE") {
            return self.execute_create(graph, query);
        }

        // Simple MATCH pattern: MATCH (n:Label) RETURN n
        if query_upper.starts_with("MATCH") {
            return self.execute_match(graph, query);
        }

        // DELETE pattern
        if query_upper.contains("DELETE") {
            return self.execute_delete(graph, query);
        }

        // Return empty for unsupported queries
        Ok(RespValue::Array(vec![
            RespValue::Array(vec![]), // Empty header
            RespValue::Array(vec![]), // Empty results
            RespValue::Array(vec![
                // Statistics
                RespValue::bulk_string_from_str("Query internal execution time: 0.1 ms"),
            ]),
        ]))
    }

    /// Execute CREATE statement
    fn execute_create(&self, graph: &mut Graph, query: &str) -> ProtocolResult<RespValue> {
        // Simple parser for: CREATE (n:Label {key: value, ...})
        let mut nodes_created = 0;
        let rels_created = 0;
        let mut properties_set = 0;

        // Extract node patterns
        let mut i = 0;
        let chars: Vec<char> = query.chars().collect();

        while i < chars.len() {
            if chars[i] == '(' {
                // Found node pattern start
                let start = i;
                let mut depth = 1;
                i += 1;

                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }

                let pattern: String = chars[start..i].iter().collect();
                let (labels, properties) = self.parse_node_pattern(&pattern);

                properties_set += properties.len();
                graph.create_node(labels, properties);
                nodes_created += 1;
            } else {
                i += 1;
            }
        }

        Ok(RespValue::Array(vec![
            RespValue::Array(vec![]), // Empty header
            RespValue::Array(vec![]), // Empty results
            RespValue::Array(vec![
                RespValue::bulk_string_from_str(&format!("Nodes created: {}", nodes_created)),
                RespValue::bulk_string_from_str(&format!(
                    "Relationships created: {}",
                    rels_created
                )),
                RespValue::bulk_string_from_str(&format!("Properties set: {}", properties_set)),
            ]),
        ]))
    }

    /// Parse a node pattern like (n:Label {key: value})
    fn parse_node_pattern(
        &self,
        pattern: &str,
    ) -> (Vec<String>, HashMap<String, serde_json::Value>) {
        let mut labels = Vec::new();
        let mut properties = HashMap::new();

        // Remove parentheses
        let inner = pattern.trim_start_matches('(').trim_end_matches(')');

        // Find where properties start (if any)
        let (label_part, props_part) = if let Some(brace_pos) = inner.find('{') {
            (&inner[..brace_pos], Some(&inner[brace_pos..]))
        } else {
            (inner, None)
        };

        // Parse labels - split by : but only in the label part
        let parts: Vec<&str> = label_part.split(':').collect();
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                continue; // Skip variable name
            }
            let label = part.trim();
            if !label.is_empty() {
                labels.push(label.to_string());
            }
        }

        // Parse properties if present
        if let Some(props_str) = props_part {
            let props_inner = props_str.trim_start_matches('{').trim_end_matches('}');
            for prop in props_inner.split(',') {
                if let Some(colon_pos) = prop.find(':') {
                    let key = prop[..colon_pos].trim().to_string();
                    let value_str = prop[colon_pos + 1..].trim();

                    // Parse value
                    let value = if value_str.starts_with('\'') || value_str.starts_with('"') {
                        let s = value_str.trim_matches('\'').trim_matches('"').to_string();
                        serde_json::Value::String(s)
                    } else if let Ok(n) = value_str.parse::<i64>() {
                        serde_json::Value::Number(n.into())
                    } else if let Ok(f) = value_str.parse::<f64>() {
                        serde_json::json!(f)
                    } else if value_str == "true" {
                        serde_json::Value::Bool(true)
                    } else if value_str == "false" {
                        serde_json::Value::Bool(false)
                    } else {
                        serde_json::Value::String(value_str.to_string())
                    };

                    properties.insert(key, value);
                }
            }
        }

        (labels, properties)
    }

    /// Execute MATCH statement
    fn execute_match(&self, graph: &Graph, query: &str) -> ProtocolResult<RespValue> {
        // Simple parser for: MATCH (n:Label) RETURN n
        let _query_upper = query.to_uppercase();

        // Find label in pattern
        let mut results = Vec::new();
        let header = vec![RespValue::bulk_string_from_str("n")];

        // Extract label from pattern like (n:Label)
        if let Some(colon_pos) = query.find(':') {
            let after_colon = &query[colon_pos + 1..];
            let label_end = after_colon
                .find(|c: char| c == ')' || c == ' ' || c == '{')
                .unwrap_or(after_colon.len());
            let label = &after_colon[..label_end];

            // Find nodes with this label
            for node in graph.find_nodes_by_label(label) {
                let mut node_arr = Vec::new();

                // Build node representation
                let mut node_map = HashMap::new();
                node_map.insert("id".to_string(), serde_json::Value::Number(node.id.into()));
                node_map.insert(
                    "labels".to_string(),
                    serde_json::Value::Array(
                        node.labels
                            .iter()
                            .map(|l| serde_json::Value::String(l.clone()))
                            .collect(),
                    ),
                );
                node_map.insert("properties".to_string(), serde_json::json!(node.properties));

                let node_str = serde_json::to_string(&node_map).unwrap_or_default();
                node_arr.push(RespValue::bulk_string_from_str(&node_str));

                results.push(RespValue::Array(node_arr));
            }
        }

        Ok(RespValue::Array(vec![
            RespValue::Array(header),
            RespValue::Array(results.clone()),
            RespValue::Array(vec![RespValue::bulk_string_from_str(&format!(
                "Query internal execution time: 0.1 ms, {} results",
                results.len()
            ))]),
        ]))
    }

    /// Execute DELETE statement
    fn execute_delete(&self, _graph: &mut Graph, _query: &str) -> ProtocolResult<RespValue> {
        // For now, this is a placeholder
        // A full implementation would parse the DELETE pattern

        Ok(RespValue::Array(vec![
            RespValue::Array(vec![]),
            RespValue::Array(vec![]),
            RespValue::Array(vec![
                RespValue::bulk_string_from_str("Nodes deleted: 0"),
                RespValue::bulk_string_from_str("Relationships deleted: 0"),
            ]),
        ]))
    }

    /// GRAPH.EXPLAIN graph_key query
    async fn cmd_graph_explain(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'graph.explain' command".to_string(),
            ));
        }

        let graph_key = self.get_string_arg(args, 0, "GRAPH.EXPLAIN")?;
        let query = self.get_string_arg(args, 1, "GRAPH.EXPLAIN")?;

        // Return a simplified execution plan
        let plan = vec![
            format!("Results"),
            format!("    Project"),
            format!("        Filter"),
            format!("            Node By Label Scan | (n:*)"),
        ];

        let plan_resp: Vec<RespValue> = plan
            .iter()
            .map(|s| RespValue::bulk_string_from_str(s))
            .collect();

        debug!("GRAPH.EXPLAIN {} {}", graph_key, query);
        Ok(RespValue::Array(plan_resp))
    }

    /// GRAPH.PROFILE graph_key query
    async fn cmd_graph_profile(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'graph.profile' command".to_string(),
            ));
        }

        let graph_key = self.get_string_arg(args, 0, "GRAPH.PROFILE")?;
        let query = self.get_string_arg(args, 1, "GRAPH.PROFILE")?;

        // Execute query and return with profiling info
        let graphs = get_graphs();
        let mut graphs_guard = graphs.write().await;

        if !graphs_guard.contains_key(&graph_key) {
            graphs_guard.insert(graph_key.clone(), Graph::new(graph_key.clone()));
        }

        let graph = graphs_guard.get_mut(&graph_key).unwrap();
        let start = std::time::Instant::now();
        let result = self.execute_cypher(graph, &query)?;
        let elapsed = start.elapsed();

        // Add profiling info
        let profile = vec![
            format!(
                "Results | Records produced: 0, Execution time: {:.3} ms",
                elapsed.as_secs_f64() * 1000.0
            ),
            format!("    Project | Records produced: 0"),
            format!("        Filter | Records produced: 0"),
            format!("            Node By Label Scan | Records produced: 0"),
        ];

        let mut response = Vec::new();
        for line in profile {
            response.push(RespValue::bulk_string_from_str(&line));
        }

        debug!("GRAPH.PROFILE {} {} -> {:?}", graph_key, query, result);
        Ok(RespValue::Array(response))
    }

    /// GRAPH.DELETE graph_key
    async fn cmd_graph_delete(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("GRAPH.DELETE", args, 1)?;

        let graph_key = self.get_string_arg(args, 0, "GRAPH.DELETE")?;

        let graphs = get_graphs();
        let mut graphs_guard = graphs.write().await;

        let removed = graphs_guard.remove(&graph_key).is_some();

        info!("GRAPH.DELETE {} -> {}", graph_key, removed);
        Ok(RespValue::ok())
    }

    /// GRAPH.LIST
    async fn cmd_graph_list(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        let graphs = get_graphs();
        let graphs_guard = graphs.read().await;

        let graph_names: Vec<RespValue> = graphs_guard
            .keys()
            .map(|k| RespValue::bulk_string_from_str(k))
            .collect();

        debug!("GRAPH.LIST -> {} graphs", graph_names.len());
        Ok(RespValue::Array(graph_names))
    }

    /// GRAPH.CONFIG GET name | GRAPH.CONFIG SET name value
    async fn cmd_graph_config(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'graph.config' command".to_string(),
            ));
        }

        let subcommand = self.get_string_arg(args, 0, "GRAPH.CONFIG")?.to_uppercase();

        match subcommand.as_str() {
            "GET" => {
                if args.len() < 2 {
                    return Err(crate::protocols::error::ProtocolError::RespError(
                        "ERR wrong number of arguments for 'graph.config get'".to_string(),
                    ));
                }

                let config_name = self.get_string_arg(args, 1, "GRAPH.CONFIG")?;

                // Return default config values
                let value = match config_name.to_uppercase().as_str() {
                    "TIMEOUT" => "0", // No timeout
                    "RESULTSET_SIZE" => "10000",
                    "MAX_QUEUED_QUERIES" => "25",
                    "THREAD_COUNT" => "4",
                    _ => "unknown",
                };

                Ok(RespValue::Array(vec![
                    RespValue::bulk_string_from_str(&config_name),
                    RespValue::bulk_string_from_str(value),
                ]))
            }
            "SET" => {
                if args.len() < 3 {
                    return Err(crate::protocols::error::ProtocolError::RespError(
                        "ERR wrong number of arguments for 'graph.config set'".to_string(),
                    ));
                }

                // Config setting is acknowledged but not persisted in this implementation
                Ok(RespValue::ok())
            }
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown subcommand '{}'",
                subcommand
            ))),
        }
    }

    /// GRAPH.SLOWLOG graph_key
    async fn cmd_graph_slowlog(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("GRAPH.SLOWLOG", args, 1)?;

        // Return empty slow log
        Ok(RespValue::Array(vec![]))
    }
}

#[async_trait]
impl CommandHandler for GraphCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "GRAPH.QUERY" => self.cmd_graph_query(args).await,
            "GRAPH.EXPLAIN" => self.cmd_graph_explain(args).await,
            "GRAPH.PROFILE" => self.cmd_graph_profile(args).await,
            "GRAPH.DELETE" => self.cmd_graph_delete(args).await,
            "GRAPH.LIST" => self.cmd_graph_list(args).await,
            "GRAPH.CONFIG" => self.cmd_graph_config(args).await,
            "GRAPH.SLOWLOG" => self.cmd_graph_slowlog(args).await,
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown graph command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "GRAPH.QUERY",
            "GRAPH.EXPLAIN",
            "GRAPH.PROFILE",
            "GRAPH.DELETE",
            "GRAPH.LIST",
            "GRAPH.CONFIG",
            "GRAPH.SLOWLOG",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_create_node() {
        let mut graph = Graph::new("test".to_string());

        let mut props = HashMap::new();
        props.insert("name".to_string(), serde_json::json!("Alice"));

        let id = graph.create_node(vec!["Person".to_string()], props);
        assert_eq!(id, 1);

        let nodes = graph.find_nodes_by_label("Person");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, 1);
    }

    #[test]
    fn test_graph_create_relationship() {
        let mut graph = Graph::new("test".to_string());

        let id1 = graph.create_node(vec!["Person".to_string()], HashMap::new());
        let id2 = graph.create_node(vec!["Person".to_string()], HashMap::new());

        let rel_id = graph
            .create_relationship(id1, id2, "KNOWS".to_string(), HashMap::new())
            .unwrap();
        assert_eq!(rel_id, 1);

        let out_rels = graph.get_outgoing_relationships(id1);
        assert_eq!(out_rels.len(), 1);
        assert_eq!(out_rels[0].rel_type, "KNOWS");

        let in_rels = graph.get_incoming_relationships(id2);
        assert_eq!(in_rels.len(), 1);
    }

    #[test]
    fn test_graph_stats() {
        let mut graph = Graph::new("test".to_string());

        graph.create_node(vec!["Person".to_string()], HashMap::new());
        graph.create_node(vec!["Person".to_string()], HashMap::new());
        graph.create_node(vec!["Company".to_string()], HashMap::new());

        let stats = graph.stats();
        assert_eq!(stats["node_count"], 3);
        assert_eq!(stats["label_count"], 2);
    }

    #[test]
    fn test_parse_node_pattern() {
        let handler = GraphCommands {
            base: BaseCommandHandler::new(
                Arc::new(orbit_client::OrbitClient::new(
                    "http://127.0.0.1:50051".to_string(),
                )),
                Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new()),
            ),
        };

        let (labels, props) = handler.parse_node_pattern("(n:Person {name: 'Alice', age: 30})");
        assert!(labels.contains(&"Person".to_string()));
        assert_eq!(props.get("name"), Some(&serde_json::json!("Alice")));
        assert_eq!(props.get("age"), Some(&serde_json::json!(30)));
    }
}
