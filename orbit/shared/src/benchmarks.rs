//! Performance benchmarks for critical paths in Orbit applications
//!
//! This module provides comprehensive benchmarking utilities to measure
//! and track performance of critical operations.

use crate::error_handling::SecurityValidator;
use crate::exception::OrbitError;
use crate::security_patterns::{AttackDetector, RateLimiter};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Performance benchmark suite
pub struct BenchmarkSuite {
    results: Arc<RwLock<HashMap<String, BenchmarkResult>>>,
}

/// Benchmark result with statistical information
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub samples: Vec<Duration>,
    pub mean: Duration,
    pub median: Duration,
    pub min: Duration,
    pub max: Duration,
    pub std_dev: Duration,
    pub ops_per_sec: f64,
}

impl BenchmarkResult {
    pub fn new(name: String, mut samples: Vec<Duration>) -> Self {
        samples.sort();
        let total_duration: Duration = samples.iter().sum();
        let count = samples.len() as f64;

        let mean = total_duration / samples.len() as u32;
        let median = samples[samples.len() / 2];
        let min = samples.first().copied().unwrap_or_default();
        let max = samples.last().copied().unwrap_or_default();

        // Calculate standard deviation
        let mean_nanos = mean.as_nanos() as f64;
        let variance: f64 = samples
            .iter()
            .map(|d| {
                let diff = d.as_nanos() as f64 - mean_nanos;
                diff * diff
            })
            .sum::<f64>()
            / count;
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);

        let ops_per_sec = if mean.as_secs_f64() > 0.0 {
            1.0 / mean.as_secs_f64()
        } else {
            f64::INFINITY
        };

        Self {
            name,
            samples,
            mean,
            median,
            min,
            max,
            std_dev,
            ops_per_sec,
        }
    }
}

impl BenchmarkSuite {
    /// Create a new benchmark suite
    pub fn new() -> Self {
        Self {
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Benchmark an operation
    pub async fn benchmark<F, T, Fut>(
        &self,
        name: &str,
        iterations: usize,
        operation: F,
    ) -> Result<BenchmarkResult, OrbitError>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, OrbitError>> + Send,
        T: Send,
    {
        let mut samples = Vec::with_capacity(iterations);

        info!("Starting benchmark: {} ({} iterations)", name, iterations);

        for i in 0..iterations {
            let start = Instant::now();

            match operation().await {
                Ok(_) => {
                    let duration = start.elapsed();
                    samples.push(duration);
                }
                Err(e) => {
                    warn!("Benchmark iteration {} failed for {}: {}", i, name, e);
                    return Err(e);
                }
            }
        }

        let result = BenchmarkResult::new(name.to_string(), samples);

        info!(
            "Benchmark {} completed: mean={:?}, median={:?}, ops/sec={:.2}",
            name, result.mean, result.median, result.ops_per_sec
        );

        // Store result
        let mut results = self.results.write().await;
        results.insert(name.to_string(), result.clone());

        Ok(result)
    }

    /// Benchmark a synchronous operation
    pub async fn benchmark_sync<F, T>(
        &self,
        name: &str,
        iterations: usize,
        operation: F,
    ) -> Result<BenchmarkResult, OrbitError>
    where
        F: Fn() -> Result<T, OrbitError> + Send + Sync,
        T: Send,
    {
        self.benchmark(name, iterations, || async { operation() })
            .await
    }

    /// Get all benchmark results
    pub async fn get_results(&self) -> HashMap<String, BenchmarkResult> {
        self.results.read().await.clone()
    }

    /// Generate performance report
    pub async fn generate_report(&self) -> String {
        let results = self.results.read().await;
        let mut report = String::new();

        report.push_str("# Performance Benchmark Report\n\n");

        if results.is_empty() {
            report.push_str("No benchmark results available.\n");
            return report;
        }

        // Sort by operations per second (descending)
        let mut sorted_results: Vec<_> = results.values().collect();
        sorted_results.sort_by(|a, b| {
            b.ops_per_sec
                .partial_cmp(&a.ops_per_sec)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        report
            .push_str("| Benchmark | Mean | Median | Min | Max | Std Dev | Ops/Sec | Samples |\n");
        report
            .push_str("|-----------|------|--------|-----|-----|---------|---------|----------|\n");

        for result in &sorted_results {
            report.push_str(&format!(
                "| {} | {:?} | {:?} | {:?} | {:?} | {:?} | {:.2} | {} |\n",
                result.name,
                result.mean,
                result.median,
                result.min,
                result.max,
                result.std_dev,
                result.ops_per_sec,
                result.samples.len()
            ));
        }

        report.push_str("\n## Performance Analysis\n\n");

        // Find fastest and slowest operations
        if let (Some(fastest), Some(slowest)) = (sorted_results.first(), sorted_results.last()) {
            report.push_str(&format!(
                "- **Fastest operation**: {} ({:.2} ops/sec)\n",
                fastest.name, fastest.ops_per_sec
            ));
            report.push_str(&format!(
                "- **Slowest operation**: {} ({:.2} ops/sec)\n",
                slowest.name, slowest.ops_per_sec
            ));

            if fastest.ops_per_sec > 0.0 && slowest.ops_per_sec > 0.0 {
                let ratio = fastest.ops_per_sec / slowest.ops_per_sec;
                report.push_str(&format!(
                    "- **Performance ratio**: {ratio:.2}x difference\n"
                ));
            }
        }

        report
    }
}

/// Built-in benchmarks for common operations
pub struct StandardBenchmarks;

impl StandardBenchmarks {
    /// Benchmark security validation operations
    pub async fn benchmark_security_validation(suite: &BenchmarkSuite) -> Result<(), OrbitError> {
        // Benchmark SQL validation
        suite
            .benchmark_sync("security_sql_validation", 1000, || {
                let input = "SELECT * FROM users WHERE id = 1";
                input.validate_sql_safe()
            })
            .await?;

        // Benchmark XSS validation
        suite
            .benchmark_sync("security_xss_validation", 1000, || {
                let input = "<div>Hello World</div>";
                input.validate_xss_safe()
            })
            .await?;

        // Benchmark length validation
        suite
            .benchmark_sync("security_length_validation", 1000, || {
                let input = "This is a test string for length validation";
                input.validate_length(1000)
            })
            .await?;

        Ok(())
    }

    /// Benchmark rate limiting operations
    pub async fn benchmark_rate_limiting(suite: &BenchmarkSuite) -> Result<(), OrbitError> {
        let rate_limiter = RateLimiter::new(1000, Duration::from_secs(60));

        suite
            .benchmark("rate_limiter_check", 500, || async {
                rate_limiter.is_allowed("benchmark_client").await
            })
            .await?;

        suite
            .benchmark("rate_limiter_stats", 100, || async {
                rate_limiter.get_usage("benchmark_client").await
            })
            .await?;

        Ok(())
    }

    /// Benchmark attack detection
    pub async fn benchmark_attack_detection(suite: &BenchmarkSuite) -> Result<(), OrbitError> {
        let detector = AttackDetector::new()?;

        suite
            .benchmark("attack_detection_safe", 200, || async {
                detector
                    .detect_attack("Hello world", "benchmark_client")
                    .await
            })
            .await?;

        suite
            .benchmark("attack_detection_malicious", 200, || async {
                detector
                    .detect_attack("'; DROP TABLE users; --", "benchmark_client")
                    .await
            })
            .await?;

        Ok(())
    }

    /// Benchmark string operations
    pub async fn benchmark_string_operations(suite: &BenchmarkSuite) -> Result<(), OrbitError> {
        suite
            .benchmark_sync("string_clone", 10000, || {
                let s = "This is a test string for cloning benchmarks";
                let _cloned = s.to_string();
                Ok(())
            })
            .await?;

        suite
            .benchmark_sync("string_concatenation", 5000, || {
                let s1 = "Hello";
                let s2 = "World";
                let _result = format!("{s1} {s2}");
                Ok(())
            })
            .await?;

        Ok(())
    }

    /// Run all standard benchmarks
    pub async fn run_all(suite: &BenchmarkSuite) -> Result<(), OrbitError> {
        info!("Running standard benchmark suite");

        Self::benchmark_security_validation(suite).await?;
        Self::benchmark_rate_limiting(suite).await?;
        Self::benchmark_attack_detection(suite).await?;
        Self::benchmark_string_operations(suite).await?;

        info!("Standard benchmark suite completed");
        Ok(())
    }
}

/// Macro to easily benchmark code blocks
#[macro_export]
macro_rules! benchmark_block {
    ($suite:expr, $name:expr, $iterations:expr, $block:block) => {{
        $suite
            .benchmark_sync($name, $iterations, || {
                $block;
                Ok(())
            })
            .await
    }};
}

impl Default for BenchmarkSuite {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_benchmark_suite() {
        let suite = BenchmarkSuite::new();

        // Simple benchmark test
        let result = suite
            .benchmark_sync("test_operation", 10, || {
                // Simulate some work
                std::thread::sleep(Duration::from_millis(1));
                Ok(42)
            })
            .await
            .unwrap();

        assert_eq!(result.name, "test_operation");
        assert_eq!(result.samples.len(), 10);
        assert!(result.mean.as_millis() >= 1);
        assert!(result.ops_per_sec > 0.0);
    }

    #[tokio::test]
    async fn test_benchmark_result_statistics() {
        let samples = vec![
            Duration::from_millis(1),
            Duration::from_millis(2),
            Duration::from_millis(3),
            Duration::from_millis(4),
            Duration::from_millis(5),
        ];

        let result = BenchmarkResult::new("test".to_string(), samples);

        assert_eq!(result.min, Duration::from_millis(1));
        assert_eq!(result.max, Duration::from_millis(5));
        assert_eq!(result.median, Duration::from_millis(3));
        assert!(result.ops_per_sec > 0.0);
    }

    #[tokio::test]
    async fn test_standard_benchmarks() {
        let suite = BenchmarkSuite::new();

        // Run a subset of standard benchmarks for testing
        StandardBenchmarks::benchmark_string_operations(&suite)
            .await
            .unwrap();

        let results = suite.get_results().await;
        assert!(!results.is_empty());
        assert!(results.contains_key("string_clone"));
        assert!(results.contains_key("string_concatenation"));
    }

    #[tokio::test]
    async fn test_benchmark_macro() {
        let suite = BenchmarkSuite::new();

        let result = benchmark_block!(suite, "macro_test", 5, {
            let _x = 42 + 24;
        })
        .unwrap();

        assert_eq!(result.name, "macro_test");
        assert_eq!(result.samples.len(), 5);
    }
}
