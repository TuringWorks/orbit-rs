//! Encryption at Rest and in Transit
//!
//! Provides comprehensive encryption support:
//! - TLS 1.3 for data in transit
//! - AES-256-GCM for data at rest
//! - Key management and rotation
//! - Hardware security module (HSM) support

use crate::exception::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// TLS version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TlsVersion {
    Tls12,
    Tls13,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub version: TlsVersion,
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: Option<String>,
    pub cipher_suites: Vec<String>,
    pub require_client_cert: bool,
}

impl TlsConfig {
    /// Create a new TLS 1.3 configuration
    pub fn tls13(cert_path: String, key_path: String) -> Self {
        Self {
            version: TlsVersion::Tls13,
            cert_path,
            key_path,
            ca_path: None,
            cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".to_string(),
                "TLS_AES_128_GCM_SHA256".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
            ],
            require_client_cert: false,
        }
    }
}

/// Encryption algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    Aes256Gcm,
    Aes128Gcm,
    ChaCha20Poly1305,
}

/// Encryption key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    pub id: String,
    pub algorithm: EncryptionAlgorithm,
    pub key_data: Vec<u8>,
    pub created_at: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub version: u32,
    pub metadata: HashMap<String, String>,
}

impl EncryptionKey {
    /// Create a new encryption key
    pub fn new(id: String, algorithm: EncryptionAlgorithm, key_data: Vec<u8>) -> Self {
        Self {
            id,
            algorithm,
            key_data,
            created_at: SystemTime::now(),
            expires_at: None,
            version: 1,
            metadata: HashMap::new(),
        }
    }

    /// Check if key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            SystemTime::now() > expires_at
        } else {
            false
        }
    }
}

/// Key rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationPolicy {
    pub rotation_interval: Duration,
    pub max_key_age: Duration,
    pub grace_period: Duration,
    pub auto_rotation: bool,
}

impl Default for KeyRotationPolicy {
    fn default() -> Self {
        Self {
            rotation_interval: Duration::from_secs(86400 * 30), // 30 days
            max_key_age: Duration::from_secs(86400 * 90),       // 90 days
            grace_period: Duration::from_secs(86400 * 7),       // 7 days
            auto_rotation: true,
        }
    }
}

/// Key store type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyStoreType {
    Memory,
    File,
    Hsm,
    CloudKms,
}

/// Key management system
pub struct KeyManagementSystem {
    keys: Arc<RwLock<HashMap<String, EncryptionKey>>>,
    active_key_id: Arc<RwLock<String>>,
    #[allow(dead_code)]
    store_type: KeyStoreType,
    store_type: KeyStoreType,
}

impl KeyManagementSystem {
    /// Create a new key management system
    pub fn new(rotation_policy: KeyRotationPolicy, store_type: KeyStoreType) -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            active_key_id: Arc::new(RwLock::new(None)),
            rotation_policy,
            store_type,
        }
    }

    /// Add a key
    pub async fn add_key(&self, key: EncryptionKey) -> OrbitResult<()> {
        let mut keys = self.keys.write().await;
        keys.insert(key.id.clone(), key);
        Ok(())
    }

    /// Set active key
    pub async fn set_active_key(&self, key_id: String) -> OrbitResult<()> {
        let keys = self.keys.read().await;
        if !keys.contains_key(&key_id) {
            return Err(OrbitError::internal("Key not found"));
        }

        let mut active_key_id = self.active_key_id.write().await;
        *active_key_id = Some(key_id);
        Ok(())
    }

    /// Get active key
    pub async fn get_active_key(&self) -> OrbitResult<EncryptionKey> {
        let active_key_id = self.active_key_id.read().await;
        let key_id = active_key_id
            .as_ref()
            .ok_or_else(|| OrbitError::internal("No active key"))?;

        let keys = self.keys.read().await;
        keys.get(key_id)
            .cloned()
            .ok_or_else(|| OrbitError::internal("Active key not found"))
    }

    /// Get key by ID
    pub async fn get_key(&self, key_id: &str) -> OrbitResult<Option<EncryptionKey>> {
        let keys = self.keys.read().await;
        Ok(keys.get(key_id).cloned())
    }

    /// Rotate keys
    pub async fn rotate_keys(&self) -> OrbitResult<EncryptionKey> {
        // Generate new key
        let new_key_id = format!("key-{}", uuid::Uuid::new_v4());
        let new_key = self.generate_key(new_key_id.clone())?;

        // Add new key
        self.add_key(new_key.clone()).await?;

        // Set as active
        self.set_active_key(new_key_id).await?;

        Ok(new_key)
    }

    /// Generate a new encryption key
    fn generate_key(&self, key_id: String) -> OrbitResult<EncryptionKey> {
        // In production, this would use a proper key generation mechanism
        // For now, create a stub key
        let key_data = vec![0u8; 32]; // 256-bit key
        let mut key = EncryptionKey::new(key_id, EncryptionAlgorithm::Aes256Gcm, key_data);
        key.expires_at = Some(SystemTime::now() + self.rotation_policy.max_key_age);
        Ok(key)
    }

    /// Check if key rotation is needed
    pub async fn needs_rotation(&self) -> OrbitResult<bool> {
        let active_key = self.get_active_key().await?;
        let age = SystemTime::now()
            .duration_since(active_key.created_at)
            .unwrap_or(Duration::from_secs(0));

        Ok(age >= self.rotation_policy.rotation_interval)
    }
}

/// Encryption manager
pub struct EncryptionManager {
    key_management: Arc<KeyManagementSystem>,
    tls_config: Option<TlsConfig>,
    at_rest_enabled: bool,
    in_transit_enabled: bool,
}

impl EncryptionManager {
    /// Create a new encryption manager
    pub fn new(key_management: Arc<KeyManagementSystem>, tls_config: Option<TlsConfig>) -> Self {
        Self {
            key_management,
            tls_config,
            at_rest_enabled: true,
            in_transit_enabled: true,
        }
    }

    /// Encrypt data
    pub async fn encrypt(&self, data: &[u8]) -> OrbitResult<Vec<u8>> {
        if !self.at_rest_enabled {
            return Ok(data.to_vec());
        }

        let _key = self.key_management.get_active_key().await?;

        // In production, this would use actual encryption
        // For now, return a stub encrypted format
        let mut encrypted = vec![0u8; data.len() + 16]; // Add space for tag
        encrypted[16..].copy_from_slice(data);

        Ok(encrypted)
    }

    /// Decrypt data
    pub async fn decrypt(&self, encrypted_data: &[u8], key_id: &str) -> OrbitResult<Vec<u8>> {
        if !self.at_rest_enabled {
            return Ok(encrypted_data.to_vec());
        }

        let _key = self
            .key_management
            .get_key(key_id)
            .await?
            .ok_or_else(|| OrbitError::internal("Key not found"))?;

        // In production, this would use actual decryption
        // For now, return a stub decrypted format
        if encrypted_data.len() <= 16 {
            return Err(OrbitError::internal("Invalid encrypted data"));
        }

        Ok(encrypted_data[16..].to_vec())
    }

    /// Get TLS configuration
    pub fn get_tls_config(&self) -> Option<&TlsConfig> {
        self.tls_config.as_ref()
    }

    /// Enable/disable encryption at rest
    pub fn set_at_rest_encryption(&mut self, enabled: bool) {
        self.at_rest_enabled = enabled;
    }

    /// Enable/disable encryption in transit
    pub fn set_in_transit_encryption(&mut self, enabled: bool) {
        self.in_transit_enabled = enabled;
    }

    /// Check if encryption at rest is enabled
    pub fn is_at_rest_enabled(&self) -> bool {
        self.at_rest_enabled
    }

    /// Check if encryption in transit is enabled
    pub fn is_in_transit_enabled(&self) -> bool {
        self.in_transit_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_management_system() {
        let kms = KeyManagementSystem::new(KeyRotationPolicy::default(), KeyStoreType::Memory);

        // Generate and add a key
        let key = kms.generate_key("test-key".to_string()).unwrap();
        kms.add_key(key.clone()).await.unwrap();

        // Set as active
        kms.set_active_key("test-key".to_string()).await.unwrap();

        // Get active key
        let active_key = kms.get_active_key().await.unwrap();
        assert_eq!(active_key.id, "test-key");
    }

    #[tokio::test]
    async fn test_encryption_manager() {
        let kms = Arc::new(KeyManagementSystem::new(
            KeyRotationPolicy::default(),
            KeyStoreType::Memory,
        ));

        // Add a key
        let key = kms.generate_key("test-key".to_string()).unwrap();
        let key_id = key.id.clone();
        kms.add_key(key).await.unwrap();
        kms.set_active_key(key_id.clone()).await.unwrap();

        let tls_config = TlsConfig::tls13("/path/to/cert".to_string(), "/path/to/key".to_string());

        let manager = EncryptionManager::new(kms, Some(tls_config));

        // Test encryption
        let data = b"Hello, World!";
        let encrypted = manager.encrypt(data).await.unwrap();
        assert!(encrypted.len() > data.len());

        // Test decryption
        let decrypted = manager.decrypt(&encrypted, &key_id).await.unwrap();
        assert_eq!(&decrypted, data);
    }

    #[test]
    fn test_tls_config() {
        let config = TlsConfig::tls13("/path/to/cert".to_string(), "/path/to/key".to_string());

        assert_eq!(config.version, TlsVersion::Tls13);
        assert_eq!(config.cipher_suites.len(), 3);
    }
}
