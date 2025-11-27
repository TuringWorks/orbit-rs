//! Cypher graph types
//!
//! This module provides basic graph types used in the Cypher protocol.
//! These types are independent of the storage backend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Graph node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Graph relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRelationship {
    pub id: String,
    pub start_node: String,
    pub end_node: String,
    pub rel_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}
