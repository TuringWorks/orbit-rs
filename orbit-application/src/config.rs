use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Application name
    pub name: String,

    /// Application version
    pub version: String,

    /// Application description
    pub description: Option<String>,

    /// Environment (dev, staging, prod, etc.)
    pub environment: String,

    /// Server configuration
    pub server: ServerConfig,

    /// Health check configuration
    pub health: HealthConfig,

    /// Telemetry configuration
    pub telemetry: TelemetryConfig,

    /// Plugin configurations
    pub plugins: Vec<PluginConfig>,

    /// Custom application-specific settings
    pub settings: HashMap<String, serde_json::Value>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to bind to
    pub port: u16,

    /// Maximum number of connections
    pub max_connections: Option<usize>,

    /// Request timeout in seconds
    pub timeout_secs: Option<u64>,

    /// TLS configuration
    pub tls: Option<TlsConfig>,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: PathBuf,

    /// Path to private key file
    pub key_path: PathBuf,

    /// Path to CA bundle (optional)
    pub ca_path: Option<PathBuf>,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Enable health checks
    pub enabled: bool,

    /// Health check endpoint path
    pub endpoint: String,

    /// Check interval in seconds
    pub interval_secs: u64,

    /// Timeout for health checks in seconds
    pub timeout_secs: u64,

    /// Number of failures before marking unhealthy
    pub failure_threshold: u32,
}

/// Telemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Enable telemetry
    pub enabled: bool,

    /// Metrics configuration
    pub metrics: MetricsConfig,

    /// Tracing configuration
    pub tracing: TracingConfig,

    /// Custom telemetry settings
    pub settings: HashMap<String, serde_json::Value>,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// Metrics endpoint path
    pub endpoint: String,

    /// Collection interval in seconds
    pub interval_secs: u64,

    /// Metrics format (prometheus, json, etc.)
    pub format: String,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing
    pub enabled: bool,

    /// Log level filter
    pub level: String,

    /// Output format (json, pretty, compact)
    pub format: String,

    /// Output destination (stdout, file, etc.)
    pub output: String,

    /// File path for file output
    pub file_path: Option<PathBuf>,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin name
    pub name: String,

    /// Plugin version constraint
    pub version: Option<String>,

    /// Plugin library path or identifier
    pub path: String,

    /// Plugin-specific configuration
    pub config: HashMap<String, serde_json::Value>,

    /// Whether the plugin is enabled
    pub enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "orbit-app".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            environment: "development".to_string(),
            server: ServerConfig::default(),
            health: HealthConfig::default(),
            telemetry: TelemetryConfig::default(),
            plugins: Vec::new(),
            settings: HashMap::new(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: Some(1000),
            timeout_secs: Some(30),
            tls: None,
        }
    }
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "/health".to_string(),
            interval_secs: 30,
            timeout_secs: 5,
            failure_threshold: 3,
        }
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            settings: HashMap::new(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "/metrics".to_string(),
            interval_secs: 15,
            format: "prometheus".to_string(),
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: "info".to_string(),
            format: "json".to_string(),
            output: "stdout".to_string(),
            file_path: None,
        }
    }
}

impl AppConfig {
    /// Load configuration from environment variables and config files
    pub fn from_env() -> Result<Self, crate::AppError> {
        let mut config_builder = config::Config::builder();

        // Start with default configuration
        config_builder =
            config_builder.add_source(config::Config::try_from(&AppConfig::default())?);

        // Add config file if it exists
        if let Ok(config_path) = std::env::var("ORBIT_CONFIG_PATH") {
            config_builder =
                config_builder.add_source(config::File::with_name(&config_path).required(false));
        } else {
            // Look for config files in common locations
            for path in &["config", "config/app", "orbit.toml", "orbit.yaml"] {
                config_builder =
                    config_builder.add_source(config::File::with_name(path).required(false));
            }
        }

        // Override with environment variables
        config_builder = config_builder.add_source(
            config::Environment::with_prefix("ORBIT")
                .separator("_")
                .keep_prefix(false),
        );

        let config = config_builder.build()?;
        let app_config: AppConfig = config.try_deserialize()?;

        Ok(app_config)
    }

    /// Load configuration from a specific file path
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, crate::AppError> {
        let config = config::Config::builder()
            .add_source(config::File::from(path.as_ref()))
            .build()?;

        Ok(config.try_deserialize()?)
    }

    /// Save configuration to a file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), crate::AppError> {
        let content = if path.as_ref().extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.as_ref().extension().and_then(|s| s.to_str()) == Some("yml")
        {
            serde_yaml::to_string(self)?
        } else {
            serde_json::to_string_pretty(self)?
        };

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), crate::AppError> {
        if self.name.is_empty() {
            return Err(crate::AppError::Config(
                "Application name cannot be empty".to_string(),
            ));
        }

        if self.server.port == 0 {
            return Err(crate::AppError::Config(
                "Server port must be greater than 0".to_string(),
            ));
        }

        if self.health.enabled && self.health.endpoint.is_empty() {
            return Err(crate::AppError::Config(
                "Health endpoint cannot be empty when health checks are enabled".to_string(),
            ));
        }

        // Validate plugin configurations
        for plugin in &self.plugins {
            if plugin.enabled && plugin.name.is_empty() {
                return Err(crate::AppError::Config(
                    "Plugin name cannot be empty".to_string(),
                ));
            }

            if plugin.enabled && plugin.path.is_empty() {
                return Err(crate::AppError::Config(
                    "Plugin path cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Get a custom setting
    pub fn get_setting<T>(&self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.settings
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set a custom setting
    pub fn set_setting<T>(&mut self, key: String, value: T) -> Result<(), crate::AppError>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        self.settings.insert(key, json_value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.name, "orbit-app");
        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.environment, "development");
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert!(config.health.enabled);
        assert!(config.telemetry.enabled);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        assert!(config.validate().is_ok());

        config.name = String::new();
        assert!(config.validate().is_err());

        config = AppConfig::default();
        config.server.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_custom_settings() {
        let mut config = AppConfig::default();
        config
            .set_setting("test_key".to_string(), "test_value".to_string())
            .unwrap();

        let value: Option<String> = config.get_setting("test_key");
        assert_eq!(value, Some("test_value".to_string()));
    }
}
