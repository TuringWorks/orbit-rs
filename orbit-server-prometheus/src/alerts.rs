//! Alert management and notification system
//!
//! This module provides alerting capabilities for monitoring metrics thresholds
//! and triggering notifications when critical conditions are detected.

use crate::{PrometheusError, PrometheusResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Alert manager for monitoring and notifications
pub struct AlertManager {
    /// Alert rules configured in the system
    rules: Arc<RwLock<HashMap<String, AlertRule>>>,
    /// Active alerts currently firing
    active_alerts: Arc<RwLock<HashMap<String, ActiveAlert>>>,
    /// Alert history for audit trail
    alert_history: Arc<RwLock<Vec<AlertHistoryEntry>>>,
    /// Notification channels
    notification_channels: Arc<RwLock<Vec<NotificationChannel>>>,
}

/// Alert rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Unique rule identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Metric to monitor
    pub metric_name: String,
    /// Comparison operator
    pub operator: AlertOperator,
    /// Threshold value
    pub threshold: f64,
    /// Evaluation window duration in seconds
    pub window_seconds: u64,
    /// Alert severity level
    pub severity: AlertSeverity,
    /// Whether rule is enabled
    pub enabled: bool,
    /// Labels for routing and filtering
    pub labels: HashMap<String, String>,
    /// Notification channels to alert
    pub channels: Vec<String>,
}

/// Comparison operators for alert conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertOperator {
    /// Greater than
    GreaterThan,
    /// Greater than or equal
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal
    LessThanOrEqual,
    /// Equal to
    Equal,
    /// Not equal to
    NotEqual,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum AlertSeverity {
    /// Informational alert
    Info,
    /// Warning - requires attention
    Warning,
    /// Critical - requires immediate action
    Critical,
    /// Fatal - system failure
    Fatal,
}

/// Active alert currently firing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAlert {
    /// Alert rule ID
    pub rule_id: String,
    /// Rule name
    pub rule_name: String,
    /// Current metric value
    pub current_value: f64,
    /// Threshold value
    pub threshold_value: f64,
    /// When alert started firing
    pub started_at: DateTime<Utc>,
    /// Last evaluation time
    pub last_evaluated: DateTime<Utc>,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Additional context
    pub annotations: HashMap<String, String>,
}

/// Alert history entry for audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertHistoryEntry {
    /// Alert rule ID
    pub rule_id: String,
    /// Rule name
    pub rule_name: String,
    /// Event type (fired, resolved)
    pub event_type: AlertEventType,
    /// Metric value at time of event
    pub metric_value: f64,
    /// Timestamp of event
    pub timestamp: DateTime<Utc>,
    /// Additional details
    pub details: HashMap<String, String>,
}

/// Alert event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertEventType {
    /// Alert started firing
    Fired,
    /// Alert condition resolved
    Resolved,
    /// Alert acknowledged
    Acknowledged,
    /// Alert silenced
    Silenced,
}

/// Notification channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    /// Channel identifier
    pub id: String,
    /// Channel name
    pub name: String,
    /// Channel type
    pub channel_type: ChannelType,
    /// Channel-specific configuration
    pub config: HashMap<String, String>,
    /// Whether channel is enabled
    pub enabled: bool,
}

/// Types of notification channels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelType {
    /// Email notifications
    Email,
    /// Slack webhook
    Slack,
    /// PagerDuty integration
    PagerDuty,
    /// Webhook HTTP POST
    Webhook,
    /// SMS notifications
    SMS,
    /// Custom integration
    Custom,
}

impl AlertManager {
    /// Create a new alert manager
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
            notification_channels: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add an alert rule
    pub async fn add_rule(&self, rule: AlertRule) -> PrometheusResult<()> {
        let mut rules = self.rules.write().await;
        rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    /// Remove an alert rule
    pub async fn remove_rule(&self, rule_id: &str) -> PrometheusResult<Option<AlertRule>> {
        let mut rules = self.rules.write().await;
        Ok(rules.remove(rule_id))
    }

    /// Get all alert rules
    pub async fn get_rules(&self) -> PrometheusResult<Vec<AlertRule>> {
        let rules = self.rules.read().await;
        Ok(rules.values().cloned().collect())
    }

    /// Get a specific alert rule
    pub async fn get_rule(&self, rule_id: &str) -> PrometheusResult<Option<AlertRule>> {
        let rules = self.rules.read().await;
        Ok(rules.get(rule_id).cloned())
    }

    /// Evaluate a metric value against alert rules
    pub async fn evaluate_metric(
        &self,
        metric_name: &str,
        value: f64,
    ) -> PrometheusResult<Vec<String>> {
        let rules = self.rules.read().await;
        let mut triggered_alerts = Vec::new();

        for (rule_id, rule) in rules.iter() {
            if !rule.enabled || rule.metric_name != metric_name {
                continue;
            }

            if self.evaluate_condition(value, rule.threshold, &rule.operator) {
                triggered_alerts.push(rule_id.clone());
                self.fire_alert(rule, value).await?;
            } else {
                self.resolve_alert(rule_id).await?;
            }
        }

        Ok(triggered_alerts)
    }

    /// Evaluate a condition
    fn evaluate_condition(&self, value: f64, threshold: f64, operator: &AlertOperator) -> bool {
        match operator {
            AlertOperator::GreaterThan => value > threshold,
            AlertOperator::GreaterThanOrEqual => value >= threshold,
            AlertOperator::LessThan => value < threshold,
            AlertOperator::LessThanOrEqual => value <= threshold,
            AlertOperator::Equal => (value - threshold).abs() < f64::EPSILON,
            AlertOperator::NotEqual => (value - threshold).abs() >= f64::EPSILON,
        }
    }

    /// Fire an alert
    async fn fire_alert(&self, rule: &AlertRule, value: f64) -> PrometheusResult<()> {
        let mut active_alerts = self.active_alerts.write().await;
        
        if active_alerts.contains_key(&rule.id) {
            // Alert already active, update last evaluated
            if let Some(alert) = active_alerts.get_mut(&rule.id) {
                alert.current_value = value;
                alert.last_evaluated = Utc::now();
            }
        } else {
            // New alert firing
            let alert = ActiveAlert {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                current_value: value,
                threshold_value: rule.threshold,
                started_at: Utc::now(),
                last_evaluated: Utc::now(),
                severity: rule.severity.clone(),
                annotations: HashMap::new(),
            };
            
            active_alerts.insert(rule.id.clone(), alert);
            
            // Record in history
            self.record_alert_event(
                &rule.id,
                &rule.name,
                AlertEventType::Fired,
                value,
            ).await?;
        }

        Ok(())
    }

    /// Resolve an alert
    async fn resolve_alert(&self, rule_id: &str) -> PrometheusResult<()> {
        let mut active_alerts = self.active_alerts.write().await;
        
        if let Some(alert) = active_alerts.remove(rule_id) {
            // Record resolution in history
            self.record_alert_event(
                rule_id,
                &alert.rule_name,
                AlertEventType::Resolved,
                alert.current_value,
            ).await?;
        }

        Ok(())
    }

    /// Record an alert event in history
    async fn record_alert_event(
        &self,
        rule_id: &str,
        rule_name: &str,
        event_type: AlertEventType,
        metric_value: f64,
    ) -> PrometheusResult<()> {
        let mut history = self.alert_history.write().await;
        
        history.push(AlertHistoryEntry {
            rule_id: rule_id.to_string(),
            rule_name: rule_name.to_string(),
            event_type,
            metric_value,
            timestamp: Utc::now(),
            details: HashMap::new(),
        });

        Ok(())
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> PrometheusResult<Vec<ActiveAlert>> {
        let active_alerts = self.active_alerts.read().await;
        Ok(active_alerts.values().cloned().collect())
    }

    /// Get alert history
    pub async fn get_alert_history(
        &self,
        limit: Option<usize>,
    ) -> PrometheusResult<Vec<AlertHistoryEntry>> {
        let history = self.alert_history.read().await;
        
        let entries: Vec<_> = if let Some(limit) = limit {
            history.iter().rev().take(limit).cloned().collect()
        } else {
            history.iter().rev().cloned().collect()
        };

        Ok(entries)
    }

    /// Add a notification channel
    pub async fn add_channel(&self, channel: NotificationChannel) -> PrometheusResult<()> {
        let mut channels = self.notification_channels.write().await;
        channels.push(channel);
        Ok(())
    }

    /// Get notification channels
    pub async fn get_channels(&self) -> PrometheusResult<Vec<NotificationChannel>> {
        let channels = self.notification_channels.read().await;
        Ok(channels.clone())
    }

    /// Clear alert history
    pub async fn clear_history(&self) -> PrometheusResult<()> {
        let mut history = self.alert_history.write().await;
        history.clear();
        Ok(())
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_alert_rule_creation() {
        let manager = AlertManager::new();
        
        let rule = AlertRule {
            id: "test-rule-1".to_string(),
            name: "High CPU Usage".to_string(),
            description: "CPU usage exceeds 80%".to_string(),
            metric_name: "cpu_usage".to_string(),
            operator: AlertOperator::GreaterThan,
            threshold: 80.0,
            window_seconds: 60,
            severity: AlertSeverity::Warning,
            enabled: true,
            labels: HashMap::new(),
            channels: vec!["email".to_string()],
        };

        manager.add_rule(rule.clone()).await.unwrap();
        
        let retrieved = manager.get_rule("test-rule-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "High CPU Usage");
    }

    #[tokio::test]
    async fn test_alert_evaluation() {
        let manager = AlertManager::new();
        
        let rule = AlertRule {
            id: "test-rule-2".to_string(),
            name: "High Memory Usage".to_string(),
            description: "Memory usage exceeds 90%".to_string(),
            metric_name: "memory_usage".to_string(),
            operator: AlertOperator::GreaterThan,
            threshold: 90.0,
            window_seconds: 60,
            severity: AlertSeverity::Critical,
            enabled: true,
            labels: HashMap::new(),
            channels: vec![],
        };

        manager.add_rule(rule).await.unwrap();
        
        // Should trigger alert
        let triggered = manager.evaluate_metric("memory_usage", 95.0).await.unwrap();
        assert_eq!(triggered.len(), 1);
        
        // Check active alerts
        let active = manager.get_active_alerts().await.unwrap();
        assert_eq!(active.len(), 1);
        
        // Should resolve alert
        let _ = manager.evaluate_metric("memory_usage", 85.0).await.unwrap();
        let active = manager.get_active_alerts().await.unwrap();
        assert_eq!(active.len(), 0);
    }
}
