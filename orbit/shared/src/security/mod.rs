//! Advanced Security Framework for Orbit-RS
//!
//! This module provides comprehensive security features including:
//! - Advanced authentication mechanisms (LDAP, OAuth2, SAML)
//! - Role-based access control (RBAC) with fine-grained permissions
//! - Row-level security (RLS) for fine-grained data access control
//! - Data encryption at rest and in transit
//! - Comprehensive audit logging
//! - SQL injection prevention
//! - Security policy enforcement

pub mod audit;
pub mod authentication;
pub mod authorization;
pub mod data_masking;
pub mod encryption;
pub mod field_encryption;
pub mod multi_tenant;
pub mod policy;
pub mod row_level_security;
pub mod sql_validation;
pub mod threat_detection;

// Re-export commonly used types
pub use audit::{AuditEvent, AuditLogger, AuditPolicy, ComplianceMonitor};
pub use authentication::{
    AuthenticationManager, AuthenticationProvider, LdapAuthProvider, OAuth2AuthProvider,
    SamlAuthProvider,
};
pub use authorization::{
    AccessPolicy, Permission, RbacEngine, Role, RoleBasedAccessControl, SecurityAction,
    SecurityResource, SecuritySubject,
};
pub use data_masking::{
    AccessLevel, DataMaskingEngine, DatePrecision, FieldMaskingConfig, MaskingContext,
    MaskingPolicy, MaskingStrategy, TokenStore,
};
pub use encryption::{
    EncryptionManager, KeyManagementSystem, KeyRotationPolicy, TlsConfig, TlsVersion,
};
pub use field_encryption::{
    EncryptedFieldValue, EncryptionType, FieldEncryptionConfig, FieldEncryptionEngine,
    FieldEncryptionPolicy, SensitivityLevel,
};
pub use multi_tenant::{
    CrossTenantAction, CrossTenantPolicy, IsolationLevel, ResourceQuota, ResourceUsage,
    TenantAccessResult, TenantConfig, TenantContext, TenantId, TenantManager, TenantStatus,
};
pub use policy::{PolicyEngine, PolicyEvaluator, SecurityPolicy};
pub use row_level_security::{
    RlsAction, RlsEngine, RlsExpression, RlsPolicy, RlsPolicyType, RlsValue, RowLevelSecurity,
};
pub use sql_validation::{QueryValidator, SqlInjectionDetector};
pub use threat_detection::{AnomalyDetector, ThreatDetectionEngine};

use crate::exception::OrbitResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Comprehensive security framework
pub struct SecurityFramework {
    /// Authentication manager
    auth_manager: Arc<AuthenticationManager>,
    /// RBAC engine
    rbac_engine: Arc<RbacEngine>,
    /// Encryption manager
    encryption_manager: Arc<EncryptionManager>,
    /// Audit logger
    audit_logger: Arc<dyn AuditLogger>,
    /// Policy engine
    policy_engine: Arc<PolicyEngine>,
    /// Threat detection
    threat_detector: Arc<ThreatDetectionEngine>,
    /// SQL injection detector
    sql_validator: Arc<SqlInjectionDetector>,
}

impl SecurityFramework {
    /// Create a new security framework
    pub fn new(
        auth_manager: Arc<AuthenticationManager>,
        rbac_engine: Arc<RbacEngine>,
        encryption_manager: Arc<EncryptionManager>,
        audit_logger: Arc<dyn AuditLogger>,
        policy_engine: Arc<PolicyEngine>,
        threat_detector: Arc<ThreatDetectionEngine>,
        sql_validator: Arc<SqlInjectionDetector>,
    ) -> Self {
        Self {
            auth_manager,
            rbac_engine,
            encryption_manager,
            audit_logger,
            policy_engine,
            threat_detector,
            sql_validator,
        }
    }

    /// Get authentication manager
    pub fn auth_manager(&self) -> &Arc<AuthenticationManager> {
        &self.auth_manager
    }

    /// Get RBAC engine
    pub fn rbac_engine(&self) -> &Arc<RbacEngine> {
        &self.rbac_engine
    }

    /// Get encryption manager
    pub fn encryption_manager(&self) -> &Arc<EncryptionManager> {
        &self.encryption_manager
    }

    /// Get audit logger
    pub fn audit_logger(&self) -> &Arc<dyn AuditLogger> {
        &self.audit_logger
    }

    /// Get policy engine
    pub fn policy_engine(&self) -> &Arc<PolicyEngine> {
        &self.policy_engine
    }

    /// Get threat detector
    pub fn threat_detector(&self) -> &Arc<ThreatDetectionEngine> {
        &self.threat_detector
    }

    /// Get SQL validator
    pub fn sql_validator(&self) -> &Arc<SqlInjectionDetector> {
        &self.sql_validator
    }

    /// Validate and authorize a query
    pub async fn validate_query(
        &self,
        query: &str,
        subject: &SecuritySubject,
        resource: &SecurityResource,
    ) -> OrbitResult<bool> {
        // 1. Check for SQL injection
        self.sql_validator.validate(query)?;

        // 2. Check authorization
        let authorized = self
            .rbac_engine
            .check_access(subject, resource, &SecurityAction::Read)
            .await?;

        // 3. Log audit event
        if authorized {
            self.audit_logger
                .log(AuditEvent::new_query(subject, resource, query))
                .await?;
        }

        Ok(authorized)
    }
}

/// Security context for a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Subject making the request
    pub subject: SecuritySubject,
    /// Session identifier
    pub session_id: String,
    /// Client IP address
    pub client_ip: Option<String>,
    /// Authentication timestamp
    pub authenticated_at: std::time::SystemTime,
}

impl SecurityContext {
    /// Create a new security context
    pub fn new(subject: SecuritySubject, session_id: String) -> Self {
        Self {
            subject,
            session_id,
            client_ip: None,
            authenticated_at: std::time::SystemTime::now(),
        }
    }

    /// Add client IP
    pub fn with_client_ip(mut self, ip: String) -> Self {
        self.client_ip = Some(ip);
        self
    }
}
