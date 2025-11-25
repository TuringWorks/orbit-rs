//! Row-Level Security (RLS) Implementation
//!
//! This module provides fine-grained row-level access control, similar to PostgreSQL's
//! Row Level Security. Policies can be defined to filter which rows are visible or
//! modifiable by different users based on row attributes and user context.

use crate::exception::OrbitResult;
use crate::security::authorization::{SecurityAction, SecuritySubject};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Row-Level Security policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlsPolicy {
    /// Unique policy identifier
    pub id: String,
    /// Human-readable policy name
    pub name: String,
    /// Policy description
    pub description: String,
    /// Table/collection this policy applies to
    pub table_name: String,
    /// Actions this policy applies to (SELECT, INSERT, UPDATE, DELETE)
    pub applicable_actions: Vec<RlsAction>,
    /// Roles this policy applies to (empty means all roles)
    pub applicable_roles: Vec<String>,
    /// The expression used to filter rows (using column references)
    pub using_expression: RlsExpression,
    /// Expression for checking new row values (for INSERT/UPDATE)
    pub check_expression: Option<RlsExpression>,
    /// Whether this is a permissive or restrictive policy
    pub policy_type: RlsPolicyType,
    /// Policy priority (higher values evaluated first)
    pub priority: i32,
    /// Whether the policy is enabled
    pub enabled: bool,
}

/// RLS action types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RlsAction {
    Select,
    Insert,
    Update,
    Delete,
    All,
}

impl From<&SecurityAction> for RlsAction {
    fn from(action: &SecurityAction) -> Self {
        match action {
            SecurityAction::Read => RlsAction::Select,
            SecurityAction::Write => RlsAction::Insert,
            SecurityAction::Update => RlsAction::Update,
            SecurityAction::Delete => RlsAction::Delete,
            _ => RlsAction::All,
        }
    }
}

/// Policy type (permissive vs restrictive)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RlsPolicyType {
    /// Permissive policies are combined with OR - any matching policy grants access
    Permissive,
    /// Restrictive policies are combined with AND - all must pass
    Restrictive,
}

/// RLS expression that evaluates row visibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RlsExpression {
    /// Always true - allows all rows
    True,
    /// Always false - denies all rows
    False,
    /// Compare a column to the current user's ID
    /// e.g., user_id = current_user_id
    CurrentUserMatch { column: String },
    /// Compare a column to a user attribute
    /// e.g., department = current_user.department
    UserAttributeMatch { column: String, attribute: String },
    /// Compare a column to a specific value
    Equals { column: String, value: RlsValue },
    /// Check if column value is in a list
    In { column: String, values: Vec<RlsValue> },
    /// Check if column value is in current user's roles
    RoleMatch { column: String },
    /// Check if current user has any of the specified roles
    HasRole { roles: Vec<String> },
    /// Check if a column contains the current user's ID (for array columns)
    ContainsCurrentUser { column: String },
    /// Combine expressions with AND
    And(Vec<RlsExpression>),
    /// Combine expressions with OR
    Or(Vec<RlsExpression>),
    /// Negate an expression
    Not(Box<RlsExpression>),
    /// Custom SQL-like expression (parsed at runtime)
    Custom { expression: String },
}

/// Value types for RLS expressions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RlsValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
}

impl RlsValue {
    /// Convert JSON value to RlsValue
    pub fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::String(s) => RlsValue::String(s.clone()),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    RlsValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    RlsValue::Float(f)
                } else {
                    RlsValue::Null
                }
            }
            serde_json::Value::Bool(b) => RlsValue::Boolean(*b),
            _ => RlsValue::Null,
        }
    }

    /// Check equality with a JSON value
    pub fn matches_json(&self, value: &serde_json::Value) -> bool {
        match (self, value) {
            (RlsValue::String(s), serde_json::Value::String(v)) => s == v,
            (RlsValue::Integer(i), serde_json::Value::Number(n)) => n.as_i64() == Some(*i),
            (RlsValue::Float(f), serde_json::Value::Number(n)) => n.as_f64() == Some(*f),
            (RlsValue::Boolean(b), serde_json::Value::Bool(v)) => b == v,
            (RlsValue::Null, serde_json::Value::Null) => true,
            _ => false,
        }
    }
}

/// Row-Level Security Engine
pub struct RlsEngine {
    /// Policies organized by table name
    policies: Arc<RwLock<HashMap<String, Vec<RlsPolicy>>>>,
    /// Tables with RLS enabled
    enabled_tables: Arc<RwLock<HashMap<String, bool>>>,
}

impl RlsEngine {
    /// Create a new RLS engine
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            enabled_tables: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Enable RLS for a table
    pub async fn enable_rls(&self, table_name: &str) -> OrbitResult<()> {
        let mut enabled = self.enabled_tables.write().await;
        enabled.insert(table_name.to_string(), true);
        Ok(())
    }

    /// Disable RLS for a table
    pub async fn disable_rls(&self, table_name: &str) -> OrbitResult<()> {
        let mut enabled = self.enabled_tables.write().await;
        enabled.insert(table_name.to_string(), false);
        Ok(())
    }

    /// Check if RLS is enabled for a table
    pub async fn is_rls_enabled(&self, table_name: &str) -> bool {
        let enabled = self.enabled_tables.read().await;
        enabled.get(table_name).copied().unwrap_or(false)
    }

    /// Add a policy
    pub async fn add_policy(&self, policy: RlsPolicy) -> OrbitResult<()> {
        let mut policies = self.policies.write().await;
        let table_policies = policies.entry(policy.table_name.clone()).or_default();

        // Remove existing policy with same ID if exists
        table_policies.retain(|p| p.id != policy.id);

        // Insert and sort by priority
        table_policies.push(policy);
        table_policies.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(())
    }

    /// Remove a policy
    pub async fn remove_policy(&self, table_name: &str, policy_id: &str) -> OrbitResult<bool> {
        let mut policies = self.policies.write().await;
        if let Some(table_policies) = policies.get_mut(table_name) {
            let len_before = table_policies.len();
            table_policies.retain(|p| p.id != policy_id);
            return Ok(table_policies.len() < len_before);
        }
        Ok(false)
    }

    /// Get all policies for a table
    pub async fn get_policies(&self, table_name: &str) -> Vec<RlsPolicy> {
        let policies = self.policies.read().await;
        policies.get(table_name).cloned().unwrap_or_default()
    }

    /// Check if a row is visible to the subject for the given action
    pub async fn check_row_access(
        &self,
        table_name: &str,
        row: &HashMap<String, serde_json::Value>,
        subject: &SecuritySubject,
        action: &RlsAction,
    ) -> OrbitResult<bool> {
        // If RLS is not enabled, allow all access
        if !self.is_rls_enabled(table_name).await {
            return Ok(true);
        }

        let policies = self.policies.read().await;
        let table_policies = match policies.get(table_name) {
            Some(p) => p,
            None => return Ok(true), // No policies means allow all
        };

        // Filter applicable policies
        let applicable: Vec<&RlsPolicy> = table_policies
            .iter()
            .filter(|p| {
                p.enabled
                    && (p.applicable_actions.contains(action)
                        || p.applicable_actions.contains(&RlsAction::All))
                    && (p.applicable_roles.is_empty()
                        || p.applicable_roles.iter().any(|r| subject.roles.contains(r)))
            })
            .collect();

        if applicable.is_empty() {
            // No applicable policies - deny by default when RLS is enabled
            return Ok(false);
        }

        // Separate permissive and restrictive policies
        let permissive: Vec<&&RlsPolicy> = applicable
            .iter()
            .filter(|p| p.policy_type == RlsPolicyType::Permissive)
            .collect();
        let restrictive: Vec<&&RlsPolicy> = applicable
            .iter()
            .filter(|p| p.policy_type == RlsPolicyType::Restrictive)
            .collect();

        // All restrictive policies must pass (AND)
        for policy in &restrictive {
            if !self.evaluate_expression(&policy.using_expression, row, subject)? {
                return Ok(false);
            }
        }

        // At least one permissive policy must pass (OR)
        if !permissive.is_empty() {
            let any_permissive_pass = permissive
                .iter()
                .any(|p| self.evaluate_expression(&p.using_expression, row, subject).unwrap_or(false));

            if !any_permissive_pass {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Filter rows based on RLS policies
    pub async fn filter_rows(
        &self,
        table_name: &str,
        rows: Vec<HashMap<String, serde_json::Value>>,
        subject: &SecuritySubject,
        action: &RlsAction,
    ) -> OrbitResult<Vec<HashMap<String, serde_json::Value>>> {
        // If RLS is not enabled, return all rows
        if !self.is_rls_enabled(table_name).await {
            return Ok(rows);
        }

        let mut filtered = Vec::with_capacity(rows.len());
        for row in rows {
            if self.check_row_access(table_name, &row, subject, action).await? {
                filtered.push(row);
            }
        }
        Ok(filtered)
    }

    /// Check if a new/updated row passes the CHECK expression
    pub async fn check_row_modification(
        &self,
        table_name: &str,
        row: &HashMap<String, serde_json::Value>,
        subject: &SecuritySubject,
        action: &RlsAction,
    ) -> OrbitResult<bool> {
        // If RLS is not enabled, allow all modifications
        if !self.is_rls_enabled(table_name).await {
            return Ok(true);
        }

        let policies = self.policies.read().await;
        let table_policies = match policies.get(table_name) {
            Some(p) => p,
            None => return Ok(true),
        };

        // Check all applicable policies with CHECK expressions
        for policy in table_policies {
            if !policy.enabled {
                continue;
            }

            if !policy.applicable_actions.contains(action)
                && !policy.applicable_actions.contains(&RlsAction::All)
            {
                continue;
            }

            if !policy.applicable_roles.is_empty()
                && !policy.applicable_roles.iter().any(|r| subject.roles.contains(r))
            {
                continue;
            }

            // Check the CHECK expression if present
            if let Some(ref check_expr) = policy.check_expression {
                if !self.evaluate_expression(check_expr, row, subject)? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Evaluate an RLS expression against a row
    fn evaluate_expression(
        &self,
        expr: &RlsExpression,
        row: &HashMap<String, serde_json::Value>,
        subject: &SecuritySubject,
    ) -> OrbitResult<bool> {
        match expr {
            RlsExpression::True => Ok(true),
            RlsExpression::False => Ok(false),

            RlsExpression::CurrentUserMatch { column } => {
                if let Some(value) = row.get(column) {
                    if let Some(s) = value.as_str() {
                        return Ok(s == subject.id);
                    }
                }
                Ok(false)
            }

            RlsExpression::UserAttributeMatch { column, attribute } => {
                if let Some(row_value) = row.get(column) {
                    if let Some(user_attr) = subject.attributes.get(attribute) {
                        if let Some(s) = row_value.as_str() {
                            return Ok(s == user_attr);
                        }
                    }
                }
                Ok(false)
            }

            RlsExpression::Equals { column, value } => {
                if let Some(row_value) = row.get(column) {
                    return Ok(value.matches_json(row_value));
                }
                Ok(false)
            }

            RlsExpression::In { column, values } => {
                if let Some(row_value) = row.get(column) {
                    return Ok(values.iter().any(|v| v.matches_json(row_value)));
                }
                Ok(false)
            }

            RlsExpression::RoleMatch { column } => {
                if let Some(value) = row.get(column) {
                    if let Some(s) = value.as_str() {
                        return Ok(subject.roles.contains(&s.to_string()));
                    }
                }
                Ok(false)
            }

            RlsExpression::HasRole { roles } => {
                Ok(roles.iter().any(|r| subject.roles.contains(r)))
            }

            RlsExpression::ContainsCurrentUser { column } => {
                if let Some(value) = row.get(column) {
                    if let Some(arr) = value.as_array() {
                        return Ok(arr.iter().any(|v| {
                            v.as_str().map(|s| s == subject.id).unwrap_or(false)
                        }));
                    }
                }
                Ok(false)
            }

            RlsExpression::And(expressions) => {
                for e in expressions {
                    if !self.evaluate_expression(e, row, subject)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }

            RlsExpression::Or(expressions) => {
                for e in expressions {
                    if self.evaluate_expression(e, row, subject)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            RlsExpression::Not(inner) => {
                Ok(!self.evaluate_expression(inner, row, subject)?)
            }

            RlsExpression::Custom { expression: _ } => {
                // Custom expressions would need a full expression parser
                // For now, return true as a placeholder
                Ok(true)
            }
        }
    }
}

impl Default for RlsEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for row-level security enforcement
#[async_trait]
pub trait RowLevelSecurity: Send + Sync {
    /// Check if a row is accessible
    async fn is_row_accessible(
        &self,
        table_name: &str,
        row: &HashMap<String, serde_json::Value>,
        subject: &SecuritySubject,
        action: &RlsAction,
    ) -> OrbitResult<bool>;

    /// Filter rows based on RLS
    async fn apply_rls_filter(
        &self,
        table_name: &str,
        rows: Vec<HashMap<String, serde_json::Value>>,
        subject: &SecuritySubject,
        action: &RlsAction,
    ) -> OrbitResult<Vec<HashMap<String, serde_json::Value>>>;
}

#[async_trait]
impl RowLevelSecurity for RlsEngine {
    async fn is_row_accessible(
        &self,
        table_name: &str,
        row: &HashMap<String, serde_json::Value>,
        subject: &SecuritySubject,
        action: &RlsAction,
    ) -> OrbitResult<bool> {
        self.check_row_access(table_name, row, subject, action).await
    }

    async fn apply_rls_filter(
        &self,
        table_name: &str,
        rows: Vec<HashMap<String, serde_json::Value>>,
        subject: &SecuritySubject,
        action: &RlsAction,
    ) -> OrbitResult<Vec<HashMap<String, serde_json::Value>>> {
        self.filter_rows(table_name, rows, subject, action).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::authorization::SubjectType;

    fn create_test_subject(id: &str, roles: Vec<&str>, attrs: Vec<(&str, &str)>) -> SecuritySubject {
        SecuritySubject {
            id: id.to_string(),
            name: format!("User {}", id),
            subject_type: SubjectType::User,
            roles: roles.into_iter().map(String::from).collect(),
            attributes: attrs.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }
    }

    fn create_test_row(data: Vec<(&str, serde_json::Value)>) -> HashMap<String, serde_json::Value> {
        data.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[tokio::test]
    async fn test_rls_disabled_allows_all() {
        let engine = RlsEngine::new();
        let subject = create_test_subject("user1", vec![], vec![]);
        let row = create_test_row(vec![("id", serde_json::json!(1))]);

        // RLS not enabled, should allow access
        let result = engine
            .check_row_access("users", &row, &subject, &RlsAction::Select)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_current_user_match() {
        let engine = RlsEngine::new();

        // Enable RLS for users table
        engine.enable_rls("users").await.unwrap();

        // Add policy: users can only see their own rows
        let policy = RlsPolicy {
            id: "users_own_data".to_string(),
            name: "Users Own Data".to_string(),
            description: "Users can only access their own data".to_string(),
            table_name: "users".to_string(),
            applicable_actions: vec![RlsAction::All],
            applicable_roles: vec![],
            using_expression: RlsExpression::CurrentUserMatch {
                column: "user_id".to_string(),
            },
            check_expression: None,
            policy_type: RlsPolicyType::Permissive,
            priority: 100,
            enabled: true,
        };
        engine.add_policy(policy).await.unwrap();

        let subject = create_test_subject("user1", vec![], vec![]);

        // User can see their own row
        let own_row = create_test_row(vec![
            ("id", serde_json::json!(1)),
            ("user_id", serde_json::json!("user1")),
        ]);
        let result = engine
            .check_row_access("users", &own_row, &subject, &RlsAction::Select)
            .await
            .unwrap();
        assert!(result);

        // User cannot see other's row
        let other_row = create_test_row(vec![
            ("id", serde_json::json!(2)),
            ("user_id", serde_json::json!("user2")),
        ]);
        let result = engine
            .check_row_access("users", &other_row, &subject, &RlsAction::Select)
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_department_based_access() {
        let engine = RlsEngine::new();
        engine.enable_rls("documents").await.unwrap();

        // Users can see documents from their department
        let policy = RlsPolicy {
            id: "dept_docs".to_string(),
            name: "Department Documents".to_string(),
            description: "Users can access documents from their department".to_string(),
            table_name: "documents".to_string(),
            applicable_actions: vec![RlsAction::Select],
            applicable_roles: vec![],
            using_expression: RlsExpression::UserAttributeMatch {
                column: "department".to_string(),
                attribute: "department".to_string(),
            },
            check_expression: None,
            policy_type: RlsPolicyType::Permissive,
            priority: 100,
            enabled: true,
        };
        engine.add_policy(policy).await.unwrap();

        let subject = create_test_subject("user1", vec![], vec![("department", "engineering")]);

        // Can see engineering docs
        let eng_doc = create_test_row(vec![
            ("id", serde_json::json!(1)),
            ("department", serde_json::json!("engineering")),
        ]);
        assert!(engine
            .check_row_access("documents", &eng_doc, &subject, &RlsAction::Select)
            .await
            .unwrap());

        // Cannot see HR docs
        let hr_doc = create_test_row(vec![
            ("id", serde_json::json!(2)),
            ("department", serde_json::json!("hr")),
        ]);
        assert!(!engine
            .check_row_access("documents", &hr_doc, &subject, &RlsAction::Select)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_admin_bypass() {
        let engine = RlsEngine::new();
        engine.enable_rls("secrets").await.unwrap();

        // Regular users see nothing (restrictive)
        let restrictive_policy = RlsPolicy {
            id: "no_access".to_string(),
            name: "No Access".to_string(),
            description: "Deny all access by default".to_string(),
            table_name: "secrets".to_string(),
            applicable_actions: vec![RlsAction::All],
            applicable_roles: vec![],
            using_expression: RlsExpression::False,
            check_expression: None,
            policy_type: RlsPolicyType::Permissive,
            priority: 50,
            enabled: true,
        };
        engine.add_policy(restrictive_policy).await.unwrap();

        // Admin can see everything
        let admin_policy = RlsPolicy {
            id: "admin_access".to_string(),
            name: "Admin Access".to_string(),
            description: "Admins can access all data".to_string(),
            table_name: "secrets".to_string(),
            applicable_actions: vec![RlsAction::All],
            applicable_roles: vec!["admin".to_string()],
            using_expression: RlsExpression::True,
            check_expression: None,
            policy_type: RlsPolicyType::Permissive,
            priority: 100,
            enabled: true,
        };
        engine.add_policy(admin_policy).await.unwrap();

        let row = create_test_row(vec![("secret", serde_json::json!("top_secret"))]);

        // Regular user denied
        let regular_user = create_test_subject("user1", vec!["user"], vec![]);
        assert!(!engine
            .check_row_access("secrets", &row, &regular_user, &RlsAction::Select)
            .await
            .unwrap());

        // Admin allowed
        let admin_user = create_test_subject("admin1", vec!["admin"], vec![]);
        assert!(engine
            .check_row_access("secrets", &row, &admin_user, &RlsAction::Select)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_filter_rows() {
        let engine = RlsEngine::new();
        engine.enable_rls("posts").await.unwrap();

        let policy = RlsPolicy {
            id: "own_posts".to_string(),
            name: "Own Posts".to_string(),
            description: "Users see their own posts".to_string(),
            table_name: "posts".to_string(),
            applicable_actions: vec![RlsAction::Select],
            applicable_roles: vec![],
            using_expression: RlsExpression::CurrentUserMatch {
                column: "author_id".to_string(),
            },
            check_expression: None,
            policy_type: RlsPolicyType::Permissive,
            priority: 100,
            enabled: true,
        };
        engine.add_policy(policy).await.unwrap();

        let rows = vec![
            create_test_row(vec![("id", serde_json::json!(1)), ("author_id", serde_json::json!("user1"))]),
            create_test_row(vec![("id", serde_json::json!(2)), ("author_id", serde_json::json!("user2"))]),
            create_test_row(vec![("id", serde_json::json!(3)), ("author_id", serde_json::json!("user1"))]),
        ];

        let subject = create_test_subject("user1", vec![], vec![]);
        let filtered = engine
            .filter_rows("posts", rows, &subject, &RlsAction::Select)
            .await
            .unwrap();

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| r.get("author_id").unwrap() == "user1"));
    }

    #[tokio::test]
    async fn test_complex_expression() {
        let engine = RlsEngine::new();
        engine.enable_rls("projects").await.unwrap();

        // Users can see projects if they are the owner OR are a team member
        let policy = RlsPolicy {
            id: "project_access".to_string(),
            name: "Project Access".to_string(),
            description: "Owners and team members can access".to_string(),
            table_name: "projects".to_string(),
            applicable_actions: vec![RlsAction::Select],
            applicable_roles: vec![],
            using_expression: RlsExpression::Or(vec![
                RlsExpression::CurrentUserMatch {
                    column: "owner_id".to_string(),
                },
                RlsExpression::ContainsCurrentUser {
                    column: "team_members".to_string(),
                },
            ]),
            check_expression: None,
            policy_type: RlsPolicyType::Permissive,
            priority: 100,
            enabled: true,
        };
        engine.add_policy(policy).await.unwrap();

        let subject = create_test_subject("user1", vec![], vec![]);

        // User is owner
        let owned_project = create_test_row(vec![
            ("id", serde_json::json!(1)),
            ("owner_id", serde_json::json!("user1")),
            ("team_members", serde_json::json!(["user2", "user3"])),
        ]);
        assert!(engine
            .check_row_access("projects", &owned_project, &subject, &RlsAction::Select)
            .await
            .unwrap());

        // User is team member
        let team_project = create_test_row(vec![
            ("id", serde_json::json!(2)),
            ("owner_id", serde_json::json!("user2")),
            ("team_members", serde_json::json!(["user1", "user3"])),
        ]);
        assert!(engine
            .check_row_access("projects", &team_project, &subject, &RlsAction::Select)
            .await
            .unwrap());

        // User is neither owner nor team member
        let other_project = create_test_row(vec![
            ("id", serde_json::json!(3)),
            ("owner_id", serde_json::json!("user2")),
            ("team_members", serde_json::json!(["user3", "user4"])),
        ]);
        assert!(!engine
            .check_row_access("projects", &other_project, &subject, &RlsAction::Select)
            .await
            .unwrap());
    }
}
