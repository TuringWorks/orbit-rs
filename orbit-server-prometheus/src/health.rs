//! Health check endpoints and monitoring
//!
//! This module provides health check functionality for monitoring systems
//! to verify service availability and component health.

use crate::{PrometheusError, PrometheusResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Health check manager
pub struct HealthCheckManager {
    /// Component health status
    components: Arc<RwLock<HashMap<String, ComponentHealth>>>,
    /// Last health check timestamp
    last_check: Arc<RwLock<DateTime<Utc>>>,
    /// Health check configuration
    config: HealthCheckConfig,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Check interval in seconds
    pub check_interval_seconds: u64,
    /// Timeout for individual checks in seconds
    pub timeout_seconds: u64,
    /// Enable detailed component checks
    pub detailed_checks: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: 30,
            timeout_seconds: 5,
            detailed_checks: true,
        }
    }
}

/// Overall health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall status
    pub status: Status,
    /// Timestamp of check
    pub timestamp: DateTime<Utc>,
    /// Individual component statuses
    pub components: HashMap<String, ComponentHealth>,
    /// System information
    pub system_info: SystemInfo,
}

/// Health status levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Status {
    /// All systems operational
    Healthy,
    /// Some non-critical issues
    Degraded,
    /// Critical issues detected
    Unhealthy,
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Health status
    pub status: Status,
    /// Human-readable message
    pub message: String,
    /// Last check timestamp
    pub last_checked: DateTime<Utc>,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Service version
    pub version: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Current memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Current CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Active connections
    pub active_connections: u64,
}

impl HealthCheckManager {
    /// Create a new health check manager
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            components: Arc::new(RwLock::new(HashMap::new())),
            last_check: Arc::new(RwLock::new(Utc::now())),
            config,
        }
    }

    /// Register a component for health checking
    pub async fn register_component(
        &self,
        name: String,
        initial_status: ComponentHealth,
    ) -> PrometheusResult<()> {
        let mut components = self.components.write().await;
        components.insert(name, initial_status);
        Ok(())
    }

    /// Update component health status
    pub async fn update_component_health(
        &self,
        name: &str,
        status: Status,
        message: String,
        response_time_ms: u64,
    ) -> PrometheusResult<()> {
        let mut components = self.components.write().await;
        
        if let Some(component) = components.get_mut(name) {
            component.status = status;
            component.message = message;
            component.last_checked = Utc::now();
            component.response_time_ms = response_time_ms;
        }

        Ok(())
    }

    /// Perform comprehensive health check
    pub async fn check_health(&self) -> PrometheusResult<HealthStatus> {
        let mut last_check = self.last_check.write().await;
        *last_check = Utc::now();

        let components = self.components.read().await;
        let component_map: HashMap<String, ComponentHealth> = 
            components.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        // Determine overall status
        let overall_status = self.determine_overall_status(&component_map);

        // Gather system info
        let system_info = self.gather_system_info().await?;

        Ok(HealthStatus {
            status: overall_status,
            timestamp: *last_check,
            components: component_map,
            system_info,
        })
    }

    /// Get readiness status (for Kubernetes readiness probes)
    pub async fn check_readiness(&self) -> PrometheusResult<bool> {
        let components = self.components.read().await;
        
        // Service is ready if all critical components are healthy
        let is_ready = components.values().all(|c| c.status != Status::Unhealthy);
        
        Ok(is_ready)
    }

    /// Get liveness status (for Kubernetes liveness probes)
    pub async fn check_liveness(&self) -> PrometheusResult<bool> {
        // Basic liveness check - if we can respond, we're alive
        Ok(true)
    }

    /// Determine overall health status from components
    fn determine_overall_status(&self, components: &HashMap<String, ComponentHealth>) -> Status {
        if components.is_empty() {
            return Status::Healthy;
        }

        let has_unhealthy = components.values().any(|c| c.status == Status::Unhealthy);
        let has_degraded = components.values().any(|c| c.status == Status::Degraded);

        if has_unhealthy {
            Status::Unhealthy
        } else if has_degraded {
            Status::Degraded
        } else {
            Status::Healthy
        }
    }

    /// Gather system information
    async fn gather_system_info(&self) -> PrometheusResult<SystemInfo> {
        // In production, these would be real system metrics
        Ok(SystemInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0, // Would track actual uptime
            memory_usage_bytes: 0, // Would get from system
            cpu_usage_percent: 0.0, // Would get from system
            active_connections: 0, // Would get from connection pool
        })
    }

    /// Get health check configuration
    pub fn config(&self) -> &HealthCheckConfig {
        &self.config
    }
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new(HealthCheckConfig::default())
    }
}

/// Helper function to create a healthy component status
pub fn healthy_component(name: String, message: String) -> ComponentHealth {
    ComponentHealth {
        name,
        status: Status::Healthy,
        message,
        last_checked: Utc::now(),
        response_time_ms: 0,
        metadata: HashMap::new(),
    }
}

/// Helper function to create a degraded component status
pub fn degraded_component(name: String, message: String) -> ComponentHealth {
    ComponentHealth {
        name,
        status: Status::Degraded,
        message,
        last_checked: Utc::now(),
        response_time_ms: 0,
        metadata: HashMap::new(),
    }
}

/// Helper function to create an unhealthy component status
pub fn unhealthy_component(name: String, message: String) -> ComponentHealth {
    ComponentHealth {
        name,
        status: Status::Unhealthy,
        message,
        last_checked: Utc::now(),
        response_time_ms: 0,
        metadata: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_creation() {
        let manager = HealthCheckManager::default();
        
        let component = healthy_component(
            "database".to_string(),
            "Database is healthy".to_string(),
        );
        
        manager.register_component("database".to_string(), component).await.unwrap();
        
        let health = manager.check_health().await.unwrap();
        assert_eq!(health.status, Status::Healthy);
        assert_eq!(health.components.len(), 1);
    }

    #[tokio::test]
    async fn test_degraded_status() {
        let manager = HealthCheckManager::default();
        
        let component = degraded_component(
            "cache".to_string(),
            "Cache is degraded".to_string(),
        );
        
        manager.register_component("cache".to_string(), component).await.unwrap();
        
        let health = manager.check_health().await.unwrap();
        assert_eq!(health.status, Status::Degraded);
    }

    #[tokio::test]
    async fn test_readiness_check() {
        let manager = HealthCheckManager::default();
        
        let component = healthy_component(
            "api".to_string(),
            "API is ready".to_string(),
        );
        
        manager.register_component("api".to_string(), component).await.unwrap();
        
        let is_ready = manager.check_readiness().await.unwrap();
        assert!(is_ready);
    }

    #[tokio::test]
    async fn test_liveness_check() {
        let manager = HealthCheckManager::default();
        
        let is_alive = manager.check_liveness().await.unwrap();
        assert!(is_alive);
    }
}
