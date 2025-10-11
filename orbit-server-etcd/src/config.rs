use crate::{EtcdError, EtcdResult};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

/// etcd client configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EtcdConfig {
    /// Connection configuration
    pub connection: ConnectionConfig,

    /// Service discovery configuration
    pub discovery: DiscoveryConfig,

    /// Leader election configuration
    pub election: ElectionConfig,

    /// Health check configuration
    pub health: HealthConfig,

    /// Security configuration
    pub security: SecurityConfig,
}

/// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// etcd server endpoints
    pub endpoints: Vec<String>,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Request timeout
    pub timeout: Duration,

    /// Keep alive settings
    pub keep_alive: KeepAliveConfig,

    /// Retry configuration
    pub retry: RetryConfig,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        let endpoints = env::var("ETCD_ENDPOINTS")
            .unwrap_or_else(|_| "http://127.0.0.1:2379".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        Self {
            endpoints,
            connect_timeout: Duration::from_secs(10),
            timeout: Duration::from_secs(30),
            keep_alive: KeepAliveConfig::default(),
            retry: RetryConfig::default(),
        }
    }
}

/// Keep alive configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepAliveConfig {
    /// Enable keep alive
    pub enabled: bool,

    /// Keep alive interval
    pub interval: Duration,

    /// Keep alive timeout
    pub timeout: Duration,
}

impl Default for KeepAliveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(10),
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,

    /// Initial retry delay
    pub initial_delay: Duration,

    /// Maximum retry delay
    pub max_delay: Duration,

    /// Retry multiplier
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

/// Service discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Service registration prefix
    pub service_prefix: String,

    /// Service TTL (lease duration)
    pub service_ttl: Duration,

    /// Registration refresh interval
    pub refresh_interval: Duration,

    /// Enable automatic service deregistration
    pub auto_deregister: bool,

    /// Service metadata
    pub metadata: ServiceMetadata,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            service_prefix: "/orbit/services".to_string(),
            service_ttl: Duration::from_secs(30),
            refresh_interval: Duration::from_secs(10),
            auto_deregister: true,
            metadata: ServiceMetadata::default(),
        }
    }
}

/// Service metadata configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetadata {
    /// Include hostname in metadata
    pub include_hostname: bool,

    /// Include process ID in metadata
    pub include_pid: bool,

    /// Include version information
    pub include_version: bool,

    /// Custom tags
    pub tags: Vec<String>,

    /// Custom attributes
    pub attributes: std::collections::HashMap<String, String>,
}

impl Default for ServiceMetadata {
    fn default() -> Self {
        Self {
            include_hostname: true,
            include_pid: true,
            include_version: true,
            tags: vec!["orbit".to_string()],
            attributes: std::collections::HashMap::new(),
        }
    }
}

/// Leader election configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionConfig {
    /// Election prefix for keys
    pub election_prefix: String,

    /// Election session TTL
    pub session_ttl: Duration,

    /// Election timeout
    pub timeout: Duration,

    /// Campaign timeout
    pub campaign_timeout: Duration,

    /// Enable automatic leadership renewal
    pub auto_renew: bool,
}

impl Default for ElectionConfig {
    fn default() -> Self {
        Self {
            election_prefix: "/orbit/elections".to_string(),
            session_ttl: Duration::from_secs(60),
            timeout: Duration::from_secs(30),
            campaign_timeout: Duration::from_secs(10),
            auto_renew: true,
        }
    }
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Enable health checking
    pub enabled: bool,

    /// Health check interval
    pub check_interval: Duration,

    /// Health check timeout
    pub timeout: Duration,

    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,

    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,

    /// Health check endpoint path
    pub endpoint: Option<String>,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            failure_threshold: 3,
            success_threshold: 1,
            endpoint: Some("/health".to_string()),
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityConfig {
    /// Enable TLS
    pub tls_enabled: bool,

    /// TLS configuration
    pub tls: TlsConfig,

    /// Authentication configuration
    pub auth: AuthConfig,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// CA certificate file path
    pub ca_file: Option<String>,

    /// Client certificate file path
    pub cert_file: Option<String>,

    /// Client private key file path
    pub key_file: Option<String>,

    /// Skip certificate verification (insecure)
    pub insecure_skip_verify: bool,

    /// Server name for certificate verification
    pub server_name: Option<String>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            ca_file: env::var("ETCD_TLS_CA_FILE").ok(),
            cert_file: env::var("ETCD_TLS_CERT_FILE").ok(),
            key_file: env::var("ETCD_TLS_KEY_FILE").ok(),
            insecure_skip_verify: false,
            server_name: None,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication
    pub enabled: bool,

    /// Username
    pub username: Option<String>,

    /// Password
    pub password: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            username: env::var("ETCD_USERNAME").ok(),
            password: env::var("ETCD_PASSWORD").ok(),
        }
    }
}

impl EtcdConfig {
    /// Load configuration from environment variables
    #[allow(clippy::result_large_err)]
    pub fn from_env() -> EtcdResult<Self> {
        let config = Self::default();
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from file
    #[allow(clippy::result_large_err)]
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> EtcdResult<Self> {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| EtcdError::configuration(format!("Failed to read config file: {}", e)))?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| EtcdError::configuration(format!("Failed to parse JSON config: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    #[allow(clippy::result_large_err)]
    pub fn validate(&self) -> EtcdResult<()> {
        // Validate connection configuration
        if self.connection.endpoints.is_empty() {
            return Err(EtcdError::configuration(
                "At least one etcd endpoint must be specified",
            ));
        }

        for endpoint in &self.connection.endpoints {
            if endpoint.is_empty() {
                return Err(EtcdError::configuration("etcd endpoint cannot be empty"));
            }
        }

        // Validate discovery configuration
        if self.discovery.service_prefix.is_empty() {
            return Err(EtcdError::configuration("Service prefix cannot be empty"));
        }

        if !self.discovery.service_prefix.starts_with('/') {
            return Err(EtcdError::configuration(
                "Service prefix must start with '/'",
            ));
        }

        // Validate election configuration
        if self.election.election_prefix.is_empty() {
            return Err(EtcdError::configuration("Election prefix cannot be empty"));
        }

        if !self.election.election_prefix.starts_with('/') {
            return Err(EtcdError::configuration(
                "Election prefix must start with '/'",
            ));
        }

        // Validate health configuration
        if self.health.enabled {
            if self.health.failure_threshold == 0 {
                return Err(EtcdError::configuration(
                    "Health failure threshold must be greater than 0",
                ));
            }

            if self.health.success_threshold == 0 {
                return Err(EtcdError::configuration(
                    "Health success threshold must be greater than 0",
                ));
            }
        }

        // Validate security configuration
        if self.security.auth.enabled
            && (self.security.auth.username.is_none() || self.security.auth.password.is_none())
        {
            return Err(EtcdError::configuration(
                "Username and password required when auth is enabled",
            ));
        }

        if self.security.tls_enabled {
            if self.security.tls.cert_file.is_some() && self.security.tls.key_file.is_none() {
                return Err(EtcdError::configuration(
                    "Key file required when cert file is specified",
                ));
            }

            if self.security.tls.key_file.is_some() && self.security.tls.cert_file.is_none() {
                return Err(EtcdError::configuration(
                    "Cert file required when key file is specified",
                ));
            }
        }

        Ok(())
    }

    /// Get the primary endpoint
    pub fn primary_endpoint(&self) -> &String {
        &self.connection.endpoints[0]
    }

    /// Check if TLS is enabled
    pub fn is_tls_enabled(&self) -> bool {
        self.security.tls_enabled
    }

    /// Check if authentication is enabled
    pub fn is_auth_enabled(&self) -> bool {
        self.security.auth.enabled
    }
}

/// Configuration builder for programmatic configuration
#[derive(Debug, Default)]
pub struct EtcdConfigBuilder {
    config: EtcdConfig,
}

impl EtcdConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn endpoints(mut self, endpoints: Vec<String>) -> Self {
        self.config.connection.endpoints = endpoints;
        self
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.connection.endpoints = vec![endpoint.into()];
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.connection.timeout = timeout;
        self
    }

    pub fn service_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.discovery.service_prefix = prefix.into();
        self
    }

    pub fn service_ttl(mut self, ttl: Duration) -> Self {
        self.config.discovery.service_ttl = ttl;
        self
    }

    pub fn election_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.election.election_prefix = prefix.into();
        self
    }

    pub fn enable_tls(mut self, enabled: bool) -> Self {
        self.config.security.tls_enabled = enabled;
        self
    }

    pub fn tls_files(
        mut self,
        ca_file: Option<String>,
        cert_file: Option<String>,
        key_file: Option<String>,
    ) -> Self {
        self.config.security.tls.ca_file = ca_file;
        self.config.security.tls.cert_file = cert_file;
        self.config.security.tls.key_file = key_file;
        self
    }

    pub fn auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.config.security.auth.enabled = true;
        self.config.security.auth.username = Some(username.into());
        self.config.security.auth.password = Some(password.into());
        self
    }

    #[allow(clippy::result_large_err)]
    pub fn build(self) -> EtcdResult<EtcdConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}
