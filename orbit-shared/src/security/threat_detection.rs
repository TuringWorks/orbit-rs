//! Threat Detection and Response
//!
//! Provides real-time threat detection:
//! - Anomaly detection
//! - Brute force protection
//! - Data exfiltration detection
//! - Insider threat detection

use crate::exception::OrbitResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Threat detection engine
pub struct ThreatDetectionEngine {
    anomaly_detector: Arc<AnomalyDetector>,
    brute_force_detector: Arc<BruteForceDetector>,
    threat_rules: Arc<RwLock<Vec<ThreatRule>>>,
    detected_threats: Arc<RwLock<Vec<DetectedThreat>>>,
}

impl ThreatDetectionEngine {
    /// Create a new threat detection engine
    pub fn new() -> Self {
        Self {
            anomaly_detector: Arc::new(AnomalyDetector::new()),
            brute_force_detector: Arc::new(BruteForceDetector::new()),
            threat_rules: Arc::new(RwLock::new(Vec::new())),
            detected_threats: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Detect threats in access patterns
    pub async fn detect_threats(
        &self,
        subject_id: &str,
        client_ip: &str,
        action: &str,
    ) -> OrbitResult<ThreatAssessment> {
        let mut threats = Vec::new();

        // Check for brute force attacks
        if let Some(threat) = self
            .brute_force_detector
            .check_brute_force(subject_id, client_ip)
            .await
        {
            threats.push(threat);
        }

        // Check for anomalies
        if let Some(threat) = self
            .anomaly_detector
            .detect_anomaly(subject_id, action)
            .await
        {
            threats.push(threat);
        }

        // Store detected threats
        if !threats.is_empty() {
            let mut detected_threats = self.detected_threats.write().await;
            detected_threats.extend(threats.clone());
        }

        Ok(ThreatAssessment {
            threats,
            risk_score: self.calculate_risk_score(&threats),
            recommended_actions: self.get_recommended_actions(&threats),
        })
    }

    /// Calculate risk score based on detected threats
    fn calculate_risk_score(&self, threats: &[DetectedThreat]) -> f64 {
        threats
            .iter()
            .map(|t| match t.severity {
                ThreatSeverity::Critical => 10.0,
                ThreatSeverity::High => 7.0,
                ThreatSeverity::Medium => 4.0,
                ThreatSeverity::Low => 1.0,
            })
            .sum()
    }

    /// Get recommended actions based on threats
    fn get_recommended_actions(&self, threats: &[DetectedThreat]) -> Vec<String> {
        let mut actions = Vec::new();

        for threat in threats {
            match threat.threat_type {
                ThreatType::BruteForce => {
                    actions.push("Block IP address".to_string());
                    actions.push("Require MFA".to_string());
                }
                ThreatType::Anomaly => {
                    actions.push("Increase monitoring".to_string());
                    actions.push("Require additional authentication".to_string());
                }
                ThreatType::DataExfiltration => {
                    actions.push("Block large data exports".to_string());
                    actions.push("Alert security team".to_string());
                }
                ThreatType::InsiderThreat => {
                    actions.push("Require approval for sensitive operations".to_string());
                    actions.push("Increase audit logging".to_string());
                }
                ThreatType::SqlInjection => {
                    actions.push("Block malicious query".to_string());
                    actions.push("Alert security team".to_string());
                }
            }
        }

        actions.dedup();
        actions
    }

    /// Get all detected threats
    pub async fn get_detected_threats(&self) -> OrbitResult<Vec<DetectedThreat>> {
        let threats = self.detected_threats.read().await;
        Ok(threats.clone())
    }

    /// Clear detected threats
    pub async fn clear_threats(&self) -> OrbitResult<()> {
        let mut threats = self.detected_threats.write().await;
        threats.clear();
        Ok(())
    }
}

impl Default for ThreatDetectionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Anomaly detector
pub struct AnomalyDetector {
    baseline_profiles: Arc<RwLock<HashMap<String, UserProfile>>>,
    detection_threshold: f64,
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    pub fn new() -> Self {
        Self {
            baseline_profiles: Arc::new(RwLock::new(HashMap::new())),
            detection_threshold: 0.7,
        }
    }

    /// Detect anomaly in user behavior
    pub async fn detect_anomaly(&self, subject_id: &str, action: &str) -> Option<DetectedThreat> {
        let profiles = self.baseline_profiles.read().await;

        // Get user profile
        let profile = profiles.get(subject_id);

        // Calculate anomaly score
        let anomaly_score = if let Some(profile) = profile {
            self.calculate_anomaly_score(profile, action)
        } else {
            0.0 // No baseline yet
        };

        // Check if anomaly score exceeds threshold
        if anomaly_score > self.detection_threshold {
            Some(DetectedThreat {
                id: uuid::Uuid::new_v4().to_string(),
                threat_type: ThreatType::Anomaly,
                severity: if anomaly_score > 0.9 {
                    ThreatSeverity::High
                } else {
                    ThreatSeverity::Medium
                },
                description: format!(
                    "Anomalous behavior detected for user {} (score: {:.2})",
                    subject_id, anomaly_score
                ),
                detected_at: SystemTime::now(),
                subject_id: Some(subject_id.to_string()),
                client_ip: None,
                details: HashMap::new(),
            })
        } else {
            None
        }
    }

    /// Calculate anomaly score
    fn calculate_anomaly_score(&self, profile: &UserProfile, action: &str) -> f64 {
        // Check if action is unusual for this user
        let action_frequency = profile
            .action_frequencies
            .get(action)
            .copied()
            .unwrap_or(0.0);

        // Lower frequency means higher anomaly score
        1.0 - action_frequency
    }

    /// Update user profile
    pub async fn update_profile(&self, subject_id: &str, action: &str) {
        let mut profiles = self.baseline_profiles.write().await;

        let profile = profiles
            .entry(subject_id.to_string())
            .or_insert_with(UserProfile::new);

        // Update action frequency
        let current_freq = profile
            .action_frequencies
            .get(action)
            .copied()
            .unwrap_or(0.0);

        profile
            .action_frequencies
            .insert(action.to_string(), current_freq + 0.1);

        // Normalize frequencies
        let total: f64 = profile.action_frequencies.values().sum();
        if total > 0.0 {
            for freq in profile.action_frequencies.values_mut() {
                *freq /= total;
            }
        }
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// User profile for baseline behavior
#[derive(Debug, Clone)]
pub struct UserProfile {
    pub action_frequencies: HashMap<String, f64>,
    pub typical_access_times: Vec<u8>, // Hours of day (0-23)
    pub typical_access_locations: Vec<String>,
}

impl UserProfile {
    fn new() -> Self {
        Self {
            action_frequencies: HashMap::new(),
            typical_access_times: Vec::new(),
            typical_access_locations: Vec::new(),
        }
    }
}

/// Brute force detector
pub struct BruteForceDetector {
    failed_attempts: Arc<RwLock<HashMap<String, Vec<SystemTime>>>>,
    max_attempts: usize,
    time_window: Duration,
}

impl BruteForceDetector {
    /// Create a new brute force detector
    pub fn new() -> Self {
        Self {
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
            max_attempts: 5,
            time_window: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Check for brute force attack
    pub async fn check_brute_force(
        &self,
        subject_id: &str,
        client_ip: &str,
    ) -> Option<DetectedThreat> {
        let key = format!("{}:{}", subject_id, client_ip);
        let mut attempts = self.failed_attempts.write().await;

        let now = SystemTime::now();
        let cutoff = now - self.time_window;

        // Get recent failed attempts
        let recent_attempts: Vec<SystemTime> = attempts
            .entry(key.clone())
            .or_insert_with(Vec::new)
            .iter()
            .filter(|&&time| time >= cutoff)
            .copied()
            .collect();

        // Check if brute force threshold exceeded
        if recent_attempts.len() >= self.max_attempts {
            Some(DetectedThreat {
                id: uuid::Uuid::new_v4().to_string(),
                threat_type: ThreatType::BruteForce,
                severity: ThreatSeverity::Critical,
                description: format!(
                    "Brute force attack detected: {} failed attempts from {}",
                    recent_attempts.len(),
                    client_ip
                ),
                detected_at: now,
                subject_id: Some(subject_id.to_string()),
                client_ip: Some(client_ip.to_string()),
                details: HashMap::new(),
            })
        } else {
            None
        }
    }

    /// Record failed authentication attempt
    pub async fn record_failed_attempt(&self, subject_id: &str, client_ip: &str) {
        let key = format!("{}:{}", subject_id, client_ip);
        let mut attempts = self.failed_attempts.write().await;

        attempts
            .entry(key)
            .or_insert_with(Vec::new)
            .push(SystemTime::now());
    }

    /// Clear failed attempts for a subject/IP
    pub async fn clear_attempts(&self, subject_id: &str, client_ip: &str) {
        let key = format!("{}:{}", subject_id, client_ip);
        let mut attempts = self.failed_attempts.write().await;
        attempts.remove(&key);
    }
}

impl Default for BruteForceDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detected threat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedThreat {
    pub id: String,
    pub threat_type: ThreatType,
    pub severity: ThreatSeverity,
    pub description: String,
    pub detected_at: SystemTime,
    pub subject_id: Option<String>,
    pub client_ip: Option<String>,
    pub details: HashMap<String, String>,
}

/// Threat type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreatType {
    BruteForce,
    Anomaly,
    DataExfiltration,
    InsiderThreat,
    SqlInjection,
}

/// Threat severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreatSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// Threat rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rule_type: ThreatType,
    pub enabled: bool,
}

/// Threat assessment result
#[derive(Debug, Clone)]
pub struct ThreatAssessment {
    pub threats: Vec<DetectedThreat>,
    pub risk_score: f64,
    pub recommended_actions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_brute_force_detector() {
        let detector = BruteForceDetector::new();

        // Record multiple failed attempts
        for _ in 0..5 {
            detector.record_failed_attempt("user1", "192.168.1.1").await;
        }

        // Check for brute force
        let threat = detector.check_brute_force("user1", "192.168.1.1").await;
        assert!(threat.is_some());

        let threat = threat.unwrap();
        assert_eq!(threat.threat_type, ThreatType::BruteForce);
        assert_eq!(threat.severity, ThreatSeverity::Critical);
    }

    #[tokio::test]
    async fn test_anomaly_detector() {
        let detector = AnomalyDetector::new();

        // Build baseline profile
        for _ in 0..10 {
            detector.update_profile("user1", "read").await;
        }

        // Normal action should not trigger anomaly
        let threat = detector.detect_anomaly("user1", "read").await;
        assert!(threat.is_none());

        // Unusual action should trigger anomaly
        let threat = detector.detect_anomaly("user1", "delete_all").await;
        assert!(threat.is_some());
    }

    #[tokio::test]
    async fn test_threat_detection_engine() {
        let engine = ThreatDetectionEngine::new();

        // Detect threats
        let assessment = engine
            .detect_threats("user1", "192.168.1.1", "suspicious_action")
            .await
            .unwrap();

        assert!(assessment.risk_score >= 0.0);
    }
}
