//! In-memory graph storage implementation
//!
//! This module provides a simple in-memory implementation of the GraphStorage trait
//! suitable for development, testing, and single-node deployments.

use super::{Direction, GraphNode, GraphRelationship, GraphStorage, NodeId, RelationshipId};
use crate::exception::{OrbitError, OrbitResult};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// In-memory graph storage implementation
#[derive(Debug)]
pub struct InMemoryGraphStorage {
    /// Node storage indexed by NodeId
    nodes: RwLock<HashMap<NodeId, GraphNode>>,
    /// Relationship storage indexed by RelationshipId
    relationships: RwLock<HashMap<RelationshipId, GraphRelationship>>,
    /// Index of outgoing relationships by node ID
    outgoing_rels: RwLock<HashMap<NodeId, Vec<RelationshipId>>>,
    /// Index of incoming relationships by node ID
    incoming_rels: RwLock<HashMap<NodeId, Vec<RelationshipId>>>,
    /// Label index: label -> set of node IDs
    label_index: RwLock<HashMap<String, Vec<NodeId>>>,
    /// Relationship type index: type -> set of relationship IDs
    rel_type_index: RwLock<HashMap<String, Vec<RelationshipId>>>,
}

impl InMemoryGraphStorage {
    /// Create a new in-memory graph storage
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            relationships: RwLock::new(HashMap::new()),
            outgoing_rels: RwLock::new(HashMap::new()),
            incoming_rels: RwLock::new(HashMap::new()),
            label_index: RwLock::new(HashMap::new()),
            rel_type_index: RwLock::new(HashMap::new()),
        }
    }

    /// Update the label index when a node's labels change
    async fn update_label_index(&self, node_id: &NodeId, old_labels: &[String], new_labels: &[String]) {
        let mut label_index = self.label_index.write().await;
        
        // Remove from old labels
        for label in old_labels {
            if let Some(node_ids) = label_index.get_mut(label) {
                node_ids.retain(|id| id != node_id);
                if node_ids.is_empty() {
                    label_index.remove(label);
                }
            }
        }
        
        // Add to new labels
        for label in new_labels {
            label_index.entry(label.clone()).or_default().push(node_id.clone());
        }
    }

    /// Update the relationship type index
    async fn update_rel_type_index(&self, rel_id: &RelationshipId, rel_type: &str, add: bool) {
        let mut rel_type_index = self.rel_type_index.write().await;
        
        if add {
            rel_type_index.entry(rel_type.to_string()).or_default().push(rel_id.clone());
        } else if let Some(rel_ids) = rel_type_index.get_mut(rel_type) {
            rel_ids.retain(|id| id != rel_id);
            if rel_ids.is_empty() {
                rel_type_index.remove(rel_type);
            }
        }
    }

    /// Update relationship indexes for a node
    async fn update_relationship_indexes(
        &self, 
        rel_id: &RelationshipId, 
        start_node: &NodeId, 
        end_node: &NodeId,
        add: bool
    ) {
        let mut outgoing = self.outgoing_rels.write().await;
        let mut incoming = self.incoming_rels.write().await;
        
        if add {
            outgoing.entry(start_node.clone()).or_default().push(rel_id.clone());
            incoming.entry(end_node.clone()).or_default().push(rel_id.clone());
        } else {
            if let Some(rels) = outgoing.get_mut(start_node) {
                rels.retain(|id| id != rel_id);
                if rels.is_empty() {
                    outgoing.remove(start_node);
                }
            }
            if let Some(rels) = incoming.get_mut(end_node) {
                rels.retain(|id| id != rel_id);
                if rels.is_empty() {
                    incoming.remove(end_node);
                }
            }
        }
    }

    /// Get node count for metrics
    pub async fn node_count(&self) -> usize {
        self.nodes.read().await.len()
    }

    /// Get relationship count for metrics
    pub async fn relationship_count(&self) -> usize {
        self.relationships.read().await.len()
    }
}

impl Default for InMemoryGraphStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GraphStorage for InMemoryGraphStorage {
    #[instrument(skip(self), fields(labels_count = labels.len(), props_count = properties.len()))]
    async fn create_node(
        &self,
        labels: Vec<String>,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<GraphNode> {
        let node = GraphNode::new(labels.clone(), properties);
        let node_id = node.id.clone();
        
        // Store the node
        self.nodes.write().await.insert(node_id.clone(), node.clone());
        
        // Update label index
        self.update_label_index(&node_id, &[], &labels).await;
        
        info!(node_id = %node_id, labels = ?labels, "Created graph node");
        Ok(node)
    }

    #[instrument(skip(self))]
    async fn get_node(&self, node_id: &NodeId) -> OrbitResult<Option<GraphNode>> {
        let node = self.nodes.read().await.get(node_id).cloned();
        debug!(node_id = %node_id, found = node.is_some(), "Retrieved graph node");
        Ok(node)
    }

    #[instrument(skip(self, properties), fields(props_count = properties.len()))]
    async fn update_node(
        &self,
        node_id: &NodeId,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<()> {
        let mut nodes = self.nodes.write().await;
        
        match nodes.get_mut(node_id) {
            Some(node) => {
                for (key, value) in properties {
                    node.set_property(&key, value);
                }
                info!(node_id = %node_id, "Updated node properties");
                Ok(())
            }
            None => {
                warn!(node_id = %node_id, "Attempted to update non-existent node");
                Err(OrbitError::NodeNotFound { node_id: node_id.to_string() })
            }
        }
    }

    #[instrument(skip(self))]
    async fn delete_node(&self, node_id: &NodeId) -> OrbitResult<bool> {
        // First, delete all relationships involving this node
        let relationships_to_delete: Vec<RelationshipId> = {
            let outgoing = self.outgoing_rels.read().await;
            let incoming = self.incoming_rels.read().await;
            
            let mut rels = Vec::new();
            if let Some(out_rels) = outgoing.get(node_id) {
                rels.extend(out_rels.clone());
            }
            if let Some(in_rels) = incoming.get(node_id) {
                rels.extend(in_rels.clone());
            }
            rels
        };

        // Delete relationships
        for rel_id in relationships_to_delete {
            self.delete_relationship(&rel_id).await?;
        }

        // Get node for label cleanup
        let node = {
            let nodes = self.nodes.read().await;
            nodes.get(node_id).cloned()
        };

        if let Some(node) = node {
            // Update label index
            self.update_label_index(node_id, &node.labels, &[]).await;
            
            // Remove node
            let removed = self.nodes.write().await.remove(node_id).is_some();
            
            if removed {
                info!(node_id = %node_id, "Deleted graph node");
            }
            Ok(removed)
        } else {
            Ok(false)
        }
    }

    #[instrument(skip(self, labels), fields(labels_count = labels.len()))]
    async fn add_labels(&self, node_id: &NodeId, labels: Vec<String>) -> OrbitResult<()> {
        let mut nodes = self.nodes.write().await;
        
        match nodes.get_mut(node_id) {
            Some(node) => {
                let old_labels = node.labels.clone();
                node.add_labels(labels.clone());
                let new_labels = node.labels.clone();
                
                // Drop the write lock before updating index
                drop(nodes);
                
                // Update label index
                self.update_label_index(node_id, &old_labels, &new_labels).await;
                
                info!(node_id = %node_id, labels = ?labels, "Added labels to node");
                Ok(())
            }
            None => {
                warn!(node_id = %node_id, "Attempted to add labels to non-existent node");
                Err(OrbitError::NodeNotFound { node_id: node_id.to_string() })
            }
        }
    }

    #[instrument(skip(self, labels), fields(labels_count = labels.len()))]
    async fn remove_labels(&self, node_id: &NodeId, labels: Vec<String>) -> OrbitResult<()> {
        let mut nodes = self.nodes.write().await;
        
        match nodes.get_mut(node_id) {
            Some(node) => {
                let old_labels = node.labels.clone();
                node.remove_labels(labels.clone());
                let new_labels = node.labels.clone();
                
                // Drop the write lock before updating index
                drop(nodes);
                
                // Update label index
                self.update_label_index(node_id, &old_labels, &new_labels).await;
                
                info!(node_id = %node_id, labels = ?labels, "Removed labels from node");
                Ok(())
            }
            None => {
                warn!(node_id = %node_id, "Attempted to remove labels from non-existent node");
                Err(OrbitError::NodeNotFound { node_id: node_id.to_string() })
            }
        }
    }

    #[instrument(skip(self))]
    async fn get_labels(&self, node_id: &NodeId) -> OrbitResult<Vec<String>> {
        match self.nodes.read().await.get(node_id) {
            Some(node) => {
                debug!(node_id = %node_id, labels = ?node.labels, "Retrieved node labels");
                Ok(node.labels.clone())
            }
            None => {
                warn!(node_id = %node_id, "Attempted to get labels from non-existent node");
                Err(OrbitError::NodeNotFound { node_id: node_id.to_string() })
            }
        }
    }

    #[instrument(skip(self, properties), fields(props_count = properties.len()))]
    async fn create_relationship(
        &self,
        start_node: &NodeId,
        end_node: &NodeId,
        rel_type: String,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<GraphRelationship> {
        // Verify both nodes exist
        let nodes = self.nodes.read().await;
        if !nodes.contains_key(start_node) {
            return Err(OrbitError::NodeNotFound { node_id: start_node.to_string() });
        }
        if !nodes.contains_key(end_node) {
            return Err(OrbitError::NodeNotFound { node_id: end_node.to_string() });
        }
        drop(nodes);

        let relationship = GraphRelationship::new(
            start_node.clone(),
            end_node.clone(),
            rel_type.clone(),
            properties,
        );
        let rel_id = relationship.id.clone();

        // Store relationship
        self.relationships.write().await.insert(rel_id.clone(), relationship.clone());
        
        // Update indexes
        self.update_relationship_indexes(&rel_id, start_node, end_node, true).await;
        self.update_rel_type_index(&rel_id, &rel_type, true).await;

        info!(
            rel_id = %rel_id,
            start_node = %start_node,
            end_node = %end_node,
            rel_type = rel_type,
            "Created relationship"
        );

        Ok(relationship)
    }

    #[instrument(skip(self))]
    async fn get_relationships(
        &self,
        node_id: &NodeId,
        direction: Direction,
        rel_types: Option<Vec<String>>,
    ) -> OrbitResult<Vec<GraphRelationship>> {
        let outgoing = self.outgoing_rels.read().await;
        let incoming = self.incoming_rels.read().await;
        let relationships = self.relationships.read().await;

        let mut rel_ids = Vec::new();

        match direction {
            Direction::Outgoing => {
                if let Some(rels) = outgoing.get(node_id) {
                    rel_ids.extend(rels.clone());
                }
            }
            Direction::Incoming => {
                if let Some(rels) = incoming.get(node_id) {
                    rel_ids.extend(rels.clone());
                }
            }
            Direction::Both => {
                if let Some(rels) = outgoing.get(node_id) {
                    rel_ids.extend(rels.clone());
                }
                if let Some(rels) = incoming.get(node_id) {
                    rel_ids.extend(rels.clone());
                }
            }
        }

        let mut result = Vec::new();
        for rel_id in rel_ids {
            if let Some(relationship) = relationships.get(&rel_id) {
                // Filter by relationship types if specified
                if let Some(ref types) = rel_types {
                    if !types.contains(&relationship.rel_type) {
                        continue;
                    }
                }
                result.push(relationship.clone());
            }
        }

        debug!(
            node_id = %node_id,
            direction = ?direction,
            rel_types = ?rel_types,
            found_count = result.len(),
            "Retrieved relationships"
        );

        Ok(result)
    }

    #[instrument(skip(self))]
    async fn get_relationship(&self, rel_id: &RelationshipId) -> OrbitResult<Option<GraphRelationship>> {
        let relationship = self.relationships.read().await.get(rel_id).cloned();
        debug!(rel_id = %rel_id, found = relationship.is_some(), "Retrieved relationship");
        Ok(relationship)
    }

    #[instrument(skip(self, properties), fields(props_count = properties.len()))]
    async fn update_relationship(
        &self,
        rel_id: &RelationshipId,
        properties: HashMap<String, serde_json::Value>,
    ) -> OrbitResult<()> {
        let mut relationships = self.relationships.write().await;
        
        match relationships.get_mut(rel_id) {
            Some(relationship) => {
                for (key, value) in properties {
                    relationship.set_property(&key, value);
                }
                info!(rel_id = %rel_id, "Updated relationship properties");
                Ok(())
            }
            None => {
                warn!(rel_id = %rel_id, "Attempted to update non-existent relationship");
                Err(OrbitError::Internal(format!("Relationship {} not found", rel_id)))
            }
        }
    }

    #[instrument(skip(self))]
    async fn delete_relationship(&self, rel_id: &RelationshipId) -> OrbitResult<bool> {
        // Get relationship details for cleanup
        let relationship = {
            let relationships = self.relationships.read().await;
            relationships.get(rel_id).cloned()
        };

        if let Some(rel) = relationship {
            // Update indexes
            self.update_relationship_indexes(&rel_id, &rel.start_node, &rel.end_node, false).await;
            self.update_rel_type_index(&rel_id, &rel.rel_type, false).await;
            
            // Remove relationship
            let removed = self.relationships.write().await.remove(rel_id).is_some();
            
            if removed {
                info!(rel_id = %rel_id, "Deleted relationship");
            }
            Ok(removed)
        } else {
            Ok(false)
        }
    }

    #[instrument(skip(self, property_filters), fields(limit = limit.unwrap_or(0)))]
    async fn find_nodes_by_label(
        &self,
        label: &str,
        property_filters: Option<HashMap<String, serde_json::Value>>,
        limit: Option<usize>,
    ) -> OrbitResult<Vec<GraphNode>> {
        let label_index = self.label_index.read().await;
        let nodes = self.nodes.read().await;

        let node_ids = match label_index.get(label) {
            Some(ids) => ids,
            None => {
                debug!(label = label, "No nodes found with label");
                return Ok(Vec::new());
            }
        };

        let mut result = Vec::new();
        let mut count = 0;

        for node_id in node_ids {
            if let Some(limit_val) = limit {
                if count >= limit_val {
                    break;
                }
            }

            if let Some(node) = nodes.get(node_id) {
                // Apply property filters if specified
                if let Some(ref filters) = property_filters {
                    let mut matches = true;
                    for (key, expected_value) in filters {
                        match node.get_property(key) {
                            Some(actual_value) if actual_value == expected_value => continue,
                            _ => {
                                matches = false;
                                break;
                            }
                        }
                    }
                    if !matches {
                        continue;
                    }
                }

                result.push(node.clone());
                count += 1;
            }
        }

        debug!(
            label = label,
            property_filters = ?property_filters,
            limit = limit,
            found_count = result.len(),
            "Found nodes by label"
        );

        Ok(result)
    }

    #[instrument(skip(self))]
    async fn count_nodes_by_label(&self, label: &str) -> OrbitResult<u64> {
        let label_index = self.label_index.read().await;
        let count = match label_index.get(label) {
            Some(ids) => ids.len() as u64,
            None => 0,
        };
        debug!(label = label, count = count, "Counted nodes by label");
        Ok(count)
    }

    #[instrument(skip(self))]
    async fn count_relationships_by_type(&self, rel_type: &str) -> OrbitResult<u64> {
        let rel_type_index = self.rel_type_index.read().await;
        let count = match rel_type_index.get(rel_type) {
            Some(ids) => ids.len() as u64,
            None => 0,
        };
        debug!(rel_type = rel_type, count = count, "Counted relationships by type");
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_crud_operations() {
        let storage = InMemoryGraphStorage::new();
        
        // Create node
        let labels = vec!["Person".to_string(), "Employee".to_string()];
        let mut properties = HashMap::new();
        properties.insert("name".to_string(), serde_json::Value::String("Alice".to_string()));
        properties.insert("age".to_string(), serde_json::Value::Number(serde_json::Number::from(30)));
        
        let node = storage.create_node(labels.clone(), properties.clone()).await.unwrap();
        assert_eq!(node.labels, labels);
        assert_eq!(node.properties, properties);
        
        // Get node
        let retrieved = storage.get_node(&node.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, node.id);
        
        // Update node
        let mut updates = HashMap::new();
        updates.insert("age".to_string(), serde_json::Value::Number(serde_json::Number::from(31)));
        storage.update_node(&node.id, updates).await.unwrap();
        
        let updated = storage.get_node(&node.id).await.unwrap().unwrap();
        assert_eq!(
            updated.get_property("age"),
            Some(&serde_json::Value::Number(serde_json::Number::from(31)))
        );
        
        // Delete node
        let deleted = storage.delete_node(&node.id).await.unwrap();
        assert!(deleted);
        
        let not_found = storage.get_node(&node.id).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_relationship_crud_operations() {
        let storage = InMemoryGraphStorage::new();
        
        // Create nodes first
        let node1 = storage.create_node(vec!["Person".to_string()], HashMap::new()).await.unwrap();
        let node2 = storage.create_node(vec!["Person".to_string()], HashMap::new()).await.unwrap();
        
        // Create relationship
        let mut rel_props = HashMap::new();
        rel_props.insert("since".to_string(), serde_json::Value::String("2023".to_string()));
        
        let relationship = storage.create_relationship(
            &node1.id,
            &node2.id,
            "KNOWS".to_string(),
            rel_props.clone()
        ).await.unwrap();
        
        assert_eq!(relationship.start_node, node1.id);
        assert_eq!(relationship.end_node, node2.id);
        assert_eq!(relationship.rel_type, "KNOWS");
        assert_eq!(relationship.properties, rel_props);
        
        // Get relationships
        let outgoing = storage.get_relationships(&node1.id, Direction::Outgoing, None).await.unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].id, relationship.id);
        
        let incoming = storage.get_relationships(&node2.id, Direction::Incoming, None).await.unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].id, relationship.id);
        
        // Delete relationship
        let deleted = storage.delete_relationship(&relationship.id).await.unwrap();
        assert!(deleted);
        
        let no_rels = storage.get_relationships(&node1.id, Direction::Outgoing, None).await.unwrap();
        assert!(no_rels.is_empty());
    }

    #[tokio::test]
    async fn test_label_operations() {
        let storage = InMemoryGraphStorage::new();
        
        let node = storage.create_node(vec!["Person".to_string()], HashMap::new()).await.unwrap();
        
        // Add labels
        storage.add_labels(&node.id, vec!["Employee".to_string(), "Manager".to_string()]).await.unwrap();
        
        let labels = storage.get_labels(&node.id).await.unwrap();
        assert!(labels.contains(&"Person".to_string()));
        assert!(labels.contains(&"Employee".to_string()));
        assert!(labels.contains(&"Manager".to_string()));
        
        // Remove labels
        storage.remove_labels(&node.id, vec!["Employee".to_string()]).await.unwrap();
        
        let labels = storage.get_labels(&node.id).await.unwrap();
        assert!(labels.contains(&"Person".to_string()));
        assert!(!labels.contains(&"Employee".to_string()));
        assert!(labels.contains(&"Manager".to_string()));
    }

    #[tokio::test]
    async fn test_find_nodes_by_label() {
        let storage = InMemoryGraphStorage::new();
        
        // Create nodes with different labels
        let _node1 = storage.create_node(vec!["Person".to_string()], HashMap::new()).await.unwrap();
        let _node2 = storage.create_node(vec!["Person".to_string(), "Employee".to_string()], HashMap::new()).await.unwrap();
        let _node3 = storage.create_node(vec!["Company".to_string()], HashMap::new()).await.unwrap();
        
        // Find by label
        let persons = storage.find_nodes_by_label("Person", None, None).await.unwrap();
        assert_eq!(persons.len(), 2);
        
        let employees = storage.find_nodes_by_label("Employee", None, None).await.unwrap();
        assert_eq!(employees.len(), 1);
        
        let companies = storage.find_nodes_by_label("Company", None, None).await.unwrap();
        assert_eq!(companies.len(), 1);
        
        // Count by label
        let person_count = storage.count_nodes_by_label("Person").await.unwrap();
        assert_eq!(person_count, 2);
        
        let employee_count = storage.count_nodes_by_label("Employee").await.unwrap();
        assert_eq!(employee_count, 1);
    }
}