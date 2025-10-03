use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Authentication token for transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token_id: String,
    pub issuer: String,
    pub subject: String,
    pub issued_at: SystemTime,
    pub expires_at: SystemTime,
    pub scopes: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl AuthToken {
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
    pub auth_token: Option<AuthToken>,
    pub node_id: NodeId,
    pub transaction_id: String,
    pub encryption_enabled: bool,
    pub signature_required: bool,
    pub audit_enabled: bool,
}

impl SecurityContext {
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

    pub fn with_auth_token(mut self, token: AuthToken) -> Self {
        self.auth_token = Some(token);
        self
    }

    pub fn with_encryption(mut self) -> Self {
        self.encryption_enabled = true;
        self
    }

    pub fn with_signature(mut self) -> Self {
        self.signature_required = true;
        self
    }

    pub fn with_audit(mut self, enabled: bool) -> Self {
        self.audit_enabled = enabled;
        self
    }
}

/// Required permissions for transaction operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionPermission {
    /// Can initiate transactions
    TransactionInitiate,
    /// Can commit transactions
    TransactionCommit,
    /// Can abort transactions
    TransactionAbort,
    /// Can read transaction state
    TransactionRead,
    /// Can participate in transactions
    TransactionParticipate,
    /// Can coordinate transactions
    TransactionCoordinate,
    /// Custom permission
    Custom(String),
}

impl TransactionPermission {
    pub fn to_scope(&self) -> String {
        match self {
            TransactionPermission::TransactionInitiate => "transaction:initiate".to_string(),
            TransactionPermission::TransactionCommit => "transaction:commit".to_string(),
            TransactionPermission::TransactionAbort => "transaction:abort".to_string(),
            TransactionPermission::TransactionRead => "transaction:read".to_string(),
            TransactionPermission::TransactionParticipate => {
                "transaction:participate".to_string()
            }
            TransactionPermission::TransactionCoordinate => "transaction:coordinate".to_string(),
            TransactionPermission::Custom(scope) => scope.clone(),
        }
    }
}

/// Authentication provider trait
#[async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Authenticate a request and return a token
    async fn authenticate(
        &self,
        credentials: &HashMap<String, String>,
    ) -> OrbitResult<AuthToken>;

    /// Validate an authentication token
    async fn validate_token(&self, token: &AuthToken) -> OrbitResult<bool>;

    /// Refresh an authentication token
    async fn refresh_token(&self, token: &AuthToken) -> OrbitResult<AuthToken>;

    /// Revoke an authentication token
    async fn revoke_token(&self, token_id: &str) -> OrbitResult<()>;
}

/// Authorization provider trait
#[async_trait]
pub trait AuthorizationProvider: Send + Sync {
    /// Check if a token has the required permissions
    async fn authorize(
        &self,
        token: &AuthToken,
        required_permissions: &[TransactionPermission],
    ) -> OrbitResult<bool>;

    /// Get effective permissions for a token
    async fn get_permissions(&self, token: &AuthToken) -> OrbitResult<Vec<TransactionPermission>>;
}

/// Simple in-memory authentication provider (for testing/demo)
pub struct InMemoryAuthProvider {
    users: Arc<RwLock<HashMap<String, (String, Vec<String>)>>>, // username -> (password, scopes)
    tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    token_ttl: Duration,
}

impl InMemoryAuthProvider {
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
    async fn authenticate(
        &self,
        credentials: &HashMap<String, String>,
    ) -> OrbitResult<AuthToken> {
        let username = credentials
            .get("username")
            .ok_or_else(|| OrbitError::unauthorized("Username required"))?;
        let password = credentials
            .get("password")
            .ok_or_else(|| OrbitError::unauthorized("Password required"))?;

        let users = self.users.read().await;
        let (stored_password, scopes) = users
            .get(username)
            .ok_or_else(|| OrbitError::unauthorized("Invalid credentials"))?;

        if stored_password != password {
            return Err(OrbitError::unauthorized("Invalid credentials"));
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

    async fn validate_token(&self, token: &AuthToken) -> OrbitResult<bool> {
        if !token.is_valid() {
            return Ok(false);
        }

        let tokens = self.tokens.read().await;
        Ok(tokens.contains_key(&token.token_id))
    }

    async fn refresh_token(&self, token: &AuthToken) -> OrbitResult<AuthToken> {
        if !self.validate_token(token).await? {
            return Err(OrbitError::unauthorized("Invalid token"));
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

    async fn revoke_token(&self, token_id: &str) -> OrbitResult<()> {
        let mut tokens = self.tokens.write().await;
        tokens.remove(token_id);
        debug!("Token revoked: {}", token_id);
        Ok(())
    }
}

/// Simple scope-based authorization provider
pub struct ScopeBasedAuthorizationProvider {}

impl ScopeBasedAuthorizationProvider {
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
    ) -> OrbitResult<bool> {
        if !token.is_valid() {
            return Ok(false);
        }

        let required_scopes: Vec<String> = required_permissions
            .iter()
            .map(|p| p.to_scope())
            .collect();

        Ok(token.has_all_scopes(&required_scopes))
    }

    async fn get_permissions(&self, token: &AuthToken) -> OrbitResult<Vec<TransactionPermission>> {
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
    auth_provider: Arc<dyn AuthenticationProvider>,
    authz_provider: Arc<dyn AuthorizationProvider>,
    require_authentication: bool,
    require_authorization: bool,
}

impl TransactionSecurityManager {
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

    pub fn with_authentication(mut self, enabled: bool) -> Self {
        self.require_authentication = enabled;
        self
    }

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
    ) -> OrbitResult<SecurityContext> {
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
    ) -> OrbitResult<bool> {
        if !self.require_authorization {
            return Ok(true);
        }

        let token = context
            .auth_token
            .as_ref()
            .ok_or_else(|| OrbitError::unauthorized("No authentication token"))?;

        self.authz_provider
            .authorize(token, required_permissions)
            .await
    }

    /// Validate security context
    pub async fn validate_context(&self, context: &SecurityContext) -> OrbitResult<bool> {
        if !self.require_authentication {
            return Ok(true);
        }

        let token = context
            .auth_token
            .as_ref()
            .ok_or_else(|| OrbitError::unauthorized("No authentication token"))?;

        self.auth_provider.validate_token(token).await
    }

    /// Create a security context for system operations
    pub fn create_system_context(&self, transaction_id: String, node_id: NodeId) -> SecurityContext {
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
    pub timestamp: SystemTime,
    pub transaction_id: String,
    pub operation: String,
    pub node_id: NodeId,
    pub user: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl AuditLogEntry {
    pub fn new(
        transaction_id: String,
        operation: String,
        node_id: NodeId,
        success: bool,
    ) -> Self {
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

    pub fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.error_message = Some(error);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Audit logger trait
#[async_trait]
pub trait AuditLogger: Send + Sync {
    /// Log an audit entry
    async fn log(&self, entry: AuditLogEntry) -> OrbitResult<()>;

    /// Query audit logs
    async fn query(
        &self,
        transaction_id: Option<String>,
        start_time: Option<SystemTime>,
        end_time: Option<SystemTime>,
        limit: usize,
    ) -> OrbitResult<Vec<AuditLogEntry>>;
}

/// In-memory audit logger (for testing)
pub struct InMemoryAuditLogger {
    entries: Arc<RwLock<Vec<AuditLogEntry>>>,
    max_entries: usize,
}

impl InMemoryAuditLogger {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            max_entries,
        }
    }
}

#[async_trait]
impl AuditLogger for InMemoryAuditLogger {
    async fn log(&self, entry: AuditLogEntry) -> OrbitResult<()> {
        let mut entries = self.entries.write().await;

        entries.push(entry);

        // Keep only the latest entries
        if entries.len() > self.max_entries {
            entries.drain(0..entries.len() - self.max_entries);
        }

        Ok(())
    }

    async fn query(
        &self,
        transaction_id: Option<String>,
        start_time: Option<SystemTime>,
        end_time: Option<SystemTime>,
        limit: usize,
    ) -> OrbitResult<Vec<AuditLogEntry>> {
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
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
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
            vec!["transaction:initiate".to_string(), "transaction:commit".to_string()],
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
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());

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
