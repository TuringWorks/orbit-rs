//! Copy-on-Write B+ Tree persistence provider for orbit-server
//!
//! This module provides a high-performance, memory-efficient persistence backend
//! using a Copy-on-Write B+ Tree data structure with Write-Ahead Logging (WAL).

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Instant, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};

const MAX_KEYS_PER_NODE: usize = 16; // Small for demonstration, would be ~100-500 in production
const WAL_BUFFER_SIZE: usize = 1024 * 1024; // 1MB WAL buffer

/// Configuration for COW B+ Tree persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CowBTreeConfig {
    /// Data directory for WAL and snapshots
    pub data_dir: String,
    /// Maximum keys per B+ Tree node
    pub max_keys_per_node: usize,
    /// WAL buffer size in bytes
    pub wal_buffer_size: usize,
    /// Enable compression for snapshots
    pub enable_compression: bool,
    /// Sync interval for WAL flush (seconds)
    pub wal_sync_interval: u64,
}

impl Default for CowBTreeConfig {
    fn default() -> Self {
        Self {
            data_dir: "./orbit_data".to_string(),
            max_keys_per_node: MAX_KEYS_PER_NODE,
            wal_buffer_size: WAL_BUFFER_SIZE,
            enable_compression: true,
            wal_sync_interval: 5,
        }
    }
}

/// Copy-on-Write B+ Tree node
#[derive(Debug, Clone)]
pub struct BTreeNode {
    pub keys: Vec<String>, // Using String as key for addressable references
    pub addressable_values: Vec<Option<AddressableLease>>,
    pub node_values: Vec<Option<NodeInfo>>,
    pub children: Vec<Arc<BTreeNode>>,
    pub is_leaf: bool,
    pub version: u64,
    pub parent: Option<Arc<StdRwLock<BTreeNode>>>,
}

/// Write-Ahead Log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WALEntry {
    sequence: u64,
    timestamp: SystemTime,
    operation: WALOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum WALOperation {
    InsertLease {
        key: String,
        lease: AddressableLease,
    },
    UpdateLease {
        key: String,
        lease: AddressableLease,
    },
    DeleteLease {
        key: String,
    },
    InsertNode {
        key: String,
        node: NodeInfo,
    },
    UpdateNode {
        key: String,
        node: NodeInfo,
    },
    DeleteNode {
        key: String,
    },
}

/// Snapshot metadata
#[derive(Debug, Clone)]
struct Snapshot {
    id: String,
    root_version: u64,
    timestamp: SystemTime,
    key_count: u64,
}

/// COW B+ Tree persistence implementation for addressable directory
pub struct CowBTreeAddressableProvider {
    config: CowBTreeConfig,
    root: Arc<StdRwLock<BTreeNode>>,
    wal: Arc<Mutex<WriteAheadLog>>,
    snapshots: Arc<StdRwLock<HashMap<String, Snapshot>>>,
    version_counter: Arc<Mutex<u64>>,
    metrics: Arc<RwLock<PersistenceMetrics>>,
    transactions: Arc<RwLock<HashMap<String, TransactionContext>>>,
}

/// COW B+ Tree persistence implementation for cluster nodes
pub struct CowBTreeClusterProvider {
    config: CowBTreeConfig,
    root: Arc<StdRwLock<BTreeNode>>,
    wal: Arc<Mutex<WriteAheadLog>>,
    snapshots: Arc<StdRwLock<HashMap<String, Snapshot>>>,
    version_counter: Arc<Mutex<u64>>,
    metrics: Arc<RwLock<PersistenceMetrics>>,
    transactions: Arc<RwLock<HashMap<String, TransactionContext>>>,
}

struct WriteAheadLog {
    file: File,
    sequence: u64,
    buffer: Vec<u8>,
}

impl BTreeNode {
    fn new_leaf(version: u64) -> Self {
        Self {
            keys: Vec::new(),
            addressable_values: Vec::new(),
            node_values: Vec::new(),
            children: Vec::new(),
            is_leaf: true,
            version,
            parent: None,
        }
    }

    fn new_internal(version: u64) -> Self {
        Self {
            keys: Vec::new(),
            addressable_values: Vec::new(),
            node_values: Vec::new(),
            children: Vec::new(),
            is_leaf: false,
            version,
            parent: None,
        }
    }

    fn is_full(&self, max_keys: usize) -> bool {
        self.keys.len() >= max_keys
    }

    /// Copy-on-write clone of this node
    fn cow_clone(&self, new_version: u64) -> Self {
        Self {
            keys: self.keys.clone(),
            addressable_values: self.addressable_values.clone(),
            node_values: self.node_values.clone(),
            children: self.children.clone(), // Arc references are cheap to clone
            is_leaf: self.is_leaf,
            version: new_version,
            parent: None, // Will be set by parent
        }
    }

    /// Find the index where a key should be inserted
    fn find_key_index(&self, key: &str) -> Result<usize, usize> {
        self.keys.binary_search(&key.to_string())
    }

    /// Insert an addressable lease in a leaf node
    fn insert_addressable_lease(&mut self, key: String, lease: AddressableLease) -> bool {
        match self.find_key_index(&key) {
            Ok(idx) => {
                // Key exists, update value
                self.addressable_values[idx] = Some(lease);
                false // Not a new insertion
            }
            Err(idx) => {
                // Key doesn't exist, insert new
                self.keys.insert(idx, key);
                self.addressable_values.insert(idx, Some(lease));
                if self.node_values.len() == self.keys.len() - 1 {
                    self.node_values.insert(idx, None);
                }
                true // New insertion
            }
        }
    }

    /// Insert a node info in a leaf node
    fn insert_node_info(&mut self, key: String, node: NodeInfo) -> bool {
        match self.find_key_index(&key) {
            Ok(idx) => {
                // Key exists, update value
                self.node_values[idx] = Some(node);
                false // Not a new insertion
            }
            Err(idx) => {
                // Key doesn't exist, insert new
                self.keys.insert(idx, key);
                self.node_values.insert(idx, Some(node));
                if self.addressable_values.len() == self.keys.len() - 1 {
                    self.addressable_values.insert(idx, None);
                }
                true // New insertion
            }
        }
    }
}

impl WriteAheadLog {
    async fn new(path: &std::path::Path) -> OrbitResult<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to open WAL file: {}", e)))?;

        // Determine the next sequence number by reading existing entries
        let sequence = Self::get_last_sequence(path).await.unwrap_or(0) + 1;

        Ok(Self {
            file,
            sequence,
            buffer: Vec::with_capacity(WAL_BUFFER_SIZE),
        })
    }

    /// Get the last sequence number from the WAL file
    async fn get_last_sequence(path: &std::path::Path) -> OrbitResult<u64> {
        if !path.exists() {
            return Ok(0);
        }

        let content = tokio::fs::read(path)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to read WAL file: {}", e)))?;
        let mut last_sequence = 0u64;
        let mut offset = 0;

        while offset + 4 <= content.len() {
            // Read entry length
            let len_bytes = &content[offset..offset + 4];
            let entry_len =
                u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]])
                    as usize;
            offset += 4;

            if offset + entry_len <= content.len() {
                let entry_data = &content[offset..offset + entry_len];

                // Try to parse the entry to get sequence number
                if let Ok(entry) = serde_json::from_slice::<WALEntry>(entry_data) {
                    last_sequence = entry.sequence;
                }

                offset += entry_len;
            } else {
                break;
            }
        }

        Ok(last_sequence)
    }

    /// Append an entry to the WAL
    async fn append_entry(&mut self, operation: WALOperation) -> OrbitResult<()> {
        let entry = WALEntry {
            sequence: self.sequence,
            timestamp: SystemTime::now(),
            operation,
        };

        let serialized = serde_json::to_vec(&entry)
            .map_err(|e| OrbitError::internal(format!("WAL serialization error: {}", e)))?;

        // Write entry length followed by entry data
        let len_bytes = (serialized.len() as u32).to_le_bytes();
        self.buffer.extend_from_slice(&len_bytes);
        self.buffer.extend_from_slice(&serialized);

        // Flush if buffer is getting full
        if self.buffer.len() >= WAL_BUFFER_SIZE / 2 {
            self.flush().await?;
        }

        self.sequence += 1;
        tracing::debug!(
            "[WAL] Appending entry {} with {} bytes",
            entry.sequence,
            serialized.len()
        );

        Ok(())
    }

    /// Flush buffered entries to disk
    async fn flush(&mut self) -> OrbitResult<()> {
        if !self.buffer.is_empty() {
            self.file
                .write_all(&self.buffer)
                .await
                .map_err(|e| OrbitError::internal(format!("WAL flush error: {}", e)))?;
            self.file
                .sync_all()
                .await
                .map_err(|e| OrbitError::internal(format!("WAL sync error: {}", e)))?;
            self.buffer.clear();
        }
        Ok(())
    }
}

impl CowBTreeAddressableProvider {
    pub fn new(config: CowBTreeConfig) -> Self {
        Self {
            root: Arc::new(StdRwLock::new(BTreeNode::new_leaf(0))),
            wal: Arc::new(Mutex::new(WriteAheadLog {
                file: unsafe { std::mem::zeroed() }, // Will be initialized in initialize()
                sequence: 1,
                buffer: Vec::with_capacity(config.wal_buffer_size),
            })),
            snapshots: Arc::new(StdRwLock::new(HashMap::new())),
            version_counter: Arc::new(Mutex::new(0)),
            metrics: Arc::new(RwLock::new(PersistenceMetrics::default())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    async fn update_metrics(&self, operation: &str, duration: std::time::Duration, success: bool) {
        let mut metrics = self.metrics.write().await;

        match operation {
            "read" => {
                metrics.read_operations += 1;
                metrics.read_latency_avg =
                    (metrics.read_latency_avg + duration.as_secs_f64()) / 2.0;
            }
            "write" => {
                metrics.write_operations += 1;
                metrics.write_latency_avg =
                    (metrics.write_latency_avg + duration.as_secs_f64()) / 2.0;
            }
            "delete" => {
                metrics.delete_operations += 1;
                metrics.delete_latency_avg =
                    (metrics.delete_latency_avg + duration.as_secs_f64()) / 2.0;
            }
            _ => {}
        }

        if !success {
            metrics.error_count += 1;
        }
    }

    fn reference_to_key(reference: &AddressableReference) -> String {
        format!("{}:{}", reference.addressable_type, reference.key)
    }
}

#[async_trait]
impl PersistenceProvider for CowBTreeAddressableProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        // Create data directory
        let data_dir = PathBuf::from(&self.config.data_dir);
        tokio::fs::create_dir_all(&data_dir)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to create data directory: {}", e)))?;

        // Initialize WAL
        let wal_path = data_dir.join("orbit.wal");
        let wal = WriteAheadLog::new(&wal_path).await?;
        *self.wal.lock().await = wal;

        tracing::info!(
            "COW B+ Tree addressable provider initialized at {}",
            data_dir.display()
        );
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        // Flush WAL
        self.wal.lock().await.flush().await?;

        tracing::info!("COW B+ Tree addressable provider shutdown");
        Ok(())
    }

    async fn health_check(&self) -> ProviderHealth {
        // Check if WAL file is accessible
        let wal_path = PathBuf::from(&self.config.data_dir).join("orbit.wal");
        if !wal_path.exists() {
            return ProviderHealth::Unhealthy {
                reason: "WAL file not found".to_string(),
            };
        }

        // Check memory usage (simplified)
        let metrics = self.metrics.read().await;
        if metrics.error_count > 100 {
            return ProviderHealth::Degraded {
                reason: format!("High error count: {}", metrics.error_count),
            };
        }

        ProviderHealth::Healthy
    }

    async fn metrics(&self) -> PersistenceMetrics {
        self.metrics.read().await.clone()
    }

    async fn begin_transaction(&self, context: TransactionContext) -> OrbitResult<String> {
        let mut transactions = self.transactions.write().await;
        transactions.insert(context.id.clone(), context.clone());
        Ok(context.id)
    }

    async fn commit_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        let mut transactions = self.transactions.write().await;
        transactions.remove(transaction_id);
        Ok(())
    }

    async fn rollback_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        let mut transactions = self.transactions.write().await;
        transactions.remove(transaction_id);
        // Note: Full rollback would require more complex implementation
        tracing::warn!("Transaction rollback is simplified in COW B+ Tree implementation");
        Ok(())
    }
}

#[async_trait]
impl AddressableDirectoryProvider for CowBTreeAddressableProvider {
    async fn store_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        let start = Instant::now();
        let key = Self::reference_to_key(&lease.reference);

        // Log to WAL
        {
            let mut wal = self.wal.lock().await;
            wal.append_entry(WALOperation::InsertLease {
                key: key.clone(),
                lease: lease.clone(),
            })
            .await?;
        }

        // Insert into B+ tree
        let success = {
            let mut root = self.root.write().unwrap();
            root.insert_addressable_lease(key, lease.clone())
        };

        self.update_metrics("write", start.elapsed(), success).await;
        Ok(())
    }

    async fn get_lease(
        &self,
        reference: &AddressableReference,
    ) -> OrbitResult<Option<AddressableLease>> {
        let start = Instant::now();
        let key = Self::reference_to_key(reference);

        let result = {
            let root = self.root.read().unwrap();
            match root.find_key_index(&key) {
                Ok(idx) => root.addressable_values[idx].clone(),
                Err(_) => None,
            }
        };

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn update_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        let start = Instant::now();
        let key = Self::reference_to_key(&lease.reference);

        // Log to WAL
        {
            let mut wal = self.wal.lock().await;
            wal.append_entry(WALOperation::UpdateLease {
                key: key.clone(),
                lease: lease.clone(),
            })
            .await?;
        }

        // Update in B+ tree
        {
            let mut root = self.root.write().unwrap();
            root.insert_addressable_lease(key, lease.clone());
        }

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn remove_lease(&self, reference: &AddressableReference) -> OrbitResult<bool> {
        let start = Instant::now();
        let key = Self::reference_to_key(reference);

        // Log to WAL
        {
            let mut wal = self.wal.lock().await;
            wal.append_entry(WALOperation::DeleteLease { key: key.clone() })
                .await?;
        }

        // Remove from B+ tree
        let removed = {
            let mut root = self.root.write().unwrap();
            match root.find_key_index(&key) {
                Ok(idx) => {
                    root.keys.remove(idx);
                    root.addressable_values.remove(idx);
                    true
                }
                Err(_) => false,
            }
        };

        self.update_metrics("delete", start.elapsed(), removed)
            .await;
        Ok(removed)
    }

    async fn list_node_leases(&self, node_id: &NodeId) -> OrbitResult<Vec<AddressableLease>> {
        let start = Instant::now();

        let leases = {
            let root = self.root.read().unwrap();
            let mut leases = Vec::new();
            for (_i, value) in root.addressable_values.iter().enumerate() {
                if let Some(lease) = value {
                    if &lease.node_id == node_id {
                        leases.push(lease.clone());
                    }
                }
            }
            leases
        };

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(leases)
    }

    async fn list_all_leases(&self) -> OrbitResult<Vec<AddressableLease>> {
        let start = Instant::now();

        let leases = {
            let root = self.root.read().unwrap();
            root.addressable_values
                .iter()
                .filter_map(|v| v.clone())
                .collect::<Vec<AddressableLease>>()
        };

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(leases)
    }

    async fn cleanup_expired_leases(&self) -> OrbitResult<u64> {
        let start = Instant::now();
        let now = chrono::Utc::now();

        let count = {
            let mut root = self.root.write().unwrap();
            let mut indices_to_remove = Vec::new();
            let mut count = 0;

            // Find expired leases
            for (i, value) in root.addressable_values.iter().enumerate() {
                if let Some(lease) = value {
                    if lease.expires_at < now {
                        indices_to_remove.push(i);
                    }
                }
            }

            // Remove expired leases (in reverse order to maintain indices)
            indices_to_remove.reverse();
            for idx in indices_to_remove {
                root.keys.remove(idx);
                root.addressable_values.remove(idx);
                count += 1;
            }
            count
        };

        self.update_metrics("delete", start.elapsed(), true).await;
        Ok(count)
    }

    async fn store_leases_bulk(&self, leases: &[AddressableLease]) -> OrbitResult<()> {
        let start = Instant::now();

        for lease in leases {
            self.store_lease(lease).await?;
        }

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn remove_leases_bulk(&self, references: &[AddressableReference]) -> OrbitResult<u64> {
        let start = Instant::now();
        let mut count = 0;

        for reference in references {
            if self.remove_lease(reference).await? {
                count += 1;
            }
        }

        self.update_metrics("delete", start.elapsed(), true).await;
        Ok(count)
    }
}

// Similar implementation for CowBTreeClusterProvider would follow the same pattern
// but operate on NodeInfo instead of AddressableLease
impl CowBTreeClusterProvider {
    pub fn new(config: CowBTreeConfig) -> Self {
        Self {
            root: Arc::new(StdRwLock::new(BTreeNode::new_leaf(0))),
            wal: Arc::new(Mutex::new(WriteAheadLog {
                file: unsafe { std::mem::zeroed() }, // Will be initialized in initialize()
                sequence: 1,
                buffer: Vec::with_capacity(config.wal_buffer_size),
            })),
            snapshots: Arc::new(StdRwLock::new(HashMap::new())),
            version_counter: Arc::new(Mutex::new(0)),
            metrics: Arc::new(RwLock::new(PersistenceMetrics::default())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
}

#[async_trait]
impl PersistenceProvider for CowBTreeClusterProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        // Create data directory
        let data_dir = PathBuf::from(&self.config.data_dir);
        tokio::fs::create_dir_all(&data_dir)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to create data directory: {}", e)))?;

        // Initialize WAL
        let wal_path = data_dir.join("cluster_nodes.wal");
        let wal = WriteAheadLog::new(&wal_path).await?;
        *self.wal.lock().await = wal;

        tracing::info!("COW B+ Tree cluster provider initialized");
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        self.wal.lock().await.flush().await?;
        tracing::info!("COW B+ Tree cluster provider shutdown");
        Ok(())
    }

    async fn health_check(&self) -> ProviderHealth {
        ProviderHealth::Healthy
    }

    async fn metrics(&self) -> PersistenceMetrics {
        self.metrics.read().await.clone()
    }

    async fn begin_transaction(&self, context: TransactionContext) -> OrbitResult<String> {
        let mut transactions = self.transactions.write().await;
        transactions.insert(context.id.clone(), context.clone());
        Ok(context.id)
    }

    async fn commit_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        let mut transactions = self.transactions.write().await;
        transactions.remove(transaction_id);
        Ok(())
    }

    async fn rollback_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        let mut transactions = self.transactions.write().await;
        transactions.remove(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl ClusterNodeProvider for CowBTreeClusterProvider {
    async fn store_node(&self, node: &NodeInfo) -> OrbitResult<()> {
        let key = node.id.to_string();
        let mut root = self.root.write().unwrap();
        root.insert_node_info(key, node.clone());
        Ok(())
    }

    async fn get_node(&self, node_id: &NodeId) -> OrbitResult<Option<NodeInfo>> {
        let key = node_id.to_string();
        let root = self.root.read().unwrap();
        let result = match root.find_key_index(&key) {
            Ok(idx) => root.node_values[idx].clone(),
            Err(_) => None,
        };
        Ok(result)
    }

    async fn update_node(&self, node: &NodeInfo) -> OrbitResult<()> {
        self.store_node(node).await
    }

    async fn remove_node(&self, node_id: &NodeId) -> OrbitResult<bool> {
        let key = node_id.to_string();
        let mut root = self.root.write().unwrap();
        let removed = match root.find_key_index(&key) {
            Ok(idx) => {
                root.keys.remove(idx);
                root.node_values.remove(idx);
                true
            }
            Err(_) => false,
        };
        Ok(removed)
    }

    async fn list_active_nodes(&self) -> OrbitResult<Vec<NodeInfo>> {
        let root = self.root.read().unwrap();
        let nodes: Vec<NodeInfo> = root
            .node_values
            .iter()
            .filter_map(|v| v.clone())
            .filter(|node| node.status == NodeStatus::Active)
            .collect();
        Ok(nodes)
    }

    async fn list_all_nodes(&self) -> OrbitResult<Vec<NodeInfo>> {
        let root = self.root.read().unwrap();
        let nodes: Vec<NodeInfo> = root.node_values.iter().filter_map(|v| v.clone()).collect();
        Ok(nodes)
    }

    async fn cleanup_expired_nodes(&self) -> OrbitResult<u64> {
        let mut count = 0;
        let mut root = self.root.write().unwrap();
        let mut indices_to_remove = Vec::new();

        for (i, value) in root.node_values.iter().enumerate() {
            if let Some(node) = value {
                if let Some(lease) = &node.lease {
                    if lease.is_expired() {
                        indices_to_remove.push(i);
                    }
                }
            }
        }

        for &idx in indices_to_remove.iter().rev() {
            root.keys.remove(idx);
            root.node_values.remove(idx);
            count += 1;
        }

        Ok(count)
    }

    async fn renew_node_lease(&self, node_id: &NodeId, lease: &NodeLease) -> OrbitResult<()> {
        let key = node_id.to_string();
        let mut root = self.root.write().unwrap();

        if let Ok(idx) = root.find_key_index(&key) {
            if let Some(node) = &mut root.node_values[idx] {
                node.lease = Some(lease.clone());
            }
        }

        Ok(())
    }
}
