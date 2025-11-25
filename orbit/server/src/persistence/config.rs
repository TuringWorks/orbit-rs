//! Configuration system for persistence providers
//!
//! This module provides configuration builders, validation, and factory methods
//! for creating persistence providers from configuration.

use super::{
    AWSS3Config, AzureConfig, CompositeConfig, DigitalOceanSpacesConfig, EtcdConfig,
    GCPStorageConfig, GoogleCloudConfig, MemoryConfig, PersistenceConfig, PersistenceProvider,
    PersistenceProviderRegistry, RedisConfig, S3Config,
};
use crate::persistence::{
    cloud::{S3AddressableDirectoryProvider, S3ClusterNodeProvider},
    memory::{MemoryAddressableDirectoryProvider, MemoryClusterNodeProvider},
};
use orbit_shared::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitorConfig {
    /// Health check interval in seconds
    pub check_interval: u64,
    /// Timeout for health checks in seconds
    pub check_timeout: u64,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,
    /// Enable circuit breaker pattern
    pub enable_circuit_breaker: bool,
    /// Circuit breaker open duration in seconds
    pub circuit_breaker_timeout: u64,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: 30,
            check_timeout: 5,
            failure_threshold: 3,
            success_threshold: 2,
            enable_circuit_breaker: true,
            circuit_breaker_timeout: 300,
        }
    }
}

/// Failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// Enable automatic failover
    pub enable_auto_failover: bool,
    /// Failover strategy
    pub strategy: FailoverStrategy,
    /// Maximum failover attempts before giving up
    pub max_attempts: u32,
    /// Delay between failover attempts in seconds
    pub retry_delay: u64,
    /// Enable failback to primary when it recovers
    pub enable_failback: bool,
    /// Delay before attempting failback in seconds
    pub failback_delay: u64,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enable_auto_failover: true,
            strategy: FailoverStrategy::RoundRobin,
            max_attempts: 3,
            retry_delay: 10,
            enable_failback: true,
            failback_delay: 300,
        }
    }
}

/// Failover strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailoverStrategy {
    /// Round-robin through available providers
    RoundRobin,
    /// Use providers in priority order
    Priority,
    /// Use the provider with lowest latency
    LowestLatency,
    /// Load balance based on current load
    LoadBased,
}

/// Dynamic provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicProviderConfig {
    /// Health monitoring configuration
    pub health_monitor: HealthMonitorConfig,
    /// Failover configuration
    pub failover: FailoverConfig,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Performance monitoring window in seconds
    pub performance_window: u64,
    /// Enable automatic scaling decisions
    pub enable_auto_scaling: bool,
}

impl Default for DynamicProviderConfig {
    fn default() -> Self {
        Self {
            health_monitor: HealthMonitorConfig::default(),
            failover: FailoverConfig::default(),
            enable_performance_monitoring: true,
            performance_window: 300,
            enable_auto_scaling: false,
        }
    }
}

/// Builder for creating persistence provider configurations
pub struct PersistenceConfigBuilder {
    configs: HashMap<String, PersistenceConfig>,
    default_addressable: Option<String>,
    default_cluster: Option<String>,
    dynamic_config: DynamicProviderConfig,
}

impl PersistenceConfigBuilder {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            default_addressable: None,
            default_cluster: None,
            dynamic_config: DynamicProviderConfig::default(),
        }
    }

    /// Configure health monitoring settings
    pub fn with_health_monitoring(mut self, config: HealthMonitorConfig) -> Self {
        self.dynamic_config.health_monitor = config;
        self
    }

    /// Configure failover settings
    pub fn with_failover(mut self, config: FailoverConfig) -> Self {
        self.dynamic_config.failover = config;
        self
    }

    /// Enable/disable performance monitoring
    pub fn with_performance_monitoring(
        mut self,
        enabled: bool,
        window_seconds: Option<u64>,
    ) -> Self {
        self.dynamic_config.enable_performance_monitoring = enabled;
        if let Some(window) = window_seconds {
            self.dynamic_config.performance_window = window;
        }
        self
    }

    /// Enable/disable auto-scaling
    pub fn with_auto_scaling(mut self, enabled: bool) -> Self {
        self.dynamic_config.enable_auto_scaling = enabled;
        self
    }

    /// Set complete dynamic configuration
    pub fn with_dynamic_config(mut self, config: DynamicProviderConfig) -> Self {
        self.dynamic_config = config;
        self
    }

    /// Add a memory provider configuration
    pub fn with_memory<S: Into<String>>(
        mut self,
        name: S,
        config: MemoryConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::Memory(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add an S3 provider configuration
    pub fn with_s3<S: Into<String>>(mut self, name: S, config: S3Config, is_default: bool) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::S3(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add an Azure provider configuration
    pub fn with_azure<S: Into<String>>(
        mut self,
        name: S,
        config: AzureConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::Azure(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add a Google Cloud provider configuration
    pub fn with_google_cloud<S: Into<String>>(
        mut self,
        name: S,
        config: GoogleCloudConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::GoogleCloud(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add a Digital Ocean Spaces provider configuration
    pub fn with_digitalocean_spaces<S: Into<String>>(
        mut self,
        name: S,
        config: DigitalOceanSpacesConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::DigitalOceanSpaces(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add an AWS S3 provider configuration with GPU optimizations
    pub fn with_aws_s3<S: Into<String>>(
        mut self,
        name: S,
        config: AWSS3Config,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::AWSS3(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add a Google Cloud Storage provider configuration
    pub fn with_gcp_storage<S: Into<String>>(
        mut self,
        name: S,
        config: GCPStorageConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::GCPStorage(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add an etcd provider configuration
    pub fn with_etcd<S: Into<String>>(
        mut self,
        name: S,
        config: EtcdConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::Etcd(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add a Redis provider configuration
    pub fn with_redis<S: Into<String>>(
        mut self,
        name: S,
        config: RedisConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::Redis(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add a composite provider configuration
    pub fn with_composite<S: Into<String>>(
        mut self,
        name: S,
        config: CompositeConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::Composite(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Add a RocksDB provider configuration
    pub fn with_rocksdb<S: Into<String>>(
        mut self,
        name: S,
        config: crate::persistence::rocksdb::RocksDbConfig,
        is_default: bool,
    ) -> Self {
        let name = name.into();
        self.configs
            .insert(name.clone(), PersistenceConfig::RocksDB(config));

        if is_default {
            self.default_addressable = Some(name.clone());
            self.default_cluster = Some(name);
        }

        self
    }

    /// Set different defaults for addressable and cluster providers
    pub fn with_defaults<S1, S2>(
        mut self,
        addressable_default: Option<S1>,
        cluster_default: Option<S2>,
    ) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        if let Some(name) = addressable_default {
            self.default_addressable = Some(name.into());
        }
        if let Some(name) = cluster_default {
            self.default_cluster = Some(name.into());
        }
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> OrbitResult<()> {
        if self.configs.is_empty() {
            return Err(OrbitError::configuration(
                "No persistence providers configured",
            ));
        }

        // Validate default references exist
        if let Some(ref default) = self.default_addressable {
            if !self.configs.contains_key(default) {
                return Err(OrbitError::configuration(format!(
                    "Default addressable provider '{}' not found",
                    default
                )));
            }
        }

        if let Some(ref default) = self.default_cluster {
            if !self.configs.contains_key(default) {
                return Err(OrbitError::configuration(format!(
                    "Default cluster provider '{}' not found",
                    default
                )));
            }
        }

        // Validate individual provider configurations
        for (name, config) in &self.configs {
            Self::validate_provider_config(name, config)?;
        }

        Ok(())
    }

    fn validate_provider_config(name: &str, config: &PersistenceConfig) -> OrbitResult<()> {
        match config {
            PersistenceConfig::Memory(config) => Self::validate_memory_config(name, config),
            PersistenceConfig::S3(config) => Self::validate_s3_config(name, config),
            PersistenceConfig::Azure(config) => Self::validate_azure_config(name, config),
            PersistenceConfig::GoogleCloud(config) => {
                Self::validate_google_cloud_config(name, config)
            }
            PersistenceConfig::DigitalOceanSpaces(config) => {
                Self::validate_do_spaces_config(name, config)
            }
            PersistenceConfig::Etcd(config) => Self::validate_etcd_config(name, config),
            PersistenceConfig::Redis(config) => Self::validate_redis_config(name, config),
            PersistenceConfig::Composite(config) => Self::validate_composite_config(name, config),
            PersistenceConfig::RocksDB(config) => Self::validate_rocksdb_config(name, config),
            _ => Ok(()), // Other providers would have their validations here
        }
    }

    /// Validate memory provider configuration
    fn validate_memory_config(name: &str, config: &MemoryConfig) -> OrbitResult<()> {
        if let Some(ref backup) = config.disk_backup {
            Self::validate_non_empty_path(&backup.path, name, "backup path")?;
            Self::validate_positive_value(backup.sync_interval, name, "sync interval")?;
        }
        Ok(())
    }

    /// Validate S3 provider configuration
    fn validate_s3_config(name: &str, config: &S3Config) -> OrbitResult<()> {
        Self::validate_non_empty_field(&config.endpoint, name, "endpoint")?;
        Self::validate_non_empty_field(&config.bucket, name, "bucket")?;
        Self::validate_s3_credentials(name, &config.access_key_id, &config.secret_access_key)?;
        Ok(())
    }

    /// Validate Azure provider configuration
    fn validate_azure_config(name: &str, config: &AzureConfig) -> OrbitResult<()> {
        Self::validate_non_empty_field(&config.account_name, name, "account name")?;
        Self::validate_non_empty_field(&config.container_name, name, "container name")?;
        Ok(())
    }

    /// Validate Google Cloud provider configuration
    fn validate_google_cloud_config(name: &str, config: &GoogleCloudConfig) -> OrbitResult<()> {
        Self::validate_non_empty_field(&config.project_id, name, "project ID")?;
        Self::validate_non_empty_field(&config.bucket_name, name, "bucket name")?;
        Ok(())
    }

    /// Validate Digital Ocean Spaces provider configuration
    fn validate_do_spaces_config(name: &str, config: &DigitalOceanSpacesConfig) -> OrbitResult<()> {
        Self::validate_non_empty_field(&config.endpoint, name, "endpoint")?;
        Self::validate_non_empty_field(&config.space_name, name, "space name")?;
        Self::validate_non_empty_field(&config.region, name, "region")?;
        Self::validate_s3_credentials(name, &config.access_key_id, &config.secret_access_key)?;
        Ok(())
    }

    /// Validate etcd provider configuration
    fn validate_etcd_config(name: &str, config: &EtcdConfig) -> OrbitResult<()> {
        if config.endpoints.is_empty() {
            return Self::create_config_error(name, "endpoints cannot be empty");
        }
        Self::validate_positive_value(config.lease_ttl, name, "lease TTL")?;
        Ok(())
    }

    /// Validate Redis provider configuration
    fn validate_redis_config(name: &str, config: &RedisConfig) -> OrbitResult<()> {
        Self::validate_non_empty_field(&config.url, name, "URL")?;
        Self::validate_positive_value(config.pool_size, name, "pool size")?;
        Ok(())
    }

    /// Validate composite provider configuration
    fn validate_composite_config(name: &str, config: &CompositeConfig) -> OrbitResult<()> {
        Self::validate_provider_config(&format!("{}_primary", name), &config.primary)?;
        if let Some(ref backup) = config.backup {
            Self::validate_provider_config(&format!("{}_backup", name), backup)?;
        }
        Ok(())
    }

    /// Validate RocksDB provider configuration
    fn validate_rocksdb_config(
        name: &str,
        config: &crate::persistence::rocksdb::RocksDbConfig,
    ) -> OrbitResult<()> {
        Self::validate_non_empty_field(&config.data_dir, name, "data directory")?;
        Self::validate_positive_value(config.max_background_jobs, name, "max background jobs")?;
        Self::validate_positive_value(config.write_buffer_size, name, "write buffer size")?;
        Self::validate_positive_value(
            config.max_write_buffer_number,
            name,
            "max write buffer number",
        )?;
        Self::validate_positive_value(config.target_file_size_base, name, "target file size base")?;
        Self::validate_positive_value(config.block_cache_size, name, "block cache size")?;
        Ok(())
    }

    /// Helper function to validate non-empty string fields
    fn validate_non_empty_field(
        value: &str,
        provider_name: &str,
        field_name: &str,
    ) -> OrbitResult<()> {
        if value.is_empty() {
            Self::create_config_error(provider_name, &format!("{} cannot be empty", field_name))
        } else {
            Ok(())
        }
    }

    /// Helper function to validate non-empty paths
    fn validate_non_empty_path(
        path: &str,
        provider_name: &str,
        field_name: &str,
    ) -> OrbitResult<()> {
        if path.is_empty() {
            Self::create_config_error(provider_name, &format!("{} cannot be empty", field_name))
        } else {
            Ok(())
        }
    }

    /// Helper function to validate positive numeric values
    fn validate_positive_value(
        value: impl PartialOrd + From<u8> + Copy,
        provider_name: &str,
        field_name: &str,
    ) -> OrbitResult<()> {
        if value <= 0.into() {
            Self::create_config_error(provider_name, &format!("{} must be > 0", field_name))
        } else {
            Ok(())
        }
    }

    /// Helper function to validate S3-style credentials
    fn validate_s3_credentials(
        provider_name: &str,
        access_key: &str,
        secret_key: &str,
    ) -> OrbitResult<()> {
        if access_key.is_empty() || secret_key.is_empty() {
            Self::create_config_error(provider_name, "access credentials cannot be empty")
        } else {
            Ok(())
        }
    }

    /// Helper function to create configuration errors
    fn create_config_error(provider_name: &str, message: &str) -> OrbitResult<()> {
        let provider_type = provider_name.split('_').next().unwrap_or("Unknown");
        let capitalized = format!(
            "{}{}",
            provider_type.chars().next().unwrap().to_uppercase(),
            &provider_type[1..]
        );

        Err(OrbitError::configuration(format!(
            "{} provider '{}': {}",
            capitalized, provider_name, message
        )))
    }

    /// Build the configuration
    pub fn build(self) -> OrbitResult<PersistenceProviderConfig> {
        self.validate()?;

        Ok(PersistenceProviderConfig {
            configs: self.configs,
            default_addressable: self.default_addressable,
            default_cluster: self.default_cluster,
            dynamic_config: self.dynamic_config,
        })
    }
}

impl Default for PersistenceConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Finalized persistence provider configuration
#[derive(Debug, Clone)]
pub struct PersistenceProviderConfig {
    configs: HashMap<String, PersistenceConfig>,
    default_addressable: Option<String>,
    default_cluster: Option<String>,
    dynamic_config: DynamicProviderConfig,
}

impl PersistenceProviderConfig {
    /// Create a new builder
    pub fn builder() -> PersistenceConfigBuilder {
        PersistenceConfigBuilder::new()
    }

    /// Create default memory-only configuration
    pub fn default_memory() -> Self {
        Self::builder()
            .with_memory("memory", MemoryConfig::default(), true)
            .build()
            .expect("Default memory configuration should be valid")
    }

    /// Load configuration from file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> OrbitResult<Self> {
        let path_ref = path.as_ref();
        let content = tokio::fs::read_to_string(path_ref)
            .await
            .map_err(|e| OrbitError::configuration(format!("Failed to read config file: {}", e)))?;

        if path_ref.extension().and_then(|s| s.to_str()) == Some("toml") {
            Self::from_toml(&content)
        } else {
            Self::from_json(&content)
        }
    }

    /// Load configuration from TOML string
    pub fn from_toml(content: &str) -> OrbitResult<Self> {
        let config: ConfigFile = toml::from_str(content).map_err(|e| {
            OrbitError::configuration(format!("Failed to parse TOML config: {}", e))
        })?;

        Self::from_config_file(config)
    }

    /// Load configuration from JSON string
    pub fn from_json(content: &str) -> OrbitResult<Self> {
        let config: ConfigFile = serde_json::from_str(content).map_err(|e| {
            OrbitError::configuration(format!("Failed to parse JSON config: {}", e))
        })?;

        Self::from_config_file(config)
    }

    fn from_config_file(config_file: ConfigFile) -> OrbitResult<Self> {
        let mut builder = Self::builder();

        for (name, provider_config) in config_file.providers {
            let _is_default = config_file.defaults.addressable.as_ref() == Some(&name)
                || config_file.defaults.cluster.as_ref() == Some(&name);

            builder.configs.insert(name, provider_config);
        }

        builder.default_addressable = config_file.defaults.addressable;
        builder.default_cluster = config_file.defaults.cluster;

        builder.build()
    }

    /// Create provider registry from this configuration
    pub async fn create_registry(&self) -> OrbitResult<PersistenceProviderRegistry> {
        let registry = PersistenceProviderRegistry::new();

        for (name, config) in &self.configs {
            self.register_providers(&registry, name.clone(), config)
                .await?;
        }

        Ok(registry)
    }

    async fn register_providers(
        &self,
        registry: &PersistenceProviderRegistry,
        name: String,
        config: &PersistenceConfig,
    ) -> OrbitResult<()> {
        let is_default_addressable = self.default_addressable.as_ref() == Some(&name);
        let is_default_cluster = self.default_cluster.as_ref() == Some(&name);

        match config {
            PersistenceConfig::Memory(config) => {
                let addressable_provider =
                    Arc::new(MemoryAddressableDirectoryProvider::new(config.clone()));
                let cluster_provider = Arc::new(MemoryClusterNodeProvider::new(config.clone()));

                addressable_provider.initialize().await?;
                cluster_provider.initialize().await?;

                registry
                    .register_addressable_provider(
                        format!("{}_addressable", name),
                        addressable_provider,
                        is_default_addressable,
                    )
                    .await?;

                registry
                    .register_cluster_provider(
                        format!("{}_cluster", name),
                        cluster_provider,
                        is_default_cluster,
                    )
                    .await?;
            }
            PersistenceConfig::S3(config) => {
                let addressable_provider =
                    Arc::new(S3AddressableDirectoryProvider::new(config.clone()));
                let cluster_provider = Arc::new(S3ClusterNodeProvider::new(config.clone()));

                addressable_provider.initialize().await?;
                cluster_provider.initialize().await?;

                registry
                    .register_addressable_provider(
                        format!("{}_addressable", name),
                        addressable_provider,
                        is_default_addressable,
                    )
                    .await?;

                registry
                    .register_cluster_provider(
                        format!("{}_cluster", name),
                        cluster_provider,
                        is_default_cluster,
                    )
                    .await?;
            }
            PersistenceConfig::DigitalOceanSpaces(config) => {
                let addressable_provider = Arc::new(
                    crate::persistence::cloud::DigitalOceanSpacesAddressableDirectoryProvider::new(
                        config.clone(),
                    ),
                );
                let cluster_provider = Arc::new(
                    crate::persistence::cloud::DigitalOceanSpacesClusterNodeProvider::new(
                        config.clone(),
                    ),
                );

                addressable_provider.initialize().await?;
                cluster_provider.initialize().await?;

                registry
                    .register_addressable_provider(
                        format!("{}_addressable", name),
                        addressable_provider,
                        is_default_addressable,
                    )
                    .await?;

                registry
                    .register_cluster_provider(
                        format!("{}_cluster", name),
                        cluster_provider,
                        is_default_cluster,
                    )
                    .await?;
            }
            PersistenceConfig::RocksDB(config) => {
                let addressable_provider = Arc::new(
                    crate::persistence::rocksdb::RocksDbAddressableProvider::new(config.clone())?,
                );
                let cluster_provider = Arc::new(
                    crate::persistence::rocksdb::RocksDbClusterProvider::new(config.clone())?,
                );

                addressable_provider.initialize().await?;
                cluster_provider.initialize().await?;

                registry
                    .register_addressable_provider(
                        format!("{}_addressable", name),
                        addressable_provider,
                        is_default_addressable,
                    )
                    .await?;

                registry
                    .register_cluster_provider(
                        format!("{}_cluster", name),
                        cluster_provider,
                        is_default_cluster,
                    )
                    .await?;
            }
            // Add other provider types here...
            _ => {
                return Err(OrbitError::configuration(format!(
                    "Provider type not yet implemented: {:?}",
                    config
                )));
            }
        }

        Ok(())
    }

    /// Get all configured provider names
    pub fn provider_names(&self) -> Vec<&String> {
        self.configs.keys().collect()
    }

    /// Get default provider names
    pub fn defaults(&self) -> (Option<&String>, Option<&String>) {
        (
            self.default_addressable.as_ref(),
            self.default_cluster.as_ref(),
        )
    }

    /// Get dynamic configuration
    pub fn dynamic_config(&self) -> &DynamicProviderConfig {
        &self.dynamic_config
    }

    /// Create dynamic provider manager from this configuration
    pub async fn create_dynamic_manager(
        &self,
    ) -> OrbitResult<crate::persistence::dynamic::DynamicProviderManager> {
        let manager =
            crate::persistence::dynamic::DynamicProviderManager::new(self.dynamic_config.clone());

        // Register all configured providers with the dynamic manager
        for (name, config) in &self.configs {
            let _is_primary = self.default_addressable.as_ref() == Some(name)
                || self.default_cluster.as_ref() == Some(name);

            // Create provider based on config type
            // TODO: This needs to be implemented based on the actual provider interfaces
            // For now, this is a placeholder showing the structure
            match config {
                PersistenceConfig::Memory(_config) => {
                    // Create memory provider and register
                    // let provider = Arc::new(MemoryProvider::new(config.clone()));
                    // manager.register_provider(name.clone(), provider, is_primary).await?;
                }
                PersistenceConfig::S3(_config) => {
                    // Create S3 provider and register
                    // let provider = Arc::new(S3Provider::new(config.clone()));
                    // manager.register_provider(name.clone(), provider, is_primary).await?;
                }
                _ => {
                    // Handle other provider types
                }
            }
        }

        // Start health monitoring if enabled
        if self.dynamic_config.enable_performance_monitoring {
            manager.start_health_monitoring().await?;
        }

        Ok(manager)
    }
}

/// Configuration file structure for serialization/deserialization
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    pub providers: HashMap<String, PersistenceConfig>,
    pub defaults: DefaultProviders,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DefaultProviders {
    pub addressable: Option<String>,
    pub cluster: Option<String>,
}

/// Configuration validation error details
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub provider_name: String,
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Provider '{}' field '{}': {}",
            self.provider_name, self.field, self.message
        )
    }
}

/// Environment variable configuration helper
pub struct EnvironmentConfig;

impl EnvironmentConfig {
    /// Create S3 configuration from environment variables
    pub fn s3_from_env(prefix: &str) -> OrbitResult<S3Config> {
        let get_env = |key: &str| -> OrbitResult<String> {
            std::env::var(format!("{}_{}", prefix, key)).map_err(|_| {
                OrbitError::configuration(format!(
                    "Missing environment variable: {}_{}",
                    prefix, key
                ))
            })
        };

        let get_env_opt =
            |key: &str| -> Option<String> { std::env::var(format!("{}_{}", prefix, key)).ok() };

        Ok(S3Config {
            endpoint: get_env("ENDPOINT")?,
            region: get_env("REGION")?,
            bucket: get_env("BUCKET")?,
            access_key_id: get_env("ACCESS_KEY_ID")?,
            secret_access_key: get_env("SECRET_ACCESS_KEY")?,
            prefix: get_env_opt("PREFIX"),
            enable_ssl: get_env_opt("ENABLE_SSL")
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            connection_timeout: get_env_opt("CONNECTION_TIMEOUT").and_then(|s| s.parse().ok()),
            read_timeout: get_env_opt("READ_TIMEOUT").and_then(|s| s.parse().ok()),
            write_timeout: get_env_opt("WRITE_TIMEOUT").and_then(|s| s.parse().ok()),
            retry_count: get_env_opt("RETRY_COUNT")
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
        })
    }

    /// Create Azure configuration from environment variables
    pub fn azure_from_env(prefix: &str) -> OrbitResult<AzureConfig> {
        let get_env = |key: &str| -> OrbitResult<String> {
            std::env::var(format!("{}_{}", prefix, key)).map_err(|_| {
                OrbitError::configuration(format!(
                    "Missing environment variable: {}_{}",
                    prefix, key
                ))
            })
        };

        let get_env_opt =
            |key: &str| -> Option<String> { std::env::var(format!("{}_{}", prefix, key)).ok() };

        Ok(AzureConfig {
            account_name: get_env("ACCOUNT_NAME")?,
            account_key: get_env("ACCOUNT_KEY")?,
            container_name: get_env("CONTAINER_NAME")?,
            endpoint: get_env_opt("ENDPOINT"),
            prefix: get_env_opt("PREFIX"),
            connection_timeout: get_env_opt("CONNECTION_TIMEOUT").and_then(|s| s.parse().ok()),
            retry_count: get_env_opt("RETRY_COUNT")
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
        })
    }

    /// Create etcd configuration from environment variables
    pub fn etcd_from_env(prefix: &str) -> OrbitResult<EtcdConfig> {
        let get_env = |key: &str| -> OrbitResult<String> {
            std::env::var(format!("{}_{}", prefix, key)).map_err(|_| {
                OrbitError::configuration(format!(
                    "Missing environment variable: {}_{}",
                    prefix, key
                ))
            })
        };

        let get_env_opt =
            |key: &str| -> Option<String> { std::env::var(format!("{}_{}", prefix, key)).ok() };

        let endpoints_str = get_env("ENDPOINTS")?;
        let endpoints: Vec<String> = endpoints_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        Ok(EtcdConfig {
            endpoints,
            username: get_env_opt("USERNAME"),
            password: get_env_opt("PASSWORD"),
            ca_cert: get_env_opt("CA_CERT"),
            client_cert: get_env_opt("CLIENT_CERT"),
            client_key: get_env_opt("CLIENT_KEY"),
            prefix: get_env_opt("PREFIX").unwrap_or_else(|| "orbit".to_string()),
            connection_timeout: get_env_opt("CONNECTION_TIMEOUT").and_then(|s| s.parse().ok()),
            lease_ttl: get_env_opt("LEASE_TTL")
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
        })
    }

    /// Create Digital Ocean Spaces configuration from environment variables
    pub fn digitalocean_spaces_from_env(prefix: &str) -> OrbitResult<DigitalOceanSpacesConfig> {
        let get_env = |key: &str| -> OrbitResult<String> {
            std::env::var(format!("{}_{}", prefix, key)).map_err(|_| {
                OrbitError::configuration(format!(
                    "Missing environment variable: {}_{}",
                    prefix, key
                ))
            })
        };

        let get_env_opt =
            |key: &str| -> Option<String> { std::env::var(format!("{}_{}", prefix, key)).ok() };

        // Parse tags from environment variable format: "key1=value1,key2=value2"
        let mut tags = std::collections::HashMap::new();
        if let Some(tags_str) = get_env_opt("TAGS") {
            for pair in tags_str.split(',') {
                if let Some((key, value)) = pair.split_once('=') {
                    tags.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }

        Ok(DigitalOceanSpacesConfig {
            endpoint: get_env("ENDPOINT")?,
            region: get_env("REGION")?,
            space_name: get_env("SPACE_NAME")?,
            access_key_id: get_env("ACCESS_KEY_ID")?,
            secret_access_key: get_env("SECRET_ACCESS_KEY")?,
            prefix: get_env_opt("PREFIX"),
            enable_ssl: get_env_opt("ENABLE_SSL")
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            enable_cdn: get_env_opt("ENABLE_CDN")
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            cdn_endpoint: get_env_opt("CDN_ENDPOINT"),
            connection_timeout: get_env_opt("CONNECTION_TIMEOUT").and_then(|s| s.parse().ok()),
            read_timeout: get_env_opt("READ_TIMEOUT").and_then(|s| s.parse().ok()),
            write_timeout: get_env_opt("WRITE_TIMEOUT").and_then(|s| s.parse().ok()),
            retry_count: get_env_opt("RETRY_COUNT")
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            enable_encryption: get_env_opt("ENABLE_ENCRYPTION")
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            tags,
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = PersistenceProviderConfig::builder()
            .with_memory("memory", MemoryConfig::default(), true)
            .build()
            .unwrap();

        let (addressable_default, cluster_default) = config.defaults();
        assert_eq!(addressable_default, Some(&"memory".to_string()));
        assert_eq!(cluster_default, Some(&"memory".to_string()));
    }

    #[test]
    fn test_config_validation() {
        // Test missing bucket in S3 config
        let result = PersistenceProviderConfig::builder()
            .with_s3(
                "s3",
                S3Config {
                    endpoint: "http://localhost:9000".to_string(),
                    region: "us-east-1".to_string(),
                    bucket: "".to_string(), // Empty bucket should fail validation
                    access_key_id: "access".to_string(),
                    secret_access_key: "secret".to_string(),
                    prefix: None,
                    enable_ssl: false,
                    connection_timeout: None,
                    read_timeout: None,
                    write_timeout: None,
                    retry_count: 3,
                },
                true,
            )
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_default_memory_config() {
        let config = PersistenceProviderConfig::default_memory();
        assert_eq!(config.provider_names().len(), 1);
        assert!(config.provider_names().contains(&&"memory".to_string()));
    }

}
