//! Centralized default configuration constants
//!
//! This module provides a single source of truth for all default configuration
//! values used throughout the Orbit-RS codebase.

use std::time::Duration;

// ===== Connection Pool Defaults =====

pub const DEFAULT_MIN_CONNECTIONS: usize = 2;
pub const DEFAULT_MAX_CONNECTIONS: usize = 20;
pub const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 5;
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300;
pub const DEFAULT_MAX_LIFETIME_SECS: u64 = 3600;
pub const DEFAULT_HEALTH_CHECK_INTERVAL_SECS: u64 = 30;
pub const DEFAULT_TARGET_UTILIZATION: f64 = 0.75;

// ===== Cache Defaults =====

pub const DEFAULT_RESULT_CACHE_SIZE: usize = 1000;
pub const DEFAULT_PLAN_CACHE_SIZE: usize = 500;
pub const DEFAULT_CACHE_TTL_SECONDS: u64 = 3600;
pub const DEFAULT_CACHE_MAX_SIZE: usize = 10000;

// ===== GraphRAG Defaults =====

pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.7;
pub const DEFAULT_MAX_ENTITIES_PER_DOCUMENT: usize = 100;
pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.8;
pub const DEFAULT_MAX_HOPS: usize = 3;
pub const DEFAULT_MAX_RESULTS: usize = 10;

// ===== Batch Processing Defaults =====

pub const DEFAULT_BATCH_SIZE: usize = 100;
pub const DEFAULT_MAX_BATCH_SIZE: usize = 1000;
pub const DEFAULT_MIN_BATCH_SIZE: usize = 10;
pub const DEFAULT_BATCH_TIMEOUT_MS: u64 = 100;

// ===== Circuit Breaker Defaults =====

pub const DEFAULT_FAILURE_THRESHOLD: usize = 5;
pub const DEFAULT_SUCCESS_THRESHOLD: usize = 2;
pub const DEFAULT_TIMEOUT_DURATION_SECS: u64 = 60;
pub const DEFAULT_HALF_OPEN_MAX_CALLS: usize = 3;

// ===== Retry Defaults =====

pub const DEFAULT_MAX_RETRIES: u32 = 3;
pub const DEFAULT_BASE_DELAY_MS: u64 = 100;
pub const DEFAULT_MAX_DELAY_MS: u64 = 30000;
pub const DEFAULT_BACKOFF_MULTIPLIER: f64 = 2.0;

// ===== Network Defaults =====

pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_KEEPALIVE_INTERVAL_SECS: u64 = 60;
pub const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 100;

// ===== Storage Defaults =====

pub const DEFAULT_WRITE_BUFFER_SIZE: usize = 64 * 1024 * 1024; // 64MB
pub const DEFAULT_MAX_WRITE_BUFFER_NUMBER: i32 = 3;
pub const DEFAULT_TARGET_FILE_SIZE_BASE: u64 = 64 * 1024 * 1024; // 64MB
pub const DEFAULT_BLOCK_CACHE_SIZE: usize = 512 * 1024 * 1024; // 512MB
pub const DEFAULT_BLOOM_FILTER_BITS_PER_KEY: i32 = 10;

// ===== Lease Defaults =====

pub const DEFAULT_LEASE_DURATION_SECS: u64 = 30;
pub const DEFAULT_LEASE_RENEWAL_INTERVAL_SECS: u64 = 10;
pub const DEFAULT_LEASE_TIMEOUT_SECS: u64 = 60;

// ===== Cluster Defaults =====

pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 1000;
pub const DEFAULT_ELECTION_TIMEOUT_MS: u64 = 5000;
pub const DEFAULT_MAX_ENTRIES_PER_REQUEST: usize = 100;
pub const DEFAULT_SNAPSHOT_THRESHOLD: u64 = 10000;

// ===== Security Defaults =====

pub const DEFAULT_MAX_INPUT_LENGTH: usize = 10000;
pub const DEFAULT_SESSION_TIMEOUT_MINS: u64 = 30;
pub const DEFAULT_MAX_LOGIN_ATTEMPTS: usize = 5;
pub const DEFAULT_LOCKOUT_DURATION_MINS: u64 = 15;

// ===== Query Defaults =====

pub const DEFAULT_MAX_QUERY_DEPTH: usize = 10;
pub const DEFAULT_MAX_QUERY_COMPLEXITY: usize = 1000;
pub const DEFAULT_QUERY_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_MAX_RESULT_SIZE: usize = 10000;

// ===== Spatial Defaults =====

pub const DEFAULT_SPATIAL_PRECISION: usize = 12;
pub const DEFAULT_MAX_SPATIAL_RESULTS: usize = 1000;
pub const DEFAULT_SPATIAL_BUFFER_METERS: f64 = 1000.0;

// ===== Helper Functions =====

/// Get default connection timeout as Duration
pub fn default_connection_timeout() -> Duration {
    Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT_SECS)
}

/// Get default idle timeout as Duration
pub fn default_idle_timeout() -> Duration {
    Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS)
}

/// Get default max lifetime as Duration
pub fn default_max_lifetime() -> Duration {
    Duration::from_secs(DEFAULT_MAX_LIFETIME_SECS)
}

/// Get default health check interval as Duration
pub fn default_health_check_interval() -> Duration {
    Duration::from_secs(DEFAULT_HEALTH_CHECK_INTERVAL_SECS)
}

/// Get default cache TTL as Duration
pub fn default_cache_ttl() -> Duration {
    Duration::from_secs(DEFAULT_CACHE_TTL_SECONDS)
}

/// Get default batch timeout as Duration
pub fn default_batch_timeout() -> Duration {
    Duration::from_millis(DEFAULT_BATCH_TIMEOUT_MS)
}

/// Get default request timeout as Duration
pub fn default_request_timeout() -> Duration {
    Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)
}

/// Get default query timeout as Duration
pub fn default_query_timeout() -> Duration {
    Duration::from_secs(DEFAULT_QUERY_TIMEOUT_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_durations() {
        assert_eq!(default_connection_timeout(), Duration::from_secs(5));
        assert_eq!(default_idle_timeout(), Duration::from_secs(300));
        assert_eq!(default_max_lifetime(), Duration::from_secs(3600));
    }

    #[test]
    fn test_default_sizes() {
        assert!(DEFAULT_MIN_CONNECTIONS < DEFAULT_MAX_CONNECTIONS);
        assert!(DEFAULT_MIN_BATCH_SIZE < DEFAULT_MAX_BATCH_SIZE);
        assert!(DEFAULT_RESULT_CACHE_SIZE < DEFAULT_CACHE_MAX_SIZE);
    }

    #[test]
    fn test_default_thresholds() {
        assert!(DEFAULT_CONFIDENCE_THRESHOLD > 0.0 && DEFAULT_CONFIDENCE_THRESHOLD <= 1.0);
        assert!(DEFAULT_SIMILARITY_THRESHOLD > 0.0 && DEFAULT_SIMILARITY_THRESHOLD <= 1.0);
        assert!(DEFAULT_TARGET_UTILIZATION > 0.0 && DEFAULT_TARGET_UTILIZATION <= 1.0);
    }
}
