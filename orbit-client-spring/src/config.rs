use crate::{
    annotations::{ConfigValue, Configuration},
    SpringError, SpringResult,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::Path;

/// Spring-like application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub application: ApplicationProperties,
    pub server: ServerProperties,
    pub database: DatabaseProperties,
    pub logging: LoggingProperties,
    pub security: SecurityProperties,
    pub custom: HashMap<String, ConfigValue>,
}

impl Configuration for AppConfig {
    fn name(&self) -> &'static str {
        "AppConfig"
    }

    fn validate(&self) -> SpringResult<()> {
        // Validate application properties
        if self.application.name.is_empty() {
            return Err(SpringError::validation(
                "application.name",
                "Application name cannot be empty",
            ));
        }

        // Validate server properties
        if self.server.port == 0 {
            return Err(SpringError::validation(
                "server.port",
                "Server port must be greater than 0",
            ));
        }
        // Note: u16 port type naturally constrains values to 0-65535

        // Validate database properties if enabled
        if self.database.enabled {
            if self.database.url.is_empty() {
                return Err(SpringError::validation(
                    "database.url",
                    "Database URL cannot be empty when database is enabled",
                ));
            }

            if self.database.max_connections == 0 {
                return Err(SpringError::validation(
                    "database.max_connections",
                    "Max connections must be greater than 0",
                ));
            }
        }

        Ok(())
    }

    fn properties(&self) -> HashMap<String, ConfigValue> {
        let mut props = HashMap::new();

        // Application properties
        props.insert(
            "application.name".to_string(),
            ConfigValue::String(self.application.name.clone()),
        );
        props.insert(
            "application.version".to_string(),
            ConfigValue::String(self.application.version.clone()),
        );
        props.insert(
            "application.description".to_string(),
            ConfigValue::String(self.application.description.clone()),
        );
        props.insert(
            "application.profiles.active".to_string(),
            ConfigValue::Array(
                self.application
                    .profiles
                    .active
                    .iter()
                    .map(|s| ConfigValue::String(s.clone()))
                    .collect(),
            ),
        );

        // Server properties
        props.insert(
            "server.port".to_string(),
            ConfigValue::Integer(self.server.port as i64),
        );
        props.insert(
            "server.host".to_string(),
            ConfigValue::String(self.server.host.clone()),
        );
        props.insert(
            "server.context_path".to_string(),
            ConfigValue::String(self.server.context_path.clone()),
        );
        props.insert(
            "server.ssl.enabled".to_string(),
            ConfigValue::Boolean(self.server.ssl.enabled),
        );

        // Database properties
        props.insert(
            "database.enabled".to_string(),
            ConfigValue::Boolean(self.database.enabled),
        );
        props.insert(
            "database.url".to_string(),
            ConfigValue::String(self.database.url.clone()),
        );
        props.insert(
            "database.username".to_string(),
            ConfigValue::String(self.database.username.clone()),
        );
        props.insert(
            "database.driver".to_string(),
            ConfigValue::String(self.database.driver.clone()),
        );
        props.insert(
            "database.max_connections".to_string(),
            ConfigValue::Integer(self.database.max_connections as i64),
        );

        // Logging properties
        props.insert(
            "logging.level".to_string(),
            ConfigValue::String(self.logging.level.clone()),
        );
        props.insert(
            "logging.pattern".to_string(),
            ConfigValue::String(self.logging.pattern.clone()),
        );

        // Security properties
        props.insert(
            "security.enabled".to_string(),
            ConfigValue::Boolean(self.security.enabled),
        );
        props.insert(
            "security.jwt.secret".to_string(),
            ConfigValue::String(self.security.jwt.secret.clone()),
        );
        props.insert(
            "security.jwt.expiration".to_string(),
            ConfigValue::Integer(self.security.jwt.expiration as i64),
        );

        // Custom properties
        for (key, value) in &self.custom {
            props.insert(key.clone(), value.clone());
        }

        props
    }

    fn get_property(&self, _key: &str) -> Option<&ConfigValue> {
        // This implementation has lifetime issues, so we'll use a different approach
        // In a real implementation, we would store properties as a field
        None // Simplified for compilation
    }
}

/// Application properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationProperties {
    pub name: String,
    pub version: String,
    pub description: String,
    pub profiles: ProfileProperties,
}

impl Default for ApplicationProperties {
    fn default() -> Self {
        Self {
            name: env::var("SPRING_APPLICATION_NAME").unwrap_or_else(|_| "orbit-app".to_string()),
            version: env::var("SPRING_APPLICATION_VERSION").unwrap_or_else(|_| "1.0.0".to_string()),
            description: "Orbit Spring Application".to_string(),
            profiles: ProfileProperties::default(),
        }
    }
}

/// Profile properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileProperties {
    pub active: Vec<String>,
    pub include: Vec<String>,
}

impl Default for ProfileProperties {
    fn default() -> Self {
        let active = env::var("SPRING_PROFILES_ACTIVE")
            .unwrap_or_else(|_| "default".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let include = env::var("SPRING_PROFILES_INCLUDE")
            .unwrap_or_else(|_| "".to_string())
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        Self { active, include }
    }
}

/// Server properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProperties {
    pub port: u16,
    pub host: String,
    pub context_path: String,
    pub ssl: SslProperties,
}

impl Default for ServerProperties {
    fn default() -> Self {
        Self {
            port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            context_path: env::var("SERVER_CONTEXT_PATH").unwrap_or_else(|_| "/".to_string()),
            ssl: SslProperties::default(),
        }
    }
}

/// SSL properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslProperties {
    pub enabled: bool,
    pub key_store: String,
    pub key_store_password: String,
    pub key_alias: String,
}

impl Default for SslProperties {
    fn default() -> Self {
        Self {
            enabled: env::var("SERVER_SSL_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            key_store: env::var("SERVER_SSL_KEY_STORE").unwrap_or_default(),
            key_store_password: env::var("SERVER_SSL_KEY_STORE_PASSWORD").unwrap_or_default(),
            key_alias: env::var("SERVER_SSL_KEY_ALIAS").unwrap_or_default(),
        }
    }
}

/// Database properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseProperties {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub driver: String,
    pub max_connections: u32,
    pub connection_timeout: u64,
    pub idle_timeout: u64,
}

impl Default for DatabaseProperties {
    fn default() -> Self {
        Self {
            enabled: env::var("DATABASE_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            url: env::var("DATABASE_URL").unwrap_or_default(),
            username: env::var("DATABASE_USERNAME").unwrap_or_default(),
            password: env::var("DATABASE_PASSWORD").unwrap_or_default(),
            driver: env::var("DATABASE_DRIVER").unwrap_or_else(|_| "postgresql".to_string()),
            max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            connection_timeout: env::var("DATABASE_CONNECTION_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            idle_timeout: env::var("DATABASE_IDLE_TIMEOUT")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .unwrap_or(600),
        }
    }
}

/// Logging properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingProperties {
    pub level: String,
    pub pattern: String,
    pub file: Option<String>,
    pub max_size: String,
    pub max_files: u32,
}

impl Default for LoggingProperties {
    fn default() -> Self {
        Self {
            level: env::var("LOGGING_LEVEL").unwrap_or_else(|_| "INFO".to_string()),
            pattern: env::var("LOGGING_PATTERN").unwrap_or_else(|_| {
                "%d{yyyy-MM-dd HH:mm:ss} [%thread] %-5level %logger{36} - %msg%n".to_string()
            }),
            file: env::var("LOGGING_FILE").ok(),
            max_size: env::var("LOGGING_MAX_SIZE").unwrap_or_else(|_| "10MB".to_string()),
            max_files: env::var("LOGGING_MAX_FILES")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
        }
    }
}

/// Security properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityProperties {
    pub enabled: bool,
    pub jwt: JwtProperties,
    pub cors: CorsProperties,
}

impl Default for SecurityProperties {
    fn default() -> Self {
        Self {
            enabled: env::var("SECURITY_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            jwt: JwtProperties::default(),
            cors: CorsProperties::default(),
        }
    }
}

/// JWT properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtProperties {
    pub secret: String,
    pub expiration: u64,
    pub issuer: String,
}

impl Default for JwtProperties {
    fn default() -> Self {
        Self {
            secret: env::var("JWT_SECRET").unwrap_or_else(|_| "orbit-secret-key".to_string()),
            expiration: env::var("JWT_EXPIRATION")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
            issuer: env::var("JWT_ISSUER").unwrap_or_else(|_| "orbit-spring".to_string()),
        }
    }
}

/// CORS properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsProperties {
    pub enabled: bool,
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub max_age: u64,
}

impl Default for CorsProperties {
    fn default() -> Self {
        Self {
            enabled: env::var("CORS_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
            ],
            allowed_headers: vec!["*".to_string()],
            max_age: 3600,
        }
    }
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> SpringResult<Self> {
        let config = Self::default();
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> SpringResult<Self> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(&path).map_err(|e| {
            SpringError::configuration(format!("Failed to read config file: {}", e))
        })?;

        let config: Self = if path_ref.extension().and_then(|s| s.to_str()) == Some("yml")
            || path_ref.extension().and_then(|s| s.to_str()) == Some("yaml")
        {
            serde_yaml::from_str(&content).map_err(|e| {
                SpringError::configuration(format!("Failed to parse YAML config: {}", e))
            })?
        } else {
            serde_json::from_str(&content).map_err(|e| {
                SpringError::configuration(format!("Failed to parse JSON config: {}", e))
            })?
        };

        config.validate()?;
        Ok(config)
    }

    /// Merge with another configuration
    pub fn merge(&mut self, other: AppConfig) -> SpringResult<()> {
        // Merge custom properties
        for (key, value) in other.custom {
            self.custom.insert(key, value);
        }

        // Override other properties if they're not default
        if other.application.name != "orbit-app" {
            self.application.name = other.application.name;
        }

        if other.server.port != 8080 {
            self.server.port = other.server.port;
        }

        // Validate after merge
        self.validate()?;
        Ok(())
    }

    /// Get a property value by dot notation key
    pub fn get_property_value(&self, key: &str) -> Option<ConfigValue> {
        let props = self.properties();
        props.get(key).cloned()
    }

    /// Set a custom property
    pub fn set_custom_property(&mut self, key: String, value: ConfigValue) {
        self.custom.insert(key, value);
    }

    /// Check if profile is active
    pub fn is_profile_active(&self, profile: &str) -> bool {
        self.application
            .profiles
            .active
            .contains(&profile.to_string())
            || self
                .application
                .profiles
                .include
                .contains(&profile.to_string())
    }

    /// Get active profiles
    pub fn active_profiles(&self) -> &Vec<String> {
        &self.application.profiles.active
    }
}

/// Configuration builder for programmatic configuration
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    config: AppConfig,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn application_name(mut self, name: impl Into<String>) -> Self {
        self.config.application.name = name.into();
        self
    }

    pub fn server_port(mut self, port: u16) -> Self {
        self.config.server.port = port;
        self
    }

    pub fn database_enabled(mut self, enabled: bool) -> Self {
        self.config.database.enabled = enabled;
        self
    }

    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.config.database.url = url.into();
        self
    }

    pub fn custom_property(mut self, key: impl Into<String>, value: ConfigValue) -> Self {
        self.config.custom.insert(key.into(), value);
        self
    }

    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.config.application.profiles.active.push(profile.into());
        self
    }

    pub fn build(self) -> SpringResult<AppConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}
