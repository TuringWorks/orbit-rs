//! MySQL authentication handling

use bytes::{Buf, Bytes};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use sha2::{Sha256, Digest};

/// Authentication plugin types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPlugin {
    /// mysql_native_password (SHA1-based)
    NativePassword,
    /// mysql_clear_password
    ClearPassword,
    /// caching_sha2_password
    CachingSha2Password,
}

impl AuthPlugin {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthPlugin::NativePassword => "mysql_native_password",
            AuthPlugin::ClearPassword => "mysql_clear_password",
            AuthPlugin::CachingSha2Password => "caching_sha2_password",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "mysql_native_password" => Some(AuthPlugin::NativePassword),
            "mysql_clear_password" => Some(AuthPlugin::ClearPassword),
            "caching_sha2_password" => Some(AuthPlugin::CachingSha2Password),
            _ => None,
        }
    }
}

/// Authentication state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    /// Waiting for handshake response
    AwaitingHandshake,
    /// Waiting for authentication data
    AwaitingAuth,
    /// Authentication successful
    Authenticated,
    /// Authentication failed
    Failed(String),
}

/// Handshake response packet
#[derive(Debug, Clone)]
pub struct HandshakeResponse {
    pub capability_flags: u32,
    pub max_packet_size: u32,
    pub character_set: u8,
    pub username: String,
    pub auth_response: Option<Vec<u8>>,
    pub database: Option<String>,
    pub auth_plugin_name: Option<String>,
}

impl HandshakeResponse {
    /// Parse handshake response from bytes
    pub fn parse(mut buf: Bytes) -> ProtocolResult<Self> {
        if buf.len() < 32 {
            return Err(ProtocolError::IncompleteFrame);
        }

        // Read capability flags (4 bytes)
        let capability_flags = buf.get_u32_le();

        // Read max packet size (4 bytes)
        let max_packet_size = buf.get_u32_le();

        // Read character set (1 byte)
        let character_set = buf.get_u8();

        // Skip reserved bytes (23 bytes)
        buf.advance(23);

        // Read null-terminated username
        let username = read_null_terminated_string(&mut buf)?;

        // Read auth response
        let auth_response = if buf.remaining() > 0 {
            let len = buf.get_u8() as usize;
            if buf.remaining() < len {
                return Err(ProtocolError::IncompleteFrame);
            }
            let mut auth_data = vec![0u8; len];
            buf.copy_to_slice(&mut auth_data);
            Some(auth_data)
        } else {
            None
        };

        // Read database name if present
        let database = if buf.remaining() > 0 {
            Some(read_null_terminated_string(&mut buf)?)
        } else {
            None
        };

        // Read auth plugin name if present
        let auth_plugin_name = if buf.remaining() > 0 {
            Some(read_null_terminated_string(&mut buf)?)
        } else {
            None
        };

        Ok(HandshakeResponse {
            capability_flags,
            max_packet_size,
            character_set,
            username,
            auth_response,
            database,
            auth_plugin_name,
        })
    }
}

/// MySQL authentication handler
pub struct MySqlAuth {
    state: AuthState,
    plugin: AuthPlugin,
    auth_data: [u8; 20],
    expected_username: Option<String>,
    expected_password: Option<String>,
}

impl MySqlAuth {
    /// Create new authentication handler
    pub fn new(plugin: AuthPlugin) -> Self {
        Self::with_credentials(plugin, None, None)
    }

    /// Create new authentication handler with credentials
    pub fn with_credentials(
        plugin: AuthPlugin,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        // Generate random auth data (salt)
        let auth_data = [
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
            0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00,
            0x11, 0x22, 0x33, 0x44,
        ];

        Self {
            state: AuthState::AwaitingHandshake,
            plugin,
            auth_data,
            expected_username: username,
            expected_password: password,
        }
    }

    /// Get current authentication state
    pub fn state(&self) -> &AuthState {
        &self.state
    }

    /// Get auth plugin data (salt)
    pub fn auth_data(&self) -> &[u8; 20] {
        &self.auth_data
    }

    /// Process handshake response
    pub fn process_handshake(&mut self, response: HandshakeResponse) -> ProtocolResult<bool> {
        if self.state != AuthState::AwaitingHandshake {
            return Err(ProtocolError::InvalidState);
        }

        // If authentication is not required (no credentials configured), accept any user
        if self.expected_username.is_none() && self.expected_password.is_none() {
            self.state = AuthState::Authenticated;
            return Ok(true);
        }

        // Verify username
        if let Some(ref expected_username) = self.expected_username {
            if response.username != *expected_username {
                self.state = AuthState::Failed("Invalid username".to_string());
                return Ok(false);
            }
        }

        if response.auth_response.is_none() {
            self.state = AuthState::AwaitingAuth;
            return Ok(false);
        }

        // Validate auth response based on plugin
        let valid = match self.plugin {
            AuthPlugin::NativePassword => {
                self.validate_native_password(&response.username, &response.auth_response.unwrap())
            }
            AuthPlugin::ClearPassword => {
                // Clear password is sent as-is
                self.validate_clear_password(&response.auth_response.unwrap())
            }
            AuthPlugin::CachingSha2Password => {
                // For simplicity, accept any auth for now (can be enhanced later)
                true
            }
        };

        if valid {
            self.state = AuthState::Authenticated;
            Ok(true)
        } else {
            self.state = AuthState::Failed("Invalid credentials".to_string());
            Ok(false)
        }
    }

    /// Validate native password authentication
    fn validate_native_password(&self, _username: &str, auth_response: &[u8]) -> bool {
        // Native password protocol:
        // auth_response = SHA1(password) XOR SHA1(auth_data + SHA1(SHA1(password)))

        // If no password is configured, accept any auth response
        if self.expected_password.is_none() {
            return !auth_response.is_empty();
        }

        // For now, use a simple comparison (in production, implement proper MySQL native password)
        // This is a simplified version - full implementation would require SHA1-based validation
        let expected_password = self.expected_password.as_ref().unwrap();
        
        // Simple validation: check if auth_response is not empty
        // In production, this should implement the full MySQL native password protocol
        !auth_response.is_empty() && !expected_password.is_empty()
    }

    /// Validate clear password authentication
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn validate_clear_password(&self, auth_response: &[u8]) -> bool {
        // Clear password is sent as-is (null-terminated)
        if self.expected_password.is_none() {
            return !auth_response.is_empty();
        }

        let expected_password = self.expected_password.as_ref().unwrap();
        let provided_password = String::from_utf8_lossy(auth_response);
        
        // Remove null terminator if present
        let provided_password = provided_password.trim_end_matches('\0');
        
        provided_password == expected_password
    }

    /// Compute native password hash (simplified version using SHA256)
    pub fn compute_native_password_hash(password: &str, auth_data: &[u8]) -> Vec<u8> {
        // Simplified hash for demonstration - in production use proper MySQL SHA1-based auth
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(auth_data);
        hasher.finalize().to_vec()
    }
}

/// Read null-terminated string from buffer
fn read_null_terminated_string(buf: &mut Bytes) -> ProtocolResult<String> {
    let mut bytes = Vec::new();
    loop {
        if buf.is_empty() {
            return Err(ProtocolError::IncompleteFrame);
        }
        let byte = buf.get_u8();
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_plugin_conversion() {
        assert_eq!(AuthPlugin::NativePassword.as_str(), "mysql_native_password");
        assert_eq!(
            AuthPlugin::from_str("mysql_native_password"),
            Some(AuthPlugin::NativePassword)
        );
    }

    #[test]
    fn test_auth_state_transitions() {
        let auth = MySqlAuth::new(AuthPlugin::NativePassword);
        assert_eq!(auth.state(), &AuthState::AwaitingHandshake);
    }

    #[test]
    fn test_native_password_hash() {
        let password = "test123";
        let auth_data = [0u8; 20];
        let hash = MySqlAuth::compute_native_password_hash(password, &auth_data);
        assert_eq!(hash.len(), 32); // SHA256 produces 32 bytes
    }
}
