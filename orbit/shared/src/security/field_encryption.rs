//! Field-Level Encryption (FLE)
//!
//! Provides fine-grained encryption at the field/column level, allowing sensitive
//! data to be encrypted while non-sensitive data remains in plaintext. This enables:
//!
//! - Compliance with data protection regulations (GDPR, HIPAA, PCI-DSS)
//! - Minimizing encryption overhead by encrypting only sensitive fields
//! - Different encryption keys per field for defense in depth
//! - Deterministic encryption for searchable encrypted fields
//!
//! # Usage
//!
//! ```rust,ignore
//! use orbit_shared::security::field_encryption::{FieldEncryptionEngine, FieldEncryptionPolicy};
//!
//! let engine = FieldEncryptionEngine::new(key_management);
//!
//! // Define encryption policy for a table
//! let policy = FieldEncryptionPolicy::new("users")
//!     .encrypt_field("ssn", EncryptionType::Deterministic)
//!     .encrypt_field("credit_card", EncryptionType::Randomized)
//!     .encrypt_field("email", EncryptionType::Deterministic);
//!
//! engine.register_policy(policy).await?;
//!
//! // Encrypt row data
//! let encrypted_row = engine.encrypt_row("users", &row).await?;
//! ```

use crate::exception::{OrbitError, OrbitResult};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Nonce size for AES-256-GCM (96 bits = 12 bytes)
const NONCE_SIZE: usize = 12;

/// Encryption type for field-level encryption
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionType {
    /// Randomized encryption - same plaintext produces different ciphertext each time
    /// Most secure, but cannot be used for equality searches
    Randomized,

    /// Deterministic encryption - same plaintext always produces same ciphertext
    /// Less secure than randomized, but enables equality searches on encrypted data
    Deterministic,
}

/// Sensitivity level for fields
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SensitivityLevel {
    /// Public data - no encryption needed
    Public,
    /// Internal data - basic encryption
    Internal,
    /// Confidential data - strong encryption, audit logging
    Confidential,
    /// Restricted data - strongest encryption, strict access control
    Restricted,
}

/// Configuration for a single encrypted field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldEncryptionConfig {
    /// Field name
    pub field_name: String,
    /// Encryption type (randomized vs deterministic)
    pub encryption_type: EncryptionType,
    /// Sensitivity level
    pub sensitivity_level: SensitivityLevel,
    /// Optional custom key ID (uses default if not specified)
    pub key_id: Option<String>,
    /// Whether to mask the field in logs (always true for encrypted fields)
    pub mask_in_logs: bool,
    /// Optional algorithm override
    pub algorithm: Option<String>,
}

impl FieldEncryptionConfig {
    /// Create a new field encryption config
    pub fn new(field_name: &str, encryption_type: EncryptionType) -> Self {
        Self {
            field_name: field_name.to_string(),
            encryption_type,
            sensitivity_level: SensitivityLevel::Confidential,
            key_id: None,
            mask_in_logs: true,
            algorithm: None,
        }
    }

    /// Set sensitivity level
    pub fn with_sensitivity(mut self, level: SensitivityLevel) -> Self {
        self.sensitivity_level = level;
        self
    }

    /// Set custom key ID
    pub fn with_key(mut self, key_id: &str) -> Self {
        self.key_id = Some(key_id.to_string());
        self
    }
}

/// Policy defining which fields to encrypt for a table/collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldEncryptionPolicy {
    /// Table or collection name
    pub table_name: String,
    /// Map of field name to encryption config
    pub encrypted_fields: HashMap<String, FieldEncryptionConfig>,
    /// Default key ID for this policy
    pub default_key_id: Option<String>,
    /// Whether to encrypt all unlisted fields
    pub encrypt_unlisted: bool,
    /// Policy version for migration support
    pub version: u32,
    /// Whether the policy is enabled
    pub enabled: bool,
}

impl FieldEncryptionPolicy {
    /// Create a new field encryption policy for a table
    pub fn new(table_name: &str) -> Self {
        Self {
            table_name: table_name.to_string(),
            encrypted_fields: HashMap::new(),
            default_key_id: None,
            encrypt_unlisted: false,
            version: 1,
            enabled: true,
        }
    }

    /// Add a field to encrypt with randomized encryption
    pub fn encrypt_field(mut self, field_name: &str, encryption_type: EncryptionType) -> Self {
        let config = FieldEncryptionConfig::new(field_name, encryption_type);
        self.encrypted_fields.insert(field_name.to_string(), config);
        self
    }

    /// Add a field with full configuration
    pub fn encrypt_field_with_config(mut self, config: FieldEncryptionConfig) -> Self {
        self.encrypted_fields
            .insert(config.field_name.clone(), config);
        self
    }

    /// Set default key ID
    pub fn with_default_key(mut self, key_id: &str) -> Self {
        self.default_key_id = Some(key_id.to_string());
        self
    }

    /// Check if a field is encrypted
    pub fn is_field_encrypted(&self, field_name: &str) -> bool {
        self.encrypted_fields.contains_key(field_name)
    }

    /// Get encryption config for a field
    pub fn get_field_config(&self, field_name: &str) -> Option<&FieldEncryptionConfig> {
        self.encrypted_fields.get(field_name)
    }
}

/// Encrypted field value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedFieldValue {
    /// Encrypted data (base64 encoded for JSON compatibility)
    pub ciphertext: String,
    /// Key ID used for encryption
    pub key_id: String,
    /// Encryption type used
    pub encryption_type: EncryptionType,
    /// Version of the encryption scheme
    pub version: u32,
    /// Original field type for deserialization
    pub original_type: String,
}

impl EncryptedFieldValue {
    /// Create marker prefix for encrypted values
    pub fn marker() -> &'static str {
        "$FLE$"
    }

    /// Check if a string value is an encrypted field marker
    pub fn is_encrypted_marker(value: &str) -> bool {
        value.starts_with(Self::marker())
    }

    /// Serialize to a marked string
    pub fn to_marked_string(&self) -> OrbitResult<String> {
        let json = serde_json::to_string(self).map_err(|e| {
            OrbitError::internal(format!("Failed to serialize encrypted value: {}", e))
        })?;
        Ok(format!("{}{}", Self::marker(), json))
    }

    /// Deserialize from a marked string
    pub fn from_marked_string(value: &str) -> OrbitResult<Self> {
        if !Self::is_encrypted_marker(value) {
            return Err(OrbitError::internal("Not an encrypted field marker"));
        }
        let json = &value[Self::marker().len()..];
        serde_json::from_str(json)
            .map_err(|e| OrbitError::internal(format!("Failed to parse encrypted value: {}", e)))
    }
}

/// Field-Level Encryption Engine
pub struct FieldEncryptionEngine {
    /// Encryption keys indexed by key ID
    keys: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// Policies indexed by table name
    policies: Arc<RwLock<HashMap<String, FieldEncryptionPolicy>>>,
    /// Default key ID
    default_key_id: Arc<RwLock<String>>,
}

impl FieldEncryptionEngine {
    /// Create a new field encryption engine
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            policies: Arc::new(RwLock::new(HashMap::new())),
            default_key_id: Arc::new(RwLock::new("default".to_string())),
        }
    }

    /// Add an encryption key
    pub async fn add_key(&self, key_id: &str, key_data: Vec<u8>) -> OrbitResult<()> {
        if key_data.len() != 32 {
            return Err(OrbitError::internal("Key must be 32 bytes for AES-256"));
        }
        let mut keys = self.keys.write().await;
        keys.insert(key_id.to_string(), key_data);
        Ok(())
    }

    /// Generate and add a random encryption key
    pub async fn generate_key(&self, key_id: &str) -> OrbitResult<()> {
        let mut key_data = vec![0u8; 32];
        OsRng.fill_bytes(&mut key_data);
        self.add_key(key_id, key_data).await
    }

    /// Set default key ID
    pub async fn set_default_key(&self, key_id: &str) -> OrbitResult<()> {
        let keys = self.keys.read().await;
        if !keys.contains_key(key_id) {
            return Err(OrbitError::internal(format!("Key not found: {}", key_id)));
        }
        drop(keys);

        let mut default = self.default_key_id.write().await;
        *default = key_id.to_string();
        Ok(())
    }

    /// Register a field encryption policy
    pub async fn register_policy(&self, policy: FieldEncryptionPolicy) -> OrbitResult<()> {
        let mut policies = self.policies.write().await;
        policies.insert(policy.table_name.clone(), policy);
        Ok(())
    }

    /// Get policy for a table
    pub async fn get_policy(&self, table_name: &str) -> Option<FieldEncryptionPolicy> {
        let policies = self.policies.read().await;
        policies.get(table_name).cloned()
    }

    /// Encrypt a single field value
    pub async fn encrypt_field(
        &self,
        value: &serde_json::Value,
        config: &FieldEncryptionConfig,
    ) -> OrbitResult<EncryptedFieldValue> {
        // Serialize value to bytes
        let plaintext = serde_json::to_vec(value)
            .map_err(|e| OrbitError::internal(format!("Failed to serialize value: {}", e)))?;

        // Get key
        let key_id = config.key_id.clone().unwrap_or_else(|| {
            self.default_key_id
                .try_read()
                .map(|k| k.clone())
                .unwrap_or_default()
        });

        let keys = self.keys.read().await;
        let key = keys
            .get(&key_id)
            .ok_or_else(|| OrbitError::internal(format!("Key not found: {}", key_id)))?;

        // Encrypt based on type
        let ciphertext = match config.encryption_type {
            EncryptionType::Randomized => self.encrypt_randomized(&plaintext, key)?,
            EncryptionType::Deterministic => self.encrypt_deterministic(&plaintext, key)?,
        };

        Ok(EncryptedFieldValue {
            ciphertext: STANDARD.encode(&ciphertext),
            key_id,
            encryption_type: config.encryption_type,
            version: 1,
            original_type: self.get_json_type_name(value),
        })
    }

    /// Decrypt a single field value
    pub async fn decrypt_field(
        &self,
        encrypted: &EncryptedFieldValue,
    ) -> OrbitResult<serde_json::Value> {
        // Decode ciphertext
        let ciphertext = STANDARD
            .decode(&encrypted.ciphertext)
            .map_err(|e| OrbitError::internal(format!("Failed to decode ciphertext: {}", e)))?;

        // Get key
        let keys = self.keys.read().await;
        let key = keys
            .get(&encrypted.key_id)
            .ok_or_else(|| OrbitError::internal(format!("Key not found: {}", encrypted.key_id)))?;

        // Decrypt
        let plaintext = match encrypted.encryption_type {
            EncryptionType::Randomized => self.decrypt_randomized(&ciphertext, key)?,
            EncryptionType::Deterministic => self.decrypt_deterministic(&ciphertext, key)?,
        };

        // Deserialize
        serde_json::from_slice(&plaintext)
            .map_err(|e| OrbitError::internal(format!("Failed to deserialize value: {}", e)))
    }

    /// Encrypt a row according to its table's policy
    pub async fn encrypt_row(
        &self,
        table_name: &str,
        row: &HashMap<String, serde_json::Value>,
    ) -> OrbitResult<HashMap<String, serde_json::Value>> {
        let policies = self.policies.read().await;
        let policy = match policies.get(table_name) {
            Some(p) if p.enabled => p,
            _ => return Ok(row.clone()), // No policy or disabled, return as-is
        };

        let mut encrypted_row = HashMap::new();

        for (field_name, value) in row {
            if let Some(config) = policy.encrypted_fields.get(field_name) {
                // Encrypt this field
                let encrypted = self.encrypt_field(value, config).await?;
                let marked = encrypted.to_marked_string()?;
                encrypted_row.insert(field_name.clone(), serde_json::Value::String(marked));
            } else {
                // Keep unencrypted
                encrypted_row.insert(field_name.clone(), value.clone());
            }
        }

        Ok(encrypted_row)
    }

    /// Decrypt a row according to its table's policy
    pub async fn decrypt_row(
        &self,
        table_name: &str,
        row: &HashMap<String, serde_json::Value>,
    ) -> OrbitResult<HashMap<String, serde_json::Value>> {
        let policies = self.policies.read().await;
        let policy = match policies.get(table_name) {
            Some(p) if p.enabled => p,
            _ => return Ok(row.clone()),
        };

        let mut decrypted_row = HashMap::new();

        for (field_name, value) in row {
            if policy.encrypted_fields.contains_key(field_name) {
                // Try to decrypt
                if let serde_json::Value::String(s) = value {
                    if EncryptedFieldValue::is_encrypted_marker(s) {
                        let encrypted = EncryptedFieldValue::from_marked_string(s)?;
                        let decrypted = self.decrypt_field(&encrypted).await?;
                        decrypted_row.insert(field_name.clone(), decrypted);
                        continue;
                    }
                }
            }
            decrypted_row.insert(field_name.clone(), value.clone());
        }

        Ok(decrypted_row)
    }

    /// Encrypt multiple rows in batch
    pub async fn encrypt_rows(
        &self,
        table_name: &str,
        rows: &[HashMap<String, serde_json::Value>],
    ) -> OrbitResult<Vec<HashMap<String, serde_json::Value>>> {
        let mut encrypted_rows = Vec::with_capacity(rows.len());
        for row in rows {
            encrypted_rows.push(self.encrypt_row(table_name, row).await?);
        }
        Ok(encrypted_rows)
    }

    /// Decrypt multiple rows in batch
    pub async fn decrypt_rows(
        &self,
        table_name: &str,
        rows: &[HashMap<String, serde_json::Value>],
    ) -> OrbitResult<Vec<HashMap<String, serde_json::Value>>> {
        let mut decrypted_rows = Vec::with_capacity(rows.len());
        for row in rows {
            decrypted_rows.push(self.decrypt_row(table_name, row).await?);
        }
        Ok(decrypted_rows)
    }

    /// Create searchable token for deterministic encryption
    /// This allows equality searches on encrypted fields
    pub async fn create_search_token(
        &self,
        table_name: &str,
        field_name: &str,
        value: &serde_json::Value,
    ) -> OrbitResult<String> {
        let policies = self.policies.read().await;
        let policy = policies
            .get(table_name)
            .ok_or_else(|| OrbitError::internal(format!("No policy for table: {}", table_name)))?;

        let config = policy
            .encrypted_fields
            .get(field_name)
            .ok_or_else(|| OrbitError::internal(format!("Field not encrypted: {}", field_name)))?;

        if config.encryption_type != EncryptionType::Deterministic {
            return Err(OrbitError::internal(
                "Search tokens only available for deterministic encryption",
            ));
        }

        let encrypted = self.encrypt_field(value, config).await?;
        Ok(encrypted.ciphertext)
    }

    // Internal encryption methods

    /// Randomized encryption using AES-256-GCM with random nonce
    fn encrypt_randomized(&self, plaintext: &[u8], key: &[u8]) -> OrbitResult<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| OrbitError::internal(format!("Invalid key: {}", e)))?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| OrbitError::internal(format!("Encryption failed: {}", e)))?;

        // Prepend nonce
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt randomized encryption
    fn decrypt_randomized(&self, ciphertext: &[u8], key: &[u8]) -> OrbitResult<Vec<u8>> {
        if ciphertext.len() < NONCE_SIZE {
            return Err(OrbitError::internal("Ciphertext too short"));
        }

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| OrbitError::internal(format!("Invalid key: {}", e)))?;

        let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
        let encrypted = &ciphertext[NONCE_SIZE..];

        cipher
            .decrypt(nonce, encrypted)
            .map_err(|_| OrbitError::internal("Decryption failed"))
    }

    /// Deterministic encryption using AES-256-GCM with derived nonce
    fn encrypt_deterministic(&self, plaintext: &[u8], key: &[u8]) -> OrbitResult<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| OrbitError::internal(format!("Invalid key: {}", e)))?;

        // Derive nonce deterministically from plaintext and key
        let nonce_bytes = self.derive_deterministic_nonce(plaintext, key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| OrbitError::internal(format!("Encryption failed: {}", e)))?;

        // Prepend nonce
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt deterministic encryption
    fn decrypt_deterministic(&self, ciphertext: &[u8], key: &[u8]) -> OrbitResult<Vec<u8>> {
        // Same as randomized decryption
        self.decrypt_randomized(ciphertext, key)
    }

    /// Derive deterministic nonce from plaintext and key
    fn derive_deterministic_nonce(&self, plaintext: &[u8], key: &[u8]) -> [u8; NONCE_SIZE] {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(plaintext);
        let hash = hasher.finalize();

        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&hash[..NONCE_SIZE]);
        nonce
    }

    /// Get JSON type name for a value
    fn get_json_type_name(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(_) => "bool".to_string(),
            serde_json::Value::Number(_) => "number".to_string(),
            serde_json::Value::String(_) => "string".to_string(),
            serde_json::Value::Array(_) => "array".to_string(),
            serde_json::Value::Object(_) => "object".to_string(),
        }
    }
}

impl Default for FieldEncryptionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_engine() -> FieldEncryptionEngine {
        let engine = FieldEncryptionEngine::new();
        engine.generate_key("default").await.unwrap();
        engine.set_default_key("default").await.unwrap();
        engine
    }

    #[tokio::test]
    async fn test_randomized_encryption() {
        let engine = create_test_engine().await;

        let config = FieldEncryptionConfig::new("ssn", EncryptionType::Randomized);
        let value = serde_json::json!("123-45-6789");

        // Encrypt twice
        let encrypted1 = engine.encrypt_field(&value, &config).await.unwrap();
        let encrypted2 = engine.encrypt_field(&value, &config).await.unwrap();

        // Randomized should produce different ciphertexts
        assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);

        // But both should decrypt to same value
        let decrypted1 = engine.decrypt_field(&encrypted1).await.unwrap();
        let decrypted2 = engine.decrypt_field(&encrypted2).await.unwrap();
        assert_eq!(decrypted1, value);
        assert_eq!(decrypted2, value);
    }

    #[tokio::test]
    async fn test_deterministic_encryption() {
        let engine = create_test_engine().await;

        let config = FieldEncryptionConfig::new("email", EncryptionType::Deterministic);
        let value = serde_json::json!("test@example.com");

        // Encrypt twice
        let encrypted1 = engine.encrypt_field(&value, &config).await.unwrap();
        let encrypted2 = engine.encrypt_field(&value, &config).await.unwrap();

        // Deterministic should produce same ciphertext
        assert_eq!(encrypted1.ciphertext, encrypted2.ciphertext);

        // And should decrypt correctly
        let decrypted = engine.decrypt_field(&encrypted1).await.unwrap();
        assert_eq!(decrypted, value);
    }

    #[tokio::test]
    async fn test_row_encryption() {
        let engine = create_test_engine().await;

        // Register policy
        let policy = FieldEncryptionPolicy::new("users")
            .encrypt_field("ssn", EncryptionType::Randomized)
            .encrypt_field("email", EncryptionType::Deterministic);
        engine.register_policy(policy).await.unwrap();

        // Create test row
        let row: HashMap<String, serde_json::Value> = [
            ("id".to_string(), serde_json::json!(1)),
            ("name".to_string(), serde_json::json!("John Doe")),
            ("ssn".to_string(), serde_json::json!("123-45-6789")),
            ("email".to_string(), serde_json::json!("john@example.com")),
        ]
        .into_iter()
        .collect();

        // Encrypt row
        let encrypted_row = engine.encrypt_row("users", &row).await.unwrap();

        // Verify: id and name should be unchanged
        assert_eq!(encrypted_row.get("id"), row.get("id"));
        assert_eq!(encrypted_row.get("name"), row.get("name"));

        // ssn and email should be encrypted markers
        let ssn = encrypted_row.get("ssn").unwrap().as_str().unwrap();
        let email = encrypted_row.get("email").unwrap().as_str().unwrap();
        assert!(EncryptedFieldValue::is_encrypted_marker(ssn));
        assert!(EncryptedFieldValue::is_encrypted_marker(email));

        // Decrypt row
        let decrypted_row = engine.decrypt_row("users", &encrypted_row).await.unwrap();
        assert_eq!(decrypted_row, row);
    }

    #[tokio::test]
    async fn test_search_token() {
        let engine = create_test_engine().await;

        let policy = FieldEncryptionPolicy::new("users")
            .encrypt_field("email", EncryptionType::Deterministic);
        engine.register_policy(policy).await.unwrap();

        // Create search token
        let value = serde_json::json!("test@example.com");
        let token1 = engine
            .create_search_token("users", "email", &value)
            .await
            .unwrap();
        let token2 = engine
            .create_search_token("users", "email", &value)
            .await
            .unwrap();

        // Same value should produce same token
        assert_eq!(token1, token2);

        // Different value should produce different token
        let other_value = serde_json::json!("other@example.com");
        let other_token = engine
            .create_search_token("users", "email", &other_value)
            .await
            .unwrap();
        assert_ne!(token1, other_token);
    }

    #[tokio::test]
    async fn test_multiple_keys() {
        let engine = FieldEncryptionEngine::new();
        engine.generate_key("key1").await.unwrap();
        engine.generate_key("key2").await.unwrap();
        engine.set_default_key("key1").await.unwrap();

        // Encrypt with different keys
        let config1 = FieldEncryptionConfig::new("field1", EncryptionType::Randomized);
        let config2 =
            FieldEncryptionConfig::new("field2", EncryptionType::Randomized).with_key("key2");

        let value = serde_json::json!("secret data");

        let encrypted1 = engine.encrypt_field(&value, &config1).await.unwrap();
        let encrypted2 = engine.encrypt_field(&value, &config2).await.unwrap();

        assert_eq!(encrypted1.key_id, "key1");
        assert_eq!(encrypted2.key_id, "key2");

        // Both should decrypt correctly
        let decrypted1 = engine.decrypt_field(&encrypted1).await.unwrap();
        let decrypted2 = engine.decrypt_field(&encrypted2).await.unwrap();
        assert_eq!(decrypted1, value);
        assert_eq!(decrypted2, value);
    }

    #[tokio::test]
    async fn test_complex_data_types() {
        let engine = create_test_engine().await;

        let config = FieldEncryptionConfig::new("data", EncryptionType::Randomized);

        // Test various JSON types
        let values = vec![
            serde_json::json!(null),
            serde_json::json!(true),
            serde_json::json!(42),
            serde_json::json!(3.14),
            serde_json::json!("string"),
            serde_json::json!(["array", "values"]),
            serde_json::json!({"nested": {"object": true}}),
        ];

        for value in values {
            let encrypted = engine.encrypt_field(&value, &config).await.unwrap();
            let decrypted = engine.decrypt_field(&encrypted).await.unwrap();
            assert_eq!(decrypted, value);
        }
    }

    #[tokio::test]
    async fn test_sensitivity_levels() {
        let config = FieldEncryptionConfig::new("field", EncryptionType::Randomized)
            .with_sensitivity(SensitivityLevel::Restricted);

        assert_eq!(config.sensitivity_level, SensitivityLevel::Restricted);
        assert!(config.mask_in_logs);

        // Verify ordering
        assert!(SensitivityLevel::Restricted > SensitivityLevel::Confidential);
        assert!(SensitivityLevel::Confidential > SensitivityLevel::Internal);
        assert!(SensitivityLevel::Internal > SensitivityLevel::Public);
    }

    #[tokio::test]
    async fn test_batch_encryption() {
        let engine = create_test_engine().await;

        let policy = FieldEncryptionPolicy::new("orders")
            .encrypt_field("credit_card", EncryptionType::Randomized);
        engine.register_policy(policy).await.unwrap();

        let rows: Vec<HashMap<String, serde_json::Value>> = (0..3)
            .map(|i| {
                [
                    ("id".to_string(), serde_json::json!(i)),
                    (
                        "credit_card".to_string(),
                        serde_json::json!(format!("4111111111111{:03}", i)),
                    ),
                ]
                .into_iter()
                .collect()
            })
            .collect();

        let encrypted = engine.encrypt_rows("orders", &rows).await.unwrap();
        assert_eq!(encrypted.len(), 3);

        let decrypted = engine.decrypt_rows("orders", &encrypted).await.unwrap();
        assert_eq!(decrypted, rows);
    }
}
