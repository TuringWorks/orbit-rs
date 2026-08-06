//! Per-profile circuit breaker.
//!
//! Without one, a dead provider costs every request its full timeout before failing over. With one,
//! the first few requests pay that cost and the rest fail immediately, which is the difference
//! between a degraded system and a stalled one.
//!
//! State is held in atomics rather than behind a lock: the hot path is a read on every request and
//! a write only on a state transition.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Breaker tuning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BreakerConfig {
    /// Consecutive unhealthy failures that open the circuit. `0` disables the breaker.
    pub failure_threshold: u32,
    /// How long the circuit stays open before admitting a probe request.
    pub open_duration_ms: u64,
    /// Consecutive successes in half-open state required to close the circuit again.
    pub success_threshold: u32,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_duration_ms: 30_000,
            success_threshold: 2,
        }
    }
}

impl BreakerConfig {
    /// A configuration that never trips.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            failure_threshold: 0,
            open_duration_ms: 0,
            success_threshold: 0,
        }
    }

    /// Whether this configuration will ever open a circuit.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.failure_threshold > 0
    }
}

/// Observable breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerState {
    /// Requests pass through.
    Closed,
    /// Requests are rejected without being sent.
    Open,
    /// A limited number of probe requests are admitted.
    HalfOpen,
}

impl BreakerState {
    /// Stable identifier for metrics and `LLM.STATS`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            BreakerState::Closed => "closed",
            BreakerState::Open => "open",
            BreakerState::HalfOpen => "half_open",
        }
    }
}

/// A circuit breaker for one model profile.
///
/// Time is injected through [`CircuitBreaker::state_at`] / [`CircuitBreaker::record_failure_at`] so
/// the state machine is testable without sleeping. The convenience wrappers stamp `Instant::now()`
/// at the edge.
#[derive(Debug)]
pub struct CircuitBreaker {
    config: BreakerConfig,
    consecutive_failures: AtomicU32,
    half_open_successes: AtomicU32,
    /// Millis since `origin` at which the circuit opened; `0` means "not open".
    opened_at_ms: AtomicU64,
    origin: Instant,
}

impl CircuitBreaker {
    /// Build a breaker with the given configuration.
    #[must_use]
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            config,
            consecutive_failures: AtomicU32::new(0),
            half_open_successes: AtomicU32::new(0),
            opened_at_ms: AtomicU64::new(0),
            origin: Instant::now(),
        }
    }

    fn elapsed_ms(&self, now: Instant) -> u64 {
        now.saturating_duration_since(self.origin).as_millis() as u64
    }

    /// Current state as of `now`.
    #[must_use]
    pub fn state_at(&self, now: Instant) -> BreakerState {
        if !self.config.is_enabled() {
            return BreakerState::Closed;
        }
        let opened_at = self.opened_at_ms.load(Ordering::Acquire);
        if opened_at == 0 {
            return BreakerState::Closed;
        }
        let open_for = self.elapsed_ms(now).saturating_sub(opened_at);
        if open_for >= self.config.open_duration_ms {
            BreakerState::HalfOpen
        } else {
            BreakerState::Open
        }
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> BreakerState {
        self.state_at(Instant::now())
    }

    /// Whether a request should be admitted as of `now`.
    #[must_use]
    pub fn allows_request_at(&self, now: Instant) -> bool {
        !matches!(self.state_at(now), BreakerState::Open)
    }

    /// Whether a request should be admitted.
    #[must_use]
    pub fn allows_request(&self) -> bool {
        self.allows_request_at(Instant::now())
    }

    /// Consecutive failures recorded since the last success.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Record a successful call as of `now`.
    pub fn record_success_at(&self, now: Instant) {
        if !self.config.is_enabled() {
            return;
        }
        match self.state_at(now) {
            BreakerState::HalfOpen => {
                let successes = self.half_open_successes.fetch_add(1, Ordering::AcqRel) + 1;
                if successes >= self.config.success_threshold.max(1) {
                    self.close();
                }
            }
            BreakerState::Closed => {
                self.consecutive_failures.store(0, Ordering::Release);
            }
            // A success cannot be observed while open, because no request was sent.
            BreakerState::Open => {}
        }
    }

    /// Record a successful call.
    pub fn record_success(&self) {
        self.record_success_at(Instant::now());
    }

    /// Record a failure that indicates provider ill-health, as of `now`.
    ///
    /// Callers must filter on [`crate::LlmError::indicates_provider_unhealthy`] first: a 400 means
    /// the caller sent a bad prompt, and tripping a breaker on it removes a healthy provider from
    /// service because one query was malformed.
    pub fn record_failure_at(&self, now: Instant) {
        if !self.config.is_enabled() {
            return;
        }
        if self.state_at(now) == BreakerState::HalfOpen {
            // A probe failed: re-open for another full interval rather than accumulating toward
            // the threshold again.
            self.half_open_successes.store(0, Ordering::Release);
            self.open_at(now);
            return;
        }
        let failures = self.consecutive_failures.fetch_add(1, Ordering::AcqRel) + 1;
        if failures >= self.config.failure_threshold {
            self.open_at(now);
        }
    }

    /// Record a failure that indicates provider ill-health.
    pub fn record_failure(&self) {
        self.record_failure_at(Instant::now());
    }

    fn open_at(&self, now: Instant) {
        // `max(1)` keeps 0 as the unambiguous "not open" sentinel even if the breaker opens within
        // the first millisecond of process life.
        self.opened_at_ms
            .store(self.elapsed_ms(now).max(1), Ordering::Release);
    }

    fn close(&self) {
        self.opened_at_ms.store(0, Ordering::Release);
        self.consecutive_failures.store(0, Ordering::Release);
        self.half_open_successes.store(0, Ordering::Release);
    }

    /// Force the circuit closed, discarding accumulated failures.
    ///
    /// Used when a profile is re-registered: the new configuration has not failed yet, and
    /// inheriting the old one's failure count would open a circuit on a provider never called.
    pub fn reset(&self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn breaker() -> CircuitBreaker {
        CircuitBreaker::new(BreakerConfig {
            failure_threshold: 3,
            open_duration_ms: 1_000,
            success_threshold: 2,
        })
    }

    #[test]
    fn opens_after_threshold_consecutive_failures() {
        let b = breaker();
        let t0 = b.origin + Duration::from_millis(10);

        assert_eq!(b.state_at(t0), BreakerState::Closed);
        b.record_failure_at(t0);
        b.record_failure_at(t0);
        assert_eq!(b.state_at(t0), BreakerState::Closed, "below threshold");
        b.record_failure_at(t0);
        assert_eq!(b.state_at(t0), BreakerState::Open);
        assert!(!b.allows_request_at(t0));
    }

    #[test]
    fn a_success_resets_the_failure_run() {
        let b = breaker();
        let t0 = b.origin + Duration::from_millis(10);

        b.record_failure_at(t0);
        b.record_failure_at(t0);
        b.record_success_at(t0);
        assert_eq!(b.consecutive_failures(), 0);
        b.record_failure_at(t0);
        b.record_failure_at(t0);
        assert_eq!(
            b.state_at(t0),
            BreakerState::Closed,
            "the run restarted, so two failures is still below threshold"
        );
    }

    #[test]
    fn transitions_to_half_open_after_the_open_interval() {
        let b = breaker();
        let t0 = b.origin + Duration::from_millis(10);
        for _ in 0..3 {
            b.record_failure_at(t0);
        }
        assert_eq!(b.state_at(t0), BreakerState::Open);
        assert_eq!(
            b.state_at(t0 + Duration::from_millis(999)),
            BreakerState::Open
        );
        assert_eq!(
            b.state_at(t0 + Duration::from_millis(1_000)),
            BreakerState::HalfOpen
        );
        assert!(b.allows_request_at(t0 + Duration::from_millis(1_000)));
    }

    #[test]
    fn half_open_closes_after_enough_successes() {
        let b = breaker();
        let t0 = b.origin + Duration::from_millis(10);
        for _ in 0..3 {
            b.record_failure_at(t0);
        }
        let probe = t0 + Duration::from_millis(1_500);
        b.record_success_at(probe);
        assert_eq!(
            b.state_at(probe),
            BreakerState::HalfOpen,
            "one success is not enough"
        );
        b.record_success_at(probe);
        assert_eq!(b.state_at(probe), BreakerState::Closed);
    }

    #[test]
    fn a_failed_probe_reopens_for_a_full_interval() {
        let b = breaker();
        let t0 = b.origin + Duration::from_millis(10);
        for _ in 0..3 {
            b.record_failure_at(t0);
        }
        let probe = t0 + Duration::from_millis(1_500);
        assert_eq!(b.state_at(probe), BreakerState::HalfOpen);
        b.record_failure_at(probe);
        assert_eq!(b.state_at(probe), BreakerState::Open);
        assert_eq!(
            b.state_at(probe + Duration::from_millis(999)),
            BreakerState::Open,
            "the open interval restarted from the failed probe"
        );
    }

    #[test]
    fn disabled_breaker_never_opens() {
        let b = CircuitBreaker::new(BreakerConfig::disabled());
        let t0 = b.origin + Duration::from_millis(10);
        for _ in 0..1_000 {
            b.record_failure_at(t0);
        }
        assert_eq!(b.state_at(t0), BreakerState::Closed);
        assert!(b.allows_request_at(t0));
    }

    #[test]
    fn reset_clears_an_open_circuit() {
        let b = breaker();
        let t0 = b.origin + Duration::from_millis(10);
        for _ in 0..3 {
            b.record_failure_at(t0);
        }
        assert_eq!(b.state_at(t0), BreakerState::Open);
        b.reset();
        assert_eq!(b.state_at(t0), BreakerState::Closed);
        assert_eq!(b.consecutive_failures(), 0);
    }
}
