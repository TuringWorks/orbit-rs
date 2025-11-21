use crate::addressable::AddressableReference;
use crate::exception::{OrbitError, OrbitResult};
use crate::security::encryption::{EncryptionManager, KeyManagementSystem, KeyRotationPolicy, KeyStoreType};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Actor state snapshot containing serialized state and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSnapshot {
    /// Actor reference this snapshot belongs to
    pub actor_reference: AddressableReference,
    /// Unique identifier for this snapshot
    pub snapshot_id: String,
    /// Version number for optimistic concurrency control
    pub version: u64,
    /// Serialized actor state
    pub state_data: serde_json::Value,
    /// Timestamp when snapshot was created
    pub created_at: i64,
    /// Timestamp when snapshot was last updated
    pub updated_at: i64,
    /// Hash of the state data for integrity checking
    pub state_hash: String,
    /// Tags for categorizing snapshots
    pub tags: HashMap<String, String>,
    /// TTL for automatic cleanup (in seconds)
    pub ttl_seconds: Option<u64>,
}

impl ActorSnapshot {
    pub fn new(
        actor_reference: AddressableReference,
        state_data: serde_json::Value,
        version: u64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        let state_hash = Self::calculate_hash(&state_data);

        Self {
            actor_reference,
            snapshot_id: Uuid::new_v4().to_string(),
            version,
            state_data,
            created_at: now,
            updated_at: now,
            state_hash,
            tags: HashMap::new(),
            ttl_seconds: None,
        }
    }

    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }

    /// Calculate SHA-256 hash of the state data
    fn calculate_hash(data: &serde_json::Value) -> String {
        use sha2::{Digest, Sha256};
        let serialized = serde_json::to_string(data).unwrap_or_default();
        let hash = Sha256::digest(serialized.as_bytes());
        format!("{hash:x}")
    }

    /// Verify integrity of the snapshot
    pub fn verify_integrity(&self) -> bool {
        Self::calculate_hash(&self.state_data) == self.state_hash
    }

    /// Check if snapshot has expired based on TTL
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_seconds {
            let expiry_time = self.created_at + (ttl * 1000) as i64;
            chrono::Utc::now().timestamp_millis() > expiry_time
        } else {
            false
        }
    }
}

/// Configuration for persistence layer
#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    /// Enable automatic snapshots
    pub auto_snapshot: bool,
    /// Interval between automatic snapshots
    pub snapshot_interval: Duration,
    /// Maximum number of snapshots to keep per actor
    pub max_snapshots_per_actor: usize,
    /// Batch size for bulk operations
    pub batch_size: usize,
    /// Compression level (0-9, 0 = no compression)
    pub compression_level: u8,
    /// Enable encryption for state data
    pub encryption_enabled: bool,
    /// Connection timeout for database operations
    pub connection_timeout: Duration,
    /// Maximum retries for failed operations
    pub max_retries: u32,
    /// Enable metrics collection
    pub metrics_enabled: bool,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            auto_snapshot: true,
            snapshot_interval: Duration::from_secs(60),
            max_snapshots_per_actor: 10,
            batch_size: 100,
            compression_level: 6,
            encryption_enabled: false,
            connection_timeout: Duration::from_secs(30),
            max_retries: 3,
            metrics_enabled: true,
        }
    }
}

/// Persistence metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct PersistenceMetrics {
    pub snapshots_created: u64,
    pub snapshots_loaded: u64,
    pub snapshots_deleted: u64,
    pub total_storage_bytes: u64,
    pub average_save_time_ms: f64,
    pub average_load_time_ms: f64,
    pub failed_operations: u64,
    pub last_operation_time: i64,
}

/// Trait for implementing different persistence backends
#[async_trait]
pub trait PersistenceBackend: Send + Sync {
    /// Save an actor snapshot
    async fn save_snapshot(&self, snapshot: &ActorSnapshot) -> OrbitResult<()>;

    /// Load the latest snapshot for an actor
    async fn load_snapshot(
        &self,
        actor_ref: &AddressableReference,
    ) -> OrbitResult<Option<ActorSnapshot>>;

    /// Load a specific snapshot version
    async fn load_snapshot_version(
        &self,
        actor_ref: &AddressableReference,
        version: u64,
    ) -> OrbitResult<Option<ActorSnapshot>>;

    /// List all snapshots for an actor
    async fn list_snapshots(
        &self,
        actor_ref: &AddressableReference,
    ) -> OrbitResult<Vec<ActorSnapshot>>;

    /// Delete a specific snapshot
    async fn delete_snapshot(
        &self,
        actor_ref: &AddressableReference,
        snapshot_id: &str,
    ) -> OrbitResult<()>;

    /// Delete all snapshots for an actor
    async fn delete_all_snapshots(&self, actor_ref: &AddressableReference) -> OrbitResult<()>;

    /// Cleanup expired snapshots
    async fn cleanup_expired_snapshots(&self) -> OrbitResult<u64>;

    /// Get backend health status
    async fn health_check(&self) -> OrbitResult<bool>;

    /// Get storage statistics
    async fn get_storage_stats(&self) -> OrbitResult<PersistenceMetrics>;
}

/// In-memory persistence backend for testing and development
#[derive(Debug)]
pub struct MemoryPersistenceBackend {
    storage: Arc<RwLock<HashMap<AddressableReference, Vec<ActorSnapshot>>>>,
    metrics: Arc<RwLock<PersistenceMetrics>>,
}

impl MemoryPersistenceBackend {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(PersistenceMetrics::default())),
        }
    }
}

impl Default for MemoryPersistenceBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PersistenceBackend for MemoryPersistenceBackend {
    async fn save_snapshot(&self, snapshot: &ActorSnapshot) -> OrbitResult<()> {
        let start_time = Instant::now();

        {
            let mut storage = self.storage.write().await;
            let snapshots = storage
                .entry(snapshot.actor_reference.clone())
                .or_insert_with(Vec::new);

            // Remove existing snapshot with same version
            snapshots.retain(|s| s.version != snapshot.version);

            // Add new snapshot
            snapshots.push(snapshot.clone());

            // Sort by version (newest first)
            snapshots.sort_by(|a, b| b.version.cmp(&a.version));

            // Limit number of snapshots
            if snapshots.len() > 10 {
                snapshots.truncate(10);
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.snapshots_created += 1;
            metrics.average_save_time_ms = start_time.elapsed().as_millis() as f64;
            metrics.last_operation_time = chrono::Utc::now().timestamp_millis();
        }

        debug!("Saved snapshot for actor: {}", snapshot.actor_reference);
        Ok(())
    }

    async fn load_snapshot(
        &self,
        actor_ref: &AddressableReference,
    ) -> OrbitResult<Option<ActorSnapshot>> {
        let start_time = Instant::now();

        let snapshot = {
            let storage = self.storage.read().await;
            storage
                .get(actor_ref)
                .and_then(|snapshots| snapshots.first())
                .cloned()
        };

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.snapshots_loaded += 1;
            metrics.average_load_time_ms = start_time.elapsed().as_millis() as f64;
            metrics.last_operation_time = chrono::Utc::now().timestamp_millis();
        }

        if snapshot.is_some() {
            debug!("Loaded snapshot for actor: {}", actor_ref);
        }

        Ok(snapshot)
    }

    async fn load_snapshot_version(
        &self,
        actor_ref: &AddressableReference,
        version: u64,
    ) -> OrbitResult<Option<ActorSnapshot>> {
        let storage = self.storage.read().await;
        let snapshot = storage
            .get(actor_ref)
            .and_then(|snapshots| snapshots.iter().find(|s| s.version == version))
            .cloned();

        Ok(snapshot)
    }

    async fn list_snapshots(
        &self,
        actor_ref: &AddressableReference,
    ) -> OrbitResult<Vec<ActorSnapshot>> {
        let storage = self.storage.read().await;
        let snapshots = storage.get(actor_ref).cloned().unwrap_or_default();

        Ok(snapshots)
    }

    async fn delete_snapshot(
        &self,
        actor_ref: &AddressableReference,
        snapshot_id: &str,
    ) -> OrbitResult<()> {
        {
            let mut storage = self.storage.write().await;
            if let Some(snapshots) = storage.get_mut(actor_ref) {
                snapshots.retain(|s| s.snapshot_id != snapshot_id);
                if snapshots.is_empty() {
                    storage.remove(actor_ref);
                }
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.snapshots_deleted += 1;
            metrics.last_operation_time = chrono::Utc::now().timestamp_millis();
        }

        debug!("Deleted snapshot {} for actor: {}", snapshot_id, actor_ref);
        Ok(())
    }

    async fn delete_all_snapshots(&self, actor_ref: &AddressableReference) -> OrbitResult<()> {
        let count = {
            let mut storage = self.storage.write().await;
            if let Some(snapshots) = storage.remove(actor_ref) {
                snapshots.len()
            } else {
                0
            }
        };

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.snapshots_deleted += count as u64;
            metrics.last_operation_time = chrono::Utc::now().timestamp_millis();
        }

        debug!("Deleted all snapshots for actor: {}", actor_ref);
        Ok(())
    }

    async fn cleanup_expired_snapshots(&self) -> OrbitResult<u64> {
        let mut deleted_count = 0u64;

        {
            let mut storage = self.storage.write().await;
            let mut to_remove = Vec::new();

            for (actor_ref, snapshots) in storage.iter_mut() {
                let original_len = snapshots.len();
                snapshots.retain(|s| !s.is_expired());
                deleted_count += (original_len - snapshots.len()) as u64;

                if snapshots.is_empty() {
                    to_remove.push(actor_ref.clone());
                }
            }

            for actor_ref in to_remove {
                storage.remove(&actor_ref);
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.snapshots_deleted += deleted_count;
            metrics.last_operation_time = chrono::Utc::now().timestamp_millis();
        }

        if deleted_count > 0 {
            info!("Cleaned up {} expired snapshots", deleted_count);
        }

        Ok(deleted_count)
    }

    async fn health_check(&self) -> OrbitResult<bool> {
        // For memory backend, always healthy
        Ok(true)
    }

    async fn get_storage_stats(&self) -> OrbitResult<PersistenceMetrics> {
        let metrics = self.metrics.read().await;

        // Calculate total storage bytes
        let storage = self.storage.read().await;
        let total_snapshots = storage.values().map(|v| v.len()).sum::<usize>();

        let mut stats = metrics.clone();
        stats.total_storage_bytes = (total_snapshots * 1024) as u64; // Rough estimate

        Ok(stats)
    }
}

/// Encrypted snapshot for storing encrypted state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedSnapshot {
    /// Actor reference this snapshot belongs to
    pub actor_reference: AddressableReference,
    /// Unique identifier for this snapshot
    pub snapshot_id: String,
    /// Version number for optimistic concurrency control
    pub version: u64,
    /// Encrypted state data (ciphertext)
    pub encrypted_data: Vec<u8>,
    /// Key ID used for encryption (needed for decryption)
    pub key_id: String,
    /// Timestamp when snapshot was created
    pub created_at: i64,
    /// Timestamp when snapshot was last updated
    pub updated_at: i64,
    /// Hash of the original (plaintext) state data for integrity checking
    pub state_hash: String,
    /// Tags for categorizing snapshots
    pub tags: HashMap<String, String>,
    /// TTL for automatic cleanup (in seconds)
    pub ttl_seconds: Option<u64>,
}

/// Encrypted persistence backend that wraps another backend with AES-256-GCM encryption
pub struct EncryptedPersistenceBackend {
    inner: Arc<dyn PersistenceBackend>,
    encryption_manager: Arc<EncryptionManager>,
}

impl EncryptedPersistenceBackend {
    /// Create a new encrypted persistence backend
    pub fn new(inner: Arc<dyn PersistenceBackend>, encryption_manager: Arc<EncryptionManager>) -> Self {
        Self {
            inner,
            encryption_manager,
        }
    }

    /// Create with auto-initialized key management
    pub async fn with_auto_key_management(inner: Arc<dyn PersistenceBackend>) -> OrbitResult<Self> {
        let kms = Arc::new(KeyManagementSystem::new(
            KeyRotationPolicy::default(),
            KeyStoreType::Memory,
        ));

        // Generate and set initial key
        kms.rotate_keys().await?;

        let encryption_manager = Arc::new(EncryptionManager::new(kms, None));

        Ok(Self {
            inner,
            encryption_manager,
        })
    }

    /// Encrypt a snapshot's state data
    async fn encrypt_snapshot(&self, snapshot: &ActorSnapshot) -> OrbitResult<EncryptedSnapshot> {
        // Serialize state data to JSON bytes
        let state_bytes = serde_json::to_vec(&snapshot.state_data)
            .map_err(|e| OrbitError::internal(format!("Failed to serialize state: {}", e)))?;

        // Encrypt with key ID tracking
        let (encrypted_data, key_id) = self.encryption_manager.encrypt_with_key_id(&state_bytes).await?;

        Ok(EncryptedSnapshot {
            actor_reference: snapshot.actor_reference.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            version: snapshot.version,
            encrypted_data,
            key_id,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
            state_hash: snapshot.state_hash.clone(),
            tags: snapshot.tags.clone(),
            ttl_seconds: snapshot.ttl_seconds,
        })
    }

    /// Decrypt an encrypted snapshot back to original
    async fn decrypt_snapshot(&self, encrypted: &EncryptedSnapshot) -> OrbitResult<ActorSnapshot> {
        // Decrypt the data
        let decrypted_bytes = self.encryption_manager
            .decrypt(&encrypted.encrypted_data, &encrypted.key_id)
            .await?;

        // Deserialize state data
        let state_data: serde_json::Value = serde_json::from_slice(&decrypted_bytes)
            .map_err(|e| OrbitError::internal(format!("Failed to deserialize decrypted state: {}", e)))?;

        Ok(ActorSnapshot {
            actor_reference: encrypted.actor_reference.clone(),
            snapshot_id: encrypted.snapshot_id.clone(),
            version: encrypted.version,
            state_data,
            created_at: encrypted.created_at,
            updated_at: encrypted.updated_at,
            state_hash: encrypted.state_hash.clone(),
            tags: encrypted.tags.clone(),
            ttl_seconds: encrypted.ttl_seconds,
        })
    }
}

#[async_trait]
impl PersistenceBackend for EncryptedPersistenceBackend {
    async fn save_snapshot(&self, snapshot: &ActorSnapshot) -> OrbitResult<()> {
        // Encrypt the snapshot
        let encrypted = self.encrypt_snapshot(snapshot).await?;

        // Store encrypted data as the state_data in a wrapper ActorSnapshot
        // This allows us to use the existing backend without changes
        let wrapper_state = serde_json::to_value(&encrypted)
            .map_err(|e| OrbitError::internal(format!("Failed to serialize encrypted snapshot: {}", e)))?;

        let wrapper_snapshot = ActorSnapshot {
            actor_reference: snapshot.actor_reference.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            version: snapshot.version,
            state_data: wrapper_state,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
            state_hash: snapshot.state_hash.clone(),
            tags: snapshot.tags.clone(),
            ttl_seconds: snapshot.ttl_seconds,
        };

        self.inner.save_snapshot(&wrapper_snapshot).await?;
        debug!("Saved encrypted snapshot for actor: {}", snapshot.actor_reference);
        Ok(())
    }

    async fn load_snapshot(&self, actor_ref: &AddressableReference) -> OrbitResult<Option<ActorSnapshot>> {
        if let Some(wrapper_snapshot) = self.inner.load_snapshot(actor_ref).await? {
            // Deserialize the encrypted snapshot from the wrapper
            let encrypted: EncryptedSnapshot = serde_json::from_value(wrapper_snapshot.state_data)
                .map_err(|e| OrbitError::internal(format!("Failed to deserialize encrypted snapshot: {}", e)))?;

            // Decrypt and return
            let decrypted = self.decrypt_snapshot(&encrypted).await?;
            debug!("Loaded and decrypted snapshot for actor: {}", actor_ref);
            Ok(Some(decrypted))
        } else {
            Ok(None)
        }
    }

    async fn load_snapshot_version(
        &self,
        actor_ref: &AddressableReference,
        version: u64,
    ) -> OrbitResult<Option<ActorSnapshot>> {
        if let Some(wrapper_snapshot) = self.inner.load_snapshot_version(actor_ref, version).await? {
            let encrypted: EncryptedSnapshot = serde_json::from_value(wrapper_snapshot.state_data)
                .map_err(|e| OrbitError::internal(format!("Failed to deserialize encrypted snapshot: {}", e)))?;

            let decrypted = self.decrypt_snapshot(&encrypted).await?;
            Ok(Some(decrypted))
        } else {
            Ok(None)
        }
    }

    async fn list_snapshots(&self, actor_ref: &AddressableReference) -> OrbitResult<Vec<ActorSnapshot>> {
        let wrapper_snapshots = self.inner.list_snapshots(actor_ref).await?;
        let mut decrypted_snapshots = Vec::with_capacity(wrapper_snapshots.len());

        for wrapper in wrapper_snapshots {
            match serde_json::from_value::<EncryptedSnapshot>(wrapper.state_data) {
                Ok(encrypted) => match self.decrypt_snapshot(&encrypted).await {
                    Ok(decrypted) => decrypted_snapshots.push(decrypted),
                    Err(e) => {
                        warn!("Failed to decrypt snapshot {}: {}", wrapper.snapshot_id, e);
                    }
                },
                Err(e) => {
                    warn!("Failed to deserialize encrypted snapshot {}: {}", wrapper.snapshot_id, e);
                }
            }
        }

        Ok(decrypted_snapshots)
    }

    async fn delete_snapshot(&self, actor_ref: &AddressableReference, snapshot_id: &str) -> OrbitResult<()> {
        self.inner.delete_snapshot(actor_ref, snapshot_id).await
    }

    async fn delete_all_snapshots(&self, actor_ref: &AddressableReference) -> OrbitResult<()> {
        self.inner.delete_all_snapshots(actor_ref).await
    }

    async fn cleanup_expired_snapshots(&self) -> OrbitResult<u64> {
        self.inner.cleanup_expired_snapshots().await
    }

    async fn health_check(&self) -> OrbitResult<bool> {
        self.inner.health_check().await
    }

    async fn get_storage_stats(&self) -> OrbitResult<PersistenceMetrics> {
        self.inner.get_storage_stats().await
    }
}

/// Actor state manager that handles persistence operations
pub struct ActorStateManager {
    backend: Arc<dyn PersistenceBackend>,
    config: PersistenceConfig,
    /// Cache of recently accessed snapshots
    cache: Arc<RwLock<HashMap<AddressableReference, (ActorSnapshot, Instant)>>>,
}

impl ActorStateManager {
    pub fn new(backend: Arc<dyn PersistenceBackend>, config: PersistenceConfig) -> Self {
        Self {
            backend,
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Save actor state
    pub async fn save_state<T: Serialize>(
        &self,
        actor_ref: &AddressableReference,
        state: &T,
        version: u64,
    ) -> OrbitResult<()> {
        let state_data = serde_json::to_value(state)
            .map_err(|e| OrbitError::internal(format!("Failed to serialize state: {e}")))?;

        let snapshot = ActorSnapshot::new(actor_ref.clone(), state_data, version);

        // Save to backend
        self.backend.save_snapshot(&snapshot).await?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(actor_ref.clone(), (snapshot, Instant::now()));
        }

        info!(
            "Saved state for actor: {} (version: {})",
            actor_ref, version
        );
        Ok(())
    }

    /// Load actor state
    pub async fn load_state<T: for<'de> Deserialize<'de>>(
        &self,
        actor_ref: &AddressableReference,
    ) -> OrbitResult<Option<(T, u64)>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some((snapshot, cached_at)) = cache.get(actor_ref) {
                if cached_at.elapsed() < Duration::from_secs(60) {
                    // Cache for 1 minute
                    let state: T =
                        serde_json::from_value(snapshot.state_data.clone()).map_err(|e| {
                            OrbitError::internal(format!("Failed to deserialize cached state: {e}"))
                        })?;
                    return Ok(Some((state, snapshot.version)));
                }
            }
        }

        // Load from backend
        if let Some(snapshot) = self.backend.load_snapshot(actor_ref).await? {
            if !snapshot.verify_integrity() {
                error!("Snapshot integrity check failed for actor: {}", actor_ref);
                return Err(OrbitError::internal("Snapshot integrity check failed"));
            }

            let state: T = serde_json::from_value(snapshot.state_data.clone())
                .map_err(|e| OrbitError::internal(format!("Failed to deserialize state: {e}")))?;

            // Update cache
            {
                let mut cache = self.cache.write().await;
                cache.insert(actor_ref.clone(), (snapshot.clone(), Instant::now()));
            }

            info!(
                "Loaded state for actor: {} (version: {})",
                actor_ref, snapshot.version
            );
            Ok(Some((state, snapshot.version)))
        } else {
            Ok(None)
        }
    }

    /// Load a specific version of actor state
    pub async fn load_state_version<T: for<'de> Deserialize<'de>>(
        &self,
        actor_ref: &AddressableReference,
        version: u64,
    ) -> OrbitResult<Option<T>> {
        if let Some(snapshot) = self
            .backend
            .load_snapshot_version(actor_ref, version)
            .await?
        {
            if !snapshot.verify_integrity() {
                error!(
                    "Snapshot integrity check failed for actor: {} (version: {})",
                    actor_ref, version
                );
                return Err(OrbitError::internal("Snapshot integrity check failed"));
            }

            let state: T = serde_json::from_value(snapshot.state_data)
                .map_err(|e| OrbitError::internal(format!("Failed to deserialize state: {e}")))?;

            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    /// Delete actor state
    pub async fn delete_state(&self, actor_ref: &AddressableReference) -> OrbitResult<()> {
        self.backend.delete_all_snapshots(actor_ref).await?;

        // Remove from cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(actor_ref);
        }

        info!("Deleted state for actor: {}", actor_ref);
        Ok(())
    }

    /// Get state history for an actor
    pub async fn get_state_history(
        &self,
        actor_ref: &AddressableReference,
    ) -> OrbitResult<Vec<ActorSnapshot>> {
        self.backend.list_snapshots(actor_ref).await
    }

    /// Start background cleanup tasks
    pub async fn start_background_tasks(&self) -> OrbitResult<()> {
        let backend = Arc::clone(&self.backend);
        let _config = self.config.clone();

        // Start cleanup task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                if let Err(e) = backend.cleanup_expired_snapshots().await {
                    error!("Snapshot cleanup failed: {}", e);
                }
            }
        });

        // Start cache cleanup task
        let cache = Arc::clone(&self.cache);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // 1 minute
            loop {
                interval.tick().await;
                let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes

                let mut cache_guard = cache.write().await;
                cache_guard.retain(|_, (_, cached_at)| *cached_at > cutoff);
            }
        });

        info!("Actor state manager background tasks started");
        Ok(())
    }

    /// Get persistence metrics
    pub async fn get_metrics(&self) -> OrbitResult<PersistenceMetrics> {
        self.backend.get_storage_stats().await
    }

    /// Perform health check
    pub async fn health_check(&self) -> OrbitResult<bool> {
        self.backend.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressable::{AddressableReference, Key};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestState {
        value: i32,
        name: String,
    }

    #[tokio::test]
    async fn test_memory_persistence_backend() {
        let backend = MemoryPersistenceBackend::new();
        let actor_ref = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test-1".to_string(),
            },
        };

        let state_data = serde_json::json!({"value": 42, "name": "test"});
        let snapshot = ActorSnapshot::new(actor_ref.clone(), state_data, 1);

        // Save snapshot
        backend.save_snapshot(&snapshot).await.unwrap();

        // Load snapshot
        let loaded = backend.load_snapshot(&actor_ref).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().version, 1);

        // List snapshots
        let snapshots = backend.list_snapshots(&actor_ref).await.unwrap();
        assert_eq!(snapshots.len(), 1);
    }

    #[tokio::test]
    async fn test_actor_state_manager() {
        let backend = Arc::new(MemoryPersistenceBackend::new());
        let config = PersistenceConfig::default();
        let manager = ActorStateManager::new(backend, config);

        let actor_ref = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test-1".to_string(),
            },
        };

        let test_state = TestState {
            value: 42,
            name: "test state".to_string(),
        };

        // Save state
        manager
            .save_state(&actor_ref, &test_state, 1)
            .await
            .unwrap();

        // Load state
        let loaded: Option<(TestState, u64)> = manager.load_state(&actor_ref).await.unwrap();
        assert!(loaded.is_some());
        let (loaded_state, version) = loaded.unwrap();
        assert_eq!(loaded_state, test_state);
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn test_snapshot_integrity() {
        let state_data = serde_json::json!({"test": "data"});
        let actor_ref = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test-1".to_string(),
            },
        };

        let snapshot = ActorSnapshot::new(actor_ref, state_data, 1);
        assert!(snapshot.verify_integrity());
    }

    #[tokio::test]
    async fn test_snapshot_expiry() {
        let state_data = serde_json::json!({"test": "data"});
        let actor_ref = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test-1".to_string(),
            },
        };

        let snapshot = ActorSnapshot::new(actor_ref, state_data, 1).with_ttl(1); // 1 second TTL

        // Should not be expired initially
        assert!(!snapshot.is_expired());

        // Wait and check expiry (in real test, we'd manipulate the timestamp)
        // For now, just verify the logic works
    }

    #[tokio::test]
    async fn test_encrypted_persistence_backend() {
        // Create inner memory backend
        let inner = Arc::new(MemoryPersistenceBackend::new());

        // Create encrypted backend with auto key management
        let encrypted_backend = EncryptedPersistenceBackend::with_auto_key_management(inner)
            .await
            .unwrap();

        let actor_ref = AddressableReference {
            addressable_type: "EncryptedActor".to_string(),
            key: Key::StringKey {
                key: "encrypted-test-1".to_string(),
            },
        };

        // Create a snapshot with sensitive data
        let state_data = serde_json::json!({
            "secret_value": 12345,
            "password_hash": "hashed_password_here",
            "api_key": "secret_api_key"
        });
        let snapshot = ActorSnapshot::new(actor_ref.clone(), state_data.clone(), 1);

        // Save encrypted snapshot
        encrypted_backend.save_snapshot(&snapshot).await.unwrap();

        // Load and decrypt snapshot
        let loaded = encrypted_backend.load_snapshot(&actor_ref).await.unwrap();
        assert!(loaded.is_some());
        let loaded_snapshot = loaded.unwrap();

        // Verify data integrity after encryption/decryption roundtrip
        assert_eq!(loaded_snapshot.version, 1);
        assert_eq!(loaded_snapshot.state_data, state_data);
        assert!(loaded_snapshot.verify_integrity());
    }

    #[tokio::test]
    async fn test_encrypted_persistence_multiple_versions() {
        let inner = Arc::new(MemoryPersistenceBackend::new());
        let encrypted_backend = EncryptedPersistenceBackend::with_auto_key_management(inner)
            .await
            .unwrap();

        let actor_ref = AddressableReference {
            addressable_type: "VersionedActor".to_string(),
            key: Key::StringKey {
                key: "versioned-1".to_string(),
            },
        };

        // Save multiple versions
        for version in 1..=5 {
            let state_data = serde_json::json!({
                "counter": version,
                "data": format!("version_{}", version)
            });
            let snapshot = ActorSnapshot::new(actor_ref.clone(), state_data, version);
            encrypted_backend.save_snapshot(&snapshot).await.unwrap();
        }

        // List all snapshots
        let snapshots = encrypted_backend.list_snapshots(&actor_ref).await.unwrap();
        assert!(snapshots.len() >= 1); // At least the latest version

        // Load specific version
        let version_3 = encrypted_backend.load_snapshot_version(&actor_ref, 3).await.unwrap();
        assert!(version_3.is_some());
        let v3 = version_3.unwrap();
        assert_eq!(v3.version, 3);
        assert_eq!(v3.state_data["counter"], 3);
    }

    #[tokio::test]
    async fn test_encrypted_state_manager() {
        let inner = Arc::new(MemoryPersistenceBackend::new());
        let encrypted_backend = Arc::new(
            EncryptedPersistenceBackend::with_auto_key_management(inner)
                .await
                .unwrap()
        );

        let config = PersistenceConfig {
            encryption_enabled: true,
            ..Default::default()
        };

        let manager = ActorStateManager::new(encrypted_backend, config);

        let actor_ref = AddressableReference {
            addressable_type: "SecureActor".to_string(),
            key: Key::StringKey {
                key: "secure-1".to_string(),
            },
        };

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct SecretState {
            api_key: String,
            balance: i64,
        }

        let secret_state = SecretState {
            api_key: "sk_live_super_secret_key".to_string(),
            balance: 1000000,
        };

        // Save encrypted state
        manager.save_state(&actor_ref, &secret_state, 1).await.unwrap();

        // Load and verify
        let loaded: Option<(SecretState, u64)> = manager.load_state(&actor_ref).await.unwrap();
        assert!(loaded.is_some());
        let (loaded_state, version) = loaded.unwrap();
        assert_eq!(loaded_state, secret_state);
        assert_eq!(version, 1);
    }
}
