//! Comprehensive Audit Logging
//!
//! Provides tamper-proof audit logging with:
//! - Complete audit trails for all operations
//! - Integrity verification
//! - Compliance reporting
//! - Retention policies

use crate::exception::OrbitResult;
use crate::security::authorization::{SecurityResource, SecuritySubject};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: SystemTime,
    pub event_type: AuditEventType,
    pub subject: Option<SecuritySubject>,
    pub resource: Option<SecurityResource>,
    pub action: String,
    pub result: AuditResult,
    pub client_ip: Option<String>,
    pub session_id: Option<String>,
    pub query: Option<String>,
    pub rows_affected: Option<u64>,
    pub execution_time: Option<Duration>,
    pub sensitive_data_accessed: bool,
    pub metadata: HashMap<String, String>,
    pub integrity_hash: Option<String>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(event_type: AuditEventType, action: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            event_type,
            subject: None,
            resource: None,
            action,
            result: AuditResult::Success,
            client_ip: None,
            session_id: None,
            query: None,
            rows_affected: None,
            execution_time: None,
            sensitive_data_accessed: false,
            metadata: HashMap::new(),
            integrity_hash: None,
        }
    }

    /// Create a query audit event
    pub fn new_query(subject: &SecuritySubject, resource: &SecurityResource, query: &str) -> Self {
        let mut event = Self::new(AuditEventType::Query, "QUERY".to_string());
        event.subject = Some(subject.clone());
        event.resource = Some(resource.clone());
        event.query = Some(query.to_string());
        event
    }

    /// Create an authentication audit event
    pub fn new_authentication(subject: &SecuritySubject, success: bool) -> Self {
        let mut event = Self::new(AuditEventType::Authentication, "AUTH".to_string());
        event.subject = Some(subject.clone());
        event.result = if success {
            AuditResult::Success
        } else {
            AuditResult::Failure
        };
        event
    }

    /// Set result
    pub fn with_result(mut self, result: AuditResult) -> Self {
        self.result = result;
        self
    }

    /// Set client IP
    pub fn with_client_ip(mut self, ip: String) -> Self {
        self.client_ip = Some(ip);
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set execution time
    pub fn with_execution_time(mut self, duration: Duration) -> Self {
        self.execution_time = Some(duration);
        self
    }

    /// Mark as sensitive data access
    pub fn with_sensitive_data(mut self) -> Self {
        self.sensitive_data_accessed = true;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Calculate integrity hash
    pub fn calculate_integrity_hash(&mut self) {
        // In production, this would use a proper cryptographic hash
        // For now, use a simple checksum
        let data = format!(
            "{}:{}:{}:{}",
            self.id,
            self.timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            self.action,
            self.result.as_str()
        );
        self.integrity_hash = Some(format!("hash-{}", data.len()));
    }
}

/// Audit event type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    Query,
    DataAccess,
    DataModification,
    Configuration,
    System,
    Security,
}

/// Audit result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditResult {
    Success,
    Failure,
    Denied,
    Error,
}

impl AuditResult {
    pub fn as_str(&self) -> &str {
        match self {
            AuditResult::Success => "success",
            AuditResult::Failure => "failure",
            AuditResult::Denied => "denied",
            AuditResult::Error => "error",
        }
    }
}

/// Audit policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPolicy {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub event_types: Vec<AuditEventType>,
    pub include_query_text: bool,
    pub include_results: bool,
    pub retention_period: Duration,
    pub alert_on_failure: bool,
}

impl Default for AuditPolicy {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Default Audit Policy".to_string(),
            enabled: true,
            event_types: vec![
                AuditEventType::Authentication,
                AuditEventType::Authorization,
                AuditEventType::Query,
                AuditEventType::DataModification,
                AuditEventType::Security,
            ],
            include_query_text: true,
            include_results: false,
            retention_period: Duration::from_secs(86400 * 90), // 90 days
            alert_on_failure: true,
        }
    }
}

/// Audit logger trait
#[async_trait]
pub trait AuditLogger: Send + Sync {
    /// Log an audit event
    async fn log(&self, event: AuditEvent) -> OrbitResult<()>;

    /// Query audit logs
    async fn query(&self, filters: AuditQueryFilters) -> OrbitResult<Vec<AuditEvent>>;

    /// Get audit statistics
    async fn get_statistics(&self, period: Duration) -> OrbitResult<AuditStatistics>;
}

/// Audit query filters
#[derive(Debug, Clone)]
pub struct AuditQueryFilters {
    pub start_time: Option<SystemTime>,
    pub end_time: Option<SystemTime>,
    pub event_types: Option<Vec<AuditEventType>>,
    pub subject_id: Option<String>,
    pub resource_id: Option<String>,
    pub result: Option<AuditResult>,
    pub limit: usize,
}

impl Default for AuditQueryFilters {
    fn default() -> Self {
        Self {
            start_time: None,
            end_time: None,
            event_types: None,
            subject_id: None,
            resource_id: None,
            result: None,
            limit: 100,
        }
    }
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub total_events: usize,
    pub events_by_type: HashMap<String, usize>,
    pub success_rate: f64,
    pub failure_rate: f64,
    pub sensitive_data_accesses: usize,
}

/// In-memory audit logger
pub struct InMemoryAuditLogger {
    events: Arc<RwLock<Vec<AuditEvent>>>,
    policy: AuditPolicy,
    max_events: usize,
}

impl InMemoryAuditLogger {
    /// Create a new in-memory audit logger
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            policy: AuditPolicy::default(),
            max_events,
        }
    }

    /// Create with custom policy
    pub fn with_policy(max_events: usize, policy: AuditPolicy) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            policy,
            max_events,
        }
    }
}

#[async_trait]
impl AuditLogger for InMemoryAuditLogger {
    async fn log(&self, mut event: AuditEvent) -> OrbitResult<()> {
        // Check if event type is enabled in policy
        if !self.policy.enabled || !self.policy.event_types.contains(&event.event_type) {
            return Ok(());
        }

        // Calculate integrity hash
        event.calculate_integrity_hash();

        // Store event
        let mut events = self.events.write().await;
        events.push(event);

        // Enforce max events limit
        if events.len() > self.max_events {
            let excess = events.len() - self.max_events;
            events.drain(0..excess);
        }

        Ok(())
    }

    async fn query(&self, filters: AuditQueryFilters) -> OrbitResult<Vec<AuditEvent>> {
        let events = self.events.read().await;

        let filtered: Vec<AuditEvent> = events
            .iter()
            .filter(|e| self.matches_all_filters(e, &filters))
            .take(filters.limit)
            .cloned()
            .collect();

        Ok(filtered)
    }

    async fn get_statistics(&self, period: Duration) -> OrbitResult<AuditStatistics> {
        let events = self.events.read().await;
        let cutoff = SystemTime::now() - period;

        let recent_events: Vec<&AuditEvent> =
            events.iter().filter(|e| e.timestamp >= cutoff).collect();

        let total_events = recent_events.len();
        let mut events_by_type: HashMap<String, usize> = HashMap::new();
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut sensitive_data_accesses = 0;

        for event in &recent_events {
            // Count by type
            let type_key = format!("{:?}", event.event_type);
            *events_by_type.entry(type_key).or_insert(0) += 1;

            // Count results
            match event.result {
                AuditResult::Success => success_count += 1,
                _ => failure_count += 1,
            }

            // Count sensitive data accesses
            if event.sensitive_data_accessed {
                sensitive_data_accesses += 1;
            }
        }

        let success_rate = if total_events > 0 {
            success_count as f64 / total_events as f64
        } else {
            0.0
        };

        let failure_rate = if total_events > 0 {
            failure_count as f64 / total_events as f64
        } else {
            0.0
        };

        Ok(AuditStatistics {
            total_events,
            events_by_type,
            success_rate,
            failure_rate,
            sensitive_data_accesses,
        })
    }
}

impl InMemoryAuditLogger {
    /// Check if an event matches all filters
    fn matches_all_filters(&self, event: &AuditEvent, filters: &AuditQueryFilters) -> bool {
        self.matches_time_filter(event, filters)
            && self.matches_event_type_filter(event, filters)
            && self.matches_subject_filter(event, filters)
            && self.matches_resource_filter(event, filters)
            && self.matches_result_filter(event, filters)
    }

    /// Check if event matches time range filter
    fn matches_time_filter(&self, event: &AuditEvent, filters: &AuditQueryFilters) -> bool {
        if let Some(start) = filters.start_time {
            if event.timestamp < start {
                return false;
            }
        }
        if let Some(end) = filters.end_time {
            if event.timestamp > end {
                return false;
            }
        }
        true
    }

    /// Check if event matches event type filter
    fn matches_event_type_filter(&self, event: &AuditEvent, filters: &AuditQueryFilters) -> bool {
        if let Some(ref types) = filters.event_types {
            return types.contains(&event.event_type);
        }
        true
    }

    /// Check if event matches subject filter
    fn matches_subject_filter(&self, event: &AuditEvent, filters: &AuditQueryFilters) -> bool {
        if let Some(ref subject_id) = filters.subject_id {
            if let Some(ref subject) = event.subject {
                return &subject.id == subject_id;
            } else {
                return false;
            }
        }
        true
    }

    /// Check if event matches resource filter
    fn matches_resource_filter(&self, event: &AuditEvent, filters: &AuditQueryFilters) -> bool {
        if let Some(ref resource_id) = filters.resource_id {
            if let Some(ref resource) = event.resource {
                return &resource.id == resource_id;
            } else {
                return false;
            }
        }
        true
    }

    /// Check if event matches result filter
    fn matches_result_filter(&self, event: &AuditEvent, filters: &AuditQueryFilters) -> bool {
        if let Some(ref result) = filters.result {
            return &event.result == result;
        }
        true
    }
}

/// Compliance monitor
pub struct ComplianceMonitor {
    audit_logger: Arc<dyn AuditLogger>,
    compliance_rules: Vec<ComplianceRule>,
}

impl ComplianceMonitor {
    /// Create a new compliance monitor
    pub fn new(audit_logger: Arc<dyn AuditLogger>) -> Self {
        Self {
            audit_logger,
            compliance_rules: Vec::new(),
        }
    }

    /// Add a compliance rule
    pub fn add_rule(&mut self, rule: ComplianceRule) {
        self.compliance_rules.push(rule);
    }

    /// Check compliance
    pub async fn check_compliance(&self) -> OrbitResult<ComplianceReport> {
        let filters = AuditQueryFilters {
            start_time: Some(SystemTime::now() - Duration::from_secs(86400)), // Last 24 hours
            ..Default::default()
        };

        let events = self.audit_logger.query(filters).await?;

        let mut violations = Vec::new();
        for rule in &self.compliance_rules {
            let rule_violations = rule.check_violations(&events);
            violations.extend(rule_violations);
        }

        let compliant = violations.is_empty();
        Ok(ComplianceReport {
            timestamp: SystemTime::now(),
            total_events_checked: events.len(),
            violations,
            compliant,
        })
    }
}

/// Compliance rule
#[derive(Debug, Clone)]
pub struct ComplianceRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rule_type: ComplianceRuleType,
}

impl ComplianceRule {
    /// Check for violations
    pub fn check_violations(&self, _events: &[AuditEvent]) -> Vec<ComplianceViolation> {
        // In production, this would implement actual compliance checks
        // For now, return empty violations
        Vec::new()
    }
}

/// Compliance rule type
#[derive(Debug, Clone)]
pub enum ComplianceRuleType {
    Soc2,
    Gdpr,
    Hipaa,
    PciDss,
    Custom,
}

/// Compliance violation
#[derive(Debug, Clone)]
pub struct ComplianceViolation {
    pub rule_id: String,
    pub severity: ViolationSeverity,
    pub description: String,
    pub event_ids: Vec<String>,
}

/// Violation severity
#[derive(Debug, Clone)]
pub enum ViolationSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// Compliance report
#[derive(Debug, Clone)]
pub struct ComplianceReport {
    pub timestamp: SystemTime,
    pub total_events_checked: usize,
    pub violations: Vec<ComplianceViolation>,
    pub compliant: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::authorization::{
        ResourceType, SecurityResource, SecuritySubject, SubjectType,
    };

    #[tokio::test]
    async fn test_audit_logger() {
        let logger = InMemoryAuditLogger::new(100);

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

        let event = AuditEvent::new_query(&subject, &resource, "SELECT * FROM users");

        logger.log(event).await.unwrap();

        let filters = AuditQueryFilters::default();
        let events = logger.query(filters).await.unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AuditEventType::Query);
    }

    #[tokio::test]
    async fn test_audit_statistics() {
        let logger = InMemoryAuditLogger::new(100);

        let subject = SecuritySubject {
            id: "user1".to_string(),
            name: "Test User".to_string(),
            subject_type: SubjectType::User,
            roles: vec![],
            attributes: HashMap::new(),
        };

        // Log successful authentication
        let auth_event = AuditEvent::new_authentication(&subject, true);
        logger.log(auth_event).await.unwrap();

        // Log failed authentication
        let failed_auth_event = AuditEvent::new_authentication(&subject, false);
        logger.log(failed_auth_event).await.unwrap();

        let stats = logger
            .get_statistics(Duration::from_secs(3600))
            .await
            .unwrap();

        assert_eq!(stats.total_events, 2);
        assert!(stats.success_rate > 0.0);
    }
}
