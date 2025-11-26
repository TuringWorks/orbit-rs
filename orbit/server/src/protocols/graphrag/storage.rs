//! GraphRAG storage with RocksDB persistence
//!
//! This module provides persistent storage for GraphRAG knowledge graphs using RocksDB.
//! It stores entities, relationships, embeddings, and metadata with full persistence support.

#![cfg(feature = "storage-rocksdb")]

use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_shared::graphrag::EntityType;
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// GraphRAG entity node stored in RocksDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGNode {
    /// Node ID
    pub id: String,
    /// Entity text/name
    pub text: String,
    /// Entity type
    pub entity_type: EntityType,
    /// Labels for graph queries
    pub labels: Vec<String>,
    /// Properties/metadata
    pub properties: HashMap<String, serde_json::Value>,
    /// Confidence score
    pub confidence: f32,
    /// Source documents
    pub source_documents: Vec<String>,
    /// Embeddings (model_name -> embedding vector)
    pub embeddings: HashMap<String, Vec<f32>>,
    /// Creation timestamp
    pub created_at: i64,
    /// Last update timestamp
    pub updated_at: i64,
}

/// GraphRAG relationship stored in RocksDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGRelationship {
    /// Relationship ID
    pub id: String,
    /// Source entity ID
    pub from_entity_id: String,
    /// Target entity ID
    pub to_entity_id: String,
    /// Relationship type
    pub relationship_type: String,
    /// Properties/metadata
    pub properties: HashMap<String, serde_json::Value>,
    /// Confidence score
    pub confidence: f32,
    /// Source text
    pub source_text: String,
    /// Source document
    pub source_document: String,
    /// Creation timestamp
    pub created_at: i64,
    /// Last update timestamp
    pub updated_at: i64,
}

/// GraphRAG knowledge graph metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRAGMetadata {
    /// Knowledge graph name
    pub kg_name: String,
    /// Total nodes
    pub node_count: u64,
    /// Total relationships
    pub relationship_count: u64,
    /// Entities by type
    pub entities_by_type: HashMap<String, u64>,
    /// Relationships by type
    pub relationships_by_type: HashMap<String, u64>,
    /// Last update timestamp
    pub last_updated: i64,
}

/// GraphRAG storage with RocksDB persistence
pub struct GraphRAGStorage {
    /// RocksDB instance
    db: Arc<RwLock<Option<Arc<DB>>>>,
    /// Data directory path
    data_dir: PathBuf,
    /// Knowledge graph name
    kg_name: String,
    /// In-memory cache for nodes
    nodes: Arc<RwLock<HashMap<String, GraphRAGNode>>>,
    /// In-memory cache for relationships
    relationships: Arc<RwLock<HashMap<String, GraphRAGRelationship>>>,
    /// Metadata cache
    metadata: Arc<RwLock<Option<GraphRAGMetadata>>>,
}

impl GraphRAGStorage {
    /// Create a new GraphRAG storage with data directory
    pub fn new<P: AsRef<Path>>(data_dir: P, kg_name: String) -> Self {
        Self {
            db: Arc::new(RwLock::new(None)),
            data_dir: data_dir.as_ref().to_path_buf(),
            kg_name,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            relationships: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize RocksDB storage
    pub async fn initialize(&self) -> ProtocolResult<()> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Define column families for GraphRAG data
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new("nodes", Options::default()),
            ColumnFamilyDescriptor::new("relationships", Options::default()),
            ColumnFamilyDescriptor::new("metadata", Options::default()),
            ColumnFamilyDescriptor::new("embeddings", Options::default()),
            ColumnFamilyDescriptor::new("entity_index", Options::default()), // For text-based lookup
            ColumnFamilyDescriptor::new("rel_index", Options::default()), // For relationship lookup
        ];

        let db_path = self.data_dir.join("rocksdb");
        match DB::open_cf_descriptors(&opts, &db_path, cf_descriptors) {
            Ok(db) => {
                let db_arc = Arc::new(db);
                {
                    let mut db_guard = self.db.write().await;
                    *db_guard = Some(db_arc.clone());
                }
                // Load existing data from RocksDB
                self.load_from_rocksdb(&db_arc).await?;
                info!(
                    "GraphRAGStorage: RocksDB initialized for '{}' at {:?}",
                    self.kg_name, db_path
                );
            }
            Err(e) => {
                error!("Failed to open GraphRAG RocksDB at {:?}: {}", db_path, e);
                return Err(ProtocolError::Other(format!(
                    "Failed to initialize GraphRAG RocksDB: {}",
                    e
                )));
            }
        }
        Ok(())
    }

    /// Load data from RocksDB on startup
    async fn load_from_rocksdb(&self, db: &Arc<DB>) -> ProtocolResult<()> {
        // Load nodes
        let nodes_cf = db
            .cf_handle("nodes")
            .ok_or_else(|| ProtocolError::Other("Nodes column family not found".to_string()))?;

        let node_iter = db.iterator_cf(nodes_cf, rocksdb::IteratorMode::Start);
        let mut nodes = self.nodes.write().await;
        let mut node_count = 0;

        for item in node_iter {
            match item {
                Ok((_key, value)) => {
                    if let Ok(node) = serde_json::from_slice::<GraphRAGNode>(&value) {
                        // Only load nodes for this knowledge graph
                        if node
                            .properties
                            .get("kg_name")
                            .and_then(|v| v.as_str())
                            .map(|n| n == self.kg_name)
                            .unwrap_or(false)
                        {
                            nodes.insert(node.id.clone(), node);
                            node_count += 1;
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading node from RocksDB: {}", e);
                }
            }
        }

        // Load relationships
        let rels_cf = db.cf_handle("relationships").ok_or_else(|| {
            ProtocolError::Other("Relationships column family not found".to_string())
        })?;

        let rel_iter = db.iterator_cf(rels_cf, rocksdb::IteratorMode::Start);
        let mut relationships = self.relationships.write().await;
        let mut rel_count = 0;

        for item in rel_iter {
            match item {
                Ok((_key, value)) => {
                    if let Ok(rel) = serde_json::from_slice::<GraphRAGRelationship>(&value) {
                        relationships.insert(rel.id.clone(), rel);
                        rel_count += 1;
                    }
                }
                Err(e) => {
                    error!("Error reading relationship from RocksDB: {}", e);
                }
            }
        }

        // Load metadata
        let metadata_cf = db
            .cf_handle("metadata")
            .ok_or_else(|| ProtocolError::Other("Metadata column family not found".to_string()))?;

        let metadata_key = format!("kg:{}", self.kg_name);
        if let Ok(Some(value)) = db.get_cf(metadata_cf, metadata_key.as_bytes()) {
            if let Ok(metadata) = serde_json::from_slice::<GraphRAGMetadata>(&value) {
                let mut meta_guard = self.metadata.write().await;
                *meta_guard = Some(metadata);
            }
        }

        info!(
            "GraphRAGStorage: Loaded {} nodes and {} relationships for '{}' from RocksDB",
            node_count, rel_count, self.kg_name
        );
        Ok(())
    }

    /// Store an entity node
    pub async fn store_node(&self, node: GraphRAGNode) -> ProtocolResult<()> {
        // Add kg_name to properties for filtering
        let mut node_with_kg = node.clone();
        node_with_kg
            .properties
            .insert("kg_name".to_string(), serde_json::json!(self.kg_name));

        // Store in memory
        self.nodes
            .write()
            .await
            .insert(node_with_kg.id.clone(), node_with_kg.clone());

        // Persist to RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let nodes_cf = db
                .cf_handle("nodes")
                .ok_or_else(|| ProtocolError::Other("Nodes column family not found".to_string()))?;

            let key = format!("node:{}:{}", self.kg_name, node_with_kg.id);
            let value = serde_json::to_vec(&node_with_kg)
                .map_err(|e| ProtocolError::Other(format!("Failed to serialize node: {}", e)))?;

            db.put_cf(nodes_cf, key.as_bytes(), &value).map_err(|e| {
                ProtocolError::Other(format!("Failed to persist node to RocksDB: {}", e))
            })?;

            // Update entity index for text-based lookup
            let entity_index_cf = db.cf_handle("entity_index").ok_or_else(|| {
                ProtocolError::Other("Entity index column family not found".to_string())
            })?;
            let index_key = format!("text:{}:{}", self.kg_name, node_with_kg.text.to_lowercase());
            db.put_cf(
                entity_index_cf,
                index_key.as_bytes(),
                node_with_kg.id.as_bytes(),
            )
            .map_err(|e| ProtocolError::Other(format!("Failed to update entity index: {}", e)))?;
        }

        Ok(())
    }

    /// Get a node by ID
    pub async fn get_node(&self, node_id: &str) -> ProtocolResult<Option<GraphRAGNode>> {
        // Check memory first
        let nodes = self.nodes.read().await;
        if let Some(node) = nodes.get(node_id) {
            return Ok(Some(node.clone()));
        }
        Ok(None)
    }

    /// Find node by entity text
    pub async fn find_node_by_text(&self, text: &str) -> ProtocolResult<Option<GraphRAGNode>> {
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let entity_index_cf = db.cf_handle("entity_index").ok_or_else(|| {
                ProtocolError::Other("Entity index column family not found".to_string())
            })?;

            let index_key = format!("text:{}:{}", self.kg_name, text.to_lowercase());
            if let Ok(Some(node_id_bytes)) = db.get_cf(entity_index_cf, index_key.as_bytes()) {
                if let Ok(node_id) = String::from_utf8(node_id_bytes) {
                    return self.get_node(&node_id).await;
                }
            }
        }
        Ok(None)
    }

    /// Store a relationship
    pub async fn store_relationship(&self, rel: GraphRAGRelationship) -> ProtocolResult<()> {
        // Store in memory
        self.relationships
            .write()
            .await
            .insert(rel.id.clone(), rel.clone());

        // Persist to RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let rels_cf = db.cf_handle("relationships").ok_or_else(|| {
                ProtocolError::Other("Relationships column family not found".to_string())
            })?;

            let key = format!("rel:{}:{}", self.kg_name, rel.id);
            let value = serde_json::to_vec(&rel).map_err(|e| {
                ProtocolError::Other(format!("Failed to serialize relationship: {}", e))
            })?;

            db.put_cf(rels_cf, key.as_bytes(), &value).map_err(|e| {
                ProtocolError::Other(format!("Failed to persist relationship to RocksDB: {}", e))
            })?;

            // Update relationship index
            let rel_index_cf = db.cf_handle("rel_index").ok_or_else(|| {
                ProtocolError::Other("Relationship index column family not found".to_string())
            })?;

            // Index by from_entity
            let from_key = format!("from:{}:{}:{}", self.kg_name, rel.from_entity_id, rel.id);
            db.put_cf(rel_index_cf, from_key.as_bytes(), rel.id.as_bytes())
                .map_err(|e| {
                    ProtocolError::Other(format!("Failed to update relationship index: {}", e))
                })?;

            // Index by to_entity
            let to_key = format!("to:{}:{}:{}", self.kg_name, rel.to_entity_id, rel.id);
            db.put_cf(rel_index_cf, to_key.as_bytes(), rel.id.as_bytes())
                .map_err(|e| {
                    ProtocolError::Other(format!("Failed to update relationship index: {}", e))
                })?;
        }

        Ok(())
    }

    /// Get a relationship by ID
    pub async fn get_relationship(
        &self,
        rel_id: &str,
    ) -> ProtocolResult<Option<GraphRAGRelationship>> {
        // Check memory first
        let relationships = self.relationships.read().await;
        if let Some(rel) = relationships.get(rel_id) {
            return Ok(Some(rel.clone()));
        }
        Ok(None)
    }

    /// Get relationships for a node
    pub async fn get_node_relationships(
        &self,
        node_id: &str,
        direction: RelationshipDirection,
    ) -> ProtocolResult<Vec<GraphRAGRelationship>> {
        let relationships = self.relationships.read().await;
        let mut result = Vec::new();

        for rel in relationships.values() {
            match direction {
                RelationshipDirection::Outgoing => {
                    if rel.from_entity_id == node_id {
                        result.push(rel.clone());
                    }
                }
                RelationshipDirection::Incoming => {
                    if rel.to_entity_id == node_id {
                        result.push(rel.clone());
                    }
                }
                RelationshipDirection::Both => {
                    if rel.from_entity_id == node_id || rel.to_entity_id == node_id {
                        result.push(rel.clone());
                    }
                }
            }
        }

        Ok(result)
    }

    /// Store metadata
    pub async fn store_metadata(&self, metadata: GraphRAGMetadata) -> ProtocolResult<()> {
        // Store in memory
        {
            let mut meta_guard = self.metadata.write().await;
            *meta_guard = Some(metadata.clone());
        }

        // Persist to RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let metadata_cf = db.cf_handle("metadata").ok_or_else(|| {
                ProtocolError::Other("Metadata column family not found".to_string())
            })?;

            let key = format!("kg:{}", self.kg_name);
            let value = serde_json::to_vec(&metadata).map_err(|e| {
                ProtocolError::Other(format!("Failed to serialize metadata: {}", e))
            })?;

            db.put_cf(metadata_cf, key.as_bytes(), &value)
                .map_err(|e| {
                    ProtocolError::Other(format!("Failed to persist metadata to RocksDB: {}", e))
                })?;
        }

        Ok(())
    }

    /// Get metadata
    pub async fn get_metadata(&self) -> ProtocolResult<Option<GraphRAGMetadata>> {
        // Check memory first
        {
            let metadata = self.metadata.read().await;
            if let Some(ref meta) = *metadata {
                return Ok(Some(meta.clone()));
            }
        }

        // Load from RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let metadata_cf = db.cf_handle("metadata").ok_or_else(|| {
                ProtocolError::Other("Metadata column family not found".to_string())
            })?;

            let key = format!("kg:{}", self.kg_name);
            if let Ok(Some(value)) = db.get_cf(metadata_cf, key.as_bytes()) {
                if let Ok(metadata) = serde_json::from_slice::<GraphRAGMetadata>(&value) {
                    let mut meta_guard = self.metadata.write().await;
                    *meta_guard = Some(metadata.clone());
                    return Ok(Some(metadata));
                }
            }
        }

        Ok(None)
    }

    /// Get all nodes
    pub async fn list_nodes(&self) -> ProtocolResult<Vec<GraphRAGNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    /// Get all relationships
    pub async fn list_relationships(&self) -> ProtocolResult<Vec<GraphRAGRelationship>> {
        let relationships = self.relationships.read().await;
        Ok(relationships.values().cloned().collect())
    }

    /// Get node count
    pub async fn node_count(&self) -> ProtocolResult<usize> {
        let nodes = self.nodes.read().await;
        Ok(nodes.len())
    }

    /// Get relationship count
    pub async fn relationship_count(&self) -> ProtocolResult<usize> {
        let relationships = self.relationships.read().await;
        Ok(relationships.len())
    }
}

/// Relationship direction for queries
#[derive(Debug, Clone, Copy)]
pub enum RelationshipDirection {
    /// Outgoing relationships
    Outgoing,
    /// Incoming relationships
    Incoming,
    /// Both directions
    Both,
}
