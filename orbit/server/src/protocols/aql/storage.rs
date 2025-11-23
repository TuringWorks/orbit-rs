//! AQL document and graph storage with RocksDB persistence
//!
//! This module provides persistent storage for AQL/ArangoDB document and graph data using RocksDB.

use crate::protocols::aql::data_model::{AqlCollection, AqlDocument};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// AQL storage with RocksDB persistence
pub struct AqlStorage {
    /// RocksDB instance
    db: Arc<RwLock<Option<Arc<DB>>>>,
    /// Data directory path
    data_dir: PathBuf,
    /// In-memory cache for collections
    collections: Arc<RwLock<HashMap<String, AqlCollection>>>,
    /// In-memory cache for documents (collection -> documents)
    documents: Arc<RwLock<HashMap<String, HashMap<String, AqlDocument>>>>,
}

impl AqlStorage {
    /// Create a new AQL storage with data directory
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        Self {
            db: Arc::new(RwLock::new(None)),
            data_dir: data_dir.as_ref().to_path_buf(),
            collections: Arc::new(RwLock::new(HashMap::new())),
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize RocksDB storage
    pub async fn initialize(&self) -> ProtocolResult<()> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Define column families for AQL data
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new("collections", Options::default()),
            ColumnFamilyDescriptor::new("documents", Options::default()),
            ColumnFamilyDescriptor::new("edges", Options::default()),
            ColumnFamilyDescriptor::new("graphs", Options::default()),
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
                info!("AqlStorage: RocksDB initialized at {:?}", db_path);
            }
            Err(e) => {
                error!("Failed to open AQL RocksDB at {:?}: {}", db_path, e);
                return Err(ProtocolError::Other(format!(
                    "Failed to initialize AQL RocksDB: {}",
                    e
                )));
            }
        }
        Ok(())
    }

    /// Load data from RocksDB on startup
    async fn load_from_rocksdb(&self, db: &Arc<DB>) -> ProtocolResult<()> {
        // Load collections
        let collections_cf = db.cf_handle("collections").ok_or_else(|| {
            ProtocolError::Other("Collections column family not found".to_string())
        })?;

        let coll_iter = db.iterator_cf(collections_cf, rocksdb::IteratorMode::Start);
        let mut collections = self.collections.write().await;
        let mut coll_count = 0;

        for item in coll_iter {
            match item {
                Ok((_key, value)) => {
                    if let Ok(collection) = serde_json::from_slice::<AqlCollection>(&value) {
                        collections.insert(collection.name.clone(), collection);
                        coll_count += 1;
                    }
                }
                Err(e) => {
                    error!("Error reading collection from RocksDB: {}", e);
                }
            }
        }

        // Load documents
        let docs_cf = db.cf_handle("documents").ok_or_else(|| {
            ProtocolError::Other("Documents column family not found".to_string())
        })?;

        let doc_iter = db.iterator_cf(docs_cf, rocksdb::IteratorMode::Start);
        let mut documents = self.documents.write().await;
        let mut doc_count = 0;

        for item in doc_iter {
            match item {
                Ok((_key, value)) => {
                    if let Ok(doc) = serde_json::from_slice::<AqlDocument>(&value) {
                        // Extract collection name from _id (format: "collection/key")
                        let collection_name = doc.id.split('/').next().unwrap_or("default").to_string();
                        documents
                            .entry(collection_name)
                            .or_insert_with(HashMap::new)
                            .insert(doc.key.clone(), doc);
                        doc_count += 1;
                    }
                }
                Err(e) => {
                    error!("Error reading document from RocksDB: {}", e);
                }
            }
        }

        info!(
            "AqlStorage: Loaded {} collections and {} documents from RocksDB",
            coll_count, doc_count
        );
        Ok(())
    }

    /// Store a collection
    pub async fn store_collection(&self, collection: AqlCollection) -> ProtocolResult<()> {
        // Store in memory
        self.collections.write().await.insert(collection.name.clone(), collection.clone());

        // Persist to RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let collections_cf = db.cf_handle("collections").ok_or_else(|| {
                ProtocolError::Other("Collections column family not found".to_string())
            })?;

            let key = format!("collection:{}", collection.name);
            let value = serde_json::to_vec(&collection).map_err(|e| {
                ProtocolError::Other(format!("Failed to serialize collection: {}", e))
            })?;

            db.put_cf(collections_cf, key.as_bytes(), &value).map_err(|e| {
                ProtocolError::Other(format!("Failed to persist collection to RocksDB: {}", e))
            })?;
        }

        Ok(())
    }

    /// Get a collection by name
    pub async fn get_collection(&self, name: &str) -> ProtocolResult<Option<AqlCollection>> {
        let collections = self.collections.read().await;
        Ok(collections.get(name).cloned())
    }

    /// Store a document
    pub async fn store_document(&self, doc: AqlDocument) -> ProtocolResult<()> {
        // Extract collection name from _id (format: "collection/key")
        let collection_name = doc.id.split('/').next().unwrap_or("default").to_string();
        
        // Store in memory
        {
            let mut documents = self.documents.write().await;
            documents
                .entry(collection_name.clone())
                .or_insert_with(HashMap::new)
                .insert(doc.key.clone(), doc.clone());
        }

        // Persist to RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let docs_cf = db.cf_handle("documents").ok_or_else(|| {
                ProtocolError::Other("Documents column family not found".to_string())
            })?;

            let key = format!("doc:{}:{}", collection_name, doc.key);
            let value = serde_json::to_vec(&doc).map_err(|e| {
                ProtocolError::Other(format!("Failed to serialize document: {}", e))
            })?;

            db.put_cf(docs_cf, key.as_bytes(), &value).map_err(|e| {
                ProtocolError::Other(format!("Failed to persist document to RocksDB: {}", e))
            })?;
        }

        Ok(())
    }

    /// Get a document by collection and key
    pub async fn get_document(&self, collection: &str, key: &str) -> ProtocolResult<Option<AqlDocument>> {
        let documents = self.documents.read().await;
        if let Some(coll_docs) = documents.get(collection) {
            return Ok(coll_docs.get(key).cloned());
        }
        Ok(None)
    }

    /// Get all documents in a collection
    pub async fn get_collection_documents(&self, collection: &str) -> ProtocolResult<Vec<AqlDocument>> {
        let documents = self.documents.read().await;
        if let Some(coll_docs) = documents.get(collection) {
            return Ok(coll_docs.values().cloned().collect());
        }
        Ok(Vec::new())
    }

    /// Shutdown and close RocksDB database
    /// This explicitly releases the RocksDB lock
    pub async fn shutdown(&self) -> ProtocolResult<()> {
        let mut db_guard = self.db.write().await;
        if let Some(db) = db_guard.take() {
            // Drop the Arc to close the database
            // RocksDB will release the lock when DB is dropped
            drop(db);
            info!("AqlStorage: RocksDB closed and lock released");
        }
        Ok(())
    }
}

