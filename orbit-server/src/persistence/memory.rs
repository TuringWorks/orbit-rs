//! Memory-based persistence provider implementation
//!
//! This provider keeps data in memory using concurrent data structures and optionally
//! provides disk backup for recovery scenarios.

use super::*;
use dashmap::DashMap;
use orbit_shared::{
    AddressableLease, AddressableReference, NodeId, NodeInfo, NodeLease, NodeStatus,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::RwLock;
use tokio::time::interval;

/// In-memory persistence provider for addressable directory
pub struct MemoryAddressableDirectoryProvider {
    config: MemoryConfig,
    leases: Arc<DashMap<AddressableReference, AddressableLease>>,
    node_addressables: Arc<DashMap<NodeId, Vec<AddressableReference>>>,
    metrics: Arc<RwLock<PersistenceMetrics>>,
    transactions: Arc<DashMap<String, MemoryTransaction>>,
    backup_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

/// In-memory persistence provider for cluster nodes
pub struct MemoryClusterNodeProvider {
    config: MemoryConfig,
    nodes: Arc<DashMap<NodeId, NodeInfo>>,
    metrics: Arc<RwLock<PersistenceMetrics>>,
    transactions: Arc<DashMap<String, MemoryTransaction>>,
    #[allow(dead_code)]
    backup_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

/// Transaction state for memory provider
#[derive(Debug, Clone)]
struct MemoryTransaction {
    id: String,
    #[allow(dead_code)]
    context: TransactionContext,
    #[allow(dead_code)]
    started_at: Instant,
    operations: Vec<TransactionOperation>,
}

/// Operations within a transaction
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum TransactionOperation {
    // For addressable directory
    StoreLease(AddressableLease),
    UpdateLease(AddressableLease),
    RemoveLease(AddressableReference),
    // For cluster nodes
    StoreNode(NodeInfo),
    UpdateNode(NodeInfo),
    RemoveNode(NodeId),
}

impl MemoryAddressableDirectoryProvider {
    pub fn new(config: MemoryConfig) -> Self {
        let provider = Self {
            config: config.clone(),
            leases: Arc::new(DashMap::new()),
            node_addressables: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(PersistenceMetrics::default())),
            transactions: Arc::new(DashMap::new()),
            backup_handle: Arc::new(RwLock::new(None)),
        };

        // Start backup task if configured
        if let Some(backup_config) = &config.disk_backup {
            provider.start_backup_task(backup_config.clone());
        }

        provider
    }

    fn start_backup_task(&self, backup_config: DiskBackupConfig) {
        let leases = self.leases.clone();
        let node_addressables = self.node_addressables.clone();
        let backup_handle = self.backup_handle.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(backup_config.sync_interval));

            loop {
                interval.tick().await;

                if let Err(e) =
                    Self::backup_to_disk(&leases, &node_addressables, &backup_config).await
                {
                    tracing::warn!("Failed to backup addressable directory to disk: {}", e);
                }
            }
        });

        tokio::spawn(async move {
            let mut backup_handle = backup_handle.write().await;
            *backup_handle = Some(handle);
        });
    }

    async fn backup_to_disk(
        leases: &Arc<DashMap<AddressableReference, AddressableLease>>,
        node_addressables: &Arc<DashMap<NodeId, Vec<AddressableReference>>>,
        backup_config: &DiskBackupConfig,
    ) -> OrbitResult<()> {
        let backup_data = MemoryBackupData {
            leases: leases
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().clone()))
                .collect(),
            node_addressables: node_addressables
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().clone()))
                .collect(),
            timestamp: chrono::Utc::now(),
        };

        let serialized = match backup_config.compression {
            CompressionType::None => serde_json::to_vec(&backup_data)
                .map_err(|e| OrbitError::internal(format!("Serialization error: {}", e)))?,
            CompressionType::Gzip => {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                use std::io::Write;

                let json = serde_json::to_vec(&backup_data)
                    .map_err(|e| OrbitError::internal(format!("Serialization error: {}", e)))?;
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder
                    .write_all(&json)
                    .map_err(|e| OrbitError::internal(format!("Compression error: {}", e)))?;
                encoder
                    .finish()
                    .map_err(|e| OrbitError::internal(format!("Compression error: {}", e)))?
            }
            _ => {
                return Err(OrbitError::internal(
                    "Unsupported compression type for memory provider",
                ))
            }
        };

        fs::write(&backup_config.path, serialized)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to write backup: {}", e)))?;

        Ok(())
    }

    async fn restore_from_disk(&self, backup_config: &DiskBackupConfig) -> OrbitResult<()> {
        if !std::path::Path::new(&backup_config.path).exists() {
            return Ok(()); // No backup file exists
        }

        let data = fs::read(&backup_config.path)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to read backup: {}", e)))?;

        let backup_data: MemoryBackupData = match backup_config.compression {
            CompressionType::None => serde_json::from_slice(&data)
                .map_err(|e| OrbitError::internal(format!("Deserialization error: {}", e)))?,
            CompressionType::Gzip => {
                use flate2::read::GzDecoder;
                use std::io::Read;

                let mut decoder = GzDecoder::new(&data[..]);
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(|e| OrbitError::internal(format!("Decompression error: {}", e)))?;
                serde_json::from_slice(&decompressed)
                    .map_err(|e| OrbitError::internal(format!("Deserialization error: {}", e)))?
            }
            _ => {
                return Err(OrbitError::internal(
                    "Unsupported compression type for memory provider",
                ))
            }
        };

        // Restore data
        self.leases.clear();
        self.node_addressables.clear();

        for (reference, lease) in backup_data.leases {
            self.leases.insert(reference, lease);
        }

        for (node_id, refs) in backup_data.node_addressables {
            self.node_addressables.insert(node_id, refs);
        }

        tracing::info!(
            "Restored addressable directory from backup at {}",
            backup_data.timestamp
        );
        Ok(())
    }

    async fn update_metrics<F>(
        &self,
        operation: &str,
        duration: Duration,
        result: &OrbitResult<F>,
    ) {
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

        if result.is_err() {
            metrics.error_count += 1;
        }
    }
}

#[async_trait]
impl PersistenceProvider for MemoryAddressableDirectoryProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        // Restore from backup if configured
        if let Some(backup_config) = &self.config.disk_backup {
            self.restore_from_disk(backup_config).await?;
        }

        tracing::info!("Memory addressable directory provider initialized");
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        // Stop backup task
        let mut handle = self.backup_handle.write().await;
        if let Some(backup_handle) = handle.take() {
            backup_handle.abort();
        }

        // Final backup if configured
        if let Some(backup_config) = &self.config.disk_backup {
            Self::backup_to_disk(&self.leases, &self.node_addressables, backup_config).await?;
        }

        tracing::info!("Memory addressable directory provider shutdown");
        Ok(())
    }

    async fn health_check(&self) -> ProviderHealth {
        // Check if we're within memory limits
        if let Some(max_entries) = self.config.max_entries {
            if self.leases.len() > max_entries {
                return ProviderHealth::Degraded {
                    reason: format!(
                        "Memory usage exceeded: {} > {}",
                        self.leases.len(),
                        max_entries
                    ),
                };
            }
        }

        // Check disk backup health if configured
        if let Some(backup_config) = &self.config.disk_backup {
            if let Ok(metadata) = fs::metadata(&backup_config.path).await {
                let age = std::time::SystemTime::now()
                    .duration_since(metadata.modified().unwrap_or(std::time::UNIX_EPOCH))
                    .unwrap_or(Duration::from_secs(u64::MAX));

                if age > Duration::from_secs(backup_config.sync_interval * 3) {
                    return ProviderHealth::Degraded {
                        reason: "Backup file is stale".to_string(),
                    };
                }
            }
        }

        ProviderHealth::Healthy
    }

    async fn metrics(&self) -> PersistenceMetrics {
        self.metrics.read().await.clone()
    }

    async fn begin_transaction(&self, context: TransactionContext) -> OrbitResult<String> {
        let transaction_id = context.id.clone();
        let transaction = MemoryTransaction {
            id: transaction_id.clone(),
            context,
            started_at: Instant::now(),
            operations: Vec::new(),
        };

        self.transactions
            .insert(transaction.id.clone(), transaction);
        Ok(transaction_id)
    }

    async fn commit_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        if let Some((_, transaction)) = self.transactions.remove(transaction_id) {
            // For memory provider, operations are applied immediately, so commit is a no-op
            tracing::debug!(
                "Committed transaction {} with {} operations",
                transaction_id,
                transaction.operations.len()
            );
        }
        Ok(())
    }

    async fn rollback_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        if let Some((_, transaction)) = self.transactions.remove(transaction_id) {
            // For memory provider, we would need to reverse operations
            // This is a simplified implementation
            tracing::warn!(
                "Rolling back transaction {} with {} operations - simplified implementation",
                transaction_id,
                transaction.operations.len()
            );
        }
        Ok(())
    }
}

#[async_trait]
impl AddressableDirectoryProvider for MemoryAddressableDirectoryProvider {
    async fn store_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        let start = Instant::now();

        let result = async {
            self.leases.insert(lease.reference.clone(), lease.clone());

            // Update node mapping
            self.node_addressables
                .entry(lease.node_id.clone())
                .or_default()
                .push(lease.reference.clone());

            Ok(())
        }
        .await;

        self.update_metrics("write", start.elapsed(), &result).await;
        result
    }

    async fn get_lease(
        &self,
        reference: &AddressableReference,
    ) -> OrbitResult<Option<AddressableLease>> {
        let start = Instant::now();

        let result = async { Ok(self.leases.get(reference).map(|entry| entry.clone())) }.await;

        self.update_metrics("read", start.elapsed(), &result).await;
        result
    }

    async fn update_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        let start = Instant::now();

        let result = async {
            self.leases.insert(lease.reference.clone(), lease.clone());
            Ok(())
        }
        .await;

        self.update_metrics("write", start.elapsed(), &result).await;
        result
    }

    async fn remove_lease(&self, reference: &AddressableReference) -> OrbitResult<bool> {
        let start = Instant::now();

        let result = async {
            if let Some((_, lease)) = self.leases.remove(reference) {
                // Remove from node mapping
                if let Some(mut addressables) = self.node_addressables.get_mut(&lease.node_id) {
                    addressables.retain(|addr| addr != reference);
                }
                Ok(true)
            } else {
                Ok(false)
            }
        }
        .await;

        self.update_metrics("delete", start.elapsed(), &result)
            .await;
        result
    }

    async fn list_node_leases(&self, node_id: &NodeId) -> OrbitResult<Vec<AddressableLease>> {
        let start = Instant::now();

        let result = async {
            let mut leases = Vec::new();

            if let Some(refs) = self.node_addressables.get(node_id) {
                for reference in refs.iter() {
                    if let Some(lease) = self.leases.get(reference) {
                        leases.push(lease.clone());
                    }
                }
            }

            Ok(leases)
        }
        .await;

        self.update_metrics("read", start.elapsed(), &result).await;
        result
    }

    async fn list_all_leases(&self) -> OrbitResult<Vec<AddressableLease>> {
        let start = Instant::now();

        let result = async { Ok(self.leases.iter().map(|entry| entry.clone()).collect()) }.await;

        self.update_metrics("read", start.elapsed(), &result).await;
        result
    }

    async fn cleanup_expired_leases(&self) -> OrbitResult<u64> {
        let start = Instant::now();

        let result = async {
            let now = chrono::Utc::now();
            let mut expired = Vec::new();

            // Find expired leases
            for entry in self.leases.iter() {
                if entry.expires_at < now {
                    expired.push(entry.reference.clone());
                }
            }

            // Remove expired leases
            let mut count = 0;
            for reference in expired {
                if self.remove_lease(&reference).await.unwrap_or(false) {
                    count += 1;
                }
            }

            Ok(count)
        }
        .await;

        self.update_metrics("delete", start.elapsed(), &result)
            .await;
        result
    }

    async fn store_leases_bulk(&self, leases: &[AddressableLease]) -> OrbitResult<()> {
        let start = Instant::now();

        let result = async {
            for lease in leases {
                self.leases.insert(lease.reference.clone(), lease.clone());

                self.node_addressables
                    .entry(lease.node_id.clone())
                    .or_default()
                    .push(lease.reference.clone());
            }
            Ok(())
        }
        .await;

        self.update_metrics("write", start.elapsed(), &result).await;
        result
    }

    async fn remove_leases_bulk(&self, references: &[AddressableReference]) -> OrbitResult<u64> {
        let start = Instant::now();

        let result = async {
            let mut count = 0;
            for reference in references {
                if self.remove_lease(reference).await.unwrap_or(false) {
                    count += 1;
                }
            }
            Ok(count)
        }
        .await;

        self.update_metrics("delete", start.elapsed(), &result)
            .await;
        result
    }
}

// Similar implementation for MemoryClusterNodeProvider
impl MemoryClusterNodeProvider {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            nodes: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(PersistenceMetrics::default())),
            transactions: Arc::new(DashMap::new()),
            backup_handle: Arc::new(RwLock::new(None)),
        }
    }

    async fn update_metrics<F>(
        &self,
        operation: &str,
        duration: Duration,
        result: &OrbitResult<F>,
    ) {
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

        if result.is_err() {
            metrics.error_count += 1;
        }
    }
}

#[async_trait]
impl PersistenceProvider for MemoryClusterNodeProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        tracing::info!("Memory cluster node provider initialized");
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        tracing::info!("Memory cluster node provider shutdown");
        Ok(())
    }

    async fn health_check(&self) -> ProviderHealth {
        if let Some(max_entries) = self.config.max_entries {
            if self.nodes.len() > max_entries {
                return ProviderHealth::Degraded {
                    reason: format!(
                        "Memory usage exceeded: {} > {}",
                        self.nodes.len(),
                        max_entries
                    ),
                };
            }
        }
        ProviderHealth::Healthy
    }

    async fn metrics(&self) -> PersistenceMetrics {
        self.metrics.read().await.clone()
    }

    async fn begin_transaction(&self, context: TransactionContext) -> OrbitResult<String> {
        let transaction_id = context.id.clone();
        let transaction = MemoryTransaction {
            id: transaction_id.clone(),
            context,
            started_at: Instant::now(),
            operations: Vec::new(),
        };

        self.transactions
            .insert(transaction.id.clone(), transaction);
        Ok(transaction_id)
    }

    async fn commit_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        self.transactions.remove(transaction_id);
        Ok(())
    }

    async fn rollback_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        self.transactions.remove(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl ClusterNodeProvider for MemoryClusterNodeProvider {
    async fn store_node(&self, node: &NodeInfo) -> OrbitResult<()> {
        let start = Instant::now();
        let result = async {
            self.nodes.insert(node.id.clone(), node.clone());
            Ok(())
        }
        .await;
        self.update_metrics("write", start.elapsed(), &result).await;
        result
    }

    async fn get_node(&self, node_id: &NodeId) -> OrbitResult<Option<NodeInfo>> {
        let start = Instant::now();
        let result = async { Ok(self.nodes.get(node_id).map(|entry| entry.clone())) }.await;
        self.update_metrics("read", start.elapsed(), &result).await;
        result
    }

    async fn update_node(&self, node: &NodeInfo) -> OrbitResult<()> {
        let start = Instant::now();
        let result = async {
            self.nodes.insert(node.id.clone(), node.clone());
            Ok(())
        }
        .await;
        self.update_metrics("write", start.elapsed(), &result).await;
        result
    }

    async fn remove_node(&self, node_id: &NodeId) -> OrbitResult<bool> {
        let start = Instant::now();
        let result = async { Ok(self.nodes.remove(node_id).is_some()) }.await;
        self.update_metrics("delete", start.elapsed(), &result)
            .await;
        result
    }

    async fn list_active_nodes(&self) -> OrbitResult<Vec<NodeInfo>> {
        let start = Instant::now();
        let result = async {
            Ok(self
                .nodes
                .iter()
                .filter(|entry| entry.status == NodeStatus::Active)
                .map(|entry| entry.clone())
                .collect())
        }
        .await;
        self.update_metrics("read", start.elapsed(), &result).await;
        result
    }

    async fn list_all_nodes(&self) -> OrbitResult<Vec<NodeInfo>> {
        let start = Instant::now();
        let result = async { Ok(self.nodes.iter().map(|entry| entry.clone()).collect()) }.await;
        self.update_metrics("read", start.elapsed(), &result).await;
        result
    }

    async fn cleanup_expired_nodes(&self) -> OrbitResult<u64> {
        let start = Instant::now();
        let result = async {
            let mut expired = Vec::new();

            for entry in self.nodes.iter() {
                if let Some(lease) = &entry.lease {
                    if lease.is_expired() {
                        expired.push(entry.id.clone());
                    }
                }
            }

            let mut count = 0;
            for node_id in expired {
                if self.remove_node(&node_id).await.unwrap_or(false) {
                    count += 1;
                }
            }

            Ok(count)
        }
        .await;
        self.update_metrics("delete", start.elapsed(), &result)
            .await;
        result
    }

    async fn renew_node_lease(&self, node_id: &NodeId, lease: &NodeLease) -> OrbitResult<()> {
        let start = Instant::now();
        let result = async {
            if let Some(mut node) = self.nodes.get_mut(node_id) {
                node.lease = Some(lease.clone());
                Ok(())
            } else {
                Err(OrbitError::NodeNotFound {
                    node_id: node_id.to_string(),
                })
            }
        }
        .await;
        self.update_metrics("write", start.elapsed(), &result).await;
        result
    }
}

/// Backup data structure for memory provider
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryBackupData {
    leases: Vec<(AddressableReference, AddressableLease)>,
    node_addressables: Vec<(NodeId, Vec<AddressableReference>)>,
    timestamp: chrono::DateTime<chrono::Utc>,
}
