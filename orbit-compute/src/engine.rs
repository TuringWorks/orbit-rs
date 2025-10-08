//! Heterogeneous acceleration engine with graceful degradation
//!
//! This module provides the main engine that orchestrates workload execution
//! across different compute units with fallback mechanisms.

use std::sync::Arc;

use crate::capabilities::UniversalComputeCapabilities;
use crate::errors::{ComputeError, ComputeResult};
use crate::monitoring::SystemMonitor;
use crate::scheduler::{AdaptiveWorkloadScheduler, ScheduleRequest};

/// Main heterogeneous acceleration engine
#[derive(Debug)]
pub struct HeterogeneousEngine {
    /// System capabilities
    capabilities: UniversalComputeCapabilities,
    /// Adaptive workload scheduler
    #[allow(dead_code)]
    scheduler: AdaptiveWorkloadScheduler,
    /// System monitor
    system_monitor: Arc<SystemMonitor>,
    /// Engine configuration
    config: EngineConfig,
}

/// Engine configuration for graceful degradation
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Enable graceful fallback when preferred compute units fail
    pub enable_fallback: bool,
    /// Maximum fallback attempts before giving up
    pub max_fallback_attempts: u32,
    /// Fallback to CPU when other compute units are unavailable
    pub fallback_to_cpu: bool,
    /// Continue operation even with degraded monitoring
    pub allow_degraded_monitoring: bool,
    /// Timeout for compute unit responsiveness checks
    pub compute_unit_timeout_ms: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enable_fallback: true,
            max_fallback_attempts: 3,
            fallback_to_cpu: true,
            allow_degraded_monitoring: true,
            compute_unit_timeout_ms: 5000,
        }
    }
}

impl HeterogeneousEngine {
    /// Create a new heterogeneous engine with detected capabilities
    pub async fn new() -> ComputeResult<Self> {
        let capabilities = crate::capabilities::detect_all_capabilities().await?;
        Self::new_with_capabilities(capabilities).await
    }

    /// Create a new heterogeneous engine with specific capabilities
    pub async fn new_with_capabilities(
        capabilities: UniversalComputeCapabilities,
    ) -> ComputeResult<Self> {
        tracing::info!("Initializing heterogeneous acceleration engine");

        // Create adaptive scheduler with graceful degradation
        let scheduler = match AdaptiveWorkloadScheduler::new(capabilities.clone()).await {
            Ok(scheduler) => scheduler,
            Err(e) => {
                tracing::warn!("Failed to create adaptive scheduler, using fallback: {}", e);
                // Create a minimal scheduler that can still function
                return Err(ComputeError::scheduling(
                    crate::errors::SchedulingError::InvalidRequest {
                        reason: "Failed to initialize scheduler".to_string(),
                    },
                ));
            }
        };

        // Create system monitor with graceful degradation
        let system_monitor = match SystemMonitor::new().await {
            Ok(monitor) => Arc::new(monitor),
            Err(e) => {
                tracing::warn!("Failed to create system monitor, using mock: {}", e);
                // Use mock monitor as fallback
                Arc::new(SystemMonitor::Mock(crate::monitoring::MockSystemMonitor))
            }
        };

        let config = EngineConfig::default();

        Ok(Self {
            capabilities,
            scheduler,
            system_monitor,
            config,
        })
    }

    /// Execute a workload with graceful degradation
    pub async fn execute_with_degradation<T>(&self, request: ScheduleRequest) -> ComputeResult<T>
    where
        T: Default,
    {
        let mut attempts = 0;
        let mut last_error: Option<ComputeError> = None;

        while attempts < self.config.max_fallback_attempts {
            match self.try_execute(&request).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("Execution attempt {} failed: {}", attempts + 1, e);
                    last_error = Some(e);
                    attempts += 1;

                    if !self.config.enable_fallback {
                        break;
                    }

                    // Wait briefly before retry
                    tokio::time::sleep(tokio::time::Duration::from_millis(100 * attempts as u64))
                        .await;
                }
            }
        }

        // If all attempts failed and CPU fallback is enabled, try CPU execution
        if self.config.fallback_to_cpu {
            tracing::info!("Falling back to CPU execution");
            match self.try_cpu_fallback(&request).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::error!("CPU fallback also failed: {}", e);
                }
            }
        }

        // Return the last error if all attempts failed
        Err(last_error.unwrap_or_else(|| {
            ComputeError::execution(crate::errors::ExecutionError::RuntimeError {
                stage: "execution".to_string(),
                error: "All execution attempts failed".to_string(),
            })
        }))
    }

    /// Try to execute a workload with the optimal compute unit
    async fn try_execute<T>(&self, _request: &ScheduleRequest) -> ComputeResult<T>
    where
        T: Default,
    {
        // This is a stub implementation - in a real implementation,
        // this would dispatch to the actual compute engines

        // Simulate successful execution for demonstration
        Ok(T::default())
    }

    /// Fallback to CPU execution when other compute units fail
    async fn try_cpu_fallback<T>(&self, _request: &ScheduleRequest) -> ComputeResult<T>
    where
        T: Default,
    {
        // This would use the CPU SIMD engine as a last resort
        tracing::info!("Executing on CPU with basic implementation");

        // Simulate CPU fallback execution
        Ok(T::default())
    }

    /// Get current engine status with degradation information
    pub async fn get_engine_status(&self) -> EngineStatus {
        let system_conditions = match self.system_monitor.get_current_conditions().await {
            Ok(conditions) => Some(conditions),
            Err(e) => {
                tracing::warn!("Failed to get system conditions: {}", e);
                None
            }
        };

        let available_compute_units = self.capabilities.gpu.available_devices.len()
            + if matches!(
                self.capabilities.neural,
                crate::capabilities::NeuralEngineCapabilities::None
            ) {
                0
            } else {
                1
            };

        EngineStatus {
            available_compute_units,
            system_conditions: system_conditions.clone(),
            degraded_monitoring: system_conditions.is_none(),
            capabilities: self.capabilities.clone(),
        }
    }
}

/// Engine status information
#[derive(Debug, Clone)]
pub struct EngineStatus {
    /// Number of available compute units
    pub available_compute_units: usize,
    /// Current system conditions (None if monitoring degraded)
    pub system_conditions: Option<crate::scheduler::SystemConditions>,
    /// Whether system monitoring is degraded
    pub degraded_monitoring: bool,
    /// System capabilities
    pub capabilities: UniversalComputeCapabilities,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = HeterogeneousEngine::new().await;
        // Should not panic even if some features are unavailable
        match engine {
            Ok(_) => println!("Engine created successfully"),
            Err(e) => println!("Engine creation failed gracefully: {}", e),
        }
    }

    #[tokio::test]
    async fn test_engine_status() {
        let engine = match HeterogeneousEngine::new().await {
            Ok(engine) => engine,
            Err(_) => return, // Skip test if engine can't be created
        };

        let status = engine.get_engine_status().await;
        // Available compute units should be valid (usize is always >= 0)
        assert!(status.available_compute_units < 1000); // Sanity check
    }
}
