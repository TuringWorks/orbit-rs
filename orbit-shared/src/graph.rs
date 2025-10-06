//! Core graph database types and storage traits
//!
//! This module provides the fundamental data structures and traits for implementing
//! a distributed graph database within the Orbit actor system. It defines graph
//! nodes, relationships, and storage operations that can be distributed across
//! cluster nodes.

use crate::exception::OrbitResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub mod storage;

pub use storage::InMemoryGraphStorage;

/// Unique identifier for a graph node
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
    /// Create a new node ID
    pub fn new(id: String) -> Self {
        Self(id)
    }

    /// Create from string reference
    pub fn from_string(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new(format!("node_{}", Uuid::new_v4()))
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a graph relationship
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationshipId(pub String);

impl RelationshipId {
    /// Create a new random relationship ID
    pub fn new() -> Self {
        Self(format!("rel_{}", Uuid::new_v4()))
    }

    /// Create a relationship ID from a string
    pub fn from_string(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RelationshipId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RelationshipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Direction of graph traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Follow outgoing relationships
    Outgoing,
    /// Follow incoming relationships
    Incoming,
    /// Follow both directions
    Both,
}

/// Graph node with labels and properties
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique node identifier
    pub id: NodeId,
    /// Node labels (e.g., ["Person", "Employee"])
    pub labels: Vec<String>,
    /// Key-value properties
    pub properties: HashMap<String, serde_json::Value>,
    /// Creation timestamp
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last updated timestamp
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl GraphNode {
    /// Create a new graph node
    pub fn new(labels: Vec<String>, properties: HashMap<String, serde_json::Value>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: NodeId::default(),
            labels,
            properties,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new graph node with specific ID
    pub fn with_id(
        id: NodeId,
        labels: Vec<String>,
        properties: HashMap<String, serde_json::Value>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            labels,
            properties,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if node has a specific label
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.contains(&label.to_string())
    }

    /// Add labels to the node
    pub fn add_labels(&mut self, labels: Vec<String>) {
        for label in labels {
            if !self.labels.contains(&label) {
                self.labels.push(label);
            }
        }
        self.updated_at = chrono::Utc::now();
    }

    /// Remove labels from the node
    pub fn remove_labels(&mut self, labels: Vec<String>) {
        self.labels.retain(|l| !labels.contains(l));
        self.updated_at = chrono::Utc::now();
    }

    /// Set a property value
    pub fn set_property<V: Into<serde_json::Value>>(&mut self, key: &str, value: V) {
        self.properties.insert(key.to_string(), value.into());
        self.updated_at = chrono::Utc::now();
    }

    /// Get a property value
    pub fn get_property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }

    /// Remove a property
    pub fn remove_property(&mut self, key: &str) -> Option<serde_json::Value> {
        self.updated_at = chrono::Utc::now();
        self.properties.remove(key)
    }
}

/// Graph relationship connecting two nodes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphRelationship {
    /// Unique relationship identifier
    pub id: RelationshipId,
    /// Source node ID
    pub start_node: NodeId,
    /// Target node ID
    pub end_node: NodeId,
    /// Relationship type (e.g., "KNOWS", "WORKS_FOR")
    pub rel_type: String,
    /// Key-value properties
    pub properties: HashMap<String, serde_json::Value>,
    /// Creation timestamp
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last updated timestamp
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl GraphRelationship {
    /// Create a new relationship
    pub fn new(
        start_node: NodeId,
        end_node: NodeId,
        rel_type: String,
        properties: HashMap<String, serde_json::Value>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: RelationshipId::new(),
            start_node,
            end_node,
            rel_type,
            properties,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new relationship with specific ID
    pub fn with_id(
        id: RelationshipId,
        start_node: NodeId,
        end_node: NodeId,
        rel_type: String,
        properties: HashMap<String, serde_json::Value>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            start_node,
            end_node,
            rel_type,
            properties,
            created_at: now,
            updated_at: now,
        }
    }

    /// Set a property value
    pub fn set_property<V: Into<serde_json::Value>>(&mut self, key: &str, value: V) {
        self.properties.insert(key.to_string(), value.into());
        self.updated_at = chrono::Utc::now();
    }

    /// Get a property value
    pub fn get_property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }

    /// Remove a property
    pub fn remove_property(&mut self, key: &str) -> Option<serde_json::Value> {
        self.updated_at = chrono::Utc::now();
        self.properties.remove(key)
    }
}

/// Trait for graph storage operations
#[async_trait]
pub trait GraphStorage: Send + Sync {
    /// Create a new node
    async fn create_node(
        &self,
        labels: Vec<String>,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<GraphNode>;

    /// Get a node by ID
    async fn get_node(&self, node_id: &NodeId) -> OrbitResult<Option<GraphNode>>;

    /// Update node properties
    async fn update_node(
        &self,
        node_id: &NodeId,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<()>;

    /// Delete a node and all its relationships
    async fn delete_node(&self, node_id: &NodeId) -> OrbitResult<bool>;

    /// Add labels to a node
    async fn add_labels(&self, node_id: &NodeId, labels: Vec<String>) -> OrbitResult<()>;

    /// Remove labels from a node
    async fn remove_labels(&self, node_id: &NodeId, labels: Vec<String>) -> OrbitResult<()>;

    /// Get all labels for a node
    async fn get_labels(&self, node_id: &NodeId) -> OrbitResult<Vec<String>>;

    /// Create a relationship between two nodes
    async fn create_relationship(
        &self,
        start_node: &NodeId,
        end_node: &NodeId,
        rel_type: String,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<GraphRelationship>;

    /// Get relationships for a node
    async fn get_relationships(
        &self,
        node_id: &NodeId,
        direction: Direction,
        rel_types: Option<Vec<String>>,
    ) -> OrbitResult<Vec<GraphRelationship>>;

    /// Get a relationship by ID
    async fn get_relationship(
        &self,
        rel_id: &RelationshipId,
    ) -> OrbitResult<Option<GraphRelationship>>;

    /// Update relationship properties
    async fn update_relationship(
        &self,
        rel_id: &RelationshipId,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<()>;

    /// Delete a relationship
    async fn delete_relationship(&self, rel_id: &RelationshipId) -> OrbitResult<bool>;

    /// Find nodes by label and optional property filters
    async fn find_nodes_by_label(
        &self,
        label: &str,
        property_filters: Option<HashMap<String, serde_json::Value>>,
        limit: Option<usize>,
    ) -> OrbitResult<Vec<GraphNode>>;

    /// Count nodes with specific label
    async fn count_nodes_by_label(&self, label: &str) -> OrbitResult<u64>;

    /// Count relationships of specific type
    async fn count_relationships_by_type(&self, rel_type: &str) -> OrbitResult<u64>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let labels = vec!["Person".to_string(), "Employee".to_string()];
        let mut properties = HashMap::new();
        properties.insert(
            "name".to_string(),
            serde_json::Value::String("Alice".to_string()),
        );
        properties.insert(
            "age".to_string(),
            serde_json::Value::Number(serde_json::Number::from(30)),
        );

        let node = GraphNode::new(labels.clone(), properties.clone());

        assert_eq!(node.labels, labels);
        assert_eq!(node.properties, properties);
        assert!(node.has_label("Person"));
        assert!(node.has_label("Employee"));
        assert!(!node.has_label("Manager"));
    }

    #[test]
    fn test_node_label_operations() {
        let mut node = GraphNode::new(vec!["Person".to_string()], HashMap::new());

        // Add labels
        node.add_labels(vec!["Employee".to_string(), "Manager".to_string()]);
        assert!(node.has_label("Person"));
        assert!(node.has_label("Employee"));
        assert!(node.has_label("Manager"));

        // Remove labels
        node.remove_labels(vec!["Employee".to_string()]);
        assert!(node.has_label("Person"));
        assert!(!node.has_label("Employee"));
        assert!(node.has_label("Manager"));
    }

    #[test]
    fn test_node_property_operations() {
        let mut node = GraphNode::new(vec!["Person".to_string()], HashMap::new());

        // Set properties
        node.set_property("name", "Alice");
        node.set_property("age", 30);

        assert_eq!(
            node.get_property("name"),
            Some(&serde_json::Value::String("Alice".to_string()))
        );
        assert_eq!(
            node.get_property("age"),
            Some(&serde_json::Value::Number(serde_json::Number::from(30)))
        );

        // Remove property
        let removed = node.remove_property("age");
        assert_eq!(
            removed,
            Some(serde_json::Value::Number(serde_json::Number::from(30)))
        );
        assert_eq!(node.get_property("age"), None);
    }

    #[test]
    fn test_relationship_creation() {
        let start_node = NodeId::from_string("node1");
        let end_node = NodeId::from_string("node2");
        let mut properties = HashMap::new();
        properties.insert(
            "since".to_string(),
            serde_json::Value::String("2023".to_string()),
        );

        let relationship = GraphRelationship::new(
            start_node.clone(),
            end_node.clone(),
            "KNOWS".to_string(),
            properties.clone(),
        );

        assert_eq!(relationship.start_node, start_node);
        assert_eq!(relationship.end_node, end_node);
        assert_eq!(relationship.rel_type, "KNOWS");
        assert_eq!(relationship.properties, properties);
    }

    #[test]
    fn test_relationship_property_operations() {
        let start_node = NodeId::from_string("node1");
        let end_node = NodeId::from_string("node2");
        let mut relationship =
            GraphRelationship::new(start_node, end_node, "KNOWS".to_string(), HashMap::new());

        // Set properties
        relationship.set_property("strength", 0.8);
        relationship.set_property("verified", true);

        assert_eq!(
            relationship.get_property("strength"),
            Some(&serde_json::Value::Number(
                serde_json::Number::from_f64(0.8).unwrap()
            ))
        );
        assert_eq!(
            relationship.get_property("verified"),
            Some(&serde_json::Value::Bool(true))
        );

        // Remove property
        let removed = relationship.remove_property("verified");
        assert_eq!(removed, Some(serde_json::Value::Bool(true)));
        assert_eq!(relationship.get_property("verified"), None);
    }
}
