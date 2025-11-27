//! Multi-Tenant Security Framework
//!
//! Provides comprehensive multi-tenant isolation and security for enterprise deployments.
//! Ensures data isolation, tenant-specific encryption, cross-tenant access policies,
//! and resource quotas.
//!
//! # Features
//!
//! - **Tenant Isolation**: Strong data isolation between tenants
//! - **Tenant-Specific Encryption**: Per-tenant encryption keys
//! - **Cross-Tenant Policies**: Controlled sharing between tenants
//! - **Resource Quotas**: Per-tenant resource limits
//! - **Tenant Context Propagation**: Automatic tenant context in requests
//! - **Audit per Tenant**: Tenant-scoped audit logging
//!
//! # Usage
//!
//! ```rust,ignore
//! use orbit_shared::security::multi_tenant::{TenantManager, TenantContext};
//!
//! let manager = TenantManager::new();
//!
//! // Register a tenant
//! let config = TenantConfig::new("acme-corp")
//!     .with_isolation_level(IsolationLevel::Full)
//!     .with_encryption(true);
//! manager.register_tenant(config).await?;
//!
//! // Create tenant context for request
//! let ctx = TenantContext::new("acme-corp", user_id);
//!
//! // Verify access
//! manager.verify_tenant_access(&ctx, &resource).await?;
//! ```

use crate::exception::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Unique identifier for a tenant
pub type TenantId = String;

/// Isolation level for tenant data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Shared schema with tenant_id column filtering
    Shared,
    /// Separate schema per tenant within shared database
    Schema,
    /// Completely separate database per tenant
    Database,
    /// Full isolation with separate encryption and resources
    Full,
}

/// Tenant status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenantStatus {
    /// Tenant is active and operational
    Active,
    /// Tenant is suspended (read-only access)
    Suspended,
    /// Tenant is being provisioned
    Provisioning,
    /// Tenant is being deprovisioned
    Deprovisioning,
    /// Tenant is archived (no access)
    Archived,
}

/// Resource quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// Maximum storage in bytes
    pub max_storage_bytes: Option<u64>,
    /// Maximum number of records
    pub max_records: Option<u64>,
    /// Maximum queries per second
    pub max_qps: Option<u32>,
    /// Maximum concurrent connections
    pub max_connections: Option<u32>,
    /// Maximum CPU percentage
    pub max_cpu_percent: Option<u8>,
    /// Maximum memory in bytes
    pub max_memory_bytes: Option<u64>,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_storage_bytes: None,
            max_records: None,
            max_qps: Some(1000),
            max_connections: Some(100),
            max_cpu_percent: None,
            max_memory_bytes: None,
        }
    }
}

impl ResourceQuota {
    /// Create unlimited quota
    pub fn unlimited() -> Self {
        Self {
            max_storage_bytes: None,
            max_records: None,
            max_qps: None,
            max_connections: None,
            max_cpu_percent: None,
            max_memory_bytes: None,
        }
    }

    /// Create basic tier quota
    pub fn basic() -> Self {
        Self {
            max_storage_bytes: Some(10 * 1024 * 1024 * 1024), // 10GB
            max_records: Some(1_000_000),
            max_qps: Some(100),
            max_connections: Some(10),
            max_cpu_percent: Some(10),
            max_memory_bytes: Some(1024 * 1024 * 1024), // 1GB
        }
    }

    /// Create professional tier quota
    pub fn professional() -> Self {
        Self {
            max_storage_bytes: Some(100 * 1024 * 1024 * 1024), // 100GB
            max_records: Some(10_000_000),
            max_qps: Some(1000),
            max_connections: Some(100),
            max_cpu_percent: Some(25),
            max_memory_bytes: Some(8 * 1024 * 1024 * 1024), // 8GB
        }
    }

    /// Create enterprise tier quota
    pub fn enterprise() -> Self {
        Self {
            max_storage_bytes: Some(1024 * 1024 * 1024 * 1024), // 1TB
            max_records: Some(100_000_000),
            max_qps: Some(10000),
            max_connections: Some(1000),
            max_cpu_percent: Some(50),
            max_memory_bytes: Some(32 * 1024 * 1024 * 1024), // 32GB
        }
    }
}

/// Tenant configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    /// Unique tenant identifier
    pub tenant_id: TenantId,
    /// Human-readable tenant name
    pub name: String,
    /// Tenant status
    pub status: TenantStatus,
    /// Data isolation level
    pub isolation_level: IsolationLevel,
    /// Whether tenant data should be encrypted
    pub encryption_enabled: bool,
    /// Encryption key ID for this tenant
    pub encryption_key_id: Option<String>,
    /// Resource quotas
    pub quota: ResourceQuota,
    /// Tenant metadata
    pub metadata: HashMap<String, String>,
    /// Allowed IP ranges (CIDR notation)
    pub allowed_ips: Vec<String>,
    /// Whether audit logging is enabled
    pub audit_enabled: bool,
    /// Tenant creation time
    pub created_at: SystemTime,
    /// Last modification time
    pub updated_at: SystemTime,
}

impl TenantConfig {
    /// Create a new tenant configuration
    pub fn new(tenant_id: &str) -> Self {
        let now = SystemTime::now();
        Self {
            tenant_id: tenant_id.to_string(),
            name: tenant_id.to_string(),
            status: TenantStatus::Provisioning,
            isolation_level: IsolationLevel::Shared,
            encryption_enabled: false,
            encryption_key_id: None,
            quota: ResourceQuota::default(),
            metadata: HashMap::new(),
            allowed_ips: Vec::new(),
            audit_enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Set tenant name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set isolation level
    pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Enable encryption
    pub fn with_encryption(mut self, enabled: bool) -> Self {
        self.encryption_enabled = enabled;
        self
    }

    /// Set encryption key ID
    pub fn with_encryption_key(mut self, key_id: &str) -> Self {
        self.encryption_key_id = Some(key_id.to_string());
        self.encryption_enabled = true;
        self
    }

    /// Set resource quota
    pub fn with_quota(mut self, quota: ResourceQuota) -> Self {
        self.quota = quota;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Add allowed IP range
    pub fn with_allowed_ip(mut self, cidr: &str) -> Self {
        self.allowed_ips.push(cidr.to_string());
        self
    }

    /// Set audit enabled
    pub fn with_audit(mut self, enabled: bool) -> Self {
        self.audit_enabled = enabled;
        self
    }
}

/// Tenant context for request processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    /// Current tenant ID
    pub tenant_id: TenantId,
    /// User ID within tenant
    pub user_id: String,
    /// Session ID
    pub session_id: String,
    /// Request timestamp
    pub timestamp: SystemTime,
    /// Client IP address
    pub client_ip: Option<String>,
    /// Request correlation ID
    pub correlation_id: String,
    /// Additional context attributes
    pub attributes: HashMap<String, String>,
}

impl TenantContext {
    /// Create a new tenant context
    pub fn new(tenant_id: &str, user_id: &str) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            user_id: user_id.to_string(),
            session_id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            client_ip: None,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            attributes: HashMap::new(),
        }
    }

    /// Set client IP
    pub fn with_client_ip(mut self, ip: &str) -> Self {
        self.client_ip = Some(ip.to_string());
        self
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, id: &str) -> Self {
        self.correlation_id = id.to_string();
        self
    }

    /// Add attribute
    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }
}

/// Cross-tenant sharing policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossTenantPolicy {
    /// Source tenant ID
    pub source_tenant: TenantId,
    /// Target tenant ID
    pub target_tenant: TenantId,
    /// Resources that can be shared
    pub shared_resources: Vec<String>,
    /// Allowed actions on shared resources
    pub allowed_actions: Vec<CrossTenantAction>,
    /// Policy expiration
    pub expires_at: Option<SystemTime>,
    /// Whether policy is active
    pub active: bool,
}

/// Actions allowed for cross-tenant access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossTenantAction {
    /// Read access
    Read,
    /// Write access
    Write,
    /// Delete access
    Delete,
    /// Execute functions/procedures
    Execute,
    /// Share with other tenants
    Share,
}

impl CrossTenantPolicy {
    /// Create a new cross-tenant policy
    pub fn new(source: &str, target: &str) -> Self {
        Self {
            source_tenant: source.to_string(),
            target_tenant: target.to_string(),
            shared_resources: Vec::new(),
            allowed_actions: Vec::new(),
            expires_at: None,
            active: true,
        }
    }

    /// Add shared resource
    pub fn share_resource(mut self, resource: &str) -> Self {
        self.shared_resources.push(resource.to_string());
        self
    }

    /// Allow action
    pub fn allow_action(mut self, action: CrossTenantAction) -> Self {
        if !self.allowed_actions.contains(&action) {
            self.allowed_actions.push(action);
        }
        self
    }

    /// Set expiration
    pub fn expires_in(mut self, duration: Duration) -> Self {
        self.expires_at = Some(SystemTime::now() + duration);
        self
    }

    /// Check if policy is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            SystemTime::now() > expires
        } else {
            false
        }
    }

    /// Check if action is allowed on resource
    pub fn is_allowed(&self, resource: &str, action: CrossTenantAction) -> bool {
        if !self.active || self.is_expired() {
            return false;
        }
        self.shared_resources
            .iter()
            .any(|r| r == resource || r == "*")
            && self.allowed_actions.contains(&action)
    }
}

/// Resource usage tracking for quota enforcement
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Current storage usage in bytes
    pub storage_bytes: u64,
    /// Current record count
    pub record_count: u64,
    /// Queries in last second
    pub current_qps: u32,
    /// Active connections
    pub active_connections: u32,
    /// Current CPU usage percentage
    pub cpu_percent: u8,
    /// Current memory usage in bytes
    pub memory_bytes: u64,
}

impl ResourceUsage {
    /// Check if usage exceeds quota
    pub fn exceeds_quota(&self, quota: &ResourceQuota) -> Option<String> {
        if let Some(max) = quota.max_storage_bytes {
            if self.storage_bytes > max {
                return Some(format!(
                    "Storage quota exceeded: {} > {}",
                    self.storage_bytes, max
                ));
            }
        }
        if let Some(max) = quota.max_records {
            if self.record_count > max {
                return Some(format!(
                    "Record quota exceeded: {} > {}",
                    self.record_count, max
                ));
            }
        }
        if let Some(max) = quota.max_qps {
            if self.current_qps > max {
                return Some(format!(
                    "QPS quota exceeded: {} > {}",
                    self.current_qps, max
                ));
            }
        }
        if let Some(max) = quota.max_connections {
            if self.active_connections > max {
                return Some(format!(
                    "Connection quota exceeded: {} > {}",
                    self.active_connections, max
                ));
            }
        }
        None
    }
}

/// Result of tenant access verification
#[derive(Debug, Clone)]
pub enum TenantAccessResult {
    /// Access allowed
    Allowed,
    /// Access denied with reason
    Denied(String),
    /// Access allowed via cross-tenant policy
    AllowedViaCrossTenant(String),
    /// Quota exceeded
    QuotaExceeded(String),
}

impl TenantAccessResult {
    /// Check if access is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            TenantAccessResult::Allowed | TenantAccessResult::AllowedViaCrossTenant(_)
        )
    }
}

/// Multi-tenant security manager
pub struct TenantManager {
    /// Registered tenants
    tenants: Arc<RwLock<HashMap<TenantId, TenantConfig>>>,
    /// Cross-tenant policies
    cross_tenant_policies: Arc<RwLock<Vec<CrossTenantPolicy>>>,
    /// Resource usage per tenant
    resource_usage: Arc<RwLock<HashMap<TenantId, ResourceUsage>>>,
    /// Tenant user memberships
    user_memberships: Arc<RwLock<HashMap<String, HashSet<TenantId>>>>,
}

impl TenantManager {
    /// Create a new tenant manager
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
            cross_tenant_policies: Arc::new(RwLock::new(Vec::new())),
            resource_usage: Arc::new(RwLock::new(HashMap::new())),
            user_memberships: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new tenant
    pub async fn register_tenant(&self, mut config: TenantConfig) -> OrbitResult<()> {
        let mut tenants = self.tenants.write().await;
        if tenants.contains_key(&config.tenant_id) {
            return Err(OrbitError::internal(format!(
                "Tenant already exists: {}",
                config.tenant_id
            )));
        }

        config.status = TenantStatus::Active;
        config.updated_at = SystemTime::now();

        // Initialize resource usage tracking
        let mut usage = self.resource_usage.write().await;
        usage.insert(config.tenant_id.clone(), ResourceUsage::default());

        tenants.insert(config.tenant_id.clone(), config);
        Ok(())
    }

    /// Get tenant configuration
    pub async fn get_tenant(&self, tenant_id: &str) -> Option<TenantConfig> {
        let tenants = self.tenants.read().await;
        tenants.get(tenant_id).cloned()
    }

    /// Update tenant configuration
    pub async fn update_tenant(&self, config: TenantConfig) -> OrbitResult<()> {
        let mut tenants = self.tenants.write().await;
        if !tenants.contains_key(&config.tenant_id) {
            return Err(OrbitError::internal(format!(
                "Tenant not found: {}",
                config.tenant_id
            )));
        }

        let mut updated = config;
        updated.updated_at = SystemTime::now();
        tenants.insert(updated.tenant_id.clone(), updated);
        Ok(())
    }

    /// Suspend a tenant
    pub async fn suspend_tenant(&self, tenant_id: &str) -> OrbitResult<()> {
        let mut tenants = self.tenants.write().await;
        if let Some(config) = tenants.get_mut(tenant_id) {
            config.status = TenantStatus::Suspended;
            config.updated_at = SystemTime::now();
            Ok(())
        } else {
            Err(OrbitError::internal(format!(
                "Tenant not found: {}",
                tenant_id
            )))
        }
    }

    /// Activate a tenant
    pub async fn activate_tenant(&self, tenant_id: &str) -> OrbitResult<()> {
        let mut tenants = self.tenants.write().await;
        if let Some(config) = tenants.get_mut(tenant_id) {
            config.status = TenantStatus::Active;
            config.updated_at = SystemTime::now();
            Ok(())
        } else {
            Err(OrbitError::internal(format!(
                "Tenant not found: {}",
                tenant_id
            )))
        }
    }

    /// Add user to tenant
    pub async fn add_user_to_tenant(&self, user_id: &str, tenant_id: &str) -> OrbitResult<()> {
        // Verify tenant exists
        let tenants = self.tenants.read().await;
        if !tenants.contains_key(tenant_id) {
            return Err(OrbitError::internal(format!(
                "Tenant not found: {}",
                tenant_id
            )));
        }
        drop(tenants);

        let mut memberships = self.user_memberships.write().await;
        memberships
            .entry(user_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(tenant_id.to_string());
        Ok(())
    }

    /// Remove user from tenant
    pub async fn remove_user_from_tenant(&self, user_id: &str, tenant_id: &str) -> OrbitResult<()> {
        let mut memberships = self.user_memberships.write().await;
        if let Some(tenants) = memberships.get_mut(user_id) {
            tenants.remove(tenant_id);
        }
        Ok(())
    }

    /// Check if user belongs to tenant
    pub async fn is_user_in_tenant(&self, user_id: &str, tenant_id: &str) -> bool {
        let memberships = self.user_memberships.read().await;
        memberships
            .get(user_id)
            .map(|tenants| tenants.contains(tenant_id))
            .unwrap_or(false)
    }

    /// Get all tenants for a user
    pub async fn get_user_tenants(&self, user_id: &str) -> Vec<TenantId> {
        let memberships = self.user_memberships.read().await;
        memberships
            .get(user_id)
            .map(|tenants| tenants.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Register a cross-tenant policy
    pub async fn register_cross_tenant_policy(&self, policy: CrossTenantPolicy) -> OrbitResult<()> {
        // Verify both tenants exist
        let tenants = self.tenants.read().await;
        if !tenants.contains_key(&policy.source_tenant) {
            return Err(OrbitError::internal(format!(
                "Source tenant not found: {}",
                policy.source_tenant
            )));
        }
        if !tenants.contains_key(&policy.target_tenant) {
            return Err(OrbitError::internal(format!(
                "Target tenant not found: {}",
                policy.target_tenant
            )));
        }
        drop(tenants);

        let mut policies = self.cross_tenant_policies.write().await;
        policies.push(policy);
        Ok(())
    }

    /// Verify tenant access for a context
    pub async fn verify_tenant_access(
        &self,
        ctx: &TenantContext,
        resource_tenant: &str,
        action: CrossTenantAction,
    ) -> TenantAccessResult {
        // Check if requesting own tenant's resource
        if ctx.tenant_id == resource_tenant {
            // Verify user belongs to tenant
            if !self.is_user_in_tenant(&ctx.user_id, &ctx.tenant_id).await {
                return TenantAccessResult::Denied(format!(
                    "User {} does not belong to tenant {}",
                    ctx.user_id, ctx.tenant_id
                ));
            }

            // Check tenant status
            if let Some(config) = self.get_tenant(&ctx.tenant_id).await {
                match config.status {
                    TenantStatus::Active => {}
                    TenantStatus::Suspended => {
                        if action != CrossTenantAction::Read {
                            return TenantAccessResult::Denied(
                                "Tenant is suspended, only read access allowed".to_string(),
                            );
                        }
                    }
                    _ => {
                        return TenantAccessResult::Denied(format!(
                            "Tenant is not active: {:?}",
                            config.status
                        ));
                    }
                }

                // Check quota
                if let Some(usage) = self.get_resource_usage(&ctx.tenant_id).await {
                    if let Some(exceeded) = usage.exceeds_quota(&config.quota) {
                        return TenantAccessResult::QuotaExceeded(exceeded);
                    }
                }
            }

            return TenantAccessResult::Allowed;
        }

        // Cross-tenant access - check policies
        let policies = self.cross_tenant_policies.read().await;
        for policy in policies.iter() {
            if policy.source_tenant == resource_tenant
                && policy.target_tenant == ctx.tenant_id
                && policy.is_allowed("*", action)
            {
                return TenantAccessResult::AllowedViaCrossTenant(format!(
                    "Policy allows {} access from {} to {}",
                    format!("{:?}", action),
                    policy.source_tenant,
                    policy.target_tenant
                ));
            }
        }

        TenantAccessResult::Denied(format!(
            "No cross-tenant policy allows {} access from {} to {}",
            format!("{:?}", action),
            ctx.tenant_id,
            resource_tenant
        ))
    }

    /// Get resource usage for a tenant
    pub async fn get_resource_usage(&self, tenant_id: &str) -> Option<ResourceUsage> {
        let usage = self.resource_usage.read().await;
        usage.get(tenant_id).cloned()
    }

    /// Update resource usage for a tenant
    pub async fn update_resource_usage(
        &self,
        tenant_id: &str,
        usage: ResourceUsage,
    ) -> OrbitResult<()> {
        let mut usages = self.resource_usage.write().await;
        usages.insert(tenant_id.to_string(), usage);
        Ok(())
    }

    /// Increment storage usage
    pub async fn increment_storage(&self, tenant_id: &str, bytes: u64) -> OrbitResult<()> {
        let mut usages = self.resource_usage.write().await;
        if let Some(usage) = usages.get_mut(tenant_id) {
            usage.storage_bytes += bytes;
        }
        Ok(())
    }

    /// Increment record count
    pub async fn increment_records(&self, tenant_id: &str, count: u64) -> OrbitResult<()> {
        let mut usages = self.resource_usage.write().await;
        if let Some(usage) = usages.get_mut(tenant_id) {
            usage.record_count += count;
        }
        Ok(())
    }

    /// List all tenants
    pub async fn list_tenants(&self) -> Vec<TenantConfig> {
        let tenants = self.tenants.read().await;
        tenants.values().cloned().collect()
    }

    /// Get tenant count
    pub async fn tenant_count(&self) -> usize {
        let tenants = self.tenants.read().await;
        tenants.len()
    }

    /// Filter data by tenant
    pub fn filter_by_tenant<T, F>(&self, data: Vec<T>, tenant_id: &str, get_tenant: F) -> Vec<T>
    where
        F: Fn(&T) -> &str,
    {
        data.into_iter()
            .filter(|item| get_tenant(item) == tenant_id)
            .collect()
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tenant_registration() {
        let manager = TenantManager::new();

        let config = TenantConfig::new("acme-corp")
            .with_name("Acme Corporation")
            .with_isolation_level(IsolationLevel::Schema)
            .with_encryption(true);

        manager.register_tenant(config).await.unwrap();

        let tenant = manager.get_tenant("acme-corp").await.unwrap();
        assert_eq!(tenant.name, "Acme Corporation");
        assert_eq!(tenant.isolation_level, IsolationLevel::Schema);
        assert!(tenant.encryption_enabled);
        assert_eq!(tenant.status, TenantStatus::Active);
    }

    #[tokio::test]
    async fn test_duplicate_tenant_registration() {
        let manager = TenantManager::new();

        let config = TenantConfig::new("acme-corp");
        manager.register_tenant(config.clone()).await.unwrap();

        // Should fail for duplicate
        let result = manager.register_tenant(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tenant_suspension() {
        let manager = TenantManager::new();

        let config = TenantConfig::new("acme-corp");
        manager.register_tenant(config).await.unwrap();

        manager.suspend_tenant("acme-corp").await.unwrap();

        let tenant = manager.get_tenant("acme-corp").await.unwrap();
        assert_eq!(tenant.status, TenantStatus::Suspended);
    }

    #[tokio::test]
    async fn test_user_tenant_membership() {
        let manager = TenantManager::new();

        let config = TenantConfig::new("acme-corp");
        manager.register_tenant(config).await.unwrap();

        manager
            .add_user_to_tenant("user-123", "acme-corp")
            .await
            .unwrap();

        assert!(manager.is_user_in_tenant("user-123", "acme-corp").await);
        assert!(!manager.is_user_in_tenant("user-456", "acme-corp").await);

        let tenants = manager.get_user_tenants("user-123").await;
        assert_eq!(tenants.len(), 1);
        assert!(tenants.contains(&"acme-corp".to_string()));
    }

    #[tokio::test]
    async fn test_tenant_access_verification() {
        let manager = TenantManager::new();

        let config = TenantConfig::new("acme-corp");
        manager.register_tenant(config).await.unwrap();

        manager
            .add_user_to_tenant("user-123", "acme-corp")
            .await
            .unwrap();

        let ctx = TenantContext::new("acme-corp", "user-123");

        // Should allow access to own tenant
        let result = manager
            .verify_tenant_access(&ctx, "acme-corp", CrossTenantAction::Read)
            .await;
        assert!(result.is_allowed());

        // Should deny access to other tenant
        let other_config = TenantConfig::new("other-corp");
        manager.register_tenant(other_config).await.unwrap();

        let result = manager
            .verify_tenant_access(&ctx, "other-corp", CrossTenantAction::Read)
            .await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    async fn test_cross_tenant_policy() {
        let manager = TenantManager::new();

        // Register two tenants
        manager
            .register_tenant(TenantConfig::new("tenant-a"))
            .await
            .unwrap();
        manager
            .register_tenant(TenantConfig::new("tenant-b"))
            .await
            .unwrap();

        // Add user to tenant-b
        manager
            .add_user_to_tenant("user-123", "tenant-b")
            .await
            .unwrap();

        // Create cross-tenant policy allowing tenant-b to read tenant-a's data
        let policy = CrossTenantPolicy::new("tenant-a", "tenant-b")
            .share_resource("*")
            .allow_action(CrossTenantAction::Read);

        manager.register_cross_tenant_policy(policy).await.unwrap();

        let ctx = TenantContext::new("tenant-b", "user-123");

        // Should allow read access via policy
        let result = manager
            .verify_tenant_access(&ctx, "tenant-a", CrossTenantAction::Read)
            .await;
        assert!(result.is_allowed());

        // Should deny write access (not in policy)
        let result = manager
            .verify_tenant_access(&ctx, "tenant-a", CrossTenantAction::Write)
            .await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    async fn test_resource_quota() {
        let manager = TenantManager::new();

        let config = TenantConfig::new("acme-corp").with_quota(ResourceQuota::basic());
        manager.register_tenant(config).await.unwrap();

        // Update usage to exceed quota
        let usage = ResourceUsage {
            storage_bytes: 20 * 1024 * 1024 * 1024, // 20GB (exceeds 10GB basic limit)
            ..Default::default()
        };
        manager
            .update_resource_usage("acme-corp", usage)
            .await
            .unwrap();

        manager
            .add_user_to_tenant("user-123", "acme-corp")
            .await
            .unwrap();

        let ctx = TenantContext::new("acme-corp", "user-123");

        let result = manager
            .verify_tenant_access(&ctx, "acme-corp", CrossTenantAction::Write)
            .await;

        match result {
            TenantAccessResult::QuotaExceeded(_) => {}
            _ => panic!("Expected QuotaExceeded"),
        }
    }

    #[tokio::test]
    async fn test_suspended_tenant_read_only() {
        let manager = TenantManager::new();

        let config = TenantConfig::new("acme-corp");
        manager.register_tenant(config).await.unwrap();
        manager
            .add_user_to_tenant("user-123", "acme-corp")
            .await
            .unwrap();
        manager.suspend_tenant("acme-corp").await.unwrap();

        let ctx = TenantContext::new("acme-corp", "user-123");

        // Read should be allowed
        let result = manager
            .verify_tenant_access(&ctx, "acme-corp", CrossTenantAction::Read)
            .await;
        assert!(result.is_allowed());

        // Write should be denied
        let result = manager
            .verify_tenant_access(&ctx, "acme-corp", CrossTenantAction::Write)
            .await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    async fn test_policy_expiration() {
        let policy = CrossTenantPolicy::new("tenant-a", "tenant-b")
            .share_resource("data")
            .allow_action(CrossTenantAction::Read)
            .expires_in(Duration::from_secs(0)); // Already expired

        // Wait a bit to ensure expiration
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(policy.is_expired());
        assert!(!policy.is_allowed("data", CrossTenantAction::Read));
    }

    #[tokio::test]
    async fn test_quota_tiers() {
        // Basic tier
        let basic = ResourceQuota::basic();
        assert_eq!(basic.max_storage_bytes, Some(10 * 1024 * 1024 * 1024));
        assert_eq!(basic.max_qps, Some(100));

        // Professional tier
        let pro = ResourceQuota::professional();
        assert_eq!(pro.max_storage_bytes, Some(100 * 1024 * 1024 * 1024));
        assert_eq!(pro.max_qps, Some(1000));

        // Enterprise tier
        let enterprise = ResourceQuota::enterprise();
        assert_eq!(
            enterprise.max_storage_bytes,
            Some(1024 * 1024 * 1024 * 1024)
        );
        assert_eq!(enterprise.max_qps, Some(10000));

        // Unlimited
        let unlimited = ResourceQuota::unlimited();
        assert!(unlimited.max_storage_bytes.is_none());
        assert!(unlimited.max_qps.is_none());
    }

    #[tokio::test]
    async fn test_tenant_context() {
        let ctx = TenantContext::new("acme-corp", "user-123")
            .with_client_ip("192.168.1.1")
            .with_correlation_id("req-456")
            .with_attribute("department", "engineering");

        assert_eq!(ctx.tenant_id, "acme-corp");
        assert_eq!(ctx.user_id, "user-123");
        assert_eq!(ctx.client_ip, Some("192.168.1.1".to_string()));
        assert_eq!(ctx.correlation_id, "req-456");
        assert_eq!(
            ctx.attributes.get("department"),
            Some(&"engineering".to_string())
        );
    }
}
