//! WASM Runtime Configuration
//!
//! This module provides configuration options for the WASM runtime,
//! including resource limits, caching, and security settings.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for WASM runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// Enable WASM UDF support
    pub enabled: bool,

    /// Maximum memory per WASM instance (bytes)
    /// Default: 64MB
    pub max_memory_bytes: usize,

    /// Maximum execution time per function call
    /// Default: 30 seconds
    pub timeout: Duration,

    /// Enable module caching for faster execution
    /// Default: true
    pub enable_cache: bool,

    /// Maximum number of cached compiled modules
    /// Default: 100
    pub cache_size: usize,

    /// Enable fuel-based execution limiting
    /// Default: true (prevents infinite loops)
    pub enable_fuel: bool,

    /// Fuel units per execution (1 fuel ≈ 1 WASM instruction)
    /// Default: 1 billion (enough for most functions)
    pub fuel_limit: u64,

    /// Enable WASI (WebAssembly System Interface) support
    /// Default: false (for security)
    pub enable_wasi: bool,

    /// Maximum WASM module size (bytes)
    /// Default: 10MB
    pub max_module_size: usize,

    /// Enable parallel execution of UDFs
    /// Default: true
    pub enable_parallel: bool,

    /// Maximum concurrent WASM instances
    /// Default: 100
    pub max_concurrent_instances: usize,

    /// Enable SIMD (Single Instruction Multiple Data) support
    /// Allows vectorized operations for better performance
    /// Default: true
    pub enable_simd: bool,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            timeout: Duration::from_secs(30),
            enable_cache: true,
            cache_size: 100,
            enable_fuel: true,
            fuel_limit: 1_000_000_000, // 1 billion instructions
            enable_wasi: false,         // Disabled for security
            max_module_size: 10 * 1024 * 1024, // 10MB
            enable_parallel: true,
            max_concurrent_instances: 100,
            enable_simd: true, // Enable SIMD for better performance
        }
    }
}

impl WasmConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a development configuration (more permissive)
    pub fn development() -> Self {
        Self {
            max_memory_bytes: 128 * 1024 * 1024, // 128MB
            timeout: Duration::from_secs(300),     // 5 minutes
            enable_wasi: true,                     // Allow WASI in dev
            fuel_limit: 10_000_000_000,            // 10 billion
            ..Default::default()
        }
    }

    /// Create a production configuration (more restrictive)
    pub fn production() -> Self {
        Self {
            max_memory_bytes: 32 * 1024 * 1024, // 32MB
            timeout: Duration::from_secs(10),    // 10 seconds
            enable_wasi: false,                  // No WASI in prod
            fuel_limit: 500_000_000,             // 500 million
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.max_memory_bytes < 1024 * 1024 {
            return Err("max_memory_bytes must be at least 1MB".to_string());
        }

        if self.timeout.as_secs() < 1 {
            return Err("timeout must be at least 1 second".to_string());
        }

        if self.cache_size == 0 {
            return Err("cache_size must be greater than 0".to_string());
        }

        if self.fuel_limit < 10_000 {
            return Err("fuel_limit must be at least 10,000".to_string());
        }

        if self.max_module_size < 1024 {
            return Err("max_module_size must be at least 1KB".to_string());
        }

        if self.max_concurrent_instances == 0 {
            return Err("max_concurrent_instances must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Get timeout in milliseconds
    pub fn timeout_millis(&self) -> u64 {
        self.timeout.as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WasmConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.enable_cache);
        assert!(config.enable_fuel);
        assert!(!config.enable_wasi);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_development_config() {
        let config = WasmConfig::development();
        assert_eq!(config.max_memory_bytes, 128 * 1024 * 1024);
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert!(config.enable_wasi);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_production_config() {
        let config = WasmConfig::production();
        assert_eq!(config.max_memory_bytes, 32 * 1024 * 1024);
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert!(!config.enable_wasi);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation() {
        let mut config = WasmConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid memory
        config.max_memory_bytes = 1024;
        assert!(config.validate().is_err());
        config.max_memory_bytes = 64 * 1024 * 1024;

        // Invalid timeout
        config.timeout = Duration::from_millis(500);
        assert!(config.validate().is_err());
        config.timeout = Duration::from_secs(30);

        // Invalid cache size
        config.cache_size = 0;
        assert!(config.validate().is_err());
        config.cache_size = 100;

        // Invalid fuel limit
        config.fuel_limit = 100;
        assert!(config.validate().is_err());
        config.fuel_limit = 1_000_000_000;

        // Should be valid again
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_timeout_millis() {
        let config = WasmConfig {
            timeout: Duration::from_secs(5),
            ..Default::default()
        };
        assert_eq!(config.timeout_millis(), 5000);
    }
}
