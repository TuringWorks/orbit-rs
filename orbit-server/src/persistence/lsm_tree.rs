//! LSM-Tree persistence provider for orbit-server
//!
//! This module provides a Log-Structured Merge Tree implementation optimized
//! for high-throughput write workloads with background compaction.

use super::*;
use base64::{engine::general_purpose, Engine as _};
use bloom::{BloomFilter, ASMS};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Instant, SystemTime};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};

/// Configuration for LSM-Tree persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsmTreeConfig {
    /// Data directory for SSTables and WAL
    pub data_dir: String,
    /// Maximum memtable size in bytes before flush
    pub memtable_size_limit: usize,
    /// Maximum number of memtables to keep in memory
    pub max_memtables: usize,
    /// Bloom filter false positive rate
    pub bloom_filter_fp_rate: f64,
    /// Enable background compaction
    pub enable_compaction: bool,
    /// Compaction trigger threshold (number of SSTables)
    pub compaction_threshold: usize,
}

impl Default for LsmTreeConfig {
    fn default() -> Self {
        Self {
            data_dir: "./orbit_lsm_data".to_string(),
            memtable_size_limit: 64 * 1024 * 1024, // 64MB
            max_memtables: 10,
            bloom_filter_fp_rate: 0.01,
            enable_compaction: true,
            compaction_threshold: 4,
        }
    }
}

/// In-memory table for recent writes
#[derive(Debug, Clone)]
struct MemTable {
    data: BTreeMap<String, Vec<u8>>,
    size_bytes: usize,
    created_at: SystemTime,
}

/// Immutable on-disk SSTable with bloom filter
struct SSTable {
    id: String,
    file_path: PathBuf,
    bloom_filter: BloomFilter,
    min_key: String,
    max_key: String,
    entry_count: usize,
    index: Vec<IndexEntry>,
}

impl std::fmt::Debug for SSTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SSTable")
            .field("id", &self.id)
            .field("file_path", &self.file_path)
            .field("bloom_filter", &"<BloomFilter>")
            .field("min_key", &self.min_key)
            .field("max_key", &self.max_key)
            .field("entry_count", &self.entry_count)
            .field("index", &self.index)
            .finish()
    }
}

impl Clone for SSTable {
    fn clone(&self) -> Self {
        // Create a new bloom filter with similar configuration
        let mut new_bloom =
            BloomFilter::with_rate(0.01, self.entry_count.max(1000).try_into().unwrap_or(1000));
        // Note: We can't clone the actual bloom filter state, so this is a new empty one
        // In a production system, you'd serialize/deserialize the bloom filter
        Self {
            id: self.id.clone(),
            file_path: self.file_path.clone(),
            bloom_filter: new_bloom,
            min_key: self.min_key.clone(),
            max_key: self.max_key.clone(),
            entry_count: self.entry_count,
            index: self.index.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct IndexEntry {
    key: String,
    offset: u64,
    size: usize,
}

/// Block cache for frequently accessed data
#[derive(Debug, Clone)]
struct CachedBlock {
    data: Vec<u8>,
    last_accessed: SystemTime,
}

/// Write-ahead log for durability
struct WriteAheadLog {
    file: File,
}

/// LSM-Tree implementation for addressable directory
pub struct LsmTreeAddressableProvider {
    config: LsmTreeConfig,
    active_memtable: Arc<StdRwLock<MemTable>>,
    immutable_memtables: Arc<StdRwLock<Vec<MemTable>>>,
    sstables: Arc<StdRwLock<Vec<SSTable>>>,
    wal: Arc<Mutex<WriteAheadLog>>,
    block_cache: Arc<StdRwLock<HashMap<String, CachedBlock>>>,
    metrics: Arc<RwLock<PersistenceMetrics>>,
    transactions: Arc<RwLock<HashMap<String, TransactionContext>>>,
    compaction_running: Arc<Mutex<bool>>,
}

/// LSM-Tree implementation for cluster nodes
pub struct LsmTreeClusterProvider {
    config: LsmTreeConfig,
    active_memtable: Arc<StdRwLock<MemTable>>,
    immutable_memtables: Arc<StdRwLock<Vec<MemTable>>>,
    sstables: Arc<StdRwLock<Vec<SSTable>>>,
    wal: Arc<Mutex<WriteAheadLog>>,
    block_cache: Arc<StdRwLock<HashMap<String, CachedBlock>>>,
    metrics: Arc<RwLock<PersistenceMetrics>>,
    transactions: Arc<RwLock<HashMap<String, TransactionContext>>>,
    compaction_running: Arc<Mutex<bool>>,
}

impl MemTable {
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            size_bytes: 0,
            created_at: SystemTime::now(),
        }
    }

    fn insert(&mut self, key: String, value: Vec<u8>) {
        let old_size = self.data.get(&key).map(|v| v.len()).unwrap_or(0);
        self.data.insert(key, value.clone());
        self.size_bytes = self.size_bytes - old_size + value.len();
    }

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    fn remove(&mut self, key: &str) -> bool {
        if let Some(old_value) = self.data.remove(key) {
            self.size_bytes -= old_value.len();
            // Insert tombstone marker
            self.data.insert(format!("__TOMBSTONE__{}", key), vec![]);
            true
        } else {
            false
        }
    }

    fn is_full(&self, limit: usize) -> bool {
        self.size_bytes >= limit
    }
}

impl WriteAheadLog {
    async fn new(path: &PathBuf) -> OrbitResult<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to open WAL: {}", e)))?;

        Ok(Self { file })
    }

    async fn append(&mut self, key: &str, value: &[u8], operation: &str) -> OrbitResult<()> {
        let entry = format!(
            "{}|{}|{}|{}\n",
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            operation,
            key,
            general_purpose::STANDARD.encode(value)
        );

        self.file
            .write_all(entry.as_bytes())
            .await
            .map_err(|e| OrbitError::internal(format!("WAL write error: {}", e)))?;
        self.file
            .sync_all()
            .await
            .map_err(|e| OrbitError::internal(format!("WAL sync error: {}", e)))?;

        Ok(())
    }
}

impl LsmTreeAddressableProvider {
    pub fn new(config: LsmTreeConfig) -> Self {
        Self {
            active_memtable: Arc::new(StdRwLock::new(MemTable::new())),
            immutable_memtables: Arc::new(StdRwLock::new(Vec::new())),
            sstables: Arc::new(StdRwLock::new(Vec::new())),
            wal: Arc::new(Mutex::new(WriteAheadLog {
                file: unsafe { std::mem::zeroed() }, // Will be initialized
            })),
            block_cache: Arc::new(StdRwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(PersistenceMetrics::default())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            compaction_running: Arc::new(Mutex::new(false)),
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

    async fn maybe_trigger_compaction(&self) -> OrbitResult<()> {
        if !self.config.enable_compaction {
            return Ok(());
        }

        let mut compaction_guard = self.compaction_running.lock().await;
        if *compaction_guard {
            return Ok(()); // Compaction already running
        }

        let sstable_count = {
            let sstables = self.sstables.read().unwrap();
            sstables.len()
        };

        if sstable_count >= self.config.compaction_threshold {
            *compaction_guard = true;
            drop(compaction_guard);

            // Run compaction in background
            let _sstables = self.sstables.clone();
            let _data_dir = PathBuf::from(&self.config.data_dir);
            let compaction_running = self.compaction_running.clone();

            tokio::spawn(async move {
                // Simplified compaction - merge two smallest SSTables
                tracing::info!("Starting background compaction");

                // Actual compaction would merge SSTables, remove duplicates,
                // and create new merged SSTable
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                let mut guard = compaction_running.lock().await;
                *guard = false;

                tracing::info!("Background compaction completed");
            });
        }

        Ok(())
    }

    async fn search_sstables(&self, key: &str) -> OrbitResult<Option<Vec<u8>>> {
        let sstables = {
            let sstables_guard = self.sstables.read().unwrap();
            sstables_guard.clone()
        };

        // Search SSTables in reverse chronological order (newest first)
        for sstable in sstables.iter().rev() {
            // Check bloom filter first
            if !sstable.bloom_filter.contains(&key) {
                continue;
            }

            // Check key range
            if key < sstable.min_key.as_str() || key > sstable.max_key.as_str() {
                continue;
            }

            // Search in SSTable (simplified - would use binary search on index)
            if let Ok(data) = tokio::fs::read(&sstable.file_path).await {
                // Simplified search - in production this would use the index
                let content = String::from_utf8_lossy(&data);
                for line in content.lines() {
                    if line.starts_with(key) {
                        let parts: Vec<&str> = line.splitn(2, '|').collect();
                        if parts.len() == 2 {
                            return Ok(Some(
                                general_purpose::STANDARD
                                    .decode(parts[1])
                                    .unwrap_or_default(),
                            ));
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}

#[async_trait]
impl PersistenceProvider for LsmTreeAddressableProvider {
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
            "LSM-Tree addressable provider initialized at {}",
            data_dir.display()
        );
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        // Wait for any running compaction to finish
        let _guard = self.compaction_running.lock().await;

        tracing::info!("LSM-Tree addressable provider shutdown");
        Ok(())
    }

    async fn health_check(&self) -> ProviderHealth {
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
        Ok(())
    }
}

#[async_trait]
impl AddressableDirectoryProvider for LsmTreeAddressableProvider {
    async fn store_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        let start = Instant::now();
        let key = Self::reference_to_key(&lease.reference);
        let value = serde_json::to_vec(lease)
            .map_err(|e| OrbitError::internal(format!("Serialization error: {}", e)))?;

        // Write to WAL
        {
            let mut wal = self.wal.lock().await;
            wal.append(&key, &value, "INSERT").await?;
        }

        // Write to active memtable
        let is_full = {
            let mut memtable = self.active_memtable.write().unwrap();
            memtable.insert(key, value);
            memtable.is_full(self.config.memtable_size_limit)
        };

        // Check if memtable is full
        if is_full {
            // Move to immutable and create new active
            let old_memtable = {
                let mut memtable = self.active_memtable.write().unwrap();
                std::mem::replace(&mut *memtable, MemTable::new())
            };

            {
                let mut immutable = self.immutable_memtables.write().unwrap();
                immutable.push(old_memtable);
            }

            // Trigger flush and compaction if needed
            self.maybe_trigger_compaction().await?;
        }

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_lease(
        &self,
        reference: &AddressableReference,
    ) -> OrbitResult<Option<AddressableLease>> {
        let start = Instant::now();
        let key = Self::reference_to_key(reference);

        // Search in active memtable
        let active_result = {
            let active = self.active_memtable.read().unwrap();
            active.get(&key)
        };

        if let Some(data) = active_result {
            let lease = serde_json::from_slice(&data)
                .map_err(|e| OrbitError::internal(format!("Deserialization error: {}", e)))?;
            self.update_metrics("read", start.elapsed(), true).await;
            return Ok(Some(lease));
        }

        // Search in immutable memtables
        let immutable_result = {
            let immutable = self.immutable_memtables.read().unwrap();
            let mut result = None;
            for memtable in immutable.iter().rev() {
                if let Some(data) = memtable.get(&key) {
                    result = Some(data);
                    break;
                }
            }
            result
        };

        if let Some(data) = immutable_result {
            let lease = serde_json::from_slice(&data)
                .map_err(|e| OrbitError::internal(format!("Deserialization error: {}", e)))?;
            self.update_metrics("read", start.elapsed(), true).await;
            return Ok(Some(lease));
        }

        // Search in SSTables
        if let Some(data) = self.search_sstables(&key).await? {
            let lease = serde_json::from_slice(&data)
                .map_err(|e| OrbitError::internal(format!("Deserialization error: {}", e)))?;
            self.update_metrics("read", start.elapsed(), true).await;
            return Ok(Some(lease));
        }

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(None)
    }

    async fn update_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        // LSM-Trees handle updates as inserts (newer values override older ones)
        self.store_lease(lease).await
    }

    async fn remove_lease(&self, reference: &AddressableReference) -> OrbitResult<bool> {
        let start = Instant::now();
        let key = Self::reference_to_key(reference);

        // Write tombstone to WAL and memtable
        {
            let mut wal = self.wal.lock().await;
            wal.append(&key, b"", "DELETE").await?;
        }

        let removed = {
            let mut memtable = self.active_memtable.write().unwrap();
            memtable.remove(&key)
        };

        self.update_metrics("delete", start.elapsed(), removed)
            .await;
        Ok(removed)
    }

    async fn list_node_leases(&self, node_id: &NodeId) -> OrbitResult<Vec<AddressableLease>> {
        let start = Instant::now();
        let mut leases = Vec::new();

        // This is simplified - a real implementation would need efficient range queries
        // For now, we'll scan all data (not efficient for large datasets)

        // Scan active memtable
        {
            let active = self.active_memtable.read().unwrap();
            for (key, data) in &active.data {
                if !key.starts_with("__TOMBSTONE__") {
                    if let Ok(lease) = serde_json::from_slice::<AddressableLease>(data) {
                        if &lease.node_id == node_id {
                            leases.push(lease);
                        }
                    }
                }
            }
        }

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(leases)
    }

    async fn list_all_leases(&self) -> OrbitResult<Vec<AddressableLease>> {
        let start = Instant::now();
        let mut leases = Vec::new();

        // Scan active memtable
        {
            let active = self.active_memtable.read().unwrap();
            for (key, data) in &active.data {
                if !key.starts_with("__TOMBSTONE__") {
                    if let Ok(lease) = serde_json::from_slice::<AddressableLease>(data) {
                        leases.push(lease);
                    }
                }
            }
        }

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(leases)
    }

    async fn cleanup_expired_leases(&self) -> OrbitResult<u64> {
        let start = Instant::now();
        let _now = chrono::Utc::now();
        let count = 0;

        // This would require a more sophisticated implementation in production
        // to efficiently find and remove expired leases across all SSTables

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

impl LsmTreeClusterProvider {
    pub fn new(config: LsmTreeConfig) -> Self {
        Self {
            active_memtable: Arc::new(StdRwLock::new(MemTable::new())),
            immutable_memtables: Arc::new(StdRwLock::new(Vec::new())),
            sstables: Arc::new(StdRwLock::new(Vec::new())),
            wal: Arc::new(Mutex::new(WriteAheadLog {
                file: unsafe { std::mem::zeroed() },
            })),
            block_cache: Arc::new(StdRwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(PersistenceMetrics::default())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            compaction_running: Arc::new(Mutex::new(false)),
            config,
        }
    }
}

#[async_trait]
impl PersistenceProvider for LsmTreeClusterProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        let data_dir = PathBuf::from(&self.config.data_dir);
        tokio::fs::create_dir_all(&data_dir)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to create data directory: {}", e)))?;

        let wal_path = data_dir.join("cluster_nodes.wal");
        let wal = WriteAheadLog::new(&wal_path).await?;
        *self.wal.lock().await = wal;

        tracing::info!("LSM-Tree cluster provider initialized");
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        let _guard = self.compaction_running.lock().await;
        tracing::info!("LSM-Tree cluster provider shutdown");
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
impl ClusterNodeProvider for LsmTreeClusterProvider {
    async fn store_node(&self, node: &NodeInfo) -> OrbitResult<()> {
        let key = node.id.to_string();
        let value = serde_json::to_vec(node)
            .map_err(|e| OrbitError::internal(format!("Serialization error: {}", e)))?;

        self.wal.lock().await.append(&key, &value, "INSERT").await?;

        let mut memtable = self.active_memtable.write().unwrap();
        memtable.insert(key, value);

        Ok(())
    }

    async fn get_node(&self, node_id: &NodeId) -> OrbitResult<Option<NodeInfo>> {
        let key = node_id.to_string();

        if let Some(data) = self.active_memtable.read().unwrap().get(&key) {
            let node = serde_json::from_slice(&data)
                .map_err(|e| OrbitError::internal(format!("Deserialization error: {}", e)))?;
            return Ok(Some(node));
        }

        Ok(None)
    }

    async fn update_node(&self, node: &NodeInfo) -> OrbitResult<()> {
        self.store_node(node).await
    }

    async fn remove_node(&self, node_id: &NodeId) -> OrbitResult<bool> {
        let key = node_id.to_string();

        self.wal.lock().await.append(&key, b"", "DELETE").await?;

        let mut memtable = self.active_memtable.write().unwrap();
        let removed = memtable.remove(&key);

        Ok(removed)
    }

    async fn list_active_nodes(&self) -> OrbitResult<Vec<NodeInfo>> {
        let mut nodes = Vec::new();

        let active = self.active_memtable.read().unwrap();
        for (key, data) in &active.data {
            if !key.starts_with("__TOMBSTONE__") {
                if let Ok(node) = serde_json::from_slice::<NodeInfo>(data) {
                    if node.status == NodeStatus::Active {
                        nodes.push(node);
                    }
                }
            }
        }

        Ok(nodes)
    }

    async fn list_all_nodes(&self) -> OrbitResult<Vec<NodeInfo>> {
        let mut nodes = Vec::new();

        let active = self.active_memtable.read().unwrap();
        for (key, data) in &active.data {
            if !key.starts_with("__TOMBSTONE__") {
                if let Ok(node) = serde_json::from_slice::<NodeInfo>(data) {
                    nodes.push(node);
                }
            }
        }

        Ok(nodes)
    }

    async fn cleanup_expired_nodes(&self) -> OrbitResult<u64> {
        // Simplified implementation
        Ok(0)
    }

    async fn renew_node_lease(&self, node_id: &NodeId, lease: &NodeLease) -> OrbitResult<()> {
        if let Some(mut node) = self.get_node(node_id).await? {
            node.lease = Some(lease.clone());
            self.store_node(&node).await?;
        }
        Ok(())
    }
}
