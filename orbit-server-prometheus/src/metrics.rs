use crate::{PrometheusError, PrometheusResult};
use chrono::{DateTime, Utc};
use metrics::{Counter, Gauge, Histogram};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Core metrics for Orbit server
pub struct OrbitMetrics {
    /// HTTP request metrics
    pub http_requests_total: Counter,
    pub http_request_duration: Histogram,
    pub http_requests_in_flight: Gauge,

    /// Server metrics
    pub server_uptime: Gauge,
    pub server_connections: Gauge,
    pub server_memory_usage: Gauge,
    pub server_cpu_usage: Gauge,

    /// Database metrics
    pub db_connections_active: Gauge,
    pub db_connections_idle: Gauge,
    pub db_query_duration: Histogram,
    pub db_queries_total: Counter,

    /// Cache metrics
    pub cache_hits_total: Counter,
    pub cache_misses_total: Counter,
    pub cache_size: Gauge,

    /// Query performance metrics
    pub query_execution_time: Histogram,
    pub slow_queries_total: Counter,
    pub query_errors_total: Counter,

    /// Lock metrics
    pub lock_acquisitions_total: Counter,
    pub lock_wait_time: Histogram,
    pub lock_contentions_total: Counter,

    /// Transaction metrics
    pub transactions_total: Counter,
    pub transaction_duration: Histogram,
    pub transaction_commits_total: Counter,
    pub transaction_rollbacks_total: Counter,

    /// I/O metrics
    pub disk_reads_total: Counter,
    pub disk_writes_total: Counter,
    pub disk_read_bytes_total: Counter,
    pub disk_write_bytes_total: Counter,

    /// Custom application metrics
    pub custom_metrics: Arc<std::sync::RwLock<HashMap<String, MetricValue>>>,

    /// Metrics metadata
    pub created_at: DateTime<Utc>,
    pub last_updated: Arc<std::sync::RwLock<DateTime<Utc>>>,
}

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(f64),
    Gauge(f64),
    Histogram {
        count: u64,
        sum: f64,
        buckets: HashMap<String, u64>,
    },
    Summary {
        count: u64,
        sum: f64,
        quantiles: HashMap<String, f64>,
    },
}

/// Metric metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricMetadata {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

/// Prometheus metric types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
    Info,
    StateSet,
    Unknown,
}

impl OrbitMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        let now = Utc::now();

        Self {
            http_requests_total: metrics::counter!("orbit_http_requests_total"),
            http_request_duration: metrics::histogram!("orbit_http_request_duration_seconds"),
            http_requests_in_flight: metrics::gauge!("orbit_http_requests_in_flight"),

            server_uptime: metrics::gauge!("orbit_server_uptime_seconds"),
            server_connections: metrics::gauge!("orbit_server_connections"),
            server_memory_usage: metrics::gauge!("orbit_server_memory_usage_bytes"),
            server_cpu_usage: metrics::gauge!("orbit_server_cpu_usage_percent"),

            db_connections_active: metrics::gauge!("orbit_db_connections_active"),
            db_connections_idle: metrics::gauge!("orbit_db_connections_idle"),
            db_query_duration: metrics::histogram!("orbit_db_query_duration_seconds"),
            db_queries_total: metrics::counter!("orbit_db_queries_total"),

            cache_hits_total: metrics::counter!("orbit_cache_hits_total"),
            cache_misses_total: metrics::counter!("orbit_cache_misses_total"),
            cache_size: metrics::gauge!("orbit_cache_size_bytes"),

            query_execution_time: metrics::histogram!("orbit_query_execution_seconds"),
            slow_queries_total: metrics::counter!("orbit_slow_queries_total"),
            query_errors_total: metrics::counter!("orbit_query_errors_total"),

            lock_acquisitions_total: metrics::counter!("orbit_lock_acquisitions_total"),
            lock_wait_time: metrics::histogram!("orbit_lock_wait_seconds"),
            lock_contentions_total: metrics::counter!("orbit_lock_contentions_total"),

            transactions_total: metrics::counter!("orbit_transactions_total"),
            transaction_duration: metrics::histogram!("orbit_transaction_duration_seconds"),
            transaction_commits_total: metrics::counter!("orbit_transaction_commits_total"),
            transaction_rollbacks_total: metrics::counter!("orbit_transaction_rollbacks_total"),

            disk_reads_total: metrics::counter!("orbit_disk_reads_total"),
            disk_writes_total: metrics::counter!("orbit_disk_writes_total"),
            disk_read_bytes_total: metrics::counter!("orbit_disk_read_bytes_total"),
            disk_write_bytes_total: metrics::counter!("orbit_disk_write_bytes_total"),

            custom_metrics: Arc::new(std::sync::RwLock::new(HashMap::new())),
            created_at: now,
            last_updated: Arc::new(std::sync::RwLock::new(now)),
        }
    }

    /// Record an HTTP request
    pub fn record_http_request(&self, _method: &str, _status: u16, duration: Duration) {
        // In a real implementation, labels would be applied here
        // For now, we'll just record the basic metrics

        self.http_requests_total.increment(1);
        self.http_request_duration.record(duration.as_secs_f64());

        self.update_timestamp();
    }

    /// Update server uptime
    pub fn update_uptime(&self, start_time: Instant) {
        let uptime = start_time.elapsed().as_secs_f64();
        self.server_uptime.set(uptime);
        self.update_timestamp();
    }

    /// Update connection count
    pub fn update_connections(&self, count: i64) {
        self.server_connections.set(count as f64);
        self.update_timestamp();
    }

    /// Update memory usage
    pub fn update_memory_usage(&self, bytes: u64) {
        self.server_memory_usage.set(bytes as f64);
        self.update_timestamp();
    }

    /// Update CPU usage
    pub fn update_cpu_usage(&self, percentage: f64) {
        self.server_cpu_usage.set(percentage);
        self.update_timestamp();
    }

    /// Record database query
    pub fn record_db_query(&self, _query_type: &str, duration: Duration, _success: bool) {
        // In a real implementation, labels would be applied here

        self.db_queries_total.increment(1);
        self.db_query_duration.record(duration.as_secs_f64());

        self.update_timestamp();
    }

    /// Update database connections
    pub fn update_db_connections(&self, active: i64, idle: i64) {
        self.db_connections_active.set(active as f64);
        self.db_connections_idle.set(idle as f64);
        self.update_timestamp();
    }

    /// Record cache hit
    pub fn record_cache_hit(&self, _cache_type: &str) {
        self.cache_hits_total.increment(1);
        self.update_timestamp();
    }

    /// Record cache miss
    pub fn record_cache_miss(&self, _cache_type: &str) {
        self.cache_misses_total.increment(1);
        self.update_timestamp();
    }

    /// Update cache size
    pub fn update_cache_size(&self, _cache_type: &str, bytes: u64) {
        self.cache_size.set(bytes as f64);
        self.update_timestamp();
    }

    /// Record query execution
    pub fn record_query_execution(&self, duration: Duration, is_slow: bool, success: bool) {
        self.query_execution_time.record(duration.as_secs_f64());
        
        if is_slow {
            self.slow_queries_total.increment(1);
        }
        
        if !success {
            self.query_errors_total.increment(1);
        }
        
        self.update_timestamp();
    }

    /// Record lock acquisition
    pub fn record_lock_acquisition(&self, wait_time: Duration, contended: bool) {
        self.lock_acquisitions_total.increment(1);
        self.lock_wait_time.record(wait_time.as_secs_f64());
        
        if contended {
            self.lock_contentions_total.increment(1);
        }
        
        self.update_timestamp();
    }

    /// Record transaction
    pub fn record_transaction(&self, duration: Duration, committed: bool) {
        self.transactions_total.increment(1);
        self.transaction_duration.record(duration.as_secs_f64());
        
        if committed {
            self.transaction_commits_total.increment(1);
        } else {
            self.transaction_rollbacks_total.increment(1);
        }
        
        self.update_timestamp();
    }

    /// Record disk I/O
    pub fn record_disk_io(&self, reads: u64, writes: u64, read_bytes: u64, write_bytes: u64) {
        if reads > 0 {
            self.disk_reads_total.increment(reads);
            self.disk_read_bytes_total.increment(read_bytes);
        }
        
        if writes > 0 {
            self.disk_writes_total.increment(writes);
            self.disk_write_bytes_total.increment(write_bytes);
        }
        
        self.update_timestamp();
    }

    /// Add custom metric
    pub fn add_custom_metric(&self, name: String, value: MetricValue) -> PrometheusResult<()> {
        let mut custom_metrics = self.custom_metrics.write().map_err(|e| {
            PrometheusError::collection(format!("Failed to acquire write lock: {}", e))
        })?;

        custom_metrics.insert(name, value);
        self.update_timestamp();

        Ok(())
    }

    /// Get custom metric
    pub fn get_custom_metric(&self, name: &str) -> PrometheusResult<Option<MetricValue>> {
        let custom_metrics = self.custom_metrics.read().map_err(|e| {
            PrometheusError::collection(format!("Failed to acquire read lock: {}", e))
        })?;

        Ok(custom_metrics.get(name).cloned())
    }

    /// List all custom metrics
    pub fn list_custom_metrics(&self) -> PrometheusResult<Vec<String>> {
        let custom_metrics = self.custom_metrics.read().map_err(|e| {
            PrometheusError::collection(format!("Failed to acquire read lock: {}", e))
        })?;

        Ok(custom_metrics.keys().cloned().collect())
    }

    /// Remove custom metric
    pub fn remove_custom_metric(&self, name: &str) -> PrometheusResult<Option<MetricValue>> {
        let mut custom_metrics = self.custom_metrics.write().map_err(|e| {
            PrometheusError::collection(format!("Failed to acquire write lock: {}", e))
        })?;

        let removed = custom_metrics.remove(name);
        if removed.is_some() {
            self.update_timestamp();
        }

        Ok(removed)
    }

    /// Update last updated timestamp
    fn update_timestamp(&self) {
        if let Ok(mut timestamp) = self.last_updated.write() {
            *timestamp = Utc::now();
        }
    }

    /// Get metrics summary
    pub fn get_summary(&self) -> PrometheusResult<MetricsSummary> {
        let custom_metrics = self.custom_metrics.read().map_err(|e| {
            PrometheusError::collection(format!("Failed to acquire read lock: {}", e))
        })?;

        let last_updated = self.last_updated.read().map_err(|e| {
            PrometheusError::collection(format!("Failed to acquire read lock: {}", e))
        })?;

        Ok(MetricsSummary {
            total_metrics: 29 + custom_metrics.len(), // 29 built-in metrics + custom
            custom_metrics_count: custom_metrics.len(),
            created_at: self.created_at,
            last_updated: *last_updated,
        })
    }
}

impl Default for OrbitMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_metrics: usize,
    pub custom_metrics_count: usize,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

/// System metrics collector
pub struct SystemMetricsCollector {
    start_time: Instant,
    metrics: Arc<OrbitMetrics>,
}

impl SystemMetricsCollector {
    /// Create new system metrics collector
    pub fn new(metrics: Arc<OrbitMetrics>) -> Self {
        Self {
            start_time: Instant::now(),
            metrics,
        }
    }

    /// Collect system metrics
    pub fn collect(&self) -> PrometheusResult<()> {
        // Update uptime
        self.metrics.update_uptime(self.start_time);

        // Collect memory usage (simplified)
        self.collect_memory_usage()?;

        // Collect CPU usage (simplified)
        self.collect_cpu_usage()?;

        Ok(())
    }

    /// Collect memory usage metrics
    fn collect_memory_usage(&self) -> PrometheusResult<()> {
        // In a real implementation, this would collect actual system memory usage
        // For now, we'll use a placeholder value
        let memory_usage = self.get_process_memory_usage();
        self.metrics.update_memory_usage(memory_usage);

        Ok(())
    }

    /// Collect CPU usage metrics
    fn collect_cpu_usage(&self) -> PrometheusResult<()> {
        // In a real implementation, this would collect actual CPU usage
        // For now, we'll use a placeholder value
        let cpu_usage = self.get_process_cpu_usage();
        self.metrics.update_cpu_usage(cpu_usage);

        Ok(())
    }

    /// Get process memory usage (simplified implementation)
    fn get_process_memory_usage(&self) -> u64 {
        // This is a simplified implementation
        // In production, you would use system APIs or libraries like `sysinfo`
        1024 * 1024 * 100 // 100MB placeholder
    }

    /// Get process CPU usage (simplified implementation)
    fn get_process_cpu_usage(&self) -> f64 {
        // This is a simplified implementation
        // In production, you would calculate actual CPU usage
        5.0 // 5% placeholder
    }
}

/// Metrics formatter for different output formats
pub struct MetricsFormatter;

impl MetricsFormatter {
    /// Format metrics for Prometheus text format
    pub fn format_prometheus(_metrics: &OrbitMetrics) -> PrometheusResult<String> {
        let mut output = String::new();

        // Add help and type information
        output.push_str("# HELP orbit_http_requests_total Total HTTP requests processed\n");
        output.push_str("# TYPE orbit_http_requests_total counter\n");

        output.push_str("# HELP orbit_server_uptime_seconds Server uptime in seconds\n");
        output.push_str("# TYPE orbit_server_uptime_seconds gauge\n");

        // In a real implementation, you would iterate through all metrics
        // and format them according to the Prometheus text format specification

        Ok(output)
    }

    /// Format metrics as JSON
    pub fn format_json(metrics: &OrbitMetrics) -> PrometheusResult<String> {
        let summary = metrics.get_summary()?;

        let json_metrics = serde_json::json!({
            "summary": summary,
            "timestamp": Utc::now(),
            "status": "ok"
        });

        serde_json::to_string_pretty(&json_metrics)
            .map_err(|e| PrometheusError::serialization(format!("Failed to serialize JSON: {}", e)))
    }
}

/// Metrics validation utilities
pub struct MetricsValidator;

impl MetricsValidator {
    /// Validate metric name
    pub fn validate_name(name: &str) -> PrometheusResult<()> {
        if name.is_empty() {
            return Err(PrometheusError::metric_validation(
                name,
                "Metric name cannot be empty",
            ));
        }

        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
        {
            return Err(PrometheusError::metric_validation(
                name,
                "Metric name can only contain alphanumeric characters, underscores, and colons",
            ));
        }

        if name.starts_with(|c: char| c.is_numeric()) {
            return Err(PrometheusError::metric_validation(
                name,
                "Metric name cannot start with a number",
            ));
        }

        Ok(())
    }

    /// Validate metric labels
    pub fn validate_labels(labels: &HashMap<String, String>) -> PrometheusResult<()> {
        for (key, value) in labels {
            if key.is_empty() {
                return Err(PrometheusError::metric_validation(
                    key,
                    "Label key cannot be empty",
                ));
            }

            if key.starts_with("__") {
                return Err(PrometheusError::metric_validation(
                    key,
                    "Label keys cannot start with '__' (reserved for internal use)",
                ));
            }

            if value.is_empty() {
                return Err(PrometheusError::metric_validation(
                    format!("{}={}", key, value),
                    "Label value cannot be empty",
                ));
            }
        }

        Ok(())
    }

    /// Validate metric value
    pub fn validate_value(value: &MetricValue) -> PrometheusResult<()> {
        match value {
            MetricValue::Counter(v) | MetricValue::Gauge(v) => {
                if v.is_nan() || v.is_infinite() {
                    return Err(PrometheusError::metric_validation(
                        "value",
                        "Metric value cannot be NaN or infinite",
                    ));
                }
            }
            MetricValue::Histogram { count, sum, .. } => {
                if sum.is_nan() || sum.is_infinite() {
                    return Err(PrometheusError::metric_validation(
                        "histogram_sum",
                        "Histogram sum cannot be NaN or infinite",
                    ));
                }
                if *count == 0 && *sum != 0.0 {
                    return Err(PrometheusError::metric_validation(
                        "histogram",
                        "Histogram cannot have zero count with non-zero sum",
                    ));
                }
            }
            MetricValue::Summary { count, sum, .. } => {
                if sum.is_nan() || sum.is_infinite() {
                    return Err(PrometheusError::metric_validation(
                        "summary_sum",
                        "Summary sum cannot be NaN or infinite",
                    ));
                }
                if *count == 0 && *sum != 0.0 {
                    return Err(PrometheusError::metric_validation(
                        "summary",
                        "Summary cannot have zero count with non-zero sum",
                    ));
                }
            }
        }

        Ok(())
    }
}
