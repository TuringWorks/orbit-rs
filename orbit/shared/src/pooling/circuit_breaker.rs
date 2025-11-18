//! Circuit breaker pattern implementation for connection pooling

use crate::exception::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Time window for counting failures
    pub failure_window: Duration,
    /// Duration to keep circuit open before attempting recovery
    pub recovery_timeout: Duration,
    /// Number of successful requests needed to close circuit
    pub success_threshold: u32,
    /// Maximum number of half-open attempts
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            failure_window: Duration::from_secs(60),
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 3,
            half_open_max_calls: 3,
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests are allowed
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is half-open, testing recovery
    HalfOpen,
}

/// Internal state for circuit breaker
#[derive(Debug)]
struct CircuitState {
    state: CircuitBreakerState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
    half_open_calls: u32,
}

impl Default for CircuitState {
    fn default() -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            last_state_change: Instant::now(),
            half_open_calls: 0,
        }
    }
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::default())),
        }
    }

    /// Check if a request should be allowed
    pub async fn is_request_allowed(&self) -> bool {
        let mut state = self.state.write().await;

        match state.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if recovery timeout has elapsed
                if state.last_state_change.elapsed() >= self.config.recovery_timeout {
                    state.state = CircuitBreakerState::HalfOpen;
                    state.half_open_calls = 0;
                    state.last_state_change = Instant::now();
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => {
                if state.half_open_calls < self.config.half_open_max_calls {
                    state.half_open_calls += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful request
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitBreakerState::Closed => {
                state.failure_count = 0;
            }
            CircuitBreakerState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    state.state = CircuitBreakerState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.half_open_calls = 0;
                    state.last_state_change = Instant::now();
                }
            }
            CircuitBreakerState::Open => {
                // Should not happen, but reset if it does
                state.state = CircuitBreakerState::Closed;
                state.failure_count = 0;
                state.success_count = 0;
            }
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;
        let now = Instant::now();

        match state.state {
            CircuitBreakerState::Closed => {
                // Check if we need to reset the failure window
                if let Some(last_failure) = state.last_failure_time {
                    if now.duration_since(last_failure) > self.config.failure_window {
                        state.failure_count = 0;
                    }
                }

                state.failure_count += 1;
                state.last_failure_time = Some(now);

                if state.failure_count >= self.config.failure_threshold {
                    state.state = CircuitBreakerState::Open;
                    state.last_state_change = now;
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Failed during recovery, open the circuit again
                state.state = CircuitBreakerState::Open;
                state.success_count = 0;
                state.half_open_calls = 0;
                state.last_state_change = now;
            }
            CircuitBreakerState::Open => {
                // Already open, update last failure time
                state.last_failure_time = Some(now);
            }
        }
    }

    /// Get the current state of the circuit breaker
    pub async fn get_state(&self) -> CircuitBreakerState {
        self.state.read().await.state
    }

    /// Execute a request with circuit breaker protection
    pub async fn execute<F, T>(&self, f: F) -> OrbitResult<T>
    where
        F: std::future::Future<Output = OrbitResult<T>>,
    {
        if !self.is_request_allowed().await {
            return Err(OrbitError::internal("Circuit breaker is open"));
        }

        match f.await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(e) => {
                self.record_failure().await;
                Err(e)
            }
        }
    }

    /// Reset the circuit breaker to initial state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Initial state should be closed
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);
        assert!(cb.is_request_allowed().await);

        // Record failures
        cb.record_failure().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);

        cb.record_failure().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);

        cb.record_failure().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);
        assert!(!cb.is_request_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            success_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert!(cb.is_request_allowed().await);
        assert_eq!(cb.get_state().await, CircuitBreakerState::HalfOpen);

        // Record successes to close the circuit
        cb.record_success().await;
        cb.record_success().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;

        // Wait for recovery
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(cb.is_request_allowed().await);
        assert_eq!(cb.get_state().await, CircuitBreakerState::HalfOpen);

        // Fail during recovery
        cb.record_failure().await;
        assert_eq!(cb.get_state().await, CircuitBreakerState::Open);
    }
}
