//! Retry policy: exponential backoff with full jitter.
//!
//! Full jitter (`sleep = rand(0, backoff)`) rather than the naive `backoff ± 10%`: when a provider
//! returns 429 to a whole fleet at once, correlated retries reproduce the thundering herd that
//! caused the 429. Randomizing over the *whole* interval decorrelates them.
//!
//! The delay computation is a pure function ([`RetryPolicy::backoff_ceiling`]) so it is testable
//! without sleeping; jitter is applied at the edge in [`RetryPolicy::delay_for_attempt`].

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// How the router retries a failed attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetryPolicy {
    /// Attempts *after* the first. `0` disables retrying.
    pub max_retries: u32,
    /// Delay ceiling for the first retry.
    pub initial_backoff_ms: u64,
    /// Upper bound on any single delay, before jitter.
    pub max_backoff_ms: u64,
    /// Growth factor per attempt, as a percentage (200 = double each time).
    ///
    /// Expressed as an integer percentage rather than a float so the policy stays `Eq` and
    /// comparable in config diffs.
    pub backoff_multiplier_pct: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_backoff_ms: 250,
            max_backoff_ms: 8_000,
            backoff_multiplier_pct: 200,
        }
    }
}

impl RetryPolicy {
    /// A policy that never retries, for callers that own their own retry loop.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            max_retries: 0,
            initial_backoff_ms: 0,
            max_backoff_ms: 0,
            backoff_multiplier_pct: 100,
        }
    }

    /// Upper bound on the delay before `attempt`, before jitter is applied.
    ///
    /// `attempt` is 1-based: attempt 1 is the first *retry*.
    #[must_use]
    pub fn backoff_ceiling(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let multiplier = f64::from(self.backoff_multiplier_pct.max(100)) / 100.0;
        let scaled = self.initial_backoff_ms as f64 * multiplier.powi((attempt - 1) as i32);
        // `as u64` saturates at u64::MAX for large finite values and yields 0 for NaN; neither can
        // occur here because `scaled` is a product of finite non-negative values, but the min()
        // bound makes the result independent of that reasoning.
        let capped = scaled.min(self.max_backoff_ms as f64).max(0.0);
        Duration::from_millis(capped as u64)
    }

    /// Actual delay to sleep before `attempt`, with full jitter applied.
    ///
    /// A `server_hint` from a `Retry-After` header wins outright: the provider has told us when it
    /// will be ready, and guessing earlier only wastes a request.
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32, server_hint: Option<Duration>) -> Duration {
        if let Some(hint) = server_hint {
            return hint.min(Duration::from_millis(self.max_backoff_ms.max(1)));
        }
        let ceiling = self.backoff_ceiling(attempt);
        let ceiling_ms = ceiling.as_millis() as u64;
        if ceiling_ms == 0 {
            return Duration::ZERO;
        }
        Duration::from_millis(fastrand::u64(0..=ceiling_ms))
    }

    /// Whether another attempt is permitted after `attempts_made` total attempts.
    #[must_use]
    pub fn should_retry(&self, attempts_made: u32) -> bool {
        attempts_made <= self.max_retries
    }

    /// Total attempts this policy permits, including the first.
    #[must_use]
    pub fn total_attempts(&self) -> u32 {
        self.max_retries.saturating_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_geometrically_then_saturates() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_backoff_ms: 100,
            max_backoff_ms: 1_000,
            backoff_multiplier_pct: 200,
        };
        assert_eq!(policy.backoff_ceiling(0), Duration::ZERO);
        assert_eq!(policy.backoff_ceiling(1), Duration::from_millis(100));
        assert_eq!(policy.backoff_ceiling(2), Duration::from_millis(200));
        assert_eq!(policy.backoff_ceiling(3), Duration::from_millis(400));
        assert_eq!(policy.backoff_ceiling(4), Duration::from_millis(800));
        assert_eq!(
            policy.backoff_ceiling(5),
            Duration::from_millis(1_000),
            "capped at max_backoff_ms"
        );
        assert_eq!(policy.backoff_ceiling(50), Duration::from_millis(1_000));
    }

    #[test]
    fn jitter_stays_within_the_ceiling() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_ms: 200,
            max_backoff_ms: 5_000,
            backoff_multiplier_pct: 200,
        };
        for attempt in 1..=4 {
            let ceiling = policy.backoff_ceiling(attempt);
            for _ in 0..64 {
                let delay = policy.delay_for_attempt(attempt, None);
                assert!(
                    delay <= ceiling,
                    "attempt {attempt}: {delay:?} exceeded ceiling {ceiling:?}"
                );
            }
        }
    }

    #[test]
    fn full_jitter_actually_varies() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff_ms: 1_000,
            max_backoff_ms: 5_000,
            backoff_multiplier_pct: 200,
        };
        let samples: Vec<_> = (0..32).map(|_| policy.delay_for_attempt(2, None)).collect();
        let distinct = samples.iter().collect::<std::collections::HashSet<_>>();
        assert!(
            distinct.len() > 1,
            "full jitter must decorrelate retries, got {samples:?}"
        );
    }

    #[test]
    fn server_hint_overrides_computed_backoff() {
        let policy = RetryPolicy::default();
        let hint = Duration::from_millis(1_500);
        assert_eq!(policy.delay_for_attempt(1, Some(hint)), hint);
    }

    #[test]
    fn server_hint_is_still_bounded_by_max_backoff() {
        let policy = RetryPolicy {
            max_backoff_ms: 2_000,
            ..RetryPolicy::default()
        };
        let absurd_hint = Duration::from_secs(3_600);
        assert_eq!(
            policy.delay_for_attempt(1, Some(absurd_hint)),
            Duration::from_millis(2_000),
            "a provider cannot pin a query open for an hour"
        );
    }

    #[test]
    fn none_policy_permits_exactly_one_attempt() {
        let policy = RetryPolicy::none();
        assert_eq!(policy.total_attempts(), 1);
        assert!(policy.should_retry(0));
        assert!(!policy.should_retry(1));
        assert_eq!(policy.delay_for_attempt(1, None), Duration::ZERO);
    }

    #[test]
    fn should_retry_respects_the_budget() {
        let policy = RetryPolicy {
            max_retries: 2,
            ..RetryPolicy::default()
        };
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3), "budget of 2 retries is exhausted");
    }

    #[test]
    fn degenerate_multiplier_does_not_shrink_the_backoff() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 1_000,
            backoff_multiplier_pct: 0,
        };
        // A multiplier below 100% would make later retries fire sooner than earlier ones.
        assert_eq!(policy.backoff_ceiling(1), Duration::from_millis(100));
        assert_eq!(policy.backoff_ceiling(3), Duration::from_millis(100));
    }
}
