//! Dynamic provider management with health monitoring and failover
//!
//! This module provides health monitoring, circuit breaker patterns, and automatic
//! failover capabilities for persistence providers.

use super::{AddressableDirectoryProvider, ClusterNodeProvider, PersistenceProvider};
use crate::persistence::config::{
    DynamicProviderConfig, FailoverConfig, FailoverStrategy, HealthMonitorConfig,
};
use orbit_shared::{OrbitError, OrbitResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{debug, info, warn};

/// Health status of a provider
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderHealth {
    Healthy,
    Unhealthy,
    Unknown,
    CircuitOpen,
}

/// Performance metrics for a provider
#[derive(Debug, Clone, Default)]
pub struct ProviderMetrics {
    pub request_count: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub avg_response_time_ms: f64,
    pub last_success: Option<Instant>,
    pub last_error: Option<Instant>,
    pub circuit_breaker_opens: u64,
}

impl ProviderMetrics {
    pub fn success_rate(&self) -> f64 {
        if self.request_count == 0 {
            1.0
        } else {
            self.success_count as f64 / self.request_count as f64
        }
    }

    pub fn error_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }
}

/// Health and performance monitoring for a provider
#[derive(Debug)]
pub struct ProviderMonitor {
    pub name: String,
    pub health: RwLock<ProviderHealth>,
    pub metrics: RwLock<ProviderMetrics>,
    pub consecutive_failures: RwLock<u32>,
    pub consecutive_successes: RwLock<u32>,
    pub circuit_breaker_opened_at: RwLock<Option<Instant>>,
    pub config: HealthMonitorConfig,
}

impl ProviderMonitor {
    pub fn new(name: String, config: HealthMonitorConfig) -> Self {
        Self {
            name,
            health: RwLock::new(ProviderHealth::Unknown),
            metrics: RwLock::new(ProviderMetrics::default()),
            consecutive_failures: RwLock::new(0),
            consecutive_successes: RwLock::new(0),
            circuit_breaker_opened_at: RwLock::new(None),
            config,
        }
    }

    /// Record a successful operation
    pub async fn record_success(&self, response_time: Duration) {
        let mut metrics = self.metrics.write().await;
        metrics.request_count += 1;
        metrics.success_count += 1;
        metrics.last_success = Some(Instant::now());

        // Update rolling average response time
        let new_time_ms = response_time.as_millis() as f64;
        if metrics.avg_response_time_ms == 0.0 {
            metrics.avg_response_time_ms = new_time_ms;
        } else {
            // Simple exponential moving average
            metrics.avg_response_time_ms = 0.1 * new_time_ms + 0.9 * metrics.avg_response_time_ms;
        }

        drop(metrics);

        // Reset failure count and increment success count
        *self.consecutive_failures.write().await = 0;
        let mut successes = self.consecutive_successes.write().await;
        *successes += 1;

        // Check if we should mark as healthy
        if *successes >= self.config.success_threshold {
            let mut health = self.health.write().await;
            if *health == ProviderHealth::Unhealthy || *health == ProviderHealth::Unknown {
                *health = ProviderHealth::Healthy;
                info!(
                    "Provider {} marked as healthy after {} consecutive successes",
                    self.name, *successes
                );
            }

            // Close circuit breaker if it was open
            if *health == ProviderHealth::CircuitOpen {
                *health = ProviderHealth::Healthy;
                *self.circuit_breaker_opened_at.write().await = None;
                info!(
                    "Circuit breaker closed for provider {} after successful operation",
                    self.name
                );
            }
        }
    }

    /// Record a failed operation
    pub async fn record_failure(&self, error: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.request_count += 1;
        metrics.error_count += 1;
        metrics.last_error = Some(Instant::now());
        drop(metrics);

        // Reset success count and increment failure count
        *self.consecutive_successes.write().await = 0;
        let mut failures = self.consecutive_failures.write().await;
        *failures += 1;

        // Check if we should mark as unhealthy
        if *failures >= self.config.failure_threshold {
            let mut health = self.health.write().await;
            if *health != ProviderHealth::Unhealthy {
                *health = ProviderHealth::Unhealthy;
                warn!(
                    "Provider {} marked as unhealthy after {} consecutive failures: {}",
                    self.name, *failures, error
                );
            }

            // Open circuit breaker if enabled
            if self.config.enable_circuit_breaker && *health != ProviderHealth::CircuitOpen {
                *health = ProviderHealth::CircuitOpen;
                *self.circuit_breaker_opened_at.write().await = Some(Instant::now());
                let mut metrics = self.metrics.write().await;
                metrics.circuit_breaker_opens += 1;
                warn!(
                    "Circuit breaker opened for provider {} due to consecutive failures",
                    self.name
                );
            }
        }
    }

    /// Check if the circuit breaker should allow requests
    pub async fn is_circuit_breaker_open(&self) -> bool {
        let health = self.health.read().await;
        if *health != ProviderHealth::CircuitOpen {
            return false;
        }

        // Check if enough time has passed to try again
        let opened_at = self.circuit_breaker_opened_at.read().await;
        if let Some(opened_time) = *opened_at {
            let elapsed = opened_time.elapsed();
            if elapsed >= Duration::from_secs(self.config.circuit_breaker_timeout) {
                debug!(
                    "Circuit breaker for provider {} allowing test request after {} seconds",
                    self.name,
                    elapsed.as_secs()
                );
                return false; // Allow one test request
            }
        }

        true
    }

    /// Get current health status
    pub async fn get_health(&self) -> ProviderHealth {
        // First check circuit breaker
        if self.is_circuit_breaker_open().await {
            return ProviderHealth::CircuitOpen;
        }

        self.health.read().await.clone()
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> ProviderMetrics {
        self.metrics.read().await.clone()
    }
}

/// Dynamic provider manager with health monitoring and failover
pub struct DynamicProviderManager {
    providers: RwLock<HashMap<String, Arc<dyn PersistenceProvider>>>,
    monitors: RwLock<HashMap<String, Arc<ProviderMonitor>>>,
    config: DynamicProviderConfig,
    current_primary: RwLock<Option<String>>,
    provider_priorities: RwLock<Vec<String>>, // Ordered by priority
}

impl DynamicProviderManager {
    pub fn new(config: DynamicProviderConfig) -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            monitors: RwLock::new(HashMap::new()),
            config,
            current_primary: RwLock::new(None),
            provider_priorities: RwLock::new(Vec::new()),
        }
    }

    /// Register a provider with monitoring
    pub async fn register_provider(
        &self,
        name: String,
        provider: Arc<dyn PersistenceProvider>,
        is_primary: bool,
    ) -> OrbitResult<()> {
        let monitor = Arc::new(ProviderMonitor::new(
            name.clone(),
            self.config.health_monitor.clone(),
        ));

        self.providers.write().await.insert(name.clone(), provider);
        self.monitors.write().await.insert(name.clone(), monitor);

        if is_primary {
            *self.current_primary.write().await = Some(name.clone());
        }

        // Add to priority list
        let mut priorities = self.provider_priorities.write().await;
        if is_primary {
            priorities.insert(0, name.clone()); // Primary goes first
        } else {
            priorities.push(name);
        }

        Ok(())
    }

    /// Get the best available provider based on health and strategy
    pub async fn get_provider(&self) -> Option<(String, Arc<dyn PersistenceProvider>)> {
        let providers = self.providers.read().await;
        let monitors = self.monitors.read().await;

        match self.config.failover.strategy {
            FailoverStrategy::Priority => {
                // Use providers in priority order
                let priorities = self.provider_priorities.read().await;
                for provider_name in priorities.iter() {
                    if let (Some(provider), Some(monitor)) =
                        (providers.get(provider_name), monitors.get(provider_name))
                    {
                        let health = monitor.get_health().await;
                        if health == ProviderHealth::Healthy || health == ProviderHealth::Unknown {
                            return Some((provider_name.clone(), provider.clone()));
                        }
                    }
                }
            }
            FailoverStrategy::RoundRobin => {
                // Simple round-robin among healthy providers
                // TODO: Implement proper round-robin state tracking
                for (provider_name, provider) in providers.iter() {
                    if let Some(monitor) = monitors.get(provider_name) {
                        let health = monitor.get_health().await;
                        if health == ProviderHealth::Healthy || health == ProviderHealth::Unknown {
                            return Some((provider_name.clone(), provider.clone()));
                        }
                    }
                }
            }
            FailoverStrategy::LowestLatency => {
                // Find provider with lowest average response time
                let mut best_provider = None;
                let mut best_latency = f64::MAX;

                for (provider_name, provider) in providers.iter() {
                    if let Some(monitor) = monitors.get(provider_name) {
                        let health = monitor.get_health().await;
                        if health == ProviderHealth::Healthy || health == ProviderHealth::Unknown {
                            let metrics = monitor.get_metrics().await;
                            if metrics.avg_response_time_ms < best_latency {
                                best_latency = metrics.avg_response_time_ms;
                                best_provider = Some((provider_name.clone(), provider.clone()));
                            }
                        }
                    }
                }

                return best_provider;
            }
            FailoverStrategy::LoadBased => {
                // TODO: Implement load-based selection
                // For now, fall back to priority-based
                return self.get_provider_by_priority(&providers, &monitors).await;
            }
        }

        None
    }

    async fn get_provider_by_priority(
        &self,
        providers: &HashMap<String, Arc<dyn PersistenceProvider>>,
        monitors: &HashMap<String, Arc<ProviderMonitor>>,
    ) -> Option<(String, Arc<dyn PersistenceProvider>)> {
        let priorities = self.provider_priorities.read().await;
        for provider_name in priorities.iter() {
            if let (Some(provider), Some(monitor)) =
                (providers.get(provider_name), monitors.get(provider_name))
            {
                let health = monitor.get_health().await;
                if health == ProviderHealth::Healthy || health == ProviderHealth::Unknown {
                    return Some((provider_name.clone(), provider.clone()));
                }
            }
        }
        None
    }

    /// Execute an operation with automatic failover
    pub async fn execute_with_failover<T, F, Fut>(&self, operation: F) -> OrbitResult<T>
    where
        F: Fn(Arc<dyn PersistenceProvider>) -> Fut,
        Fut: std::future::Future<Output = OrbitResult<T>>,
    {
        let mut attempts = 0;
        let max_attempts = self.config.failover.max_attempts;

        while attempts < max_attempts {
            if let Some((provider_name, provider)) = self.get_provider().await {
                let monitor = self.monitors.read().await.get(&provider_name).cloned();

                if let Some(monitor) = monitor {
                    // Check circuit breaker
                    if monitor.is_circuit_breaker_open().await {
                        warn!(
                            "Circuit breaker open for provider {}, trying next provider",
                            provider_name
                        );
                        attempts += 1;
                        continue;
                    }

                    let start_time = Instant::now();
                    match operation(provider).await {
                        Ok(result) => {
                            monitor.record_success(start_time.elapsed()).await;
                            return Ok(result);
                        }
                        Err(err) => {
                            monitor.record_failure(&err.to_string()).await;
                            warn!("Operation failed on provider {}: {}", provider_name, err);

                            if attempts < max_attempts - 1 {
                                sleep(Duration::from_secs(self.config.failover.retry_delay)).await;
                            }
                        }
                    }
                }
            }

            attempts += 1;
        }

        Err(OrbitError::internal(
            "All providers failed after maximum retry attempts",
        ))
    }

    /// Start background health monitoring
    pub async fn start_health_monitoring(&self) -> OrbitResult<()> {
        if !self.config.enable_performance_monitoring {
            return Ok(());
        }

        let monitors = self.monitors.read().await;
        for (provider_name, monitor) in monitors.iter() {
            let _monitor_clone = monitor.clone();
            let provider_name = provider_name.clone();
            let check_interval = monitor.config.check_interval;

            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(check_interval));

                loop {
                    interval.tick().await;

                    // Perform health check
                    // TODO: Implement actual health check by calling provider.health_check()
                    debug!("Health check for provider {}", provider_name);
                }
            });
        }

        Ok(())
    }

    /// Get status of all providers
    pub async fn get_provider_status(&self) -> HashMap<String, (ProviderHealth, ProviderMetrics)> {
        let monitors = self.monitors.read().await;
        let mut status = HashMap::new();

        for (name, monitor) in monitors.iter() {
            let health = monitor.get_health().await;
            let metrics = monitor.get_metrics().await;
            status.insert(name.clone(), (health, metrics));
        }

        status
    }

    /// Get current primary provider
    pub async fn get_current_primary(&self) -> Option<String> {
        self.current_primary.read().await.clone()
    }

    /// Manually trigger failover to a specific provider
    pub async fn failover_to(&self, provider_name: &str) -> OrbitResult<()> {
        let providers = self.providers.read().await;
        if !providers.contains_key(provider_name) {
            return Err(OrbitError::configuration(format!(
                "Provider '{}' not found",
                provider_name
            )));
        }

        *self.current_primary.write().await = Some(provider_name.to_string());
        info!("Manual failover to provider: {}", provider_name);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_monitor_success() {
        let config = HealthMonitorConfig::default();
        let monitor = ProviderMonitor::new("test".to_string(), config);

        // Initially unknown
        assert_eq!(monitor.get_health().await, ProviderHealth::Unknown);

        // Record enough successes to mark as healthy
        for _ in 0..2 {
            monitor.record_success(Duration::from_millis(10)).await;
        }

        assert_eq!(monitor.get_health().await, ProviderHealth::Healthy);

        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.success_count, 2);
        assert!(metrics.avg_response_time_ms > 0.0);
    }

    #[tokio::test]
    async fn test_provider_monitor_failure() {
        let config = HealthMonitorConfig::default();
        let monitor = ProviderMonitor::new("test".to_string(), config);

        // Record enough failures to mark as unhealthy
        for i in 0..3 {
            monitor.record_failure(&format!("Error {}", i)).await;
        }

        let health = monitor.get_health().await;
        assert!(health == ProviderHealth::Unhealthy || health == ProviderHealth::CircuitOpen);

        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.error_count, 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = HealthMonitorConfig {
            circuit_breaker_timeout: 1, // 1 second for testing
            ..Default::default()
        };
        let monitor = ProviderMonitor::new("test".to_string(), config);

        // Trigger circuit breaker
        for i in 0..3 {
            monitor.record_failure(&format!("Error {}", i)).await;
        }

        assert!(monitor.is_circuit_breaker_open().await);

        // Wait for circuit breaker to allow test request
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(!monitor.is_circuit_breaker_open().await);
    }
}
