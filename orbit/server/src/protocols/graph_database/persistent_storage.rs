//! Persistent graph storage adapter
//!
//! This module provides adapters that implement GraphStorage trait using
//! persistent storage backends like CypherGraphStorage.

#![cfg(feature = "storage-rocksdb")]

use crate::protocols::cypher::storage::CypherGraphStorage;
use orbit_shared::graph::{
    Direction, GraphNode, GraphRelationship, GraphStorage, NodeId, RelationshipId,
};
use orbit_shared::{OrbitError, OrbitResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Persistent graph storage adapter using CypherGraphStorage
pub struct PersistentGraphStorage {
    /// Cypher graph storage backend
    cypher_storage: Arc<CypherGraphStorage>,
    /// Node ID mapping (orbit NodeId -> cypher node id)
    node_id_map: Arc<RwLock<HashMap<String, String>>>,
    /// Relationship ID mapping
    rel_id_map: Arc<RwLock<HashMap<String, String>>>,
}

impl PersistentGraphStorage {
    /// Create a new persistent graph storage adapter
    pub fn new(cypher_storage: Arc<CypherGraphStorage>) -> Self {
        Self {
            cypher_storage,
            node_id_map: Arc::new(RwLock::new(HashMap::new())),
            rel_id_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Convert orbit GraphNode to CypherGraphStorage GraphNode
    fn convert_node_to_cypher(
        &self,
        node: &GraphNode,
    ) -> crate::protocols::cypher::types::GraphNode {
        crate::protocols::cypher::types::GraphNode {
            id: node.id.as_str().to_string(),
            labels: node.labels.clone(),
            properties: node.properties.clone(),
        }
    }

    /// Convert CypherGraphStorage GraphNode to orbit GraphNode
    fn convert_node_from_cypher(
        &self,
        cypher_node: &crate::protocols::cypher::types::GraphNode,
    ) -> GraphNode {
        GraphNode::with_id(
            NodeId::from_string(&cypher_node.id),
            cypher_node.labels.clone(),
            cypher_node.properties.clone(),
        )
    }

    /// Convert orbit GraphRelationship to CypherGraphStorage GraphRelationship
    fn convert_rel_to_cypher(
        &self,
        rel: &GraphRelationship,
    ) -> crate::protocols::cypher::types::GraphRelationship {
        crate::protocols::cypher::types::GraphRelationship {
            id: rel.id.as_str().to_string(),
            start_node: rel.start_node.as_str().to_string(),
            end_node: rel.end_node.as_str().to_string(),
            rel_type: rel.rel_type.clone(),
            properties: rel.properties.clone(),
        }
    }

    /// Convert CypherGraphStorage GraphRelationship to orbit GraphRelationship
    fn convert_rel_from_cypher(
        &self,
        cypher_rel: &crate::protocols::cypher::types::GraphRelationship,
    ) -> GraphRelationship {
        GraphRelationship {
            id: RelationshipId::from_string(&cypher_rel.id),
            start_node: NodeId::from_string(&cypher_rel.start_node),
            end_node: NodeId::from_string(&cypher_rel.end_node),
            rel_type: cypher_rel.rel_type.clone(),
            properties: cypher_rel.properties.clone(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

#[async_trait::async_trait]
impl GraphStorage for PersistentGraphStorage {
    async fn create_node(
        &self,
        labels: Vec<String>,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<GraphNode> {
        let node = GraphNode::new(labels.clone(), properties);
        let cypher_node = self.convert_node_to_cypher(&node);

        // Store in CypherGraphStorage
        self.cypher_storage
            .store_node(cypher_node)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to store node: {}", e)))?;

        // Update ID mapping
        {
            let mut map = self.node_id_map.write().await;
            map.insert(node.id.as_str().to_string(), node.id.as_str().to_string());
        }

        Ok(node)
    }

    async fn get_node(&self, node_id: &NodeId) -> OrbitResult<Option<GraphNode>> {
        let node_id_str = node_id.as_str();

        // Try to get from CypherGraphStorage
        if let Ok(Some(cypher_node)) = self.cypher_storage.get_node(node_id_str).await {
            return Ok(Some(self.convert_node_from_cypher(&cypher_node)));
        }

        Ok(None)
    }

    async fn update_node(
        &self,
        node_id: &NodeId,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<()> {
        // Get existing node
        if let Some(mut node) = self.get_node(node_id).await? {
            // Update properties
            for (key, value) in properties {
                node.properties.insert(key, value);
            }
            node.updated_at = chrono::Utc::now();

            // Store updated node
            let cypher_node = self.convert_node_to_cypher(&node);
            self.cypher_storage
                .store_node(cypher_node)
                .await
                .map_err(|e| OrbitError::internal(format!("Failed to update node: {}", e)))?;

            Ok(())
        } else {
            Err(OrbitError::NodeNotFound {
                node_id: node_id.as_str().to_string(),
            })
        }
    }

    async fn delete_node(&self, node_id: &NodeId) -> OrbitResult<bool> {
        // For now, we'll mark as deleted by removing from index
        // Full deletion would require removing relationships too
        let node_id_str = node_id.as_str();
        {
            let mut map = self.node_id_map.write().await;
            map.remove(node_id_str);
        }
        // TODO: Implement full deletion including relationships
        Ok(true)
    }

    async fn create_relationship(
        &self,
        start_node: &NodeId,
        end_node: &NodeId,
        rel_type: String,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<GraphRelationship> {
        let rel = GraphRelationship::new(
            start_node.clone(),
            end_node.clone(),
            rel_type.clone(),
            properties,
        );

        let cypher_rel = self.convert_rel_to_cypher(&rel);

        // Store in CypherGraphStorage
        self.cypher_storage
            .store_relationship(cypher_rel)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to store relationship: {}", e)))?;

        // Update ID mapping
        {
            let mut map = self.rel_id_map.write().await;
            map.insert(rel.id.as_str().to_string(), rel.id.as_str().to_string());
        }

        Ok(rel)
    }

    async fn get_relationship(
        &self,
        rel_id: &RelationshipId,
    ) -> OrbitResult<Option<GraphRelationship>> {
        let rel_id_str = rel_id.as_str();

        // Try to get from CypherGraphStorage
        if let Ok(Some(cypher_rel)) = self.cypher_storage.get_relationship(rel_id_str).await {
            return Ok(Some(self.convert_rel_from_cypher(&cypher_rel)));
        }

        Ok(None)
    }

    async fn delete_relationship(&self, rel_id: &RelationshipId) -> OrbitResult<bool> {
        let rel_id_str = rel_id.as_str();
        {
            let mut map = self.rel_id_map.write().await;
            map.remove(rel_id_str);
        }
        // TODO: Implement full deletion from CypherGraphStorage
        Ok(true)
    }

    async fn get_relationships(
        &self,
        node_id: &NodeId,
        direction: Direction,
        rel_types: Option<Vec<String>>,
    ) -> OrbitResult<Vec<GraphRelationship>> {
        // Get relationships from CypherGraphStorage
        // For now, return empty - would need to implement relationship queries
        // This is a limitation - CypherGraphStorage doesn't have relationship queries yet
        let _ = (node_id, direction, rel_types);
        Ok(Vec::new())
    }

    async fn get_labels(&self, node_id: &NodeId) -> OrbitResult<Vec<String>> {
        if let Some(node) = self.get_node(node_id).await? {
            Ok(node.labels)
        } else {
            Err(OrbitError::NodeNotFound {
                node_id: node_id.as_str().to_string(),
            })
        }
    }

    async fn update_relationship(
        &self,
        rel_id: &RelationshipId,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<()> {
        // Get existing relationship
        if let Some(mut rel) = self.get_relationship(rel_id).await? {
            // Update properties
            for (key, value) in properties {
                rel.properties.insert(key, value);
            }
            rel.updated_at = chrono::Utc::now();

            // Store updated relationship
            let cypher_rel = self.convert_rel_to_cypher(&rel);
            self.cypher_storage
                .store_relationship(cypher_rel)
                .await
                .map_err(|e| {
                    OrbitError::internal(format!("Failed to update relationship: {}", e))
                })?;

            Ok(())
        } else {
            Err(OrbitError::internal(format!(
                "Relationship not found: {}",
                rel_id.as_str()
            )))
        }
    }

    async fn find_nodes_by_label(
        &self,
        label: &str,
        property_filters: Option<HashMap<String, serde_json::Value>>,
        limit: Option<usize>,
    ) -> OrbitResult<Vec<GraphNode>> {
        // Get nodes by label from CypherGraphStorage
        // For now, return empty - would need label-based queries
        let _ = (label, property_filters, limit);
        Ok(Vec::new())
    }

    async fn count_nodes_by_label(&self, label: &str) -> OrbitResult<u64> {
        // Count nodes by label
        // For now, return 0 - would need label-based queries
        let _ = label;
        Ok(0)
    }

    async fn count_relationships_by_type(&self, rel_type: &str) -> OrbitResult<u64> {
        // Count relationships by type
        // For now, return 0 - would need type-based queries
        let _ = rel_type;
        Ok(0)
    }

    async fn add_labels(&self, node_id: &NodeId, labels: Vec<String>) -> OrbitResult<()> {
        if let Some(mut node) = self.get_node(node_id).await? {
            node.add_labels(labels.clone());
            node.updated_at = chrono::Utc::now();

            let cypher_node = self.convert_node_to_cypher(&node);
            self.cypher_storage
                .store_node(cypher_node)
                .await
                .map_err(|e| {
                    OrbitError::internal(format!("Failed to update node labels: {}", e))
                })?;

            Ok(())
        } else {
            Err(OrbitError::NodeNotFound {
                node_id: node_id.as_str().to_string(),
            })
        }
    }

    async fn remove_labels(&self, node_id: &NodeId, labels: Vec<String>) -> OrbitResult<()> {
        if let Some(mut node) = self.get_node(node_id).await? {
            node.remove_labels(labels.clone());
            node.updated_at = chrono::Utc::now();

            let cypher_node = self.convert_node_to_cypher(&node);
            self.cypher_storage
                .store_node(cypher_node)
                .await
                .map_err(|e| {
                    OrbitError::internal(format!("Failed to update node labels: {}", e))
                })?;

            Ok(())
        } else {
            Err(OrbitError::NodeNotFound {
                node_id: node_id.as_str().to_string(),
            })
        }
    }
}
