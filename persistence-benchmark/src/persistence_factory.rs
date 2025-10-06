use crate::config::{PersistenceConfig, PersistenceBackend};
use crate::cow_btree::CowBTreePersistence;
use crate::lsm_tree::LsmTreePersistence;
use crate::rocksdb_impl::RocksDBPersistence;
use crate::{PersistenceError, PersistenceProvider};
use std::path::Path;
use std::sync::Arc;

/// Trait object for dynamic dispatch of persistence providers
pub type DynPersistenceProvider = Arc<dyn PersistenceProvider + Send + Sync>;

/// Factory for creating persistence providers based on configuration
pub struct PersistenceFactory;

impl PersistenceFactory {
    /// Create a persistence provider based on the configuration
    pub async fn create_provider(
        config: &PersistenceConfig,
    ) -> Result<DynPersistenceProvider, PersistenceError> {
        match config.backend {
            PersistenceBackend::CowBTree => {
                let cow_persistence = CowBTreePersistence::new(&config.data_dir).await?;
                Ok(Arc::new(cow_persistence))
            }
            PersistenceBackend::LsmTree => {
                let lsm_persistence = LsmTreePersistence::new(
                    config.lsm_config.clone(),
                    &config.data_dir,
                )
                .await?;
                Ok(Arc::new(lsm_persistence))
            }
            PersistenceBackend::RocksDb => {
                let rocksdb_persistence = RocksDBPersistence::new(&config.data_dir).await?;
                Ok(Arc::new(rocksdb_persistence))
            }
        }
    }
    
    /// Create a persistence provider with default configuration for the given backend
    pub async fn create_default(
        backend: PersistenceBackend,
        data_dir: &Path,
    ) -> Result<DynPersistenceProvider, PersistenceError> {
        let mut config = PersistenceConfig::default();
        config.backend = backend;
        config.data_dir = data_dir.to_path_buf();
        
        Self::create_provider(&config).await
    }
    
    /// Create all available persistence providers for benchmarking
    pub async fn create_all_providers(
        data_dir: &Path,
    ) -> Result<Vec<(String, DynPersistenceProvider)>, PersistenceError> {
        let mut providers = Vec::new();
        
        // COW B+ Tree
        {
            let cow_dir = data_dir.join("cow_btree");
            let cow_provider = Self::create_default(PersistenceBackend::CowBTree, &cow_dir).await?;
            providers.push(("COW B+ Tree".to_string(), cow_provider));
        }
        
        // LSM-Tree
        {
            let lsm_dir = data_dir.join("lsm_tree");
            let lsm_provider = Self::create_default(PersistenceBackend::LsmTree, &lsm_dir).await?;
            providers.push(("LSM-Tree".to_string(), lsm_provider));
        }
        
        // RocksDB
        {
            let rocksdb_dir = data_dir.join("rocksdb");
            let rocksdb_provider = Self::create_default(PersistenceBackend::RocksDb, &rocksdb_dir).await?;
            providers.push(("RocksDB".to_string(), rocksdb_provider));
        }
        
        Ok(providers)
    }
    
    /// Get a human-readable name for the backend
    pub fn backend_name(backend: &PersistenceBackend) -> &'static str {
        match backend {
            PersistenceBackend::CowBTree => "COW B+ Tree",
            PersistenceBackend::LsmTree => "LSM-Tree",
            PersistenceBackend::RocksDb => "RocksDB",
        }
    }
    
    /// Get expected performance characteristics for a backend
    pub fn expected_performance(backend: &PersistenceBackend) -> (u64, u64) {
        // Returns (write_latency_micros, read_latency_micros) based on benchmarks
        match backend {
            PersistenceBackend::CowBTree => (51, 1),  // 51μs write, 1μs read
            PersistenceBackend::LsmTree => (75, 5),   // Estimated: 75μs write, 5μs read
            PersistenceBackend::RocksDb => (412, 106), // 412μs write, 106μs read
        }
    }
    
    /// Get memory usage characteristics for a backend
    pub fn memory_characteristics(backend: &PersistenceBackend) -> (bool, bool, bool) {
        // Returns (is_memory_optimized, supports_zero_copy, has_write_amplification)
        match backend {
            PersistenceBackend::CowBTree => (true, true, false),   // Memory optimized, zero-copy, no write amp
            PersistenceBackend::LsmTree => (false, false, true),   // Not memory optimized, no zero-copy, has write amp
            PersistenceBackend::RocksDb => (false, false, true),   // Not memory optimized, no zero-copy, has write amp
        }
    }
}

/// Configuration builder for easier setup
pub struct PersistenceConfigBuilder {
    config: PersistenceConfig,
}

impl PersistenceConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: PersistenceConfig::default(),
        }
    }
    
    pub fn backend(mut self, backend: PersistenceBackend) -> Self {
        self.config.backend = backend;
        self
    }
    
    pub fn data_dir<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.config.data_dir = path.as_ref().to_path_buf();
        self
    }
    
    pub fn cow_max_keys(mut self, max_keys: usize) -> Self {
        self.config.cow_config.max_keys_per_node = max_keys;
        self
    }
    
    pub fn cow_wal_buffer_size(mut self, size_bytes: usize) -> Self {
        self.config.cow_config.wal_buffer_size = size_bytes;
        self
    }
    
    pub fn lsm_memtable_size(mut self, size_mb: usize) -> Self {
        self.config.lsm_config.memtable_size_mb = size_mb;
        self
    }
    
    pub fn lsm_max_levels(mut self, levels: usize) -> Self {
        self.config.lsm_config.max_levels = levels;
        self
    }
    
    pub fn lsm_enable_bloom_filters(mut self, enable: bool) -> Self {
        self.config.lsm_config.enable_bloom_filters = enable;
        self
    }
    
    pub fn lsm_compaction_threshold(mut self, threshold: usize) -> Self {
        self.config.lsm_config.compaction_threshold = threshold;
        self
    }
    
    pub fn rocksdb_compression(mut self, enable: bool) -> Self {
        self.config.rocksdb_config.compression = enable;
        self
    }
    
    pub fn rocksdb_block_cache_mb(mut self, size_mb: usize) -> Self {
        self.config.rocksdb_config.block_cache_mb = size_mb;
        self
    }
    
    pub fn build(self) -> PersistenceConfig {
        self.config
    }
    
    pub async fn create_provider(self) -> Result<DynPersistenceProvider, PersistenceError> {
        PersistenceFactory::create_provider(&self.config).await
    }
}

impl Default for PersistenceConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;
    use std::time::Duration;
    use crate::ActorLease;

    #[tokio::test]
    async fn test_factory_create_cow_btree() {
        let temp_dir = tempdir().unwrap();
        
        let config = PersistenceConfigBuilder::new()
            .backend(PersistenceBackend::CowBTree)
            .data_dir(temp_dir.path())
            .cow_max_keys(32)
            .build();
        
        let provider = PersistenceFactory::create_provider(&config).await.unwrap();
        
        // Test that it works
        let lease = ActorLease::new(
            Uuid::new_v4(),
            "test".to_string(),
            "node1".to_string(),
            Duration::from_secs(60),
        );
        
        let metrics = provider.store_lease(&lease).await.unwrap();
        assert!(metrics.success);
        
        let (retrieved, _) = provider.get_lease(&lease.key).await.unwrap();
        assert_eq!(retrieved.unwrap(), lease);
    }
    
    #[tokio::test]
    async fn test_factory_create_lsm_tree() {
        let temp_dir = tempdir().unwrap();
        
        let config = PersistenceConfigBuilder::new()
            .backend(PersistenceBackend::LsmTree)
            .data_dir(temp_dir.path())
            .lsm_memtable_size(4)
            .lsm_max_levels(3)
            .build();
        
        let provider = PersistenceFactory::create_provider(&config).await.unwrap();
        
        // Test that it works
        let lease = ActorLease::new(
            Uuid::new_v4(),
            "test".to_string(),
            "node1".to_string(),
            Duration::from_secs(60),
        );
        
        let metrics = provider.store_lease(&lease).await.unwrap();
        assert!(metrics.success);
        
        let (retrieved, _) = provider.get_lease(&lease.key).await.unwrap();
        assert_eq!(retrieved.unwrap(), lease);
    }
    
    #[tokio::test]
    async fn test_factory_create_all_providers() {
        let temp_dir = tempdir().unwrap();
        let providers = PersistenceFactory::create_all_providers(temp_dir.path()).await.unwrap();
        
        assert_eq!(providers.len(), 3);
        assert!(providers.iter().any(|(name, _)| name == "COW B+ Tree"));
        assert!(providers.iter().any(|(name, _)| name == "LSM-Tree"));
        assert!(providers.iter().any(|(name, _)| name == "RocksDB"));
    }
    
    #[test]
    fn test_backend_characteristics() {
        let (write_latency, read_latency) = PersistenceFactory::expected_performance(&PersistenceBackend::CowBTree);
        assert_eq!(write_latency, 51);
        assert_eq!(read_latency, 1);
        
        let (is_mem_opt, supports_zero_copy, has_write_amp) = 
            PersistenceFactory::memory_characteristics(&PersistenceBackend::CowBTree);
        assert!(is_mem_opt);
        assert!(supports_zero_copy);
        assert!(!has_write_amp);
    }
}