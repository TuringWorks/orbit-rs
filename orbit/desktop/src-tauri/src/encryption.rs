//! Password encryption for secure storage
//! 
//! This module provides encryption/decryption for storing passwords securely.
//! Uses AES-GCM encryption with a key derived from the system.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::path::PathBuf;
use tauri::api::path::app_data_dir;
use tauri::Config;
use tracing::{error, info, warn};

/// Encryption key manager
pub struct EncryptionManager {
    key: Aes256Gcm,
    key_file: PathBuf,
}

impl EncryptionManager {
    /// Create or load encryption key
    pub fn new(config: &Config) -> Result<Self, EncryptionError> {
        let app_name = config
            .package
            .product_name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("orbit-desktop");
        
        let app_dir = app_data_dir(config)
            .ok_or_else(|| EncryptionError::ConfigError("Could not determine app data directory".to_string()))?
            .join(app_name);
        
        std::fs::create_dir_all(&app_dir)
            .map_err(|e| EncryptionError::IoError(format!("Failed to create app directory: {}", e)))?;
        
        let key_file = app_dir.join(".encryption_key");
        
        let key = if key_file.exists() {
            // Load existing key
            let key_bytes = fs::read(&key_file)
                .map_err(|e| EncryptionError::IoError(format!("Failed to read key file: {}", e)))?;
            
            if key_bytes.len() != 32 {
                warn!("Key file has invalid length, generating new key");
                Self::generate_and_save_key(&key_file)?
            } else {
                Aes256Gcm::new_from_slice(&key_bytes)
                    .map_err(|e| EncryptionError::KeyError(format!("Invalid key: {}", e)))?
            }
        } else {
            // Generate new key
            info!("Generating new encryption key");
            Self::generate_and_save_key(&key_file)?
        };
        
        Ok(Self { key, key_file })
    }
    
    fn generate_and_save_key(key_file: &PathBuf) -> Result<Aes256Gcm, EncryptionError> {
        let key = Aes256Gcm::generate_key(&mut OsRng);
        
        // Save key to file with restricted permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(key_file.parent().unwrap())
                .map_err(|e| EncryptionError::IoError(format!("Failed to get metadata: {}", e)))?
                .permissions();
            perms.set_mode(0o700); // rwx------
            fs::set_permissions(key_file.parent().unwrap(), perms)
                .map_err(|e| EncryptionError::IoError(format!("Failed to set permissions: {}", e)))?;
        }
        
        fs::write(key_file, key.as_slice())
            .map_err(|e| EncryptionError::IoError(format!("Failed to write key file: {}", e)))?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(key_file)
                .map_err(|e| EncryptionError::IoError(format!("Failed to get key file metadata: {}", e)))?
                .permissions();
            perms.set_mode(0o600); // rw-------
            fs::set_permissions(key_file, perms)
                .map_err(|e| EncryptionError::IoError(format!("Failed to set key file permissions: {}", e)))?;
        }
        
        Ok(Aes256Gcm::new(&key))
    }
    
    /// Encrypt a password
    pub fn encrypt(&self, plaintext: &str) -> Result<String, EncryptionError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.key
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| EncryptionError::EncryptionError(e.to_string()))?;
        
        // Combine nonce and ciphertext
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        
        // Encode as base64
        Ok(general_purpose::STANDARD.encode(&combined))
    }
    
    /// Decrypt a password
    pub fn decrypt(&self, ciphertext: &str) -> Result<String, EncryptionError> {
        // Decode from base64
        let combined = general_purpose::STANDARD
            .decode(ciphertext)
            .map_err(|e| EncryptionError::DecryptionError(format!("Invalid base64: {}", e)))?;
        
        if combined.len() < 12 {
            return Err(EncryptionError::DecryptionError("Ciphertext too short".to_string()));
        }
        
        // Extract nonce (first 12 bytes) and ciphertext
        let nonce = Nonce::from_slice(&combined[..12]);
        let ciphertext = &combined[12..];
        
        // Decrypt
        let plaintext = self.key
            .decrypt(nonce, ciphertext)
            .map_err(|e| EncryptionError::DecryptionError(e.to_string()))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| EncryptionError::DecryptionError(format!("Invalid UTF-8: {}", e)))
    }
}

/// Encryption errors
#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Key error: {0}")]
    KeyError(String),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    #[error("Config error: {0}")]
    ConfigError(String),
}

