//! Per-profile usage, cost, and failure accounting.
//!
//! Counters are atomics updated on every request, so they are sampled here rather than locked. The
//! numbers back `LLM.STATS` and the Prometheus gauges.
//!
//! Cost is accumulated in micro-dollars as an integer. Summing `f64` across millions of requests
//! accumulates representation error into the column an operator is going to reconcile against an
//! invoice; integers do not drift.

use crate::types::{Cost, TokenUsage};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Micro-dollars per dollar.
const MICRO_USD: f64 = 1_000_000.0;

/// Live counters for one profile.
#[derive(Debug, Default)]
pub struct ProfileCounters {
    requests: AtomicU64,
    failures: AtomicU64,
    /// Times this profile was reached *as a fallback* for another profile.
    fallback_uses: AtomicU64,
    /// Times a request starting at this profile had to fail over.
    fallbacks_fired: AtomicU64,
    prompt_tokens: AtomicU64,
    completion_tokens: AtomicU64,
    /// Requests for which the provider reported no token counts at all.
    unreported_usage: AtomicU64,
    cost_micro_usd: AtomicU64,
    latency_ms_total: AtomicU64,
}

impl ProfileCounters {
    /// Record a completed request.
    pub fn record_success(&self, usage: &TokenUsage, cost: Option<Cost>, latency_ms: u64) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.latency_ms_total
            .fetch_add(latency_ms, Ordering::Relaxed);

        match (usage.prompt_tokens, usage.completion_tokens) {
            (None, None) => {
                // Counted separately rather than added as zero: "this provider does not report
                // tokens" and "this request used no tokens" are different facts, and collapsing
                // them makes a token total look complete when it is not.
                self.unreported_usage.fetch_add(1, Ordering::Relaxed);
            }
            (prompt, completion) => {
                if let Some(p) = prompt {
                    self.prompt_tokens
                        .fetch_add(u64::from(p), Ordering::Relaxed);
                }
                if let Some(c) = completion {
                    self.completion_tokens
                        .fetch_add(u64::from(c), Ordering::Relaxed);
                }
            }
        }

        if let Some(cost) = cost {
            let micros = (cost.total_usd() * MICRO_USD).round().max(0.0) as u64;
            self.cost_micro_usd.fetch_add(micros, Ordering::Relaxed);
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that this profile served as a fallback for another.
    pub fn record_fallback_use(&self) {
        self.fallback_uses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a request starting at this profile had to fail over.
    pub fn record_fallback_fired(&self) {
        self.fallbacks_fired.fetch_add(1, Ordering::Relaxed);
    }

    /// Take a consistent-enough snapshot for reporting.
    ///
    /// Fields are read independently, so a snapshot taken during heavy traffic may mix values from
    /// adjacent instants. That is acceptable for an operational counter and is cheaper than the
    /// lock that would avoid it.
    #[must_use]
    pub fn snapshot(&self) -> UsageSnapshot {
        let requests = self.requests.load(Ordering::Relaxed);
        let latency_total = self.latency_ms_total.load(Ordering::Relaxed);
        let failures = self.failures.load(Ordering::Relaxed);
        let successes = requests.saturating_sub(failures);

        UsageSnapshot {
            requests,
            failures,
            fallback_uses: self.fallback_uses.load(Ordering::Relaxed),
            fallbacks_fired: self.fallbacks_fired.load(Ordering::Relaxed),
            prompt_tokens: self.prompt_tokens.load(Ordering::Relaxed),
            completion_tokens: self.completion_tokens.load(Ordering::Relaxed),
            unreported_usage: self.unreported_usage.load(Ordering::Relaxed),
            cost_usd: self.cost_micro_usd.load(Ordering::Relaxed) as f64 / MICRO_USD,
            // Averaged over successes only: a failure contributes no latency sample, so dividing
            // by total requests would report a mean that is systematically too low whenever the
            // provider is failing.
            mean_latency_ms: (successes > 0).then(|| latency_total as f64 / successes as f64),
        }
    }
}

/// A point-in-time reading of one profile's counters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    /// Requests attempted.
    pub requests: u64,
    /// Requests that ended in an error.
    pub failures: u64,
    /// Times this profile served as another profile's fallback.
    pub fallback_uses: u64,
    /// Times a request starting here failed over to a fallback.
    pub fallbacks_fired: u64,
    /// Input tokens, summed over requests that reported them.
    pub prompt_tokens: u64,
    /// Output tokens, summed over requests that reported them.
    pub completion_tokens: u64,
    /// Requests whose provider reported no token counts.
    ///
    /// Non-zero means the token totals above are a lower bound, not a total.
    pub unreported_usage: u64,
    /// Spend, summed over requests whose profile carried prices.
    pub cost_usd: f64,
    /// Mean latency over successful requests, absent when there have been none.
    pub mean_latency_ms: Option<f64>,
}

impl UsageSnapshot {
    /// Whether the token totals cover every request.
    ///
    /// When this is false, `prompt_tokens`/`completion_tokens` under-report and should be presented
    /// as "at least", not as a total.
    #[must_use]
    pub fn tokens_are_complete(&self) -> bool {
        self.unreported_usage == 0
    }

    /// Observed failure rate, absent when no request has been made.
    #[must_use]
    pub fn failure_rate(&self) -> Option<f64> {
        (self.requests > 0).then(|| self.failures as f64 / self.requests as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successes_accumulate_tokens_cost_and_latency() {
        let counters = ProfileCounters::default();
        let usage = TokenUsage {
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
        };
        let cost = Cost {
            prompt_usd: 0.001,
            completion_usd: 0.002,
        };

        counters.record_success(&usage, Some(cost), 200);
        counters.record_success(&usage, Some(cost), 400);

        let snap = counters.snapshot();
        assert_eq!(snap.requests, 2);
        assert_eq!(snap.failures, 0);
        assert_eq!(snap.prompt_tokens, 200);
        assert_eq!(snap.completion_tokens, 100);
        assert!((snap.cost_usd - 0.006).abs() < 1e-9);
        assert_eq!(snap.mean_latency_ms, Some(300.0));
        assert!(snap.tokens_are_complete());
    }

    #[test]
    fn unreported_usage_is_counted_not_zeroed() {
        let counters = ProfileCounters::default();
        counters.record_success(&TokenUsage::default(), None, 10);
        counters.record_success(
            &TokenUsage {
                prompt_tokens: Some(7),
                completion_tokens: Some(3),
            },
            None,
            10,
        );

        let snap = counters.snapshot();
        assert_eq!(snap.prompt_tokens, 7);
        assert_eq!(snap.unreported_usage, 1);
        assert!(
            !snap.tokens_are_complete(),
            "one request's tokens are unknown, so 7 is a lower bound"
        );
    }

    #[test]
    fn partial_usage_records_the_half_that_was_reported() {
        let counters = ProfileCounters::default();
        counters.record_success(
            &TokenUsage {
                prompt_tokens: Some(11),
                completion_tokens: None,
            },
            None,
            5,
        );
        let snap = counters.snapshot();
        assert_eq!(snap.prompt_tokens, 11);
        assert_eq!(snap.completion_tokens, 0);
        assert_eq!(
            snap.unreported_usage, 0,
            "a half-reported request is not an unreported one"
        );
    }

    #[test]
    fn mean_latency_excludes_failures() {
        let counters = ProfileCounters::default();
        counters.record_success(&TokenUsage::default(), None, 100);
        counters.record_failure();
        counters.record_failure();

        let snap = counters.snapshot();
        assert_eq!(snap.requests, 3);
        assert_eq!(snap.failures, 2);
        assert_eq!(
            snap.mean_latency_ms,
            Some(100.0),
            "failures contribute no latency sample"
        );
        assert!((snap.failure_rate().expect("requests made") - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn an_untouched_profile_reports_no_mean_or_rate() {
        let snap = ProfileCounters::default().snapshot();
        assert_eq!(snap.requests, 0);
        assert_eq!(snap.mean_latency_ms, None);
        assert_eq!(snap.failure_rate(), None);
    }

    #[test]
    fn fallback_participation_is_recorded_on_both_sides() {
        let primary = ProfileCounters::default();
        let backup = ProfileCounters::default();

        primary.record_fallback_fired();
        backup.record_fallback_use();

        assert_eq!(primary.snapshot().fallbacks_fired, 1);
        assert_eq!(backup.snapshot().fallback_uses, 1);
    }

    #[test]
    fn cost_does_not_drift_over_many_small_charges() {
        let counters = ProfileCounters::default();
        let cost = Cost {
            prompt_usd: 0.000_001,
            completion_usd: 0.0,
        };
        for _ in 0..1_000_000 {
            counters.record_success(&TokenUsage::default(), Some(cost), 1);
        }
        let total = counters.snapshot().cost_usd;
        assert!(
            (total - 1.0).abs() < 1e-9,
            "integer micro-dollars must not accumulate float error; got {total}"
        );
    }
}
