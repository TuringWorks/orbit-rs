use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// The persistence backend to use
    pub backend: PersistenceBackend,

    /// Data directory for storage files
    pub data_dir: PathBuf,

    /// COW B+ Tree specific configuration
    pub cow_config: CowBTreeConfig,

    /// LSM-Tree specific configuration  
    pub lsm_config: LsmConfig,

    /// RocksDB specific configuration (existing)
    pub rocksdb_config: RocksDbConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersistenceBackend {
    /// Copy-on-Write B+ Tree with WAL
    CowBTree,
    /// LSM-Tree (Log-Structured Merge Tree)
    LsmTree,
    /// RocksDB (existing implementation)
    RocksDb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CowBTreeConfig {
    /// Maximum keys per B+ tree node
    pub max_keys_per_node: usize,
    /// WAL buffer size in bytes
    pub wal_buffer_size: usize,
    /// Enable automatic snapshots
    pub enable_snapshots: bool,
    /// Snapshot interval in operations
    pub snapshot_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsmConfig {
    /// MemTable size before flush (in bytes)
    pub memtable_size_mb: usize,
    /// Number of levels in the LSM tree
    pub max_levels: usize,
    /// Level size multiplier
    pub level_size_multiplier: usize,
    /// Compaction threshold (number of SSTables per level)
    pub compaction_threshold: usize,
    /// Block size for SSTables (in KB)
    pub sstable_block_size_kb: usize,
    /// Enable bloom filters for SSTables
    pub enable_bloom_filters: bool,
    /// Bloom filter bits per key
    pub bloom_filter_bits_per_key: usize,
    /// Enable compression for SSTables
    pub enable_compression: bool,
    /// Write buffer size (in MB)
    pub write_buffer_mb: usize,
    /// Cache size for frequently accessed blocks (in MB)
    pub block_cache_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocksDbConfig {
    /// Enable compression
    pub compression: bool,
    /// Block cache size in MB
    pub block_cache_mb: usize,
    /// Write buffer size in MB
    pub write_buffer_mb: usize,
    /// Maximum number of background threads
    pub max_background_jobs: i32,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            backend: PersistenceBackend::CowBTree,
            data_dir: PathBuf::from("./data"),
            cow_config: CowBTreeConfig::default(),
            lsm_config: LsmConfig::default(),
            rocksdb_config: RocksDbConfig::default(),
        }
    }
}

impl Default for CowBTreeConfig {
    fn default() -> Self {
        Self {
            max_keys_per_node: 16,
            wal_buffer_size: 1024 * 1024, // 1MB
            enable_snapshots: true,
            snapshot_interval: 1000,
        }
    }
}

impl Default for LsmConfig {
    fn default() -> Self {
        Self {
            memtable_size_mb: 64,
            max_levels: 7,
            level_size_multiplier: 10,
            compaction_threshold: 4,
            sstable_block_size_kb: 4,
            enable_bloom_filters: true,
            bloom_filter_bits_per_key: 10,
            enable_compression: true,
            write_buffer_mb: 32,
            block_cache_mb: 256,
        }
    }
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            compression: true,
            block_cache_mb: 256,
            write_buffer_mb: 64,
            max_background_jobs: 4,
        }
    }
}

impl PersistenceConfig {
    /// Load configuration from a JSON file
    pub fn from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a JSON file
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Parse backend from environment
        if let Ok(backend_str) = std::env::var("ORBIT_PERSISTENCE_BACKEND") {
            config.backend = match backend_str.to_lowercase().as_str() {
                "cow" | "cowbtree" | "cow_btree" => PersistenceBackend::CowBTree,
                "lsm" | "lsmtree" | "lsm_tree" => PersistenceBackend::LsmTree,
                "rocksdb" | "rocks" => PersistenceBackend::RocksDb,
                _ => {
                    return Err(ConfigError::InvalidConfig(format!(
                        "Unknown backend: {}",
                        backend_str
                    )))
                }
            };
        }

        // Parse data directory
        if let Ok(data_dir) = std::env::var("ORBIT_DATA_DIR") {
            config.data_dir = PathBuf::from(data_dir);
        }

        // Parse COW configuration
        if let Ok(val) = std::env::var("ORBIT_COW_MAX_KEYS") {
            config.cow_config.max_keys_per_node = val
                .parse()
                .map_err(|_| ConfigError::InvalidConfig("Invalid COW max keys".into()))?;
        }

        // Parse LSM configuration
        if let Ok(val) = std::env::var("ORBIT_LSM_MEMTABLE_SIZE_MB") {
            config.lsm_config.memtable_size_mb = val
                .parse()
                .map_err(|_| ConfigError::InvalidConfig("Invalid LSM memtable size".into()))?;
        }

        if let Ok(val) = std::env::var("ORBIT_LSM_MAX_LEVELS") {
            config.lsm_config.max_levels = val
                .parse()
                .map_err(|_| ConfigError::InvalidConfig("Invalid LSM max levels".into()))?;
        }

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate COW config
        if self.cow_config.max_keys_per_node < 2 {
            return Err(ConfigError::InvalidConfig(
                "COW max_keys_per_node must be at least 2".into(),
            ));
        }

        if self.cow_config.wal_buffer_size < 1024 {
            return Err(ConfigError::InvalidConfig(
                "COW WAL buffer size must be at least 1KB".into(),
            ));
        }

        // Validate LSM config
        if self.lsm_config.memtable_size_mb < 1 {
            return Err(ConfigError::InvalidConfig(
                "LSM memtable size must be at least 1MB".into(),
            ));
        }

        if self.lsm_config.max_levels < 2 {
            return Err(ConfigError::InvalidConfig(
                "LSM max levels must be at least 2".into(),
            ));
        }

        if self.lsm_config.level_size_multiplier < 2 {
            return Err(ConfigError::InvalidConfig(
                "LSM level size multiplier must be at least 2".into(),
            ));
        }

        if self.lsm_config.compaction_threshold < 2 {
            return Err(ConfigError::InvalidConfig(
                "LSM compaction threshold must be at least 2".into(),
            ));
        }

        // Validate RocksDB config
        if self.rocksdb_config.block_cache_mb < 1 {
            return Err(ConfigError::InvalidConfig(
                "RocksDB block cache must be at least 1MB".into(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = PersistenceConfig::default();
        assert!(matches!(config.backend, PersistenceBackend::CowBTree));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = PersistenceConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PersistenceConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized.backend, PersistenceBackend::CowBTree));
    }

    #[test]
    fn test_config_file_io() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let config = PersistenceConfig::default();
        config.to_file(&config_path).unwrap();

        let loaded_config = PersistenceConfig::from_file(&config_path).unwrap();
        assert!(matches!(
            loaded_config.backend,
            PersistenceBackend::CowBTree
        ));
    }

    #[test]
    fn test_config_validation() {
        let mut config = PersistenceConfig::default();

        // Test invalid COW config
        config.cow_config.max_keys_per_node = 1;
        assert!(config.validate().is_err());

        // Test invalid LSM config
        config.cow_config.max_keys_per_node = 16;
        config.lsm_config.max_levels = 1;
        assert!(config.validate().is_err());
    }
}
