//! Persistent storage for connections, settings, and query history
//!
//! This module handles saving and loading application data to/from disk
//! using JSON files in the user's application data directory.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::api::path::app_data_dir;
use tauri::Config;
use tracing::info;

use crate::connections::Connection;

/// Application data storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStorage {
    pub connections: Vec<StoredConnection>,
    pub settings: AppSettings,
    pub query_history: Vec<StoredQueryHistory>,
    pub version: String,
}

/// Stored connection (without active connection handles)
/// Passwords are stored encrypted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConnection {
    pub id: String,
    pub info: StoredConnectionInfo,
    /// RFC 3339. Absent for records written before this field existed, and for
    /// records whose stamp could not be read back.
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_used: Option<String>,
    #[serde(default)]
    pub query_count: u64,
}

/// Connection info with encrypted password
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConnectionInfo {
    pub name: String,
    pub connection_type: String,
    pub host: String,
    pub port: u16,
    pub database: Option<String>,
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_encrypted: Option<String>, // Encrypted password
    pub ssl_mode: Option<String>,
    pub connection_timeout: Option<u64>,
    pub additional_params: HashMap<String, String>,
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub auto_save: bool,
    pub query_timeout: u64,
    pub editor_font_size: u16,
    pub editor_theme: String,
    pub show_line_numbers: bool,
    pub word_wrap: bool,
    pub max_query_history: usize,
    pub connection_timeout: u64,
    /// Orbit-RS checkout whose local cluster the cluster panel manages.
    ///
    /// `None` means "look for one next to the working directory".
    #[serde(default)]
    pub cluster_root: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            auto_save: true,
            query_timeout: 30000,
            editor_font_size: 14,
            editor_theme: "one-dark".to_string(),
            show_line_numbers: true,
            word_wrap: false,
            max_query_history: 1000,
            connection_timeout: 5000,
            cluster_root: None,
        }
    }
}

/// Stored query history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredQueryHistory {
    pub id: String,
    pub connection_id: String,
    pub query: String,
    pub query_type: String,
    pub executed_at: String,
    pub execution_time: f64,
    pub success: bool,
    pub error: Option<String>,
}

impl Default for AppStorage {
    fn default() -> Self {
        Self {
            connections: vec![],
            settings: AppSettings::default(),
            query_history: vec![],
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Storage manager for persisting application data
#[derive(Clone)]
pub struct StorageManager {
    storage_dir: PathBuf,
    storage_file: PathBuf,
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(config: &Config) -> Result<Self, StorageError> {
        let app_name = config
            .package
            .product_name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("orbit-desktop");

        let storage_dir = app_data_dir(config)
            .ok_or_else(|| {
                StorageError::ConfigError("Could not determine app data directory".to_string())
            })?
            .join(app_name);

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&storage_dir).map_err(|e| {
            StorageError::IoError(format!("Failed to create storage directory: {}", e))
        })?;

        let storage_file = storage_dir.join("storage.json");

        info!("Storage directory: {:?}", storage_dir);
        info!("Storage file: {:?}", storage_file);

        Ok(Self {
            storage_dir,
            storage_file,
        })
    }

    /// Load application storage from disk
    pub fn load(&self) -> Result<AppStorage, StorageError> {
        if !self.storage_file.exists() {
            info!("Storage file does not exist, creating default storage");
            let default_storage = AppStorage::default();
            self.save(&default_storage)?;
            return Ok(default_storage);
        }

        let content = std::fs::read_to_string(&self.storage_file)
            .map_err(|e| StorageError::IoError(format!("Failed to read storage file: {}", e)))?;

        let storage: AppStorage = serde_json::from_str(&content).map_err(|e| {
            StorageError::ParseError(format!("Failed to parse storage file: {}", e))
        })?;

        info!(
            "Loaded storage with {} connections and {} query history entries",
            storage.connections.len(),
            storage.query_history.len()
        );

        Ok(storage)
    }

    /// Save application storage to disk
    pub fn save(&self, storage: &AppStorage) -> Result<(), StorageError> {
        let content = serde_json::to_string_pretty(storage).map_err(|e| {
            StorageError::SerializeError(format!("Failed to serialize storage: {}", e))
        })?;

        // Write to temporary file first, then rename (atomic write)
        let temp_file = self.storage_file.with_extension("tmp");
        std::fs::write(&temp_file, content)
            .map_err(|e| StorageError::IoError(format!("Failed to write storage file: {}", e)))?;

        std::fs::rename(&temp_file, &self.storage_file)
            .map_err(|e| StorageError::IoError(format!("Failed to rename storage file: {}", e)))?;

        info!(
            "Saved storage with {} connections and {} query history entries",
            storage.connections.len(),
            storage.query_history.len()
        );

        Ok(())
    }

    /// Get storage directory path
    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Get storage file path
    pub fn storage_file(&self) -> &Path {
        &self.storage_file
    }
}

/// Storage errors
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Serialize error: {0}")]
    SerializeError(String),
    #[error("Config error: {0}")]
    ConfigError(String),
}

/// Helper functions for converting between storage and runtime types

impl StoredConnection {
    /// Convert to Connection with password decryption
    /// # Errors
    /// Returns [`StorageError::ParseError`] when the stored protocol name is
    /// not one this build knows.
    pub fn to_connection(
        &self,
        enc_manager: &crate::encryption::EncryptionManager,
    ) -> Result<Connection, StorageError> {
        use crate::connections::{ConnectionInfo, ConnectionStatus, ConnectionType};
        use chrono::DateTime;

        // A password that will not decrypt is dropped rather than guessed at;
        // the connection is still usable for endpoints that need no password.
        let password = self.info.password_encrypted.as_ref().and_then(|encrypted| {
            enc_manager
                .decrypt(encrypted)
                .map_err(|e| tracing::warn!("could not decrypt password for {}: {e}", self.id))
                .ok()
        });

        let connection_type: ConnectionType = self
            .info
            .connection_type
            .parse()
            .map_err(|e| StorageError::ParseError(format!("{e}")))?;

        /// Parse an RFC 3339 stamp, reporting `None` instead of substituting
        /// the current time for one that will not parse.
        fn timestamp(raw: &str) -> Option<chrono::DateTime<chrono::Utc>> {
            DateTime::parse_from_rfc3339(raw)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        }

        let created_at = self.created_at.as_deref().and_then(timestamp);
        if created_at.is_none() && self.created_at.is_some() {
            tracing::warn!(
                "connection {} has an unreadable created_at ({:?}); reporting it as unknown",
                self.id,
                self.created_at
            );
        }

        Ok(Connection {
            id: self.id.clone(),
            info: ConnectionInfo {
                name: self.info.name.clone(),
                connection_type,
                host: self.info.host.clone(),
                port: self.info.port,
                database: self.info.database.clone(),
                username: self.info.username.clone(),
                password,
                ssl_mode: self.info.ssl_mode.clone(),
                connection_timeout: self.info.connection_timeout,
                additional_params: self.info.additional_params.clone(),
            },
            status: ConnectionStatus::Disconnected,
            created_at,
            last_used: self.last_used.as_deref().and_then(timestamp),
            query_count: self.query_count,
        })
    }
}

impl Connection {
    /// Convert to StoredConnection with password encryption
    pub fn to_stored(
        &self,
        enc_manager: &crate::encryption::EncryptionManager,
    ) -> Result<StoredConnection, StorageError> {
        // Encrypt password if present
        let password_encrypted = if let Some(password) = &self.info.password {
            Some(enc_manager.encrypt(password).map_err(|e| {
                StorageError::SerializeError(format!("Failed to encrypt password: {}", e))
            })?)
        } else {
            None
        };

        Ok(StoredConnection {
            id: self.id.clone(),
            info: StoredConnectionInfo {
                name: self.info.name.clone(),
                // Same table as `to_connection` reads back, via Display/FromStr,
                // so the two directions cannot drift apart.
                connection_type: self.info.connection_type.to_string(),
                host: self.info.host.clone(),
                port: self.info.port,
                database: self.info.database.clone(),
                username: self.info.username.clone(),
                password_encrypted,
                ssl_mode: self.info.ssl_mode.clone(),
                connection_timeout: self.info.connection_timeout,
                additional_params: self.info.additional_params.clone(),
            },
            created_at: self.created_at.map(|dt| dt.to_rfc3339()),
            last_used: self.last_used.map(|dt| dt.to_rfc3339()),
            query_count: self.query_count,
        })
    }
}
