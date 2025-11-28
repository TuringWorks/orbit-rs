//! PostgreSQL authentication implementation
//!
//! Implements multiple authentication methods:
//! - Trust (no authentication)
//! - Clear text password
//! - MD5 password
//! - SCRAM-SHA-256 (RFC 5802, RFC 7677)

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use md5;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::messages::AuthenticationResponse;
use crate::error::{ProtocolError, ProtocolResult};

/// Authentication method configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// Trust authentication (no password required)
    Trust,
    /// Clear text password
    Password,
    /// MD5-hashed password
    MD5,
    /// SCRAM-SHA-256 authentication
    ScramSha256,
}

/// User credentials stored in the system
#[derive(Debug, Clone)]
pub struct UserCredentials {
    /// Username
    pub username: String,
    /// Stored password (hashed for MD5, plaintext for other methods)
    pub password_hash: String,
    /// SCRAM-SHA-256 stored key (if using SCRAM)
    pub scram_stored_key: Option<Vec<u8>>,
    /// SCRAM-SHA-256 server key (if using SCRAM)
    pub scram_server_key: Option<Vec<u8>>,
    /// SCRAM-SHA-256 salt (if using SCRAM)
    pub scram_salt: Option<Vec<u8>>,
    /// SCRAM-SHA-256 iteration count
    pub scram_iterations: Option<u32>,
}

/// User store for authentication
#[derive(Debug, Clone)]
pub struct UserStore {
    users: Arc<RwLock<HashMap<String, UserCredentials>>>,
}

impl UserStore {
    /// Create a new user store
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a user with plaintext password (will be hashed appropriately)
    pub async fn add_user(&self, username: String, password: String, auth_method: &AuthMethod) {
        let credentials = match auth_method {
            AuthMethod::Trust | AuthMethod::Password => UserCredentials {
                username: username.clone(),
                password_hash: password,
                scram_stored_key: None,
                scram_server_key: None,
                scram_salt: None,
                scram_iterations: None,
            },
            AuthMethod::MD5 => UserCredentials {
                username: username.clone(),
                password_hash: password, // Will be hashed with salt + username during auth
                scram_stored_key: None,
                scram_server_key: None,
                scram_salt: None,
                scram_iterations: None,
            },
            AuthMethod::ScramSha256 => {
                // Generate SCRAM credentials
                let (stored_key, server_key, salt) =
                    Self::generate_scram_credentials(&password, 4096);
                UserCredentials {
                    username: username.clone(),
                    password_hash: String::new(), // Not used for SCRAM
                    scram_stored_key: Some(stored_key),
                    scram_server_key: Some(server_key),
                    scram_salt: Some(salt),
                    scram_iterations: Some(4096),
                }
            }
        };

        self.users.write().await.insert(username, credentials);
    }

    /// Get user credentials
    pub async fn get_user(&self, username: &str) -> Option<UserCredentials> {
        self.users.read().await.get(username).cloned()
    }

    /// Generate SCRAM-SHA-256 credentials from password
    fn generate_scram_credentials(password: &str, iterations: u32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use hmac::{Hmac, Mac};

        type HmacSha256 = Hmac<Sha256>;

        // Generate random salt
        let salt: Vec<u8> = (0..16).map(|_| rand::thread_rng().gen()).collect();

        // Compute salted password using PBKDF2
        let mut salted_password = vec![0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut salted_password);

        // Compute client key: HMAC(salted_password, "Client Key")
        let mut client_key_mac = HmacSha256::new_from_slice(&salted_password).unwrap();
        client_key_mac.update(b"Client Key");
        let client_key = client_key_mac.finalize().into_bytes();

        // Compute stored key: H(client_key)
        let mut hasher = Sha256::new();
        hasher.update(&client_key);
        let stored_key = hasher.finalize().to_vec();

        // Compute server key: HMAC(salted_password, "Server Key")
        let mut server_key_mac = HmacSha256::new_from_slice(&salted_password).unwrap();
        server_key_mac.update(b"Server Key");
        let server_key = server_key_mac.finalize().into_bytes().to_vec();

        (stored_key, server_key, salt)
    }
}

impl Default for UserStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Authentication manager
pub struct AuthManager {
    auth_method: AuthMethod,
    user_store: UserStore,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(auth_method: AuthMethod, user_store: UserStore) -> Self {
        Self {
            auth_method,
            user_store,
        }
    }

    /// Get the initial authentication response for the configured method
    pub fn get_initial_auth_response(&self) -> AuthenticationResponse {
        match self.auth_method {
            AuthMethod::Trust => AuthenticationResponse::Ok,
            AuthMethod::Password => AuthenticationResponse::CleartextPassword,
            AuthMethod::MD5 => {
                // Generate random 4-byte salt
                let salt: [u8; 4] = rand::random();
                AuthenticationResponse::MD5Password { salt }
            }
            AuthMethod::ScramSha256 => AuthenticationResponse::SASL {
                mechanisms: vec!["SCRAM-SHA-256".to_string()],
            },
        }
    }

    /// Verify password for the given username
    pub async fn verify_password(
        &self,
        username: &str,
        password: &str,
        salt: Option<&[u8; 4]>,
    ) -> ProtocolResult<bool> {
        match self.auth_method {
            AuthMethod::Trust => Ok(true), // Trust always succeeds
            AuthMethod::Password => {
                // Simple plaintext comparison
                let user = self.user_store.get_user(username).await;
                Ok(user.map(|u| u.password_hash == password).unwrap_or(false))
            }
            AuthMethod::MD5 => {
                let user = self.user_store.get_user(username).await;
                let salt = salt.ok_or_else(|| {
                    ProtocolError::AuthenticationError(
                        "Salt required for MD5 authentication".to_string(),
                    )
                })?;

                if let Some(user) = user {
                    let expected = compute_md5_hash(&user.password_hash, username, salt);
                    Ok(password == expected)
                } else {
                    Ok(false)
                }
            }
            AuthMethod::ScramSha256 => {
                // SCRAM verification handled separately
                Ok(false)
            }
        }
    }

    /// Get authentication method
    pub fn auth_method(&self) -> &AuthMethod {
        &self.auth_method
    }

    /// Get user store
    pub fn user_store(&self) -> &UserStore {
        &self.user_store
    }
}

/// Compute MD5 password hash for PostgreSQL
///
/// PostgreSQL MD5 authentication uses: "md5" + md5(md5(password + username) + salt)
pub fn compute_md5_hash(password: &str, username: &str, salt: &[u8; 4]) -> String {
    // First hash: md5(password + username)
    let mut combined = Vec::new();
    combined.extend_from_slice(password.as_bytes());
    combined.extend_from_slice(username.as_bytes());
    let first_hash = md5::compute(&combined);

    // Convert to hex string
    let hex_hash = format!("{:x}", first_hash);

    // Second hash: md5(hex_hash + salt)
    let mut combined2 = Vec::new();
    combined2.extend_from_slice(hex_hash.as_bytes());
    combined2.extend_from_slice(salt);
    let final_hash = md5::compute(&combined2);

    // Return with "md5" prefix
    format!("md5{:x}", final_hash)
}

/// SCRAM-SHA-256 authentication state
#[allow(dead_code)]
pub struct ScramAuth {
    #[allow(dead_code)]
    username: String,
    #[allow(dead_code)]
    client_nonce: String,
    server_nonce: String,
    salt: Vec<u8>,
    iterations: u32,
    stored_key: Vec<u8>,
    server_key: Vec<u8>,
    client_first_message_bare: String,
    server_first_message: String,
}

impl ScramAuth {
    /// Create new SCRAM authentication session
    pub fn new(
        username: String,
        client_nonce: String,
        salt: Vec<u8>,
        iterations: u32,
        stored_key: Vec<u8>,
        server_key: Vec<u8>,
    ) -> Self {
        // Generate server nonce by appending random data to client nonce
        let server_nonce_suffix: String = (0..16)
            .map(|_| {
                let ch = rand::thread_rng().gen_range(33..127) as u8;
                ch as char
            })
            .collect();
        let server_nonce = format!("{}{}", client_nonce, server_nonce_suffix);

        Self {
            username,
            client_nonce,
            server_nonce,
            salt,
            iterations,
            stored_key,
            server_key,
            client_first_message_bare: String::new(),
            server_first_message: String::new(),
        }
    }

    /// Process client-first-message and generate server-first-message
    pub fn process_client_first(&mut self, client_first: &str) -> ProtocolResult<String> {
        // Parse client-first-message: n,,n=username,r=clientnonce
        // Skip GS2 header (n,,)
        let parts: Vec<&str> = client_first.splitn(2, ",,").collect();
        if parts.len() != 2 {
            return Err(ProtocolError::AuthenticationError(
                "Invalid SCRAM client-first-message".to_string(),
            ));
        }

        self.client_first_message_bare = parts[1].to_string();

        // Build server-first-message: r=clientnonce+servernonce,s=salt,i=iterations
        self.server_first_message = format!(
            "r={},s={},i={}",
            self.server_nonce,
            BASE64.encode(&self.salt),
            self.iterations
        );

        Ok(self.server_first_message.clone())
    }

    /// Process client-final-message and generate server-final-message
    pub fn process_client_final(&self, client_final: &str) -> ProtocolResult<String> {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;

        // Parse client-final-message: c=biws,r=nonce,p=clientproof
        let mut channel_binding = None;
        let mut nonce = None;
        let mut proof = None;

        for part in client_final.split(',') {
            if let Some(value) = part.strip_prefix("c=") {
                channel_binding = Some(value);
            } else if let Some(value) = part.strip_prefix("r=") {
                nonce = Some(value);
            } else if let Some(value) = part.strip_prefix("p=") {
                proof = Some(value);
            }
        }

        let channel_binding = channel_binding.ok_or_else(|| {
            ProtocolError::AuthenticationError("Missing channel binding".to_string())
        })?;
        let nonce =
            nonce.ok_or_else(|| ProtocolError::AuthenticationError("Missing nonce".to_string()))?;
        let proof =
            proof.ok_or_else(|| ProtocolError::AuthenticationError("Missing proof".to_string()))?;

        // Verify nonce matches
        if nonce != self.server_nonce {
            return Err(ProtocolError::AuthenticationError(
                "Nonce mismatch".to_string(),
            ));
        }

        // Construct client-final-without-proof
        let client_final_without_proof = format!("c={},r={}", channel_binding, nonce);

        // Construct auth message
        let auth_message = format!(
            "{},{},{}",
            self.client_first_message_bare, self.server_first_message, client_final_without_proof
        );

        // Compute client signature: HMAC(stored_key, auth_message)
        let mut client_sig_mac = HmacSha256::new_from_slice(&self.stored_key).unwrap();
        client_sig_mac.update(auth_message.as_bytes());
        let client_signature = client_sig_mac.finalize().into_bytes();

        // Decode client proof
        let client_proof = BASE64.decode(proof).map_err(|_| {
            ProtocolError::AuthenticationError("Invalid client proof encoding".to_string())
        })?;

        // Compute client key: client_proof XOR client_signature
        if client_proof.len() != client_signature.len() {
            return Err(ProtocolError::AuthenticationError(
                "Invalid client proof length".to_string(),
            ));
        }

        let client_key: Vec<u8> = client_proof
            .iter()
            .zip(client_signature.iter())
            .map(|(a, b)| a ^ b)
            .collect();

        // Verify stored key matches H(client_key)
        let mut hasher = Sha256::new();
        hasher.update(&client_key);
        let computed_stored_key = hasher.finalize();

        if computed_stored_key.as_slice() != self.stored_key.as_slice() {
            return Err(ProtocolError::AuthenticationError(
                "Authentication failed".to_string(),
            ));
        }

        // Compute server signature: HMAC(server_key, auth_message)
        let mut server_sig_mac = HmacSha256::new_from_slice(&self.server_key).unwrap();
        server_sig_mac.update(auth_message.as_bytes());
        let server_signature = server_sig_mac.finalize().into_bytes();

        // Build server-final-message: v=serversignature
        Ok(format!("v={}", BASE64.encode(&server_signature)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5_hash() {
        // Test MD5 hash computation
        let password = "mypassword";
        let username = "myuser";
        let salt: [u8; 4] = [0x12, 0x34, 0x56, 0x78];

        let hash = compute_md5_hash(password, username, &salt);

        // Should start with "md5"
        assert!(hash.starts_with("md5"));
        assert_eq!(hash.len(), 35); // "md5" + 32 hex characters
    }

    #[tokio::test]
    async fn test_user_store() {
        let store = UserStore::new();

        // Add user with password auth
        store
            .add_user(
                "testuser".to_string(),
                "testpass".to_string(),
                &AuthMethod::Password,
            )
            .await;

        // Retrieve user
        let user = store.get_user("testuser").await;
        assert!(user.is_some());
        assert_eq!(user.unwrap().password_hash, "testpass");
    }

    #[tokio::test]
    async fn test_auth_manager_trust() {
        let store = UserStore::new();
        let manager = AuthManager::new(AuthMethod::Trust, store);

        // Trust always succeeds
        let result = manager.verify_password("anyuser", "anypass", None).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_auth_manager_password() {
        let store = UserStore::new();
        store
            .add_user(
                "testuser".to_string(),
                "correctpass".to_string(),
                &AuthMethod::Password,
            )
            .await;

        let manager = AuthManager::new(AuthMethod::Password, store);

        // Correct password
        let result = manager
            .verify_password("testuser", "correctpass", None)
            .await;
        assert!(result.unwrap());

        // Wrong password
        let result = manager.verify_password("testuser", "wrongpass", None).await;
        assert!(!result.unwrap());

        // Non-existent user
        let result = manager.verify_password("nouser", "anypass", None).await;
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_auth_manager_md5() {
        let store = UserStore::new();
        let password = "mypassword";
        let username = "myuser";

        store
            .add_user(username.to_string(), password.to_string(), &AuthMethod::MD5)
            .await;

        let manager = AuthManager::new(AuthMethod::MD5, store);

        // Generate salt and compute expected hash
        let salt: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
        let expected_hash = compute_md5_hash(password, username, &salt);

        // Verify with correct hash
        let result = manager
            .verify_password(username, &expected_hash, Some(&salt))
            .await;
        assert!(result.unwrap());

        // Verify with wrong hash
        let result = manager
            .verify_password(username, "md5wronghash", Some(&salt))
            .await;
        assert!(!result.unwrap());
    }
}
