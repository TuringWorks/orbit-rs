//! Persistence provider factory for orbit-server
//!
//! This module provides factory functions to create the appropriate persistence
//! providers based on configuration, supporting all available backends.

use super::{
    AddressableDirectoryProvider, ClusterNodeProvider, CompressionType, DiskBackupConfig,
    MemoryConfig, PersistenceConfig, PersistenceProvider, PersistenceProviderRegistry,
};
use crate::persistence::{
    cow_btree::{CowBTreeAddressableProvider, CowBTreeClusterProvider, CowBTreeConfig},
    lsm_tree::{LsmTreeAddressableProvider, LsmTreeClusterProvider, LsmTreeConfig},
    memory,
    rocksdb::{RocksDbAddressableProvider, RocksDbClusterProvider, RocksDbConfig},
    tikv::{TiKVAddressableProvider, TiKVClusterProvider, TiKVConfig},
};
use orbit_shared::OrbitError;
use orbit_shared::OrbitResult;
use std::sync::Arc;

/// Helper function to create and initialize any provider that implements PersistenceProvider
async fn create_and_initialize_provider<T, F, Fut>(factory_fn: F) -> OrbitResult<Arc<T>>
where
    T: PersistenceProvider + 'static,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = OrbitResult<T>>,
{
    let provider = factory_fn().await?;
    provider.initialize().await?;
    Ok(Arc::new(provider))
}

/// Create an addressable directory persistence provider based on configuration
pub async fn create_addressable_provider(
    config: &PersistenceConfig,
) -> OrbitResult<Arc<dyn AddressableDirectoryProvider>> {
    match config {
        PersistenceConfig::Memory(memory_config) => {
            let config = memory_config.clone();
            create_and_initialize_provider(|| async {
                Ok(memory::MemoryAddressableDirectoryProvider::new(config))
            })
            .await
            .map(|p| p as Arc<dyn AddressableDirectoryProvider>)
        }
        PersistenceConfig::CowBTree(cow_config) => {
            let config = cow_config.clone();
            create_and_initialize_provider(|| async {
                Ok(CowBTreeAddressableProvider::new(config))
            })
            .await
            .map(|p| p as Arc<dyn AddressableDirectoryProvider>)
        }
        PersistenceConfig::LsmTree(lsm_config) => {
            let config = lsm_config.clone();
            create_and_initialize_provider(|| async {
                LsmTreeAddressableProvider::new(config).await
            })
            .await
            .map(|p| p as Arc<dyn AddressableDirectoryProvider>)
        }
        PersistenceConfig::RocksDB(rocks_config) => {
            let config = rocks_config.clone();
            create_and_initialize_provider(|| async { RocksDbAddressableProvider::new(config) })
                .await
                .map(|p| p as Arc<dyn AddressableDirectoryProvider>)
        }
        PersistenceConfig::TiKV(tikv_config) => {
            let config = tikv_config.clone();
            create_and_initialize_provider(|| async { TiKVAddressableProvider::new(config).await })
                .await
                .map(|p| p as Arc<dyn AddressableDirectoryProvider>)
        }
        _ => {
            tracing::warn!(
                "Unsupported persistence config for addressable provider, falling back to memory"
            );
            create_and_initialize_provider(|| async {
                Ok(memory::MemoryAddressableDirectoryProvider::new(
                    MemoryConfig::default(),
                ))
            })
            .await
            .map(|p| p as Arc<dyn AddressableDirectoryProvider>)
        }
    }
}

/// Create a cluster node persistence provider based on configuration
pub async fn create_cluster_provider(
    config: &PersistenceConfig,
) -> OrbitResult<Arc<dyn ClusterNodeProvider>> {
    match config {
        PersistenceConfig::Memory(memory_config) => {
            let config = memory_config.clone();
            create_and_initialize_provider(|| async {
                Ok(memory::MemoryClusterNodeProvider::new(config))
            })
            .await
            .map(|p| p as Arc<dyn ClusterNodeProvider>)
        }
        PersistenceConfig::CowBTree(cow_config) => {
            let config = cow_config.clone();
            create_and_initialize_provider(|| async { Ok(CowBTreeClusterProvider::new(config)) })
                .await
                .map(|p| p as Arc<dyn ClusterNodeProvider>)
        }
        PersistenceConfig::LsmTree(lsm_config) => {
            let config = lsm_config.clone();
            create_and_initialize_provider(|| async { LsmTreeClusterProvider::new(config).await })
                .await
                .map(|p| p as Arc<dyn ClusterNodeProvider>)
        }
        PersistenceConfig::RocksDB(rocks_config) => {
            let config = rocks_config.clone();
            create_and_initialize_provider(|| async { RocksDbClusterProvider::new(config) })
                .await
                .map(|p| p as Arc<dyn ClusterNodeProvider>)
        }
        PersistenceConfig::TiKV(tikv_config) => {
            let config = tikv_config.clone();
            create_and_initialize_provider(|| async { TiKVClusterProvider::new(config).await })
                .await
                .map(|p| p as Arc<dyn ClusterNodeProvider>)
        }
        _ => {
            tracing::warn!(
                "Unsupported persistence config for cluster provider, falling back to memory"
            );
            create_and_initialize_provider(|| async {
                Ok(memory::MemoryClusterNodeProvider::new(
                    MemoryConfig::default(),
                ))
            })
            .await
            .map(|p| p as Arc<dyn ClusterNodeProvider>)
        }
    }
}

/// Helper function to parse environment variable with default value
fn parse_env_var<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr + Clone,
{
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Helper function to parse boolean environment variable with default
fn parse_env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|s| s.parse().unwrap_or(default))
        .unwrap_or(default)
}

/// Helper function to get environment variable with default string value
fn get_env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Load persistence configuration from environment variables
pub fn load_config_from_env() -> OrbitResult<PersistenceConfig> {
    let backend_type = get_env_string("ORBIT_PERSISTENCE_BACKEND", "memory");

    match backend_type.as_str() {
        "memory" => {
            let max_entries = std::env::var("ORBIT_MEMORY_MAX_ENTRIES")
                .ok()
                .and_then(|s| s.parse().ok());

            let disk_backup = if std::env::var("ORBIT_MEMORY_DISK_BACKUP").is_ok() {
                let path = get_env_string("ORBIT_MEMORY_BACKUP_PATH", "./orbit_backup.json");
                let sync_interval = parse_env_var("ORBIT_MEMORY_BACKUP_INTERVAL", 300u64);

                Some(DiskBackupConfig {
                    path,
                    sync_interval,
                    compression: CompressionType::Gzip,
                })
            } else {
                None
            };

            Ok(PersistenceConfig::Memory(MemoryConfig {
                max_entries,
                disk_backup,
            }))
        }
        "cow_btree" => {
            let config = CowBTreeConfig {
                data_dir: get_env_string("ORBIT_COW_DATA_DIR", "./orbit_cow_data"),
                max_keys_per_node: parse_env_var("ORBIT_COW_MAX_KEYS_PER_NODE", 16),
                wal_buffer_size: parse_env_var("ORBIT_COW_WAL_BUFFER_SIZE", 1024 * 1024),
                enable_compression: parse_env_bool("ORBIT_COW_ENABLE_COMPRESSION", true),
                wal_sync_interval: parse_env_var("ORBIT_COW_WAL_SYNC_INTERVAL", 5),
            };
            Ok(PersistenceConfig::CowBTree(config))
        }
        "lsm_tree" => {
            let config = LsmTreeConfig {
                data_dir: std::env::var("ORBIT_LSM_DATA_DIR")
                    .unwrap_or_else(|_| "./orbit_lsm_data".to_string()),
                memtable_size_limit: std::env::var("ORBIT_LSM_MEMTABLE_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(64 * 1024 * 1024), // 64MB
                max_memtables: std::env::var("ORBIT_LSM_MAX_MEMTABLES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
                bloom_filter_fp_rate: std::env::var("ORBIT_LSM_BLOOM_FP_RATE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.01),
                enable_compaction: std::env::var("ORBIT_LSM_ENABLE_COMPACTION")
                    .map(|s| s.parse().unwrap_or(true))
                    .unwrap_or(true),
                compaction_threshold: std::env::var("ORBIT_LSM_COMPACTION_THRESHOLD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(4),
            };
            Ok(PersistenceConfig::LsmTree(config))
        }
        "rocksdb" => {
            let config = RocksDbConfig {
                data_dir: std::env::var("ORBIT_ROCKSDB_DATA_DIR")
                    .unwrap_or_else(|_| "./orbit_rocksdb_data".to_string()),
                enable_wal: std::env::var("ORBIT_ROCKSDB_ENABLE_WAL")
                    .map(|s| s.parse().unwrap_or(true))
                    .unwrap_or(true),
                max_background_jobs: std::env::var("ORBIT_ROCKSDB_MAX_BACKGROUND_JOBS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(4),
                write_buffer_size: std::env::var("ORBIT_ROCKSDB_WRITE_BUFFER_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(128 * 1024 * 1024), // 128MB
                max_write_buffer_number: std::env::var("ORBIT_ROCKSDB_MAX_WRITE_BUFFER_NUMBER")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3),
                target_file_size_base: std::env::var("ORBIT_ROCKSDB_TARGET_FILE_SIZE_BASE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(64 * 1024 * 1024), // 64MB
                enable_statistics: std::env::var("ORBIT_ROCKSDB_ENABLE_STATISTICS")
                    .map(|s| s.parse().unwrap_or(true))
                    .unwrap_or(true),
                block_cache_size: std::env::var("ORBIT_ROCKSDB_BLOCK_CACHE_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(256 * 1024 * 1024), // 256MB
            };
            Ok(PersistenceConfig::RocksDB(config))
        }
        "tikv" => {
            let pd_endpoints_str = std::env::var("ORBIT_TIKV_PD_ENDPOINTS")
                .unwrap_or_else(|_| "127.0.0.1:2379".to_string());
            let pd_endpoints: Vec<String> = pd_endpoints_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();

            let config = TiKVConfig {
                pd_endpoints,
                connection_timeout: std::env::var("ORBIT_TIKV_CONNECTION_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                request_timeout: std::env::var("ORBIT_TIKV_REQUEST_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                max_connections: std::env::var("ORBIT_TIKV_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                enable_pessimistic_txn: std::env::var("ORBIT_TIKV_ENABLE_PESSIMISTIC_TXN")
                    .map(|s| s.parse().unwrap_or(false))
                    .unwrap_or(false),
                txn_timeout: std::env::var("ORBIT_TIKV_TXN_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok()),
                enable_async_commit: std::env::var("ORBIT_TIKV_ENABLE_ASYNC_COMMIT")
                    .map(|s| s.parse().unwrap_or(true))
                    .unwrap_or(true),
                enable_one_pc: std::env::var("ORBIT_TIKV_ENABLE_ONE_PC")
                    .map(|s| s.parse().unwrap_or(true))
                    .unwrap_or(true),
                batch_size: std::env::var("ORBIT_TIKV_BATCH_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1000),
                region_cache_size: std::env::var("ORBIT_TIKV_REGION_CACHE_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1000),
                coprocessor_pool_size: std::env::var("ORBIT_TIKV_COPROCESSOR_POOL_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(8),
                enable_tls: std::env::var("ORBIT_TIKV_ENABLE_TLS")
                    .map(|s| s.parse().unwrap_or(false))
                    .unwrap_or(false),
                ca_cert_path: std::env::var("ORBIT_TIKV_CA_CERT_PATH").ok(),
                client_cert_path: std::env::var("ORBIT_TIKV_CLIENT_CERT_PATH").ok(),
                client_key_path: std::env::var("ORBIT_TIKV_CLIENT_KEY_PATH").ok(),
                key_prefix: std::env::var("ORBIT_TIKV_KEY_PREFIX")
                    .unwrap_or_else(|_| "orbit".to_string()),
                enable_compression: std::env::var("ORBIT_TIKV_ENABLE_COMPRESSION")
                    .map(|s| s.parse().unwrap_or(true))
                    .unwrap_or(true),
                max_retries: std::env::var("ORBIT_TIKV_MAX_RETRIES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3),
                retry_delay_ms: std::env::var("ORBIT_TIKV_RETRY_DELAY_MS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100),
            };
            Ok(PersistenceConfig::TiKV(config))
        }
        _ => Err(OrbitError::configuration(format!(
            "Unknown persistence backend: {}",
            backend_type
        ))),
    }
}

/// Load persistence configuration from a TOML file
pub async fn load_config_from_file(path: &str) -> OrbitResult<PersistenceConfig> {
    let content = tokio::fs::read_to_string(path).await.map_err(|e| {
        OrbitError::configuration(format!("Failed to read config file {}: {}", path, e))
    })?;

    let config: PersistenceConfig = toml::from_str(&content).map_err(|e| {
        OrbitError::configuration(format!("Failed to parse config file {}: {}", path, e))
    })?;

    Ok(config)
}

/// Configuration builder for programmatic setup
pub struct PersistenceConfigBuilder {
    backend_type: Option<String>,
    data_dir: Option<String>,
}

impl PersistenceConfigBuilder {
    pub fn new() -> Self {
        Self {
            backend_type: None,
            data_dir: None,
        }
    }

    pub fn backend(mut self, backend_type: &str) -> Self {
        self.backend_type = Some(backend_type.to_string());
        self
    }

    pub fn data_dir(mut self, data_dir: &str) -> Self {
        self.data_dir = Some(data_dir.to_string());
        self
    }

    pub fn build(self) -> OrbitResult<PersistenceConfig> {
        let backend_type = self.backend_type.unwrap_or_else(|| "memory".to_string());
        let data_dir = self
            .data_dir
            .unwrap_or_else(|| format!("./orbit_{}_data", backend_type));

        match backend_type.as_str() {
            "memory" => Ok(PersistenceConfig::Memory(MemoryConfig::default())),
            "cow_btree" => Ok(PersistenceConfig::CowBTree(CowBTreeConfig {
                data_dir,
                ..Default::default()
            })),
            "lsm_tree" => Ok(PersistenceConfig::LsmTree(LsmTreeConfig {
                data_dir,
                ..Default::default()
            })),
            "rocksdb" => Ok(PersistenceConfig::RocksDB(RocksDbConfig {
                data_dir,
                ..Default::default()
            })),
            "tikv" => Ok(PersistenceConfig::TiKV(TiKVConfig {
                key_prefix: data_dir,
                ..Default::default()
            })),
            _ => Err(OrbitError::configuration(format!(
                "Unknown persistence backend: {}",
                backend_type
            ))),
        }
    }
}

impl Default for PersistenceConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize a complete persistence registry with the configured providers
pub async fn initialize_registry(
    addressable_config: &PersistenceConfig,
    cluster_config: &PersistenceConfig,
) -> OrbitResult<PersistenceProviderRegistry> {
    let registry = PersistenceProviderRegistry::new();

    // Create and register addressable provider
    let addressable_provider = create_addressable_provider(addressable_config).await?;
    registry
        .register_addressable_provider("default".to_string(), addressable_provider, true)
        .await?;

    // Create and register cluster provider
    let cluster_provider = create_cluster_provider(cluster_config).await?;
    registry
        .register_cluster_provider("default".to_string(), cluster_provider, true)
        .await?;

    tracing::info!("Persistence registry initialized with providers");
    Ok(registry)
}
