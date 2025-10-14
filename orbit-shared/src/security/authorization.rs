//! Role-Based Access Control (RBAC) with Fine-Grained Permissions
//!
//! Implements comprehensive RBAC with:
//! - Hierarchical roles
//! - Fine-grained permissions
//! - Resource-level access control
//! - Policy-based authorization

use crate::exception::OrbitResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Security subject (user or service)
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SecuritySubject {
    pub id: String,
    pub name: String,
    pub subject_type: SubjectType,
    pub roles: Vec<String>,
    pub attributes: HashMap<String, String>,
}

/// Type of security subject
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum SubjectType {
    User,
    Service,
    Group,
    System,
}

/// Security resource
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SecurityResource {
    pub id: String,
    pub resource_type: ResourceType,
    pub path: String,
    pub attributes: HashMap<String, String>,
}

/// Type of security resource
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourceType {
    Database,
    Schema,
    Table,
    Column,
    Row,
    Query,
    System,
}

/// Security action
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum SecurityAction {
    Read,
    Write,
    Update,
    Delete,
    Execute,
    Admin,
    Custom(String),
}

impl SecurityAction {
    /// Convert to string representation
    pub fn as_str(&self) -> &str {
        match self {
            SecurityAction::Read => "read",
            SecurityAction::Write => "write",
            SecurityAction::Update => "update",
            SecurityAction::Delete => "delete",
            SecurityAction::Execute => "execute",
            SecurityAction::Admin => "admin",
            SecurityAction::Custom(s) => s,
        }
    }
}

/// Role definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub inherits_from: Vec<String>,
    pub created_at: std::time::SystemTime,
    pub updated_at: std::time::SystemTime,
}

impl Role {
    /// Create a new role
    pub fn new(id: String, name: String, description: String) -> Self {
        let now = std::time::SystemTime::now();
        Self {
            id,
            name,
            description,
            permissions: Vec::new(),
            inherits_from: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a permission to the role
    pub fn add_permission(&mut self, permission: String) {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
            self.updated_at = std::time::SystemTime::now();
        }
    }

    /// Add role inheritance
    pub fn inherit_from(&mut self, role_id: String) {
        if !self.inherits_from.contains(&role_id) {
            self.inherits_from.push(role_id);
            self.updated_at = std::time::SystemTime::now();
        }
    }
}

/// Permission definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub resource_type: ResourceType,
    pub actions: Vec<SecurityAction>,
    pub conditions: Vec<Condition>,
}

impl Permission {
    /// Create a new permission
    pub fn new(id: String, name: String, description: String, resource_type: ResourceType) -> Self {
        Self {
            id,
            name,
            description,
            resource_type,
            actions: Vec::new(),
            conditions: Vec::new(),
        }
    }

    /// Add an action to the permission
    pub fn add_action(&mut self, action: SecurityAction) {
        self.actions.push(action);
    }

    /// Add a condition to the permission
    pub fn add_condition(&mut self, condition: Condition) {
        self.conditions.push(condition);
    }
}

/// Access condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub condition_type: ConditionType,
    pub attribute: String,
    pub operator: ConditionOperator,
    pub value: String,
}

/// Condition type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionType {
    TimeRange,
    IpAddress,
    ResourceAttribute,
    SubjectAttribute,
    Custom,
}

/// Condition operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    In,
    NotIn,
    GreaterThan,
    LessThan,
    Matches,
}

/// Access policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub subject_matcher: SubjectMatcher,
    pub resource_matcher: ResourceMatcher,
    pub action_matcher: ActionMatcher,
    pub conditions: Vec<Condition>,
    pub effect: PolicyEffect,
    pub priority: i32,
}

/// Subject matcher for policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubjectMatcher {
    Any,
    User(String),
    Role(String),
    Group(String),
    Attribute(String, String),
}

/// Resource matcher for policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceMatcher {
    Any,
    Type(ResourceType),
    Path(String),
    Pattern(String),
}

/// Action matcher for policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionMatcher {
    Any,
    Specific(SecurityAction),
    Set(Vec<SecurityAction>),
}

/// Policy effect
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PolicyEffect {
    Allow,
    Deny,
}

/// RBAC engine
pub struct RbacEngine {
    roles: Arc<RwLock<HashMap<String, Role>>>,
    permissions: Arc<RwLock<HashMap<String, Permission>>>,
    policies: Arc<RwLock<Vec<AccessPolicy>>>,
}

impl RbacEngine {
    /// Create a new RBAC engine
    pub fn new() -> Self {
        Self {
            roles: Arc::new(RwLock::new(HashMap::new())),
            permissions: Arc::new(RwLock::new(HashMap::new())),
            policies: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a role
    pub async fn add_role(&self, role: Role) -> OrbitResult<()> {
        let mut roles = self.roles.write().await;
        roles.insert(role.id.clone(), role);
        Ok(())
    }

    /// Get a role
    pub async fn get_role(&self, role_id: &str) -> OrbitResult<Option<Role>> {
        let roles = self.roles.read().await;
        Ok(roles.get(role_id).cloned())
    }

    /// Add a permission
    pub async fn add_permission(&self, permission: Permission) -> OrbitResult<()> {
        let mut permissions = self.permissions.write().await;
        permissions.insert(permission.id.clone(), permission);
        Ok(())
    }

    /// Add a policy
    pub async fn add_policy(&self, policy: AccessPolicy) -> OrbitResult<()> {
        let mut policies = self.policies.write().await;
        policies.push(policy);
        // Sort by priority (higher priority first)
        policies.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(())
    }

    /// Check if subject has access to resource for action
    pub async fn check_access(
        &self,
        subject: &SecuritySubject,
        resource: &SecurityResource,
        action: &SecurityAction,
    ) -> OrbitResult<bool> {
        // Get all effective permissions for the subject
        let permissions = self.get_effective_permissions(subject).await?;

        // Check if any permission allows the action on the resource
        for permission_id in &permissions {
            let perms = self.permissions.read().await;
            if let Some(permission) = perms.get(permission_id) {
                // Check if permission applies to this resource type
                if std::mem::discriminant(&permission.resource_type)
                    != std::mem::discriminant(&resource.resource_type)
                {
                    continue;
                }

                // Check if permission allows this action
                if permission
                    .actions
                    .iter()
                    .any(|a| std::mem::discriminant(a) == std::mem::discriminant(action))
                {
                    // Check conditions
                    if self
                        .evaluate_conditions(&permission.conditions, subject, resource)
                        .await?
                    {
                        return Ok(true);
                    }
                }
            }
        }

        // Check policies
        let policies = self.policies.read().await;
        for policy in policies.iter() {
            if self
                .policy_matches(policy, subject, resource, action)
                .await?
            {
                return Ok(policy.effect == PolicyEffect::Allow);
            }
        }

        Ok(false)
    }

    /// Get effective permissions for a subject
    async fn get_effective_permissions(
        &self,
        subject: &SecuritySubject,
    ) -> OrbitResult<HashSet<String>> {
        let mut effective_permissions = HashSet::new();
        let roles = self.roles.read().await;

        for role_id in &subject.roles {
            if let Some(role) = roles.get(role_id) {
                // Add direct permissions
                for perm in &role.permissions {
                    effective_permissions.insert(perm.clone());
                }

                // Add inherited permissions
                self.collect_inherited_permissions(role, &roles, &mut effective_permissions);
            }
        }

        Ok(effective_permissions)
    }

    /// Collect inherited permissions recursively
    #[allow(clippy::only_used_in_recursion)]
    fn collect_inherited_permissions(
        &self,
        role: &Role,
        all_roles: &HashMap<String, Role>,
        permissions: &mut HashSet<String>,
    ) {
        for parent_role_id in &role.inherits_from {
            if let Some(parent_role) = all_roles.get(parent_role_id) {
                for perm in &parent_role.permissions {
                    permissions.insert(perm.clone());
                }
                self.collect_inherited_permissions(parent_role, all_roles, permissions);
            }
        }
    }

    /// Evaluate conditions
    async fn evaluate_conditions(
        &self,
        conditions: &[Condition],
        _subject: &SecuritySubject,
        _resource: &SecurityResource,
    ) -> OrbitResult<bool> {
        // If no conditions, always allow
        if conditions.is_empty() {
            return Ok(true);
        }

        // For now, implement a simple stub
        // In production, this would evaluate each condition
        Ok(true)
    }

    /// Check if policy matches the request
    async fn policy_matches(
        &self,
        policy: &AccessPolicy,
        subject: &SecuritySubject,
        resource: &SecurityResource,
        action: &SecurityAction,
    ) -> OrbitResult<bool> {
        // Check subject matcher
        if !self.subject_matches(&policy.subject_matcher, subject) {
            return Ok(false);
        }

        // Check resource matcher
        if !self.resource_matches(&policy.resource_matcher, resource) {
            return Ok(false);
        }

        // Check action matcher
        if !self.action_matches(&policy.action_matcher, action) {
            return Ok(false);
        }

        // Evaluate conditions
        self.evaluate_conditions(&policy.conditions, subject, resource)
            .await
    }

    /// Check if subject matches
    fn subject_matches(&self, matcher: &SubjectMatcher, subject: &SecuritySubject) -> bool {
        match matcher {
            SubjectMatcher::Any => true,
            SubjectMatcher::User(id) => &subject.id == id,
            SubjectMatcher::Role(role) => subject.roles.contains(role),
            SubjectMatcher::Group(_) => false, // Stub for now
            SubjectMatcher::Attribute(key, value) => subject
                .attributes
                .get(key)
                .map(|v| v == value)
                .unwrap_or(false),
        }
    }

    /// Check if resource matches
    fn resource_matches(&self, matcher: &ResourceMatcher, resource: &SecurityResource) -> bool {
        match matcher {
            ResourceMatcher::Any => true,
            ResourceMatcher::Type(rt) => {
                std::mem::discriminant(rt) == std::mem::discriminant(&resource.resource_type)
            }
            ResourceMatcher::Path(path) => &resource.path == path,
            ResourceMatcher::Pattern(pattern) => resource.path.starts_with(pattern),
        }
    }

    /// Check if action matches
    fn action_matches(&self, matcher: &ActionMatcher, action: &SecurityAction) -> bool {
        match matcher {
            ActionMatcher::Any => true,
            ActionMatcher::Specific(a) => {
                std::mem::discriminant(a) == std::mem::discriminant(action)
            }
            ActionMatcher::Set(actions) => actions
                .iter()
                .any(|a| std::mem::discriminant(a) == std::mem::discriminant(action)),
        }
    }
}

impl Default for RbacEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Role-based access control trait
#[async_trait]
pub trait RoleBasedAccessControl: Send + Sync {
    /// Check if subject has permission for action on resource
    async fn check_permission(
        &self,
        subject: &SecuritySubject,
        resource: &SecurityResource,
        action: &SecurityAction,
    ) -> OrbitResult<bool>;

    /// Get all permissions for a subject
    async fn get_subject_permissions(
        &self,
        subject: &SecuritySubject,
    ) -> OrbitResult<Vec<Permission>>;
}

#[async_trait]
impl RoleBasedAccessControl for RbacEngine {
    async fn check_permission(
        &self,
        subject: &SecuritySubject,
        resource: &SecurityResource,
        action: &SecurityAction,
    ) -> OrbitResult<bool> {
        self.check_access(subject, resource, action).await
    }

    async fn get_subject_permissions(
        &self,
        subject: &SecuritySubject,
    ) -> OrbitResult<Vec<Permission>> {
        let permission_ids = self.get_effective_permissions(subject).await?;
        let permissions = self.permissions.read().await;

        let result: Vec<Permission> = permission_ids
            .iter()
            .filter_map(|id| permissions.get(id).cloned())
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rbac_engine_basic() {
        let engine = RbacEngine::new();

        // Create a role
        let mut role = Role::new(
            "admin".to_string(),
            "Admin Role".to_string(),
            "Administrator with full access".to_string(),
        );
        role.add_permission("perm:admin".to_string());

        engine.add_role(role).await.unwrap();

        // Create a permission
        let mut permission = Permission::new(
            "perm:admin".to_string(),
            "Admin Permission".to_string(),
            "Full administrative access".to_string(),
            ResourceType::Database,
        );
        permission.add_action(SecurityAction::Read);
        permission.add_action(SecurityAction::Write);
        permission.add_action(SecurityAction::Admin);

        engine.add_permission(permission).await.unwrap();

        // Create a subject
        let subject = SecuritySubject {
            id: "user1".to_string(),
            name: "Admin User".to_string(),
            subject_type: SubjectType::User,
            roles: vec!["admin".to_string()],
            attributes: HashMap::new(),
        };

        // Create a resource
        let resource = SecurityResource {
            id: "db1".to_string(),
            resource_type: ResourceType::Database,
            path: "/databases/db1".to_string(),
            attributes: HashMap::new(),
        };

        // Test access
        let has_access = engine
            .check_access(&subject, &resource, &SecurityAction::Read)
            .await
            .unwrap();

        assert!(has_access);
    }

    #[tokio::test]
    async fn test_policy_based_access() {
        let engine = RbacEngine::new();

        // Create a policy
        let policy = AccessPolicy {
            id: "policy1".to_string(),
            name: "Read Policy".to_string(),
            description: "Allow read access".to_string(),
            subject_matcher: SubjectMatcher::Role("reader".to_string()),
            resource_matcher: ResourceMatcher::Type(ResourceType::Table),
            action_matcher: ActionMatcher::Specific(SecurityAction::Read),
            conditions: vec![],
            effect: PolicyEffect::Allow,
            priority: 100,
        };

        engine.add_policy(policy).await.unwrap();

        // Create a subject with reader role
        let subject = SecuritySubject {
            id: "user2".to_string(),
            name: "Reader User".to_string(),
            subject_type: SubjectType::User,
            roles: vec!["reader".to_string()],
            attributes: HashMap::new(),
        };

        // Create a table resource
        let resource = SecurityResource {
            id: "table1".to_string(),
            resource_type: ResourceType::Table,
            path: "/databases/db1/tables/table1".to_string(),
            attributes: HashMap::new(),
        };

        // Test read access
        let has_access = engine
            .check_access(&subject, &resource, &SecurityAction::Read)
            .await
            .unwrap();

        assert!(has_access);

        // Test write access (should be denied)
        let has_write_access = engine
            .check_access(&subject, &resource, &SecurityAction::Write)
            .await
            .unwrap();

        assert!(!has_write_access);
    }
}
