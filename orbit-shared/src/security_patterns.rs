//! Advanced security patterns for Orbit applications
//!
//! This module provides comprehensive security utilities including rate limiting,
//! input sanitization, secure configuration management, and attack prevention.

use crate::error_handling::SecurityValidator;
use crate::exception::OrbitError;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Advanced rate limiter with sliding window algorithm
#[derive(Debug)]
pub struct RateLimiter {
    windows: Arc<RwLock<HashMap<String, SlidingWindow>>>,
    max_requests: u32,
    window_duration: Duration,
    cleanup_interval: Duration,
}

/// Sliding window for rate limiting
#[derive(Debug)]
struct SlidingWindow {
    requests: Vec<Instant>,
    last_cleanup: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(max_requests: u32, window_duration: Duration) -> Self {
        Self {
            windows: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window_duration,
            cleanup_interval: Duration::from_secs(60),
        }
    }

    /// Check if a request is allowed for the given client ID
    pub async fn is_allowed(&self, client_id: &str) -> Result<bool, OrbitError> {
        let now = Instant::now();
        let mut windows = self.windows.write().await;

        let window = windows
            .entry(client_id.to_string())
            .or_insert_with(|| SlidingWindow {
                requests: Vec::new(),
                last_cleanup: now,
            });

        // Clean old requests
        window
            .requests
            .retain(|&request_time| now.duration_since(request_time) <= self.window_duration);

        // Check if we're at the limit
        if window.requests.len() >= self.max_requests as usize {
            warn!("Rate limit exceeded for client: {}", client_id);
            return Ok(false);
        }

        // Add current request
        window.requests.push(now);
        window.last_cleanup = now;

        // Periodic cleanup of old windows
        if now.duration_since(window.last_cleanup) > self.cleanup_interval {
            self.cleanup_old_windows(&mut windows, now).await;
        }

        Ok(true)
    }

    /// Get current usage statistics for a client
    pub async fn get_usage(&self, client_id: &str) -> Result<RateLimitStats, OrbitError> {
        let windows = self.windows.read().await;

        if let Some(window) = windows.get(client_id) {
            let now = Instant::now();
            let active_requests = window
                .requests
                .iter()
                .filter(|&&request_time| now.duration_since(request_time) <= self.window_duration)
                .count();

            Ok(RateLimitStats {
                current_requests: active_requests as u32,
                max_requests: self.max_requests,
                window_duration: self.window_duration,
                reset_time: window
                    .requests
                    .first()
                    .map(|&first| first + self.window_duration)
                    .unwrap_or(now),
            })
        } else {
            let now = Instant::now();
            Ok(RateLimitStats {
                current_requests: 0,
                max_requests: self.max_requests,
                window_duration: self.window_duration,
                reset_time: now + self.window_duration,
            })
        }
    }

    /// Clean up old windows to prevent memory leaks
    async fn cleanup_old_windows(
        &self,
        windows: &mut HashMap<String, SlidingWindow>,
        now: Instant,
    ) {
        let cutoff = now - self.window_duration - self.cleanup_interval;

        windows.retain(|_, window| window.requests.iter().any(|&req_time| req_time > cutoff));
    }
}

/// Rate limit statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub current_requests: u32,
    pub max_requests: u32,
    pub window_duration: Duration,
    pub reset_time: Instant,
}

/// Secure configuration manager with environment variable validation
#[derive(Debug)]
pub struct SecureConfigManager {
    sensitive_keys: Vec<String>,
    validation_patterns: HashMap<String, String>,
}

impl SecureConfigManager {
    /// Create a new secure configuration manager
    pub fn new() -> Self {
        Self {
            sensitive_keys: vec![
                "password".to_string(),
                "secret".to_string(),
                "key".to_string(),
                "token".to_string(),
                "credential".to_string(),
            ],
            validation_patterns: HashMap::new(),
        }
    }

    /// Add a key to the sensitive keys list
    pub fn add_sensitive_key<S: Into<String>>(&mut self, key: S) -> &mut Self {
        self.sensitive_keys.push(key.into());
        self
    }

    /// Add validation pattern for a configuration key
    pub fn add_validation_pattern<K, P>(&mut self, key: K, pattern: P) -> &mut Self
    where
        K: Into<String>,
        P: Into<String>,
    {
        self.validation_patterns.insert(key.into(), pattern.into());
        self
    }

    /// Securely get configuration value with validation
    pub fn get_config(&self, key: &str) -> Result<Option<String>, OrbitError> {
        // Check if key is sensitive and log appropriately
        let is_sensitive = self
            .sensitive_keys
            .iter()
            .any(|sensitive| key.to_lowercase().contains(&sensitive.to_lowercase()));

        let value = std::env::var(key).ok();

        if let Some(ref val) = value {
            // Validate against pattern if available
            if let Some(pattern) = self.validation_patterns.get(key) {
                val.validate_allowed_chars(pattern)?;
            }

            // Basic security validation
            val.validate_length(10000)?; // Prevent extremely long values

            if is_sensitive {
                info!(
                    "Loaded sensitive configuration key: {} (length: {})",
                    key,
                    val.len()
                );
            } else {
                info!("Loaded configuration key: {} = {}", key, val);
            }
        } else if is_sensitive {
            warn!("Sensitive configuration key not found: {}", key);
        }

        Ok(value)
    }

    /// Get required configuration value
    pub fn get_required_config(&self, key: &str) -> Result<String, OrbitError> {
        self.get_config(key)?.ok_or_else(|| {
            OrbitError::internal(format!("Required configuration key missing: {}", key))
        })
    }
}

/// Input sanitization utilities
pub struct InputSanitizer;

impl InputSanitizer {
    /// Sanitize HTML content
    pub fn sanitize_html(input: &str) -> String {
        input
            .replace('&', "&amp;") // Replace & first to avoid double-encoding
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }

    /// Sanitize for SQL context (basic escaping)
    pub fn sanitize_sql_identifier(input: &str) -> Result<String, OrbitError> {
        // Validate that identifier contains only safe characters
        input.validate_allowed_chars(r"^[a-zA-Z_][a-zA-Z0-9_]*$")?;
        input.validate_length(64)?; // Reasonable identifier length limit

        Ok(input.to_string())
    }

    /// Sanitize file path to prevent directory traversal
    pub fn sanitize_file_path(input: &str) -> Result<String, OrbitError> {
        // Basic path traversal protection
        if input.contains("..") || input.contains("//") {
            return Err(OrbitError::internal(
                "Path contains dangerous patterns".to_string(),
            ));
        }

        // Validate allowed characters for file paths
        input.validate_allowed_chars(r"^[a-zA-Z0-9._/-]+$")?;
        input.validate_length(255)?; // Standard path length limit

        Ok(input.to_string())
    }

    /// Remove or escape dangerous characters from user input
    pub fn sanitize_user_input(input: &str) -> String {
        input
            .chars()
            .filter(|&c| {
                // Allow alphanumeric, space, and common safe punctuation
                c.is_alphanumeric() || " .,!?-_@#$%()[]{}=+:;".contains(c)
            })
            .collect::<String>()
            .trim()
            .to_string()
    }
}

/// Security event logging
pub struct SecurityAuditLogger {
    sensitive_operations: Vec<String>,
}

impl SecurityAuditLogger {
    pub fn new() -> Self {
        Self {
            sensitive_operations: vec![
                "login".to_string(),
                "authentication".to_string(),
                "authorization".to_string(),
                "password_change".to_string(),
                "admin_action".to_string(),
                "data_export".to_string(),
                "configuration_change".to_string(),
            ],
        }
    }

    /// Log security-relevant events
    pub fn log_security_event(
        &self,
        operation: &str,
        user_id: Option<&str>,
        client_ip: Option<&str>,
        success: bool,
        details: Option<&str>,
    ) {
        let is_sensitive = self
            .sensitive_operations
            .iter()
            .any(|op| operation.to_lowercase().contains(&op.to_lowercase()));

        let log_level = if is_sensitive || !success {
            tracing::Level::WARN
        } else {
            tracing::Level::INFO
        };

        let user = user_id.unwrap_or("unknown");
        let ip = client_ip.unwrap_or("unknown");
        let status = if success { "SUCCESS" } else { "FAILURE" };
        let extra = details.unwrap_or("");

        match log_level {
            tracing::Level::WARN => warn!(
                operation = operation,
                user = user,
                client_ip = ip,
                status = status,
                details = extra,
                "Security event logged"
            ),
            _ => info!(
                operation = operation,
                user = user,
                client_ip = ip,
                status = status,
                details = extra,
                "Security event logged"
            ),
        }

        // Log failed authentication attempts at error level
        if !success && operation.contains("auth") {
            error!(
                operation = operation,
                user = user,
                client_ip = ip,
                "Authentication failure detected"
            );
        }
    }
}

/// Attack detection patterns
pub struct AttackDetector {
    suspicious_patterns: Vec<regex::Regex>,
    rate_limiter: RateLimiter,
}

impl AttackDetector {
    pub fn new() -> Result<Self, OrbitError> {
        let patterns = vec![
            // SQL injection patterns
            r"(?i)(union|select|insert|delete|drop|alter|create|exec|execute)",
            // XSS patterns
            r"(?i)(<script|javascript:|vbscript:|onload=|onerror=)",
            // Command injection patterns
            r"(?i)(;|\||&|`|\$\(|\$\{|eval\()",
            // Path traversal patterns
            r"(\.\./|\.\.\\|%2e%2e%2f|%2e%2e%5c)",
        ];

        let mut compiled_patterns = Vec::new();
        for pattern in patterns {
            compiled_patterns.push(
                regex::Regex::new(pattern)
                    .map_err(|e| OrbitError::internal(format!("Invalid regex pattern: {}", e)))?,
            );
        }

        Ok(Self {
            suspicious_patterns: compiled_patterns,
            rate_limiter: RateLimiter::new(100, Duration::from_secs(60)), // 100 requests per minute
        })
    }

    /// Detect potential attacks in input
    pub async fn detect_attack(
        &self,
        input: &str,
        client_id: &str,
    ) -> Result<AttackDetectionResult, OrbitError> {
        // Check rate limiting first
        if !self.rate_limiter.is_allowed(client_id).await? {
            return Ok(AttackDetectionResult {
                is_attack: true,
                attack_type: "rate_limit_exceeded".to_string(),
                confidence: 1.0,
                details: "Client exceeded rate limit".to_string(),
            });
        }

        // Check for suspicious patterns
        for (i, pattern) in self.suspicious_patterns.iter().enumerate() {
            if pattern.is_match(input) {
                let attack_type = match i {
                    0 => "sql_injection",
                    1 => "xss_attempt",
                    2 => "command_injection",
                    3 => "path_traversal",
                    _ => "unknown",
                };

                return Ok(AttackDetectionResult {
                    is_attack: true,
                    attack_type: attack_type.to_string(),
                    confidence: 0.8 + (i as f32 * 0.05), // Varying confidence levels
                    details: format!("Suspicious pattern detected: {}", pattern.as_str()),
                });
            }
        }

        Ok(AttackDetectionResult {
            is_attack: false,
            attack_type: "none".to_string(),
            confidence: 0.0,
            details: "No suspicious patterns detected".to_string(),
        })
    }
}

/// Attack detection result
#[derive(Debug, Clone)]
pub struct AttackDetectionResult {
    pub is_attack: bool,
    pub attack_type: String,
    pub confidence: f32,
    pub details: String,
}

impl Default for SecureConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SecurityAuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_rate_limiter() {
        let rate_limiter = RateLimiter::new(5, Duration::from_secs(1));
        let client_id = "test_client";

        // Should allow first 5 requests
        for _ in 0..5 {
            assert!(rate_limiter.is_allowed(client_id).await.unwrap());
        }

        // 6th request should be denied
        assert!(!rate_limiter.is_allowed(client_id).await.unwrap());

        // Wait for window to reset
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should allow requests again
        assert!(rate_limiter.is_allowed(client_id).await.unwrap());
    }

    #[test]
    fn test_secure_config_manager() {
        std::env::set_var("TEST_KEY", "test_value");

        let config_manager = SecureConfigManager::new();
        let result = config_manager.get_config("TEST_KEY").unwrap();

        assert_eq!(result, Some("test_value".to_string()));

        std::env::remove_var("TEST_KEY");
    }

    #[test]
    fn test_input_sanitizer() {
        let html_input = "<script>alert('xss')</script>";
        let sanitized = InputSanitizer::sanitize_html(html_input);
        assert!(!sanitized.contains("<script>"));
        assert!(sanitized.contains("&lt;script&gt;"));

        let dangerous_path = "../../../etc/passwd";
        let result = InputSanitizer::sanitize_file_path(dangerous_path);
        assert!(result.is_err());

        let safe_input = "Hello, World! 123";
        let sanitized = InputSanitizer::sanitize_user_input(safe_input);
        assert_eq!(sanitized, "Hello, World! 123");
    }

    #[tokio::test]
    async fn test_attack_detector() {
        let detector = AttackDetector::new().unwrap();

        // Test SQL injection detection
        let sql_attack = "'; DROP TABLE users; --";
        let result = detector
            .detect_attack(sql_attack, "test_client")
            .await
            .unwrap();
        assert!(result.is_attack);
        assert_eq!(result.attack_type, "sql_injection");

        // Test safe input
        let safe_input = "Hello world";
        let result = detector
            .detect_attack(safe_input, "test_client")
            .await
            .unwrap();
        assert!(!result.is_attack);
    }

    #[test]
    fn test_security_audit_logger() {
        let logger = SecurityAuditLogger::new();

        // This test mainly ensures the logger doesn't panic
        logger.log_security_event(
            "login",
            Some("user123"),
            Some("192.168.1.100"),
            true,
            Some("Successful login"),
        );

        logger.log_security_event(
            "authentication",
            Some("user456"),
            Some("192.168.1.101"),
            false,
            Some("Failed authentication attempt"),
        );
    }
}
