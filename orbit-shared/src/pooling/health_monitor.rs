//! Connection health monitoring implementation

use crate::exception::OrbitResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Health status of a connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Connection is healthy and operational
    Healthy,
    /// Connection is degraded but still functional
    Degraded,
    /// Connection has failed
    Failed,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    /// Current health status
    pub status: HealthStatus,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Last check timestamp
    pub last_check: Instant,
    /// Error message if failed
    pub error: Option<String>,
}

impl ConnectionHealth {
    pub fn healthy(response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Healthy,
            response_time_ms,
            last_check: Instant::now(),
            error: None,
        }
    }

    pub fn degraded(response_time_ms: u64, reason: String) -> Self {
        Self {
            status: HealthStatus::Degraded,
            response_time_ms,
            last_check: Instant::now(),
            error: Some(reason),
        }
    }

    pub fn failed(error: String) -> Self {
        Self {
            status: HealthStatus::Failed,
            response_time_ms: 0,
            last_check: Instant::now(),
            error: Some(error),
        }
    }
}

/// Trait for health check implementations
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Perform a health check on the connection
    async fn check_health(&self) -> OrbitResult<ConnectionHealth>;

    /// Get the health check interval
    fn check_interval(&self) -> Duration;
}

/// Connection health monitoring
pub struct ConnectionHealthMonitor {
    health_checks: Vec<Arc<dyn HealthCheck>>,
    health_status: Arc<RwLock<Vec<ConnectionHealth>>>,
    check_interval: Duration,
    response_time_threshold_ms: u64,
    degraded_threshold_ms: u64,
}

impl ConnectionHealthMonitor {
    /// Create a new health monitor
    pub fn new(check_interval: Duration) -> Self {
        Self {
            health_checks: Vec::new(),
            health_status: Arc::new(RwLock::new(Vec::new())),
            check_interval,
            response_time_threshold_ms: 1000,
            degraded_threshold_ms: 500,
        }
    }

    /// Add a health check
    pub fn add_health_check(&mut self, check: Arc<dyn HealthCheck>) {
        self.health_checks.push(check);
    }

    /// Set response time thresholds
    pub fn with_thresholds(mut self, degraded_ms: u64, failed_ms: u64) -> Self {
        self.degraded_threshold_ms = degraded_ms;
        self.response_time_threshold_ms = failed_ms;
        self
    }

    /// Start monitoring connections
    pub async fn start_monitoring(&self) {
        let health_checks = self.health_checks.clone();
        let health_status = Arc::clone(&self.health_status);
        let interval = self.check_interval;
        let degraded_threshold = self.degraded_threshold_ms;
        let failed_threshold = self.response_time_threshold_ms;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;

                let mut results = Vec::new();

                for check in &health_checks {
                    match check.check_health().await {
                        Ok(mut health) => {
                            // Adjust status based on response time
                            if health.status == HealthStatus::Healthy {
                                if health.response_time_ms >= failed_threshold {
                                    health.status = HealthStatus::Failed;
                                    health.error = Some(format!(
                                        "Response time {}ms exceeds threshold {}ms",
                                        health.response_time_ms, failed_threshold
                                    ));
                                } else if health.response_time_ms >= degraded_threshold {
                                    health.status = HealthStatus::Degraded;
                                    health.error = Some(format!(
                                        "Response time {}ms exceeds degraded threshold {}ms",
                                        health.response_time_ms, degraded_threshold
                                    ));
                                }
                            }

                            if health.status != HealthStatus::Healthy {
                                debug!(
                                    "Connection health check: {:?} - {:?}",
                                    health.status, health.error
                                );
                            }

                            results.push(health);
                        }
                        Err(e) => {
                            warn!("Health check failed: {}", e);
                            results.push(ConnectionHealth::failed(e.to_string()));
                        }
                    }
                }

                let mut status = health_status.write().await;
                *status = results;
            }
        });
    }

    /// Get the current health status of all connections
    pub async fn get_health_status(&self) -> Vec<ConnectionHealth> {
        self.health_status.read().await.clone()
    }

    /// Get the overall health status
    pub async fn get_overall_status(&self) -> HealthStatus {
        let statuses = self.health_status.read().await;

        if statuses.is_empty() {
            return HealthStatus::Healthy;
        }

        let has_failed = statuses.iter().any(|s| s.status == HealthStatus::Failed);
        let has_degraded = statuses.iter().any(|s| s.status == HealthStatus::Degraded);

        if has_failed {
            HealthStatus::Failed
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Clone for ConnectionHealthMonitor {
    fn clone(&self) -> Self {
        Self {
            health_checks: self.health_checks.clone(),
            health_status: Arc::clone(&self.health_status),
            check_interval: self.check_interval,
            response_time_threshold_ms: self.response_time_threshold_ms,
            degraded_threshold_ms: self.degraded_threshold_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHealthCheck {
        response_time: u64,
        should_fail: bool,
    }

    #[async_trait]
    impl HealthCheck for MockHealthCheck {
        async fn check_health(&self) -> OrbitResult<ConnectionHealth> {
            if self.should_fail {
                Ok(ConnectionHealth::failed("Mock failure".to_string()))
            } else {
                Ok(ConnectionHealth::healthy(self.response_time))
            }
        }

        fn check_interval(&self) -> Duration {
            Duration::from_millis(100)
        }
    }

    #[tokio::test]
    async fn test_health_monitor_healthy() {
        let mut monitor = ConnectionHealthMonitor::new(Duration::from_millis(100));
        monitor = monitor.with_thresholds(500, 1000);

        let check = Arc::new(MockHealthCheck {
            response_time: 100,
            should_fail: false,
        });

        monitor.add_health_check(check);
        monitor.start_monitoring().await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        let status = monitor.get_overall_status().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_monitor_degraded() {
        let mut monitor = ConnectionHealthMonitor::new(Duration::from_millis(100));
        monitor = monitor.with_thresholds(500, 1000);

        let check = Arc::new(MockHealthCheck {
            response_time: 600,
            should_fail: false,
        });

        monitor.add_health_check(check);
        monitor.start_monitoring().await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        let status = monitor.get_overall_status().await;
        assert_eq!(status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn test_health_monitor_failed() {
        let mut monitor = ConnectionHealthMonitor::new(Duration::from_millis(100));

        let check = Arc::new(MockHealthCheck {
            response_time: 0,
            should_fail: true,
        });

        monitor.add_health_check(check);
        monitor.start_monitoring().await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        let status = monitor.get_overall_status().await;
        assert_eq!(status, HealthStatus::Failed);
    }
}
