//! Python UDF Configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for Python UDF system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonConfig {
    /// Python interpreter path (default: "python3")
    pub python_path: String,

    /// Number of worker processes in the pool (default: 4)
    pub pool_size: usize,

    /// Worker configuration
    pub worker: PythonWorkerConfig,

    /// Whether to use MessagePack (faster) or JSON (fallback)
    pub use_msgpack: bool,

    /// Path to worker.py script
    pub worker_script_path: Option<PathBuf>,
}

impl Default for PythonConfig {
    fn default() -> Self {
        Self {
            python_path: "python3".to_string(),
            pool_size: 4,
            worker: PythonWorkerConfig::default(),
            use_msgpack: true,
            worker_script_path: None,
        }
    }
}

/// Configuration for individual Python worker processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonWorkerConfig {
    /// Maximum memory per worker (bytes, default: 512MB)
    pub max_memory_bytes: usize,

    /// Function execution timeout (seconds, default: 30)
    pub timeout_seconds: u64,

    /// Maximum CPU time (seconds, default: 30)
    pub max_cpu_time_seconds: u64,

    /// Restart worker after N executions (0 = never, default: 1000)
    pub restart_after_executions: usize,

    /// Health check interval (seconds, default: 60)
    pub health_check_interval_seconds: u64,

    /// Whitelisted Python libraries (empty = all pre-loadable libraries allowed)
    pub allowed_libraries: Vec<String>,
}

impl Default for PythonWorkerConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            timeout_seconds: 30,
            max_cpu_time_seconds: 30,
            restart_after_executions: 1000,
            health_check_interval_seconds: 60,
            allowed_libraries: vec![
                "numpy".to_string(),
                "pandas".to_string(),
                "math".to_string(),
                "re".to_string(),
                "decimal".to_string(),
            ],
        }
    }
}

impl PythonConfig {
    /// Create configuration with custom Python path
    pub fn with_python_path(mut self, path: impl Into<String>) -> Self {
        self.python_path = path.into();
        self
    }

    /// Set pool size
    pub fn with_pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Enable/disable MessagePack
    pub fn with_msgpack(mut self, enabled: bool) -> Self {
        self.use_msgpack = enabled;
        self
    }

    /// Set worker script path
    pub fn with_worker_script(mut self, path: PathBuf) -> Self {
        self.worker_script_path = Some(path);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PythonConfig::default();
        assert_eq!(config.python_path, "python3");
        assert_eq!(config.pool_size, 4);
        assert!(config.use_msgpack);
        assert_eq!(config.worker.timeout_seconds, 30);
    }

    #[test]
    fn test_builder_pattern() {
        let config = PythonConfig::default()
            .with_python_path("/usr/bin/python3.11")
            .with_pool_size(8)
            .with_msgpack(false);

        assert_eq!(config.python_path, "/usr/bin/python3.11");
        assert_eq!(config.pool_size, 8);
        assert!(!config.use_msgpack);
    }
}
