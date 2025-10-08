use crate::{PrometheusError, PrometheusResult};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

/// Prometheus exporter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusConfig {
    /// Server configuration
    pub server: ServerConfig,

    /// Metrics collection configuration
    pub metrics: MetricsConfig,

    /// Export configuration
    pub export: ExportConfig,

    /// Dashboard configuration
    pub dashboard: DashboardConfig,

    /// Security configuration
    pub security: SecurityConfig,
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            metrics: MetricsConfig::default(),
            export: ExportConfig::default(),
            dashboard: DashboardConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host to bind to
    pub host: String,

    /// Server port to bind to
    pub port: u16,

    /// Server shutdown timeout
    pub shutdown_timeout: Duration,

    /// Maximum concurrent connections
    pub max_connections: Option<usize>,

    /// Request timeout
    pub request_timeout: Duration,

    /// Keep alive timeout
    pub keep_alive_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: env::var("PROMETHEUS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PROMETHEUS_PORT")
                .unwrap_or_else(|_| "9090".to_string())
                .parse()
                .unwrap_or(9090),
            shutdown_timeout: Duration::from_secs(30),
            max_connections: Some(1000),
            request_timeout: Duration::from_secs(30),
            keep_alive_timeout: Duration::from_secs(60),
        }
    }
}

/// Metrics collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Collection interval
    pub collection_interval: Duration,

    /// Enable system metrics collection
    pub collect_system_metrics: bool,

    /// Enable application metrics collection
    pub collect_app_metrics: bool,

    /// Enable custom metrics collection
    pub collect_custom_metrics: bool,

    /// Metrics retention period
    pub retention_period: Duration,

    /// Maximum number of metrics to store
    pub max_metrics: usize,

    /// Histogram buckets
    pub histogram_buckets: Vec<f64>,

    /// Default labels to add to all metrics
    pub default_labels: std::collections::HashMap<String, String>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        let mut default_labels = std::collections::HashMap::new();
        default_labels.insert("service".to_string(), "orbit-server".to_string());
        default_labels.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());

        Self {
            collection_interval: Duration::from_secs(15),
            collect_system_metrics: true,
            collect_app_metrics: true,
            collect_custom_metrics: true,
            retention_period: Duration::from_secs(3600), // 1 hour
            max_metrics: 100000,
            histogram_buckets: vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
            default_labels,
        }
    }
}

/// Export configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Metrics endpoint path
    pub metrics_path: String,

    /// Health check endpoint path
    pub health_path: String,

    /// Dashboard endpoint path
    pub dashboard_path: String,

    /// Enable GZIP compression
    pub enable_compression: bool,

    /// Response format
    pub format: ExportFormat,

    /// Include metadata in exports
    pub include_metadata: bool,

    /// Export timeout
    pub export_timeout: Duration,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            metrics_path: "/metrics".to_string(),
            health_path: "/health".to_string(),
            dashboard_path: "/dashboard".to_string(),
            enable_compression: true,
            format: ExportFormat::Prometheus,
            include_metadata: true,
            export_timeout: Duration::from_secs(10),
        }
    }
}

/// Export format options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    /// Prometheus text format
    Prometheus,
    /// JSON format
    Json,
    /// OpenMetrics format
    OpenMetrics,
}

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    /// Enable dashboard generation
    pub enabled: bool,

    /// Dashboard template directory
    pub template_dir: Option<String>,

    /// Dashboard output directory
    pub output_dir: Option<String>,

    /// Auto-refresh interval (in seconds)
    pub refresh_interval: u32,

    /// Dashboard title
    pub title: String,

    /// Dashboard description
    pub description: String,

    /// Enable dark theme
    pub dark_theme: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: env::var("PROMETHEUS_DASHBOARD_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            template_dir: env::var("PROMETHEUS_DASHBOARD_TEMPLATES").ok(),
            output_dir: env::var("PROMETHEUS_DASHBOARD_OUTPUT").ok(),
            refresh_interval: 30,
            title: "Orbit Metrics Dashboard".to_string(),
            description: "Comprehensive metrics dashboard for Orbit services".to_string(),
            dark_theme: true,
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable authentication
    pub enable_auth: bool,

    /// API key for authentication
    pub api_key: Option<String>,

    /// Enable TLS
    pub enable_tls: bool,

    /// TLS certificate file path
    pub cert_file: Option<String>,

    /// TLS private key file path
    pub key_file: Option<String>,

    /// Allowed IP addresses/CIDR blocks
    pub allowed_ips: Vec<String>,

    /// Enable CORS
    pub enable_cors: bool,

    /// CORS allowed origins
    pub cors_origins: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_auth: env::var("PROMETHEUS_AUTH_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            api_key: env::var("PROMETHEUS_API_KEY").ok(),
            enable_tls: env::var("PROMETHEUS_TLS_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            cert_file: env::var("PROMETHEUS_TLS_CERT").ok(),
            key_file: env::var("PROMETHEUS_TLS_KEY").ok(),
            allowed_ips: vec!["0.0.0.0/0".to_string()], // Allow all by default
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
        }
    }
}

impl PrometheusConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> PrometheusResult<Self> {
        let config = Self::default();
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from file
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> PrometheusResult<Self> {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            PrometheusError::configuration(format!("Failed to read config file: {}", e))
        })?;

        let config: Self = if path.as_ref().extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::from_str(&content).map_err(|e| {
                PrometheusError::configuration(format!("Failed to parse TOML config: {}", e))
            })?
        } else if path.as_ref().extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.as_ref().extension().and_then(|s| s.to_str()) == Some("yml")
        {
            serde_yaml::from_str(&content).map_err(|e| {
                PrometheusError::configuration(format!("Failed to parse YAML config: {}", e))
            })?
        } else {
            serde_json::from_str(&content).map_err(|e| {
                PrometheusError::configuration(format!("Failed to parse JSON config: {}", e))
            })?
        };

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> PrometheusResult<()> {
        // Validate server configuration
        if self.server.port == 0 {
            return Err(PrometheusError::configuration(
                "Server port must be greater than 0",
            ));
        }

        if self.server.port > 65535 {
            return Err(PrometheusError::configuration(
                "Server port must be less than 65536",
            ));
        }

        if self.server.host.is_empty() {
            return Err(PrometheusError::configuration(
                "Server host cannot be empty",
            ));
        }

        // Validate metrics configuration
        if self.metrics.max_metrics == 0 {
            return Err(PrometheusError::configuration(
                "Maximum metrics count must be greater than 0",
            ));
        }

        if self.metrics.histogram_buckets.is_empty() {
            return Err(PrometheusError::configuration(
                "Histogram buckets cannot be empty",
            ));
        }

        // Validate export configuration
        if self.export.metrics_path.is_empty() {
            return Err(PrometheusError::configuration(
                "Metrics path cannot be empty",
            ));
        }

        if !self.export.metrics_path.starts_with('/') {
            return Err(PrometheusError::configuration(
                "Metrics path must start with '/'",
            ));
        }

        // Validate security configuration
        if self.security.enable_auth && self.security.api_key.is_none() {
            return Err(PrometheusError::configuration(
                "API key required when authentication is enabled",
            ));
        }

        if self.security.enable_tls {
            if self.security.cert_file.is_none() {
                return Err(PrometheusError::configuration(
                    "Certificate file required when TLS is enabled",
                ));
            }

            if self.security.key_file.is_none() {
                return Err(PrometheusError::configuration(
                    "Private key file required when TLS is enabled",
                ));
            }
        }

        Ok(())
    }

    /// Get server bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    /// Get metrics URL
    pub fn metrics_url(&self) -> String {
        format!("http://{}{}", self.bind_address(), self.export.metrics_path)
    }

    /// Get health check URL
    pub fn health_url(&self) -> String {
        format!("http://{}{}", self.bind_address(), self.export.health_path)
    }
}

/// Configuration builder for programmatic configuration
#[derive(Debug, Default)]
pub struct PrometheusConfigBuilder {
    config: PrometheusConfig,
}

impl PrometheusConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.config.server.host = host.into();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.config.server.port = port;
        self
    }

    pub fn metrics_path(mut self, path: impl Into<String>) -> Self {
        self.config.export.metrics_path = path.into();
        self
    }

    pub fn collection_interval(mut self, interval: Duration) -> Self {
        self.config.metrics.collection_interval = interval;
        self
    }

    pub fn enable_auth(mut self, enabled: bool) -> Self {
        self.config.security.enable_auth = enabled;
        self
    }

    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.config.security.api_key = Some(key.into());
        self
    }

    pub fn enable_dashboard(mut self, enabled: bool) -> Self {
        self.config.dashboard.enabled = enabled;
        self
    }

    pub fn build(self) -> PrometheusResult<PrometheusConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}
