//! AQL data model types for multi-model database operations
//!
//! This module defines the core data types used in AQL operations,
//! supporting document, graph, and key-value data models.

use chrono::{DateTime, Utc};
use orbit_shared::graph::{GraphNode, GraphRelationship};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AQL value type supporting all ArangoDB data types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AqlValue {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Number value (integer or float)
    Number(serde_json::Number),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<AqlValue>),
    /// Object with key-value pairs
    Object(HashMap<String, AqlValue>),
    /// Date/time value
    DateTime(DateTime<Utc>),
}

impl From<serde_json::Value> for AqlValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => AqlValue::Null,
            serde_json::Value::Bool(b) => AqlValue::Bool(b),
            serde_json::Value::Number(n) => AqlValue::Number(n),
            serde_json::Value::String(s) => AqlValue::String(s),
            serde_json::Value::Array(arr) => {
                AqlValue::Array(arr.into_iter().map(AqlValue::from).collect())
            }
            serde_json::Value::Object(obj) => AqlValue::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, AqlValue::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<AqlValue> for serde_json::Value {
    fn from(value: AqlValue) -> Self {
        match value {
            AqlValue::Null => serde_json::Value::Null,
            AqlValue::Bool(b) => serde_json::Value::Bool(b),
            AqlValue::Number(n) => serde_json::Value::Number(n),
            AqlValue::String(s) => serde_json::Value::String(s),
            AqlValue::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(serde_json::Value::from).collect())
            }
            AqlValue::Object(obj) => serde_json::Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
            AqlValue::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
        }
    }
}

/// AQL document representing a single document in a collection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AqlDocument {
    /// Document key (unique within collection)
    #[serde(rename = "_key")]
    pub key: String,
    /// Document ID (collection/key)
    #[serde(rename = "_id")]
    pub id: String,
    /// Document revision
    #[serde(rename = "_rev")]
    pub revision: String,
    /// Document data
    #[serde(flatten)]
    pub data: HashMap<String, AqlValue>,
}

impl AqlDocument {
    /// Create a new AQL document
    pub fn new(collection: &str, key: String, data: HashMap<String, AqlValue>) -> Self {
        let id = format!("{}/{}", collection, key);
        let revision = format!("_{}", chrono::Utc::now().timestamp());

        Self {
            key,
            id,
            revision,
            data,
        }
    }

    /// Get a field value by name
    pub fn get(&self, field_name: &str) -> Option<AqlValue> {
        match field_name {
            "_key" => Some(AqlValue::String(self.key.clone())),
            "_id" => Some(AqlValue::String(self.id.clone())),
            "_rev" => Some(AqlValue::String(self.revision.clone())),
            _ => self.data.get(field_name).cloned(),
        }
    }

    /// Set a field value in the document
    pub fn set(&mut self, field: String, value: AqlValue) {
        self.data.insert(field, value);
    }

    /// Convert to JSON value
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "_key".to_string(),
            serde_json::Value::String(self.key.clone()),
        );
        obj.insert(
            "_id".to_string(),
            serde_json::Value::String(self.id.clone()),
        );
        obj.insert(
            "_rev".to_string(),
            serde_json::Value::String(self.revision.clone()),
        );

        for (key, value) in &self.data {
            obj.insert(key.clone(), serde_json::Value::from(value.clone()));
        }

        serde_json::Value::Object(obj)
    }
}

/// AQL collection metadata and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AqlCollection {
    /// Collection name
    pub name: String,
    /// Collection type (document or edge)
    pub collection_type: CollectionType,
    /// Collection status
    pub status: CollectionStatus,
    /// Number of documents
    pub count: u64,
    /// Collection indexes
    pub indexes: Vec<AqlIndex>,
}

/// Collection type enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CollectionType {
    /// Document collection
    Document,
    /// Edge collection (for graphs)
    Edge,
}

/// Collection status enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CollectionStatus {
    /// Collection is being created
    NewBorn,
    /// Collection is being unloaded
    Unloading,
    /// Collection is fully loaded and operational
    Loaded,
    /// Collection is being loaded
    Loading,
    /// Collection is being deleted
    Deleted,
}

/// AQL index for optimizing queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AqlIndex {
    /// Index ID
    pub id: String,
    /// Index type
    pub index_type: IndexType,
    /// Fields included in the index
    pub fields: Vec<String>,
    /// Whether the index is unique
    pub unique: bool,
    /// Whether the index is sparse
    pub sparse: bool,
}

/// Index type enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IndexType {
    /// Primary key index
    Primary,
    /// Hash index for equality lookups
    Hash,
    /// Skip list index for range queries
    SkipList,
    /// Persistent index for sorted access
    Persistent,
    /// Full-text search index
    Fulltext,
    /// Geospatial index
    Geo,
    /// TTL (time-to-live) index
    Ttl,
}

/// AQL edge document for graph operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AqlEdge {
    /// Edge key
    #[serde(rename = "_key")]
    pub key: String,
    /// Edge ID
    #[serde(rename = "_id")]
    pub id: String,
    /// Edge revision
    #[serde(rename = "_rev")]
    pub revision: String,
    /// Source vertex ID
    #[serde(rename = "_from")]
    pub from: String,
    /// Target vertex ID
    #[serde(rename = "_to")]
    pub to: String,
    /// Edge data
    #[serde(flatten)]
    pub data: HashMap<String, AqlValue>,
}

impl AqlEdge {
    /// Create a new AQL edge
    pub fn new(
        collection: &str,
        key: String,
        from: String,
        to: String,
        data: HashMap<String, AqlValue>,
    ) -> Self {
        let id = format!("{}/{}", collection, key);
        let revision = format!("_{}", chrono::Utc::now().timestamp());

        Self {
            key,
            id,
            revision,
            from,
            to,
            data,
        }
    }

    /// Convert to graph relationship
    pub fn to_graph_relationship(&self) -> GraphRelationship {
        use orbit_shared::graph::{GraphRelationship, NodeId, RelationshipId};

        // Extract collection name from _from and _to
        let start_node = NodeId::from_string(&self.from);
        let end_node = NodeId::from_string(&self.to);
        let rel_id = RelationshipId::from_string(&self.id);

        // Convert data to serde_json::Value for compatibility
        let properties: std::collections::HashMap<String, serde_json::Value> = self
            .data
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::from(v.clone())))
            .collect();

        GraphRelationship::with_id(
            rel_id,
            start_node,
            end_node,
            "RELATED".to_string(), // Default relationship type
            properties,
        )
    }
}

/// AQL vertex document for graph operations
pub type AqlVertex = AqlDocument;

impl AqlDocument {
    /// Convert to graph node
    pub fn to_graph_node(&self, labels: Vec<String>) -> GraphNode {
        use orbit_shared::graph::{GraphNode, NodeId};

        let node_id = NodeId::from_string(&self.id);

        // Convert data to serde_json::Value for compatibility
        let properties: std::collections::HashMap<String, serde_json::Value> = self
            .data
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::from(v.clone())))
            .collect();

        GraphNode::with_id(node_id, labels, properties)
    }
}

/// AQL traversal result for graph operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AqlTraversalResult {
    /// Visited vertices
    pub vertices: Vec<AqlVertex>,
    /// Traversed edges
    pub edges: Vec<AqlEdge>,
    /// Traversal paths
    pub paths: Vec<AqlPath>,
}

/// AQL path representing a sequence of vertices and edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AqlPath {
    /// Vertices in the path
    pub vertices: Vec<AqlVertex>,
    /// Edges in the path
    pub edges: Vec<AqlEdge>,
}

/// AQL bind variable for parameterized queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AqlBindVars {
    /// Variable bindings
    pub vars: HashMap<String, AqlValue>,
}

impl AqlBindVars {
    /// Create new empty bind variables
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    /// Add a bind variable
    pub fn bind<V: Into<AqlValue>>(&mut self, name: String, value: V) -> &mut Self {
        self.vars.insert(name, value.into());
        self
    }

    /// Get a bind variable value
    pub fn get(&self, name: &str) -> Option<&AqlValue> {
        self.vars.get(name)
    }
}

impl Default for AqlBindVars {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aql_document_creation() {
        let mut data = HashMap::new();
        data.insert("name".to_string(), AqlValue::String("Alice".to_string()));
        data.insert(
            "age".to_string(),
            AqlValue::Number(serde_json::Number::from(30)),
        );

        let doc = AqlDocument::new("users", "alice".to_string(), data);

        assert_eq!(doc.key, "alice");
        assert_eq!(doc.id, "users/alice");
        assert_eq!(doc.get("name"), Some(AqlValue::String("Alice".to_string())));
        assert_eq!(doc.get("_key"), Some(AqlValue::String("alice".to_string())));
    }

    #[test]
    fn test_aql_value_conversion() {
        let json_val = serde_json::json!({
            "name": "test",
            "count": 42,
            "active": true,
            "tags": ["rust", "database"]
        });

        let aql_val = AqlValue::from(json_val.clone());
        let back_to_json = serde_json::Value::from(aql_val);

        assert_eq!(json_val, back_to_json);
    }

    #[test]
    fn test_aql_edge_creation() {
        let mut data = HashMap::new();
        data.insert(
            "weight".to_string(),
            AqlValue::Number(serde_json::Number::from(1)),
        );
        data.insert("label".to_string(), AqlValue::String("knows".to_string()));

        let edge = AqlEdge::new(
            "relationships",
            "edge1".to_string(),
            "users/alice".to_string(),
            "users/bob".to_string(),
            data,
        );

        assert_eq!(edge.key, "edge1");
        assert_eq!(edge.id, "relationships/edge1");
        assert_eq!(edge.from, "users/alice");
        assert_eq!(edge.to, "users/bob");
    }

    #[test]
    fn test_bind_vars() {
        let mut bind_vars = AqlBindVars::new();
        bind_vars
            .bind("name".to_string(), AqlValue::String("Alice".to_string()))
            .bind(
                "age".to_string(),
                AqlValue::Number(serde_json::Number::from(30)),
            );

        assert_eq!(
            bind_vars.get("name"),
            Some(&AqlValue::String("Alice".to_string()))
        );
        assert_eq!(
            bind_vars.get("age"),
            Some(&AqlValue::Number(serde_json::Number::from(30)))
        );
        assert_eq!(bind_vars.get("missing"), None);
    }

    #[test]
    fn test_document_to_graph_node_conversion() {
        let mut data = HashMap::new();
        data.insert("name".to_string(), AqlValue::String("Alice".to_string()));
        data.insert(
            "age".to_string(),
            AqlValue::Number(serde_json::Number::from(30)),
        );

        let doc = AqlDocument::new("users", "alice".to_string(), data);
        let labels = vec!["Person".to_string(), "User".to_string()];
        let graph_node = doc.to_graph_node(labels.clone());

        assert_eq!(graph_node.labels, labels);
        assert!(graph_node.has_label("Person"));
        assert!(graph_node.has_label("User"));
    }
}
