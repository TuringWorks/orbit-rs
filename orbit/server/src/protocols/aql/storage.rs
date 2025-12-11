//! AQL document and graph storage with RocksDB persistence
//!
//! This module provides persistent storage for AQL/ArangoDB document and graph data using RocksDB.

#![cfg(feature = "storage-rocksdb")]

use crate::protocols::aql::data_model::{AqlCollection, AqlDocument, AqlValue};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use async_trait::async_trait;
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Trait for AQL storage providers
/// This abstraction allows both RocksDB-based and unified storage backends
/// to be used interchangeably with AQL servers.
#[async_trait]
pub trait AqlStorageProvider: Send + Sync {
    /// Initialize the storage backend
    async fn initialize(&self) -> ProtocolResult<()>;

    /// Store a collection
    async fn store_collection(&self, collection: AqlCollection) -> ProtocolResult<()>;

    /// Get a collection by name
    async fn get_collection(&self, name: &str) -> ProtocolResult<Option<AqlCollection>>;

    /// Store a document
    async fn store_document(&self, doc: AqlDocument) -> ProtocolResult<()>;

    /// Get a document by collection and key
    async fn get_document(
        &self,
        collection: &str,
        key: &str,
    ) -> ProtocolResult<Option<AqlDocument>>;

    /// Get all documents in a collection
    async fn get_collection_documents(&self, collection: &str) -> ProtocolResult<Vec<AqlDocument>>;

    /// Delete a document by collection and key
    async fn delete_document(&self, collection: &str, key: &str) -> ProtocolResult<bool>;

    /// Update a document (merge with existing data)
    async fn update_document(
        &self,
        collection: &str,
        key: &str,
        updates: HashMap<String, AqlValue>,
    ) -> ProtocolResult<Option<AqlDocument>>;

    /// Check if a document exists
    async fn document_exists(&self, collection: &str, key: &str) -> bool;

    /// Shutdown the storage backend
    async fn shutdown(&self) -> ProtocolResult<()>;
}

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
        let docs_cf = db
            .cf_handle("documents")
            .ok_or_else(|| ProtocolError::Other("Documents column family not found".to_string()))?;

        let doc_iter = db.iterator_cf(docs_cf, rocksdb::IteratorMode::Start);
        let mut documents = self.documents.write().await;
        let mut doc_count = 0;

        for item in doc_iter {
            match item {
                Ok((_key, value)) => {
                    if let Ok(doc) = serde_json::from_slice::<AqlDocument>(&value) {
                        // Extract collection name from _id (format: "collection/key")
                        let collection_name =
                            doc.id.split('/').next().unwrap_or("default").to_string();
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
        self.collections
            .write()
            .await
            .insert(collection.name.clone(), collection.clone());

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

            db.put_cf(collections_cf, key.as_bytes(), &value)
                .map_err(|e| {
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
    pub async fn get_document(
        &self,
        collection: &str,
        key: &str,
    ) -> ProtocolResult<Option<AqlDocument>> {
        let documents = self.documents.read().await;
        if let Some(coll_docs) = documents.get(collection) {
            return Ok(coll_docs.get(key).cloned());
        }
        Ok(None)
    }

    /// Get all documents in a collection
    pub async fn get_collection_documents(
        &self,
        collection: &str,
    ) -> ProtocolResult<Vec<AqlDocument>> {
        let documents = self.documents.read().await;
        if let Some(coll_docs) = documents.get(collection) {
            return Ok(coll_docs.values().cloned().collect());
        }
        Ok(Vec::new())
    }

    /// Delete a document by collection and key
    pub async fn delete_document(&self, collection: &str, key: &str) -> ProtocolResult<bool> {
        // Delete from memory
        let deleted = {
            let mut documents = self.documents.write().await;
            if let Some(coll_docs) = documents.get_mut(collection) {
                coll_docs.remove(key).is_some()
            } else {
                false
            }
        };

        // Delete from RocksDB
        if deleted {
            let db_guard = self.db.read().await;
            if let Some(ref db) = *db_guard {
                let docs_cf = db.cf_handle("documents").ok_or_else(|| {
                    ProtocolError::Other("Documents column family not found".to_string())
                })?;

                let db_key = format!("doc:{}:{}", collection, key);
                db.delete_cf(docs_cf, db_key.as_bytes()).map_err(|e| {
                    ProtocolError::Other(format!("Failed to delete document from RocksDB: {}", e))
                })?;
            }
        }

        Ok(deleted)
    }

    /// Update a document (merge with existing data)
    pub async fn update_document(
        &self,
        collection: &str,
        key: &str,
        updates: HashMap<String, AqlValue>,
    ) -> ProtocolResult<Option<AqlDocument>> {
        // Get and update in memory
        let updated_doc = {
            let mut documents = self.documents.write().await;
            if let Some(coll_docs) = documents.get_mut(collection) {
                if let Some(doc) = coll_docs.get_mut(key) {
                    // Merge updates into existing document
                    for (field, value) in updates {
                        doc.data.insert(field, value);
                    }
                    // Update revision
                    doc.revision = format!("_{}", chrono::Utc::now().timestamp());
                    Some(doc.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Persist to RocksDB if updated
        if let Some(ref doc) = updated_doc {
            let db_guard = self.db.read().await;
            if let Some(ref db) = *db_guard {
                let docs_cf = db.cf_handle("documents").ok_or_else(|| {
                    ProtocolError::Other("Documents column family not found".to_string())
                })?;

                let db_key = format!("doc:{}:{}", collection, key);
                let value = serde_json::to_vec(&doc).map_err(|e| {
                    ProtocolError::Other(format!("Failed to serialize document: {}", e))
                })?;

                db.put_cf(docs_cf, db_key.as_bytes(), &value).map_err(|e| {
                    ProtocolError::Other(format!("Failed to persist document to RocksDB: {}", e))
                })?;
            }
        }

        Ok(updated_doc)
    }

    /// Check if a document exists
    pub async fn document_exists(&self, collection: &str, key: &str) -> bool {
        let documents = self.documents.read().await;
        if let Some(coll_docs) = documents.get(collection) {
            return coll_docs.contains_key(key);
        }
        false
    }

    /// List all collection names
    pub async fn list_collections(&self) -> ProtocolResult<Vec<String>> {
        let collections = self.collections.read().await;
        Ok(collections.keys().cloned().collect())
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

/// Implement the AqlStorageProvider trait for AqlStorage
#[async_trait]
impl AqlStorageProvider for AqlStorage {
    async fn initialize(&self) -> ProtocolResult<()> {
        AqlStorage::initialize(self).await
    }

    async fn store_collection(&self, collection: AqlCollection) -> ProtocolResult<()> {
        AqlStorage::store_collection(self, collection).await
    }

    async fn get_collection(&self, name: &str) -> ProtocolResult<Option<AqlCollection>> {
        AqlStorage::get_collection(self, name).await
    }

    async fn store_document(&self, doc: AqlDocument) -> ProtocolResult<()> {
        AqlStorage::store_document(self, doc).await
    }

    async fn get_document(
        &self,
        collection: &str,
        key: &str,
    ) -> ProtocolResult<Option<AqlDocument>> {
        AqlStorage::get_document(self, collection, key).await
    }

    async fn get_collection_documents(&self, collection: &str) -> ProtocolResult<Vec<AqlDocument>> {
        AqlStorage::get_collection_documents(self, collection).await
    }

    async fn delete_document(&self, collection: &str, key: &str) -> ProtocolResult<bool> {
        AqlStorage::delete_document(self, collection, key).await
    }

    async fn update_document(
        &self,
        collection: &str,
        key: &str,
        updates: HashMap<String, AqlValue>,
    ) -> ProtocolResult<Option<AqlDocument>> {
        AqlStorage::update_document(self, collection, key, updates).await
    }

    async fn document_exists(&self, collection: &str, key: &str) -> bool {
        AqlStorage::document_exists(self, collection, key).await
    }

    async fn shutdown(&self) -> ProtocolResult<()> {
        AqlStorage::shutdown(self).await
    }
}
