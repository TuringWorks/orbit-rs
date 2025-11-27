use crate::cluster::NodeId;
use crate::error::{EngineError, EngineResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Type alias for user credentials storage (username -> (password, scopes))
type UserCredentials = HashMap<String, (String, Vec<String>)>;

/// Authentication token for transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// Unique identifier for this token
    pub token_id: String,
    /// Entity that issued this token
    pub issuer: String,
    /// Subject (user/entity) this token is for
    pub subject: String,
    /// Timestamp when the token was created
    pub issued_at: SystemTime,
    /// Timestamp when the token expires
    pub expires_at: SystemTime,
    /// Scopes (permissions) granted by this token
    pub scopes: Vec<String>,
    /// Additional metadata associated with the token
    pub metadata: HashMap<String, String>,
}

impl AuthToken {
    /// Creates a new authentication token with the given issuer, subject, scopes, and TTL.
    pub fn new(issuer: String, subject: String, scopes: Vec<String>, ttl: Duration) -> Self {
        let now = SystemTime::now();
        Self {
            token_id: uuid::Uuid::new_v4().to_string(),
            issuer,
            subject,
            issued_at: now,
            expires_at: now + ttl,
            scopes,
            metadata: HashMap::new(),
        }
    }

    /// Check if token is valid
    pub fn is_valid(&self) -> bool {
        SystemTime::now() < self.expires_at
    }

    /// Check if token has a specific scope
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Check if token has all required scopes
    pub fn has_all_scopes(&self, required_scopes: &[String]) -> bool {
        required_scopes.iter().all(|req| self.has_scope(req))
    }

    /// Check if token has any of the required scopes
    pub fn has_any_scope(&self, required_scopes: &[String]) -> bool {
        required_scopes.iter().any(|req| self.has_scope(req))
    }
}

/// Transaction security context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Authentication token for this security context
    pub auth_token: Option<AuthToken>,
    /// Node identifier for this context
    pub node_id: NodeId,
    /// Transaction identifier
    pub transaction_id: String,
    /// Whether encryption is enabled for this transaction
    pub encryption_enabled: bool,
    /// Whether signature verification is required
    pub signature_required: bool,
    /// Whether audit logging is enabled
    pub audit_enabled: bool,
}

impl SecurityContext {
    /// Creates a new security context for a transaction on the given node.
    pub fn new(transaction_id: String, node_id: NodeId) -> Self {
        Self {
            auth_token: None,
            node_id,
            transaction_id,
            encryption_enabled: false,
            signature_required: false,
            audit_enabled: true,
        }
    }

    /// Sets the authentication token for this context.
    pub fn with_auth_token(mut self, token: AuthToken) -> Self {
        self.auth_token = Some(token);
        self
    }

    /// Enables encryption for transactions in this context.
    pub fn with_encryption(mut self) -> Self {
        self.encryption_enabled = true;
        self
    }

    /// Requires signature verification for transactions in this context.
    pub fn with_signature(mut self) -> Self {
        self.signature_required = true;
        self
    }

    /// Sets whether audit logging is enabled for this context.
    pub fn with_audit(mut self, enabled: bool) -> Self {
        self.audit_enabled = enabled;
        self
    }
}

/// Required permissions for transaction operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionPermission {
    /// Permission to initiate transactions
    TransactionInitiate,
    /// Permission to commit transactions
    TransactionCommit,
    /// Permission to abort transactions
    TransactionAbort,
    /// Permission to read transaction state
    TransactionRead,
    /// Permission to participate in transactions
    TransactionParticipate,
    /// Permission to coordinate transactions
    TransactionCoordinate,
    /// Custom permission with arbitrary scope
    Custom(String),
}

impl TransactionPermission {
    /// Converts this permission to its scope string representation.
    pub fn to_scope(&self) -> String {
        match self {
            TransactionPermission::TransactionInitiate => "transaction:initiate".to_string(),
            TransactionPermission::TransactionCommit => "transaction:commit".to_string(),
            TransactionPermission::TransactionAbort => "transaction:abort".to_string(),
            TransactionPermission::TransactionRead => "transaction:read".to_string(),
            TransactionPermission::TransactionParticipate => "transaction:participate".to_string(),
            TransactionPermission::TransactionCoordinate => "transaction:coordinate".to_string(),
            TransactionPermission::Custom(scope) => scope.clone(),
        }
    }
}

/// Authentication provider trait
#[async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Authenticate a request and return a token
    async fn authenticate(&self, credentials: &HashMap<String, String>) -> EngineResult<AuthToken>;

    /// Validate an authentication token
    async fn validate_token(&self, token: &AuthToken) -> EngineResult<bool>;

    /// Refresh an authentication token
    async fn refresh_token(&self, token: &AuthToken) -> EngineResult<AuthToken>;

    /// Revoke an authentication token
    async fn revoke_token(&self, token_id: &str) -> EngineResult<()>;
}

/// Authorization provider trait
#[async_trait]
pub trait AuthorizationProvider: Send + Sync {
    /// Check if a token has the required permissions
    async fn authorize(
        &self,
        token: &AuthToken,
        required_permissions: &[TransactionPermission],
    ) -> EngineResult<bool>;

    /// Get effective permissions for a token
    async fn get_permissions(&self, token: &AuthToken) -> EngineResult<Vec<TransactionPermission>>;
}

/// Simple in-memory authentication provider (for testing/demo)
pub struct InMemoryAuthProvider {
    /// Stored user credentials
    users: Arc<RwLock<UserCredentials>>,
    /// Issued tokens mapped by token ID
    tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    /// Time-to-live duration for issued tokens
    token_ttl: Duration,
}

impl InMemoryAuthProvider {
    /// Creates a new in-memory authentication provider with the given token TTL.
    pub fn new(token_ttl: Duration) -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
            token_ttl,
        }
    }

    /// Add a user (for testing/setup)
    pub async fn add_user(&self, username: String, password: String, scopes: Vec<String>) {
        let mut users = self.users.write().await;
        users.insert(username, (password, scopes));
    }
}

#[async_trait]
impl AuthenticationProvider for InMemoryAuthProvider {
    async fn authenticate(&self, credentials: &HashMap<String, String>) -> EngineResult<AuthToken> {
        let username = credentials
            .get("username")
            .ok_or_else(|| EngineError::internal("Username required"))?;
        let password = credentials
            .get("password")
            .ok_or_else(|| EngineError::internal("Password required"))?;

        let users = self.users.read().await;
        let (stored_password, scopes) = users
            .get(username)
            .ok_or_else(|| EngineError::internal("Invalid credentials"))?;

        if stored_password != password {
            return Err(EngineError::internal("Invalid credentials"));
        }

        let token = AuthToken::new(
            "orbit-auth".to_string(),
            username.clone(),
            scopes.clone(),
            self.token_ttl,
        );

        let mut tokens = self.tokens.write().await;
        tokens.insert(token.token_id.clone(), token.clone());

        info!("User authenticated: {}", username);
        Ok(token)
    }

    async fn validate_token(&self, token: &AuthToken) -> EngineResult<bool> {
        if !token.is_valid() {
            return Ok(false);
        }

        let tokens = self.tokens.read().await;
        Ok(tokens.contains_key(&token.token_id))
    }

    async fn refresh_token(&self, token: &AuthToken) -> EngineResult<AuthToken> {
        if !self.validate_token(token).await? {
            return Err(EngineError::internal("Invalid token"));
        }

        let new_token = AuthToken::new(
            token.issuer.clone(),
            token.subject.clone(),
            token.scopes.clone(),
            self.token_ttl,
        );

        let mut tokens = self.tokens.write().await;
        tokens.remove(&token.token_id);
        tokens.insert(new_token.token_id.clone(), new_token.clone());

        debug!("Token refreshed for user: {}", token.subject);
        Ok(new_token)
    }

    async fn revoke_token(&self, token_id: &str) -> EngineResult<()> {
        let mut tokens = self.tokens.write().await;
        tokens.remove(token_id);
        debug!("Token revoked: {}", token_id);
        Ok(())
    }
}

/// Simple scope-based authorization provider
pub struct ScopeBasedAuthorizationProvider {}

impl ScopeBasedAuthorizationProvider {
    /// Creates a new scope-based authorization provider.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ScopeBasedAuthorizationProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthorizationProvider for ScopeBasedAuthorizationProvider {
    async fn authorize(
        &self,
        token: &AuthToken,
        required_permissions: &[TransactionPermission],
    ) -> EngineResult<bool> {
        if !token.is_valid() {
            return Ok(false);
        }

        let required_scopes: Vec<String> =
            required_permissions.iter().map(|p| p.to_scope()).collect();

        Ok(token.has_all_scopes(&required_scopes))
    }

    async fn get_permissions(&self, token: &AuthToken) -> EngineResult<Vec<TransactionPermission>> {
        if !token.is_valid() {
            return Ok(Vec::new());
        }

        let permissions: Vec<TransactionPermission> = token
            .scopes
            .iter()
            .map(|scope| match scope.as_str() {
                "transaction:initiate" => TransactionPermission::TransactionInitiate,
                "transaction:commit" => TransactionPermission::TransactionCommit,
                "transaction:abort" => TransactionPermission::TransactionAbort,
                "transaction:read" => TransactionPermission::TransactionRead,
                "transaction:participate" => TransactionPermission::TransactionParticipate,
                "transaction:coordinate" => TransactionPermission::TransactionCoordinate,
                _ => TransactionPermission::Custom(scope.clone()),
            })
            .collect();

        Ok(permissions)
    }
}

/// Transaction security manager
pub struct TransactionSecurityManager {
    /// Authentication provider instance
    auth_provider: Arc<dyn AuthenticationProvider>,
    /// Authorization provider instance
    authz_provider: Arc<dyn AuthorizationProvider>,
    /// Whether authentication is required
    require_authentication: bool,
    /// Whether authorization is required
    require_authorization: bool,
}

impl TransactionSecurityManager {
    /// Creates a new transaction security manager with the given providers.
    pub fn new(
        auth_provider: Arc<dyn AuthenticationProvider>,
        authz_provider: Arc<dyn AuthorizationProvider>,
    ) -> Self {
        Self {
            auth_provider,
            authz_provider,
            require_authentication: true,
            require_authorization: true,
        }
    }

    /// Sets whether authentication is required.
    pub fn with_authentication(mut self, enabled: bool) -> Self {
        self.require_authentication = enabled;
        self
    }

    /// Sets whether authorization is required.
    pub fn with_authorization(mut self, enabled: bool) -> Self {
        self.require_authorization = enabled;
        self
    }

    /// Authenticate credentials and create security context
    pub async fn authenticate(
        &self,
        credentials: &HashMap<String, String>,
        transaction_id: String,
        node_id: NodeId,
    ) -> EngineResult<SecurityContext> {
        if !self.require_authentication {
            return Ok(SecurityContext::new(transaction_id, node_id));
        }

        let token = self.auth_provider.authenticate(credentials).await?;

        Ok(SecurityContext::new(transaction_id, node_id).with_auth_token(token))
    }

    /// Check if security context has required permissions
    pub async fn check_permissions(
        &self,
        context: &SecurityContext,
        required_permissions: &[TransactionPermission],
    ) -> EngineResult<bool> {
        if !self.require_authorization {
            return Ok(true);
        }

        let token = context
            .auth_token
            .as_ref()
            .ok_or_else(|| EngineError::internal("No authentication token"))?;

        self.authz_provider
            .authorize(token, required_permissions)
            .await
    }

    /// Validate security context
    pub async fn validate_context(&self, context: &SecurityContext) -> EngineResult<bool> {
        if !self.require_authentication {
            return Ok(true);
        }

        let token = context
            .auth_token
            .as_ref()
            .ok_or_else(|| EngineError::internal("No authentication token"))?;

        self.auth_provider.validate_token(token).await
    }

    /// Create a security context for system operations
    pub fn create_system_context(
        &self,
        transaction_id: String,
        node_id: NodeId,
    ) -> SecurityContext {
        let token = AuthToken::new(
            "orbit-system".to_string(),
            "system".to_string(),
            vec![
                "transaction:initiate".to_string(),
                "transaction:commit".to_string(),
                "transaction:abort".to_string(),
                "transaction:read".to_string(),
                "transaction:participate".to_string(),
                "transaction:coordinate".to_string(),
            ],
            Duration::from_secs(3600),
        );

        SecurityContext::new(transaction_id, node_id).with_auth_token(token)
    }
}

/// Audit log entry for transaction operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Timestamp when this operation occurred
    pub timestamp: SystemTime,
    /// Transaction ID for this operation
    pub transaction_id: String,
    /// Type of operation performed
    pub operation: String,
    /// Node where the operation was performed
    pub node_id: NodeId,
    /// User who performed the operation, if available
    pub user: Option<String>,
    /// Whether the operation succeeded
    pub success: bool,
    /// Error message if the operation failed
    pub error_message: Option<String>,
    /// Additional metadata about the operation
    pub metadata: HashMap<String, String>,
}

impl AuditLogEntry {
    /// Creates a new audit log entry for the given transaction and operation.
    pub fn new(transaction_id: String, operation: String, node_id: NodeId, success: bool) -> Self {
        Self {
            timestamp: SystemTime::now(),
            transaction_id,
            operation,
            node_id,
            user: None,
            success,
            error_message: None,
            metadata: HashMap::new(),
        }
    }

    /// Sets the user who performed the operation.
    pub fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }

    /// Sets the error message for a failed operation.
    pub fn with_error(mut self, error: String) -> Self {
        self.error_message = Some(error);
        self
    }

    /// Adds metadata to this audit entry.
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Audit logger trait
#[async_trait]
pub trait AuditLogger: Send + Sync {
    /// Log an audit entry
    async fn log(&self, entry: AuditLogEntry) -> EngineResult<()>;

    /// Query audit logs
    async fn query(
        &self,
        transaction_id: Option<String>,
        start_time: Option<SystemTime>,
        end_time: Option<SystemTime>,
        limit: usize,
    ) -> EngineResult<Vec<AuditLogEntry>>;
}

/// In-memory audit logger (for testing)
pub struct InMemoryAuditLogger {
    /// Stored audit log entries
    entries: Arc<RwLock<Vec<AuditLogEntry>>>,
    /// Maximum number of entries to keep
    max_entries: usize,
}

impl InMemoryAuditLogger {
    /// Creates a new in-memory audit logger with the given maximum entry count.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            max_entries,
        }
    }
}

#[async_trait]
impl AuditLogger for InMemoryAuditLogger {
    async fn log(&self, entry: AuditLogEntry) -> EngineResult<()> {
        let mut entries = self.entries.write().await;

        entries.push(entry);

        // Keep only the latest entries
        if entries.len() > self.max_entries {
            let excess = entries.len() - self.max_entries;
            entries.drain(0..excess);
        }

        Ok(())
    }

    async fn query(
        &self,
        transaction_id: Option<String>,
        start_time: Option<SystemTime>,
        end_time: Option<SystemTime>,
        limit: usize,
    ) -> EngineResult<Vec<AuditLogEntry>> {
        let entries = self.entries.read().await;

        let filtered: Vec<AuditLogEntry> = entries
            .iter()
            .filter(|e| {
                if let Some(ref tx_id) = transaction_id {
                    if &e.transaction_id != tx_id {
                        return false;
                    }
                }
                if let Some(start) = start_time {
                    if e.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_time {
                    if e.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .cloned()
            .collect();

        Ok(filtered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_token_creation() {
        let token = AuthToken::new(
            "test-issuer".to_string(),
            "test-user".to_string(),
            vec!["transaction:initiate".to_string()],
            Duration::from_secs(3600),
        );

        assert!(token.is_valid());
        assert!(token.has_scope("transaction:initiate"));
        assert!(!token.has_scope("transaction:commit"));
    }

    #[test]
    fn test_security_context() {
        let node_id = "test-node".to_string();
        let token = AuthToken::new(
            "test-issuer".to_string(),
            "test-user".to_string(),
            vec!["transaction:initiate".to_string()],
            Duration::from_secs(3600),
        );

        let context = SecurityContext::new("test-tx".to_string(), node_id)
            .with_auth_token(token)
            .with_encryption()
            .with_signature();

        assert!(context.auth_token.is_some());
        assert!(context.encryption_enabled);
        assert!(context.signature_required);
    }

    #[tokio::test]
    async fn test_in_memory_auth_provider() {
        let provider = InMemoryAuthProvider::new(Duration::from_secs(3600));

        provider
            .add_user(
                "test-user".to_string(),
                "test-password".to_string(),
                vec!["transaction:initiate".to_string()],
            )
            .await;

        let mut credentials = HashMap::new();
        credentials.insert("username".to_string(), "test-user".to_string());
        credentials.insert("password".to_string(), "test-password".to_string());

        let token = provider.authenticate(&credentials).await.unwrap();
        assert!(token.is_valid());
        assert_eq!(token.subject, "test-user");

        let is_valid = provider.validate_token(&token).await.unwrap();
        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_scope_based_authorization() {
        let provider = ScopeBasedAuthorizationProvider::new();

        let token = AuthToken::new(
            "test-issuer".to_string(),
            "test-user".to_string(),
            vec![
                "transaction:initiate".to_string(),
                "transaction:commit".to_string(),
            ],
            Duration::from_secs(3600),
        );

        let authorized = provider
            .authorize(
                &token,
                &[
                    TransactionPermission::TransactionInitiate,
                    TransactionPermission::TransactionCommit,
                ],
            )
            .await
            .unwrap();
        assert!(authorized);

        let not_authorized = provider
            .authorize(&token, &[TransactionPermission::TransactionCoordinate])
            .await
            .unwrap();
        assert!(!not_authorized);
    }

    #[tokio::test]
    async fn test_audit_logger() {
        let logger = InMemoryAuditLogger::new(100);
        let node_id = "test-node".to_string();

        let entry = AuditLogEntry::new(
            "test-tx".to_string(),
            "commit".to_string(),
            node_id.clone(),
            true,
        )
        .with_user("test-user".to_string());

        logger.log(entry).await.unwrap();

        let entries = logger
            .query(Some("test-tx".to_string()), None, None, 10)
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].transaction_id, "test-tx");
    }
}
