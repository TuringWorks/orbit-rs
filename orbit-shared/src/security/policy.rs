//! Security Policy Engine
//!
//! Provides centralized security policy management and enforcement:
//! - Policy definition and storage
//! - Policy evaluation
//! - Policy conflict resolution
//! - Policy compliance checking

use crate::exception::OrbitResult;
use crate::security::authorization::{SecurityAction, SecurityResource, SecuritySubject};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Security policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub priority: i32,
    pub rules: Vec<PolicyRule>,
    pub created_at: std::time::SystemTime,
    pub updated_at: std::time::SystemTime,
}

impl SecurityPolicy {
    /// Create a new security policy
    pub fn new(id: String, name: String, description: String) -> Self {
        let now = std::time::SystemTime::now();
        Self {
            id,
            name,
            description,
            version: "1.0.0".to_string(),
            enabled: true,
            priority: 0,
            rules: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a rule to the policy
    pub fn add_rule(&mut self, rule: PolicyRule) {
        self.rules.push(rule);
        self.updated_at = std::time::SystemTime::now();
    }
}

/// Policy rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub rule_type: PolicyRuleType,
    pub condition: PolicyCondition,
    pub action: PolicyAction,
    pub enabled: bool,
}

/// Policy rule type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyRuleType {
    Access,
    Encryption,
    Audit,
    Compliance,
    Custom,
}

/// Policy condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCondition {
    pub subject_conditions: Vec<SubjectCondition>,
    pub resource_conditions: Vec<ResourceCondition>,
    pub action_conditions: Vec<ActionCondition>,
    pub environmental_conditions: Vec<EnvironmentalCondition>,
}

impl PolicyCondition {
    /// Create an empty policy condition
    pub fn new() -> Self {
        Self {
            subject_conditions: Vec::new(),
            resource_conditions: Vec::new(),
            action_conditions: Vec::new(),
            environmental_conditions: Vec::new(),
        }
    }
}

impl Default for PolicyCondition {
    fn default() -> Self {
        Self::new()
    }
}

/// Subject condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectCondition {
    pub attribute: String,
    pub operator: ConditionOperator,
    pub value: String,
}

/// Resource condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCondition {
    pub attribute: String,
    pub operator: ConditionOperator,
    pub value: String,
}

/// Action condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCondition {
    pub action_type: String,
    pub operator: ConditionOperator,
    pub value: String,
}

/// Environmental condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalCondition {
    pub condition_type: EnvironmentalConditionType,
    pub operator: ConditionOperator,
    pub value: String,
}

/// Environmental condition type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvironmentalConditionType {
    Time,
    IpAddress,
    Location,
    DeviceType,
    Custom,
}

/// Condition operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    GreaterThan,
    LessThan,
    Matches,
}

/// Policy action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyAction {
    Allow,
    Deny,
    RequireMfa,
    RequireEncryption,
    Audit,
    Alert,
    Custom(String),
}

/// Policy evaluation result
#[derive(Debug, Clone)]
pub struct PolicyEvaluationResult {
    pub allowed: bool,
    pub matched_policies: Vec<String>,
    pub required_actions: Vec<PolicyAction>,
    pub reasons: Vec<String>,
}

/// Policy evaluator trait
#[async_trait]
pub trait PolicyEvaluator: Send + Sync {
    /// Evaluate policies for a request
    async fn evaluate(
        &self,
        subject: &SecuritySubject,
        resource: &SecurityResource,
        action: &SecurityAction,
    ) -> OrbitResult<PolicyEvaluationResult>;
}

/// Policy engine
pub struct PolicyEngine {
    policies: Arc<RwLock<HashMap<String, SecurityPolicy>>>,
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a policy
    pub async fn add_policy(&self, policy: SecurityPolicy) -> OrbitResult<()> {
        let mut policies = self.policies.write().await;
        policies.insert(policy.id.clone(), policy);
        Ok(())
    }

    /// Get a policy
    pub async fn get_policy(&self, policy_id: &str) -> OrbitResult<Option<SecurityPolicy>> {
        let policies = self.policies.read().await;
        Ok(policies.get(policy_id).cloned())
    }

    /// Remove a policy
    pub async fn remove_policy(&self, policy_id: &str) -> OrbitResult<()> {
        let mut policies = self.policies.write().await;
        policies.remove(policy_id);
        Ok(())
    }

    /// List all policies
    pub async fn list_policies(&self) -> OrbitResult<Vec<SecurityPolicy>> {
        let policies = self.policies.read().await;
        Ok(policies.values().cloned().collect())
    }

    /// Evaluate policies for a request
    pub async fn evaluate(
        &self,
        subject: &SecuritySubject,
        resource: &SecurityResource,
        action: &SecurityAction,
    ) -> OrbitResult<PolicyEvaluationResult> {
        let policies = self.policies.read().await;

        let mut matched_policies = Vec::new();
        let mut required_actions = Vec::new();
        let mut reasons = Vec::new();
        let mut denied = false;
        let mut allowed = false;

        // Sort policies by priority (higher priority first)
        let mut sorted_policies: Vec<&SecurityPolicy> = policies
            .values()
            .filter(|p| p.enabled)
            .collect();
        sorted_policies.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Evaluate each policy
        for policy in sorted_policies {
            for rule in &policy.rules {
                if !rule.enabled {
                    continue;
                }

                if self.evaluate_rule(rule, subject, resource, action) {
                    matched_policies.push(policy.id.clone());

                    match rule.action {
                        PolicyAction::Allow => {
                            allowed = true;
                            reasons.push(format!("Allowed by policy: {}", policy.name));
                        }
                        PolicyAction::Deny => {
                            denied = true;
                            reasons.push(format!("Denied by policy: {}", policy.name));
                        }
                        ref other => {
                            required_actions.push(other.clone());
                        }
                    }
                }
            }
        }

        // Deny takes precedence over allow
        let final_allowed = allowed && !denied;

        Ok(PolicyEvaluationResult {
            allowed: final_allowed,
            matched_policies,
            required_actions,
            reasons,
        })
    }

    /// Evaluate a single rule
    fn evaluate_rule(
        &self,
        rule: &PolicyRule,
        _subject: &SecuritySubject,
        _resource: &SecurityResource,
        _action: &SecurityAction,
    ) -> bool {
        // In production, this would implement actual condition evaluation
        // For now, implement a stub that returns true
        match rule.rule_type {
            PolicyRuleType::Access => true,
            PolicyRuleType::Encryption => true,
            PolicyRuleType::Audit => true,
            PolicyRuleType::Compliance => true,
            PolicyRuleType::Custom => true,
        }
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PolicyEvaluator for PolicyEngine {
    async fn evaluate(
        &self,
        subject: &SecuritySubject,
        resource: &SecurityResource,
        action: &SecurityAction,
    ) -> OrbitResult<PolicyEvaluationResult> {
        self.evaluate(subject, resource, action).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::authorization::{ResourceType, SubjectType};

    #[tokio::test]
    async fn test_policy_engine() {
        let engine = PolicyEngine::new();

        // Create a policy
        let mut policy = SecurityPolicy::new(
            "policy1".to_string(),
            "Test Policy".to_string(),
            "A test security policy".to_string(),
        );

        // Add a rule
        let rule = PolicyRule {
            id: "rule1".to_string(),
            name: "Allow Read".to_string(),
            rule_type: PolicyRuleType::Access,
            condition: PolicyCondition::new(),
            action: PolicyAction::Allow,
            enabled: true,
        };

        policy.add_rule(rule);

        // Add policy to engine
        engine.add_policy(policy).await.unwrap();

        // Create test subject
        let subject = SecuritySubject {
            id: "user1".to_string(),
            name: "Test User".to_string(),
            subject_type: SubjectType::User,
            roles: vec![],
            attributes: HashMap::new(),
        };

        // Create test resource
        let resource = SecurityResource {
            id: "db1".to_string(),
            resource_type: ResourceType::Database,
            path: "/databases/db1".to_string(),
            attributes: HashMap::new(),
        };

        // Evaluate policy
        let result = engine
            .evaluate(&subject, &resource, &SecurityAction::Read)
            .await
            .unwrap();

        assert!(result.allowed);
        assert_eq!(result.matched_policies.len(), 1);
    }

    #[tokio::test]
    async fn test_policy_priority() {
        let engine = PolicyEngine::new();

        // Create allow policy with lower priority
        let mut allow_policy = SecurityPolicy::new(
            "allow".to_string(),
            "Allow Policy".to_string(),
            "Allow access".to_string(),
        );
        allow_policy.priority = 1;
        allow_policy.add_rule(PolicyRule {
            id: "allow_rule".to_string(),
            name: "Allow Read".to_string(),
            rule_type: PolicyRuleType::Access,
            condition: PolicyCondition::new(),
            action: PolicyAction::Allow,
            enabled: true,
        });

        // Create deny policy with higher priority
        let mut deny_policy = SecurityPolicy::new(
            "deny".to_string(),
            "Deny Policy".to_string(),
            "Deny access".to_string(),
        );
        deny_policy.priority = 10;
        deny_policy.add_rule(PolicyRule {
            id: "deny_rule".to_string(),
            name: "Deny Write".to_string(),
            rule_type: PolicyRuleType::Access,
            condition: PolicyCondition::new(),
            action: PolicyAction::Deny,
            enabled: true,
        });

        engine.add_policy(allow_policy).await.unwrap();
        engine.add_policy(deny_policy).await.unwrap();

        let subject = SecuritySubject {
            id: "user1".to_string(),
            name: "Test User".to_string(),
            subject_type: SubjectType::User,
            roles: vec![],
            attributes: HashMap::new(),
        };

        let resource = SecurityResource {
            id: "db1".to_string(),
            resource_type: ResourceType::Database,
            path: "/databases/db1".to_string(),
            attributes: HashMap::new(),
        };

        // Evaluate - deny should take precedence
        let result = engine
            .evaluate(&subject, &resource, &SecurityAction::Write)
            .await
            .unwrap();

        // Deny takes precedence over allow
        assert!(!result.allowed);
    }
}
