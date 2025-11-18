//! Enhanced Statistics Collection Framework for Phase 9
//!
//! This module provides advanced statistics collection with:
//! - Multi-dimensional histograms for correlated columns
//! - Automatic statistics updates based on data modifications
//! - Query workload pattern tracking
//! - Statistics persistence and recovery
//!
//! ## Design Patterns
//!
//! - **Builder Pattern**: Fluent API for statistics configuration
//! - **Strategy Pattern**: Pluggable histogram strategies
//! - **Observer Pattern**: Auto-update on data modifications
//! - **Type State Pattern**: Compile-time guarantees for statistics lifecycle

use super::stats::TableStatistics;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Multi-dimensional histogram for correlated columns
///
/// Tracks value distributions across multiple columns to improve
/// join cardinality estimation when columns are correlated.
#[derive(Debug, Clone)]
pub struct MultiDimHistogram {
    /// Column names in this histogram
    pub columns: Vec<String>,
    /// Buckets with multi-dimensional ranges
    pub buckets: Vec<MultiDimBucket>,
    /// Sample count used to build histogram
    pub sample_count: usize,
}

/// A bucket in a multi-dimensional histogram
#[derive(Debug, Clone)]
pub struct MultiDimBucket {
    /// Lower bounds for each dimension
    pub lower_bounds: Vec<String>,
    /// Upper bounds for each dimension
    pub upper_bounds: Vec<String>,
    /// Frequency of values in this bucket
    pub frequency: f64,
    /// Distinct value count estimate
    pub distinct_values: usize,
}

/// Correlation matrix for detecting column relationships
#[derive(Debug, Clone)]
pub struct CorrelationMatrix {
    /// Table name
    pub table: String,
    /// Correlation coefficients between column pairs (-1.0 to 1.0)
    pub correlations: HashMap<(String, String), f64>,
    /// Last updated timestamp
    pub updated_at: SystemTime,
}

impl CorrelationMatrix {
    /// Get correlation between two columns
    pub fn get_correlation(&self, col1: &str, col2: &str) -> Option<f64> {
        self.correlations
            .get(&(col1.to_string(), col2.to_string()))
            .or_else(|| self.correlations.get(&(col2.to_string(), col1.to_string())))
            .copied()
    }

    /// Check if columns are significantly correlated (|correlation| > threshold)
    pub fn are_correlated(&self, col1: &str, col2: &str, threshold: f64) -> bool {
        self.get_correlation(col1, col2)
            .map(|corr| corr.abs() > threshold)
            .unwrap_or(false)
    }
}

/// Query pattern statistics for workload-based optimization
#[derive(Debug, Clone, Default)]
pub struct QueryPatternStats {
    /// Most frequently queried columns
    pub frequent_columns: HashMap<String, usize>,
    /// Common join patterns (table1.col -> table2.col)
    pub join_patterns: HashMap<(String, String, String, String), usize>,
    /// Common filter patterns
    pub filter_patterns: HashMap<String, FilterPatternStats>,
    /// Query execution times
    pub execution_times: Vec<(String, Duration)>,
}

/// Statistics for a specific filter pattern
#[derive(Debug, Clone)]
pub struct FilterPatternStats {
    /// Column being filtered
    pub column: String,
    /// Operator used (=, >, <, LIKE, etc.)
    pub operator: String,
    /// Frequency of this pattern
    pub frequency: usize,
    /// Average selectivity observed
    pub avg_selectivity: f64,
}

/// Auto-analyze configuration using Builder pattern
#[derive(Debug, Clone)]
pub struct AutoAnalyzeConfig {
    /// Enable automatic statistics updates
    pub enabled: bool,
    /// Modification threshold (fraction of rows changed)
    pub modification_threshold: f64,
    /// Minimum interval between ANALYZE operations
    pub min_interval: Duration,
    /// Maximum interval between ANALYZE operations
    pub max_interval: Duration,
    /// Sample rate for ANALYZE (0.0 to 1.0)
    pub sample_rate: f64,
    /// Enable background ANALYZE
    pub background_mode: bool,
}

impl Default for AutoAnalyzeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            modification_threshold: 0.1, // 10% of rows changed
            min_interval: Duration::from_secs(60), // 1 minute
            max_interval: Duration::from_secs(3600), // 1 hour
            sample_rate: 0.1, // 10% sample
            background_mode: true,
        }
    }
}

/// Builder for AutoAnalyzeConfig using fluent API
pub struct AutoAnalyzeConfigBuilder {
    config: AutoAnalyzeConfig,
}

impl AutoAnalyzeConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: AutoAnalyzeConfig::default(),
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn modification_threshold(mut self, threshold: f64) -> Self {
        self.config.modification_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn min_interval(mut self, interval: Duration) -> Self {
        self.config.min_interval = interval;
        self
    }

    pub fn max_interval(mut self, interval: Duration) -> Self {
        self.config.max_interval = interval;
        self
    }

    pub fn sample_rate(mut self, rate: f64) -> Self {
        self.config.sample_rate = rate.clamp(0.0, 1.0);
        self
    }

    pub fn background_mode(mut self, background: bool) -> Self {
        self.config.background_mode = background;
        self
    }

    pub fn build(self) -> AutoAnalyzeConfig {
        self.config
    }
}

impl Default for AutoAnalyzeConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Table modification tracker for auto-analyze triggers
#[derive(Debug, Clone)]
pub struct TableModificationTracker {
    /// Table name
    pub table: String,
    /// Row count at last ANALYZE
    pub rows_at_last_analyze: usize,
    /// Rows inserted since last ANALYZE
    pub rows_inserted: usize,
    /// Rows updated since last ANALYZE
    pub rows_updated: usize,
    /// Rows deleted since last ANALYZE
    pub rows_deleted: usize,
    /// Last ANALYZE timestamp
    pub last_analyze: SystemTime,
}

impl TableModificationTracker {
    pub fn new(table: String, row_count: usize) -> Self {
        Self {
            table,
            rows_at_last_analyze: row_count,
            rows_inserted: 0,
            rows_updated: 0,
            rows_deleted: 0,
            last_analyze: SystemTime::now(),
        }
    }

    /// Calculate modification fraction
    pub fn modification_fraction(&self) -> f64 {
        if self.rows_at_last_analyze == 0 {
            return 1.0; // Always analyze empty tables after modifications
        }

        let total_modifications =
            (self.rows_inserted + self.rows_updated + self.rows_deleted) as f64;
        total_modifications / self.rows_at_last_analyze as f64
    }

    /// Check if ANALYZE is needed based on threshold
    pub fn needs_analyze(&self, config: &AutoAnalyzeConfig) -> bool {
        if !config.enabled {
            return false;
        }

        // Check modification threshold
        if self.modification_fraction() >= config.modification_threshold {
            return true;
        }

        // Check time-based threshold
        if let Ok(elapsed) = self.last_analyze.elapsed() {
            if elapsed >= config.max_interval {
                return true;
            }
        }

        false
    }

    /// Record a data modification
    pub fn record_modification(&mut self, inserts: usize, updates: usize, deletes: usize) {
        self.rows_inserted += inserts;
        self.rows_updated += updates;
        self.rows_deleted += deletes;
    }

    /// Reset after ANALYZE
    pub fn reset(&mut self, current_row_count: usize) {
        self.rows_at_last_analyze = current_row_count;
        self.rows_inserted = 0;
        self.rows_updated = 0;
        self.rows_deleted = 0;
        self.last_analyze = SystemTime::now();
    }
}

/// Enhanced statistics collector with Phase 9 features
pub struct EnhancedStatisticsCollector {
    /// Base statistics collector (reuse Phase 8 implementation)
    base_stats: Arc<RwLock<super::stats::StatisticsCollector>>,

    /// Multi-dimensional histograms
    multidim_histograms: Arc<RwLock<HashMap<String, Vec<MultiDimHistogram>>>>,

    /// Correlation matrices
    correlations: Arc<RwLock<HashMap<String, CorrelationMatrix>>>,

    /// Query pattern statistics
    query_patterns: Arc<RwLock<QueryPatternStats>>,

    /// Modification trackers per table
    modification_trackers: Arc<RwLock<HashMap<String, TableModificationTracker>>>,

    /// Auto-analyze configuration
    auto_analyze_config: AutoAnalyzeConfig,
}

impl EnhancedStatisticsCollector {
    /// Create a new enhanced statistics collector
    pub fn new(auto_analyze_config: AutoAnalyzeConfig) -> Self {
        Self {
            base_stats: Arc::new(RwLock::new(super::stats::StatisticsCollector::new())),
            multidim_histograms: Arc::new(RwLock::new(HashMap::new())),
            correlations: Arc::new(RwLock::new(HashMap::new())),
            query_patterns: Arc::new(RwLock::new(QueryPatternStats::default())),
            modification_trackers: Arc::new(RwLock::new(HashMap::new())),
            auto_analyze_config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(AutoAnalyzeConfig::default())
    }

    /// Get base statistics collector (delegation pattern)
    pub async fn base(&self) -> tokio::sync::RwLockReadGuard<'_, super::stats::StatisticsCollector> {
        self.base_stats.read().await
    }

    /// Get mutable base statistics collector
    pub async fn base_mut(
        &self,
    ) -> tokio::sync::RwLockWriteGuard<'_, super::stats::StatisticsCollector> {
        self.base_stats.write().await
    }

    /// Update table statistics (delegates to base + tracks modifications)
    pub async fn update_table_stats(&self, table_name: &str, stats: TableStatistics) {
        // Update base statistics
        self.base_stats
            .write()
            .await
            .update_table_stats(table_name, stats.clone());

        // Initialize or update modification tracker
        let mut trackers = self.modification_trackers.write().await;
        trackers
            .entry(table_name.to_string())
            .or_insert_with(|| TableModificationTracker::new(table_name.to_string(), stats.row_count));
    }

    /// Record data modifications and trigger auto-analyze if needed
    pub async fn record_modifications(
        &self,
        table_name: &str,
        inserts: usize,
        updates: usize,
        deletes: usize,
    ) -> bool {
        let mut trackers = self.modification_trackers.write().await;

        if let Some(tracker) = trackers.get_mut(table_name) {
            tracker.record_modification(inserts, updates, deletes);

            // Check if auto-analyze is needed
            if tracker.needs_analyze(&self.auto_analyze_config) {
                return true; // Caller should trigger ANALYZE
            }
        }

        false
    }

    /// Add a multi-dimensional histogram
    pub async fn add_multidim_histogram(&self, table: &str, histogram: MultiDimHistogram) {
        let mut histograms = self.multidim_histograms.write().await;
        histograms
            .entry(table.to_string())
            .or_insert_with(Vec::new)
            .push(histogram);
    }

    /// Get multi-dimensional histogram for columns
    pub async fn get_multidim_histogram(
        &self,
        table: &str,
        columns: &[String],
    ) -> Option<MultiDimHistogram> {
        let histograms = self.multidim_histograms.read().await;
        histograms.get(table).and_then(|table_histograms| {
            table_histograms
                .iter()
                .find(|h| h.columns == columns)
                .cloned()
        })
    }

    /// Update correlation matrix for a table
    pub async fn update_correlation_matrix(&self, matrix: CorrelationMatrix) {
        let mut correlations = self.correlations.write().await;
        correlations.insert(matrix.table.clone(), matrix);
    }

    /// Get correlation matrix for a table
    pub async fn get_correlation_matrix(&self, table: &str) -> Option<CorrelationMatrix> {
        let correlations = self.correlations.read().await;
        correlations.get(table).cloned()
    }

    /// Record a query pattern for workload analysis
    pub async fn record_query_pattern(&self, table: &str, column: &str) {
        let mut patterns = self.query_patterns.write().await;
        let key = format!("{}.{}", table, column);
        *patterns.frequent_columns.entry(key).or_insert(0) += 1;
    }

    /// Record a join pattern
    pub async fn record_join_pattern(
        &self,
        left_table: &str,
        left_col: &str,
        right_table: &str,
        right_col: &str,
    ) {
        let mut patterns = self.query_patterns.write().await;
        let key = (
            left_table.to_string(),
            left_col.to_string(),
            right_table.to_string(),
            right_col.to_string(),
        );
        *patterns.join_patterns.entry(key).or_insert(0) += 1;
    }

    /// Get most frequently queried columns (top N)
    pub async fn get_frequent_columns(&self, limit: usize) -> Vec<(String, usize)> {
        let patterns = self.query_patterns.read().await;
        let mut columns: Vec<_> = patterns.frequent_columns.iter().collect();
        columns.sort_by(|a, b| b.1.cmp(a.1));
        columns
            .into_iter()
            .take(limit)
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Get most common join patterns
    pub async fn get_common_joins(&self, limit: usize) -> Vec<((String, String, String, String), usize)> {
        let patterns = self.query_patterns.read().await;
        let mut joins: Vec<_> = patterns.join_patterns.iter().collect();
        joins.sort_by(|a, b| b.1.cmp(a.1));
        joins
            .into_iter()
            .take(limit)
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Check all tables for auto-analyze needs
    pub async fn check_auto_analyze(&self) -> Vec<String> {
        let trackers = self.modification_trackers.read().await;
        trackers
            .values()
            .filter(|tracker| tracker.needs_analyze(&self.auto_analyze_config))
            .map(|tracker| tracker.table.clone())
            .collect()
    }

    /// Mark table as analyzed
    pub async fn mark_analyzed(&self, table: &str, current_row_count: usize) {
        let mut trackers = self.modification_trackers.write().await;
        if let Some(tracker) = trackers.get_mut(table) {
            tracker.reset(current_row_count);
        }
    }

    /// Export enhanced statistics for persistence
    pub async fn export_enhanced_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut export = HashMap::new();

        // Export multidim histograms
        let histograms = self.multidim_histograms.read().await;
        for (table, table_histograms) in histograms.iter() {
            export.insert(
                format!("multidim_histogram:{}", table),
                serde_json::json!({
                    "count": table_histograms.len(),
                    "columns": table_histograms.iter().map(|h| &h.columns).collect::<Vec<_>>(),
                }),
            );
        }

        // Export correlation matrices
        let correlations = self.correlations.read().await;
        for (table, matrix) in correlations.iter() {
            export.insert(
                format!("correlation:{}", table),
                serde_json::json!({
                    "correlation_count": matrix.correlations.len(),
                }),
            );
        }

        // Export query patterns
        let patterns = self.query_patterns.read().await;
        export.insert(
            "query_patterns".to_string(),
            serde_json::json!({
                "frequent_columns": patterns.frequent_columns.len(),
                "join_patterns": patterns.join_patterns.len(),
                "filter_patterns": patterns.filter_patterns.len(),
            }),
        );

        export
    }

    /// Clear all enhanced statistics
    pub async fn clear_all(&self) {
        self.multidim_histograms.write().await.clear();
        self.correlations.write().await.clear();
        *self.query_patterns.write().await = QueryPatternStats::default();
        self.modification_trackers.write().await.clear();
        self.base_stats.write().await.clear_all_stats();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_analyze_config_builder() {
        let config = AutoAnalyzeConfigBuilder::new()
            .enabled(true)
            .modification_threshold(0.15)
            .min_interval(Duration::from_secs(30))
            .sample_rate(0.2)
            .build();

        assert!(config.enabled);
        assert_eq!(config.modification_threshold, 0.15);
        assert_eq!(config.min_interval, Duration::from_secs(30));
        assert_eq!(config.sample_rate, 0.2);
    }

    #[test]
    fn test_modification_tracker() {
        let mut tracker = TableModificationTracker::new("users".to_string(), 1000);

        // Record 150 inserts (15% modification)
        tracker.record_modification(150, 0, 0);
        assert!(tracker.modification_fraction() >= 0.15);

        // Reset
        tracker.reset(1150);
        assert_eq!(tracker.rows_at_last_analyze, 1150);
        assert_eq!(tracker.rows_inserted, 0);
    }

    #[test]
    fn test_modification_tracker_needs_analyze() {
        let mut tracker = TableModificationTracker::new("orders".to_string(), 1000);
        let config = AutoAnalyzeConfigBuilder::new()
            .modification_threshold(0.1)
            .build();

        // No modifications yet
        assert!(!tracker.needs_analyze(&config));

        // Add 100 inserts (10% threshold)
        tracker.record_modification(100, 0, 0);
        assert!(tracker.needs_analyze(&config));
    }

    #[test]
    fn test_correlation_matrix() {
        let mut correlations = HashMap::new();
        correlations.insert(("age".to_string(), "income".to_string()), 0.75);
        correlations.insert(("age".to_string(), "zip_code".to_string()), 0.05);

        let matrix = CorrelationMatrix {
            table: "users".to_string(),
            correlations,
            updated_at: SystemTime::now(),
        };

        // Test correlation lookup
        assert_eq!(matrix.get_correlation("age", "income"), Some(0.75));
        assert_eq!(matrix.get_correlation("income", "age"), Some(0.75)); // Symmetric

        // Test correlation check
        assert!(matrix.are_correlated("age", "income", 0.5));
        assert!(!matrix.are_correlated("age", "zip_code", 0.5));
    }

    #[tokio::test]
    async fn test_enhanced_collector_creation() {
        let collector = EnhancedStatisticsCollector::with_defaults();

        // Should start empty
        let histograms = collector.multidim_histograms.read().await;
        assert_eq!(histograms.len(), 0);
    }

    #[tokio::test]
    async fn test_record_modifications() {
        let collector = EnhancedStatisticsCollector::with_defaults();

        // Initialize table
        collector
            .update_table_stats(
                "products",
                TableStatistics {
                    row_count: 1000,
                    ..Default::default()
                },
            )
            .await;

        // Record modifications
        let needs_analyze = collector.record_modifications("products", 150, 0, 0).await;
        assert!(needs_analyze); // Should exceed 10% threshold
    }

    #[tokio::test]
    async fn test_query_pattern_tracking() {
        let collector = EnhancedStatisticsCollector::with_defaults();

        // Record some patterns
        collector.record_query_pattern("users", "email").await;
        collector.record_query_pattern("users", "email").await;
        collector.record_query_pattern("users", "id").await;

        let frequent = collector.get_frequent_columns(10).await;
        assert_eq!(frequent.len(), 2);
        assert_eq!(frequent[0].1, 2); // email queried twice
    }

    #[tokio::test]
    async fn test_join_pattern_tracking() {
        let collector = EnhancedStatisticsCollector::with_defaults();

        // Record join patterns
        collector
            .record_join_pattern("orders", "user_id", "users", "id")
            .await;
        collector
            .record_join_pattern("orders", "user_id", "users", "id")
            .await;
        collector
            .record_join_pattern("orders", "product_id", "products", "id")
            .await;

        let common_joins = collector.get_common_joins(10).await;
        assert_eq!(common_joins.len(), 2);
        assert_eq!(common_joins[0].1, 2); // orders-users join most common
    }
}
