//! Cypher graph storage with RocksDB persistence
//!
//! This module provides persistent storage for Cypher/Neo4j graph data using RocksDB.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Graph node stored in RocksDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Graph relationship stored in RocksDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRelationship {
    pub id: String,
    pub start_node: String,
    pub end_node: String,
    pub rel_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Cypher graph storage with RocksDB persistence
#[derive(Debug)]
pub struct CypherGraphStorage {
    /// RocksDB instance
    db: Arc<RwLock<Option<Arc<DB>>>>,
    /// Data directory path
    data_dir: PathBuf,
    /// In-memory cache for nodes
    nodes: Arc<RwLock<HashMap<String, GraphNode>>>,
    /// In-memory cache for relationships
    relationships: Arc<RwLock<HashMap<String, GraphRelationship>>>,
}

impl CypherGraphStorage {
    /// Create a new Cypher graph storage with data directory
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        Self {
            db: Arc::new(RwLock::new(None)),
            data_dir: data_dir.as_ref().to_path_buf(),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            relationships: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize RocksDB storage
    pub async fn initialize(&self) -> ProtocolResult<()> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Define column families for graph data
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new("nodes", Options::default()),
            ColumnFamilyDescriptor::new("relationships", Options::default()),
            ColumnFamilyDescriptor::new("metadata", Options::default()),
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
                info!("CypherGraphStorage: RocksDB initialized at {:?}", db_path);
            }
            Err(e) => {
                error!("Failed to open Cypher RocksDB at {:?}: {}", db_path, e);
                return Err(ProtocolError::Other(format!(
                    "Failed to initialize Cypher RocksDB: {}",
                    e
                )));
            }
        }
        Ok(())
    }

    /// Load data from RocksDB on startup
    async fn load_from_rocksdb(&self, db: &Arc<DB>) -> ProtocolResult<()> {
        // Load nodes
        let nodes_cf = db.cf_handle("nodes").ok_or_else(|| {
            ProtocolError::Other("Nodes column family not found".to_string())
        })?;

        let node_iter = db.iterator_cf(nodes_cf, rocksdb::IteratorMode::Start);
        let mut nodes = self.nodes.write().await;
        let mut node_count = 0;

        for item in node_iter {
            match item {
                Ok((_key, value)) => {
                    if let Ok(node) = serde_json::from_slice::<GraphNode>(&value) {
                        nodes.insert(node.id.clone(), node);
                        node_count += 1;
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
                    if let Ok(rel) = serde_json::from_slice::<GraphRelationship>(&value) {
                        relationships.insert(rel.id.clone(), rel);
                        rel_count += 1;
                    }
                }
                Err(e) => {
                    error!("Error reading relationship from RocksDB: {}", e);
                }
            }
        }

        info!(
            "CypherGraphStorage: Loaded {} nodes and {} relationships from RocksDB",
            node_count, rel_count
        );
        Ok(())
    }

    /// Store a node
    pub async fn store_node(&self, node: GraphNode) -> ProtocolResult<()> {
        // Store in memory
        self.nodes.write().await.insert(node.id.clone(), node.clone());

        // Persist to RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let nodes_cf = db.cf_handle("nodes").ok_or_else(|| {
                ProtocolError::Other("Nodes column family not found".to_string())
            })?;

            let key = format!("node:{}", node.id);
            let value = serde_json::to_vec(&node).map_err(|e| {
                ProtocolError::Other(format!("Failed to serialize node: {}", e))
            })?;

            db.put_cf(nodes_cf, key.as_bytes(), &value).map_err(|e| {
                ProtocolError::Other(format!("Failed to persist node to RocksDB: {}", e))
            })?;
        }

        Ok(())
    }

    /// Get a node by ID
    pub async fn get_node(&self, node_id: &str) -> ProtocolResult<Option<GraphNode>> {
        // Check memory first
        let nodes = self.nodes.read().await;
        if let Some(node) = nodes.get(node_id) {
            return Ok(Some(node.clone()));
        }
        Ok(None)
    }

    /// Store a relationship
    pub async fn store_relationship(&self, rel: GraphRelationship) -> ProtocolResult<()> {
        // Store in memory
        self.relationships.write().await.insert(rel.id.clone(), rel.clone());

        // Persist to RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let rels_cf = db.cf_handle("relationships").ok_or_else(|| {
                ProtocolError::Other("Relationships column family not found".to_string())
            })?;

            let key = format!("rel:{}", rel.id);
            let value = serde_json::to_vec(&rel).map_err(|e| {
                ProtocolError::Other(format!("Failed to serialize relationship: {}", e))
            })?;

            db.put_cf(rels_cf, key.as_bytes(), &value).map_err(|e| {
                ProtocolError::Other(format!("Failed to persist relationship to RocksDB: {}", e))
            })?;
        }

        Ok(())
    }

    /// Get a relationship by ID
    pub async fn get_relationship(&self, rel_id: &str) -> ProtocolResult<Option<GraphRelationship>> {
        // Check memory first
        let relationships = self.relationships.read().await;
        if let Some(rel) = relationships.get(rel_id) {
            return Ok(Some(rel.clone()));
        }
        Ok(None)
    }

    /// Get all nodes
    pub async fn get_all_nodes(&self) -> ProtocolResult<Vec<GraphNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    /// Get all relationships
    pub async fn get_all_relationships(&self) -> ProtocolResult<Vec<GraphRelationship>> {
        let relationships = self.relationships.read().await;
        Ok(relationships.values().cloned().collect())
    }

    /// Shutdown and close RocksDB database
    /// This explicitly releases the RocksDB lock
    pub async fn shutdown(&self) -> ProtocolResult<()> {
        let mut db_guard = self.db.write().await;
        if let Some(db) = db_guard.take() {
            // Drop the Arc to close the database
            // RocksDB will release the lock when DB is dropped
            drop(db);
            info!("CypherGraphStorage: RocksDB closed and lock released");
        }
        Ok(())
    }
}

