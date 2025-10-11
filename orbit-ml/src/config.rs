//! Configuration module for the ML engine.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for the ML engine
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MLConfig {
    /// Engine settings
    pub engine: EngineConfig,

    /// Model storage configuration
    pub storage: StorageConfig,

    /// Compute configuration
    pub compute: ComputeConfig,

    /// Multi-language runtime configuration
    pub runtimes: RuntimeConfig,

    /// Security and sandboxing configuration
    pub security: SecurityConfig,

    /// Monitoring and metrics configuration
    pub monitoring: MonitoringConfig,
}

/// Engine-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Maximum number of concurrent training jobs
    pub max_concurrent_training_jobs: usize,

    /// Maximum number of concurrent inference jobs
    pub max_concurrent_inference_jobs: usize,

    /// Job timeout in seconds
    pub job_timeout_seconds: u64,

    /// Model cache size in MB
    pub model_cache_size_mb: usize,

    /// Enable automatic model optimization
    pub enable_auto_optimization: bool,

    /// Enable model versioning
    pub enable_versioning: bool,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Base directory for model storage
    pub model_storage_path: PathBuf,

    /// Base directory for dataset storage
    pub dataset_storage_path: PathBuf,

    /// Base directory for checkpoint storage
    pub checkpoint_storage_path: PathBuf,

    /// Model compression level (0-9)
    pub compression_level: u8,

    /// Enable model encryption
    pub enable_encryption: bool,

    /// Storage backend type
    pub backend: StorageBackend,
}

/// Storage backend types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    /// Local filesystem storage
    Local,

    /// Amazon S3 storage
    S3 {
        /// S3 bucket name
        bucket: String,
        /// AWS region
        region: String,
        /// AWS access key ID (optional, can use IAM roles)
        access_key_id: Option<String>,
        /// AWS secret access key (optional, can use IAM roles)
        secret_access_key: Option<String>,
    },

    /// Google Cloud Storage
    Gcs {
        /// GCS bucket name
        bucket: String,
        /// Path to GCS service account credentials file
        credentials_path: Option<PathBuf>,
    },

    /// Azure Blob Storage
    Azure {
        /// Azure storage account name
        account_name: String,
        /// Azure blob container name
        container: String,
        /// Azure storage account access key
        access_key: Option<String>,
    },
}

/// Compute configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeConfig {
    /// Enable GPU acceleration
    pub enable_gpu: bool,

    /// GPU device IDs to use
    pub gpu_device_ids: Vec<u32>,

    /// Enable distributed training
    pub enable_distributed: bool,

    /// Number of worker threads for CPU operations
    pub cpu_threads: Option<usize>,

    /// Memory limit in MB
    pub memory_limit_mb: Option<usize>,

    /// Enable mixed precision training
    pub enable_mixed_precision: bool,
}

/// Multi-language runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    /// Python runtime configuration
    pub python: PythonConfig,

    /// JavaScript runtime configuration
    pub javascript: JavaScriptConfig,

    /// Lua runtime configuration
    pub lua: LuaConfig,
}

/// Python runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonConfig {
    /// Enable Python integration
    pub enabled: bool,

    /// Python executable path
    pub python_path: Option<PathBuf>,

    /// Virtual environment path
    pub venv_path: Option<PathBuf>,

    /// Additional Python paths
    pub additional_paths: Vec<PathBuf>,

    /// Memory limit for Python processes in MB
    pub memory_limit_mb: Option<usize>,

    /// Timeout for Python operations in seconds
    pub timeout_seconds: u64,
}

/// JavaScript runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaScriptConfig {
    /// Enable JavaScript integration
    pub enabled: bool,

    /// Node.js executable path
    pub node_path: Option<PathBuf>,

    /// Deno executable path
    pub deno_path: Option<PathBuf>,

    /// Runtime type preference
    pub runtime_preference: JavaScriptRuntime,

    /// Memory limit in MB
    pub memory_limit_mb: Option<usize>,

    /// Timeout for operations in seconds
    pub timeout_seconds: u64,
}

/// JavaScript runtime types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JavaScriptRuntime {
    /// Node.js runtime
    Node,

    /// Deno runtime
    Deno,

    /// V8 embedded runtime
    V8,
}

/// Lua runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaConfig {
    /// Enable Lua integration
    pub enabled: bool,

    /// Lua version to use
    pub version: LuaVersion,

    /// Memory limit in MB
    pub memory_limit_mb: Option<usize>,

    /// Timeout for operations in seconds
    pub timeout_seconds: u64,

    /// Enable JIT compilation
    pub enable_jit: bool,
}

/// Lua versions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LuaVersion {
    /// Lua 5.1
    Lua51,

    /// Lua 5.2
    Lua52,

    /// Lua 5.3
    Lua53,

    /// Lua 5.4
    Lua54,

    /// LuaJIT
    LuaJit,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable sandboxing for external code
    pub enable_sandboxing: bool,

    /// Allowed network access for external code
    pub allow_network_access: bool,

    /// Allowed filesystem access paths
    pub allowed_fs_paths: Vec<PathBuf>,

    /// Maximum execution time for external code in seconds
    pub max_execution_time_seconds: u64,

    /// Enable code signing verification
    pub enable_code_signing: bool,

    /// Trusted certificate paths
    pub trusted_certificates: Vec<PathBuf>,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,

    /// Metrics collection interval in seconds
    pub metrics_interval_seconds: u64,

    /// Enable distributed tracing
    pub enable_tracing: bool,

    /// Tracing endpoint URL
    pub tracing_endpoint: Option<String>,

    /// Enable performance profiling
    pub enable_profiling: bool,

    /// Log level
    pub log_level: LogLevel,
}

/// Log levels
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Error level
    Error,

    /// Warning level
    Warn,

    /// Info level
    Info,

    /// Debug level
    Debug,

    /// Trace level
    Trace,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_training_jobs: 4,
            max_concurrent_inference_jobs: 16,
            job_timeout_seconds: 3600, // 1 hour
            model_cache_size_mb: 1024, // 1 GB
            enable_auto_optimization: true,
            enable_versioning: true,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            model_storage_path: PathBuf::from("./data/models"),
            dataset_storage_path: PathBuf::from("./data/datasets"),
            checkpoint_storage_path: PathBuf::from("./data/checkpoints"),
            compression_level: 6,
            enable_encryption: false,
            backend: StorageBackend::Local,
        }
    }
}

impl Default for ComputeConfig {
    fn default() -> Self {
        Self {
            enable_gpu: true,
            gpu_device_ids: vec![0],
            enable_distributed: false,
            cpu_threads: None,     // Use all available cores
            memory_limit_mb: None, // No limit
            enable_mixed_precision: false,
        }
    }
}

impl Default for PythonConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            python_path: None, // Auto-detect
            venv_path: None,
            additional_paths: Vec::new(),
            memory_limit_mb: Some(1024), // 1 GB
            timeout_seconds: 300,        // 5 minutes
        }
    }
}

impl Default for JavaScriptConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            node_path: None, // Auto-detect
            deno_path: None, // Auto-detect
            runtime_preference: JavaScriptRuntime::Node,
            memory_limit_mb: Some(512), // 512 MB
            timeout_seconds: 300,       // 5 minutes
        }
    }
}

impl Default for LuaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            version: LuaVersion::Lua54,
            memory_limit_mb: Some(256), // 256 MB
            timeout_seconds: 300,       // 5 minutes
            enable_jit: true,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_sandboxing: true,
            allow_network_access: false,
            allowed_fs_paths: vec![
                PathBuf::from("./data"),
                PathBuf::from("./models"),
                PathBuf::from("/tmp"),
            ],
            max_execution_time_seconds: 600, // 10 minutes
            enable_code_signing: false,
            trusted_certificates: Vec::new(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics_interval_seconds: 60,
            enable_tracing: false,
            tracing_endpoint: None,
            enable_profiling: false,
            log_level: LogLevel::Info,
        }
    }
}

impl MLConfig {
    /// Load configuration from file
    pub fn from_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: MLConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn to_file<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate engine config
        if self.engine.max_concurrent_training_jobs == 0 {
            return Err("max_concurrent_training_jobs must be greater than 0".to_string());
        }

        if self.engine.max_concurrent_inference_jobs == 0 {
            return Err("max_concurrent_inference_jobs must be greater than 0".to_string());
        }

        // Validate storage config
        if self.storage.compression_level > 9 {
            return Err("compression_level must be between 0 and 9".to_string());
        }

        // Validate compute config
        if self.compute.enable_gpu && self.compute.gpu_device_ids.is_empty() {
            return Err("gpu_device_ids cannot be empty when GPU is enabled".to_string());
        }

        // Validate runtime timeouts
        if self.runtimes.python.timeout_seconds == 0 {
            return Err("Python timeout must be greater than 0".to_string());
        }

        if self.runtimes.javascript.timeout_seconds == 0 {
            return Err("JavaScript timeout must be greater than 0".to_string());
        }

        if self.runtimes.lua.timeout_seconds == 0 {
            return Err("Lua timeout must be greater than 0".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = MLConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = MLConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MLConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.validate().is_ok());
    }

    #[test]
    fn test_config_file_operations() {
        let config = MLConfig::default();
        let temp_file = NamedTempFile::new().unwrap();

        // Save to file
        config.to_file(temp_file.path()).unwrap();

        // Load from file
        let loaded_config = MLConfig::from_file(temp_file.path()).unwrap();
        assert!(loaded_config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = MLConfig::default();

        // Test invalid training jobs
        config.engine.max_concurrent_training_jobs = 0;
        assert!(config.validate().is_err());

        // Reset and test invalid compression level
        config = MLConfig::default();
        config.storage.compression_level = 10;
        assert!(config.validate().is_err());

        // Reset and test GPU config
        config = MLConfig::default();
        config.compute.enable_gpu = true;
        config.compute.gpu_device_ids.clear();
        assert!(config.validate().is_err());
    }
}
