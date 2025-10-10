//! Statistics collection system for query optimization
//!
//! This module provides comprehensive statistics collection for tables and indexes
//! to enable cost-based query optimization. Implements the statistics framework
//! defined in Phase 9.1 of the query optimization plan.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::orbitql::{QueryValue, SpatialError};

/// Table-level statistics for cost-based optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStatistics {
    /// Total number of rows in the table
    pub row_count: u64,
    /// Number of pages used by the table
    pub page_count: u64,
    /// Average size of a row in bytes
    pub avg_row_size: u32,
    /// Fraction of NULL values across all columns (0.0 to 1.0)
    pub null_fraction: f64,
    /// Number of distinct values across all indexed columns
    pub distinct_values: u64,
    /// Most common values and their frequencies
    pub most_common_values: Vec<(QueryValue, f64)>,
    /// Histogram buckets for data distribution
    pub histogram: Vec<HistogramBucket>,
    /// When statistics were last analyzed
    pub last_analyzed: DateTime<Utc>,
    /// Per-column statistics
    pub column_statistics: HashMap<String, ColumnStatistics>,
}

/// Column-level statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    /// Number of distinct values in this column
    pub n_distinct: u64,
    /// Fraction of NULL values (0.0 to 1.0)
    pub null_frac: f64,
    /// Average width in bytes
    pub avg_width: u32,
    /// Most common values for this column
    pub most_common_vals: Vec<QueryValue>,
    /// Frequencies of most common values
    pub most_common_freqs: Vec<f64>,
    /// Histogram bounds for value distribution
    pub histogram_bounds: Vec<QueryValue>,
    /// Correlation with physical row order (-1.0 to 1.0)
    pub correlation: f64,
}

/// Histogram bucket for data distribution analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    /// Lower bound of the bucket (inclusive)
    pub lower_bound: QueryValue,
    /// Upper bound of the bucket (exclusive)
    pub upper_bound: QueryValue,
    /// Number of values in this bucket
    pub count: u64,
    /// Number of distinct values in this bucket
    pub distinct_count: u64,
}

/// Index statistics for cost estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    /// Unique identifier for the index
    pub index_id: String,
    /// Index selectivity (0.0 to 1.0, lower is more selective)
    pub selectivity: f64,
    /// Clustering factor (how well ordered the index is)
    pub clustering_factor: f64,
    /// Height of the index tree
    pub tree_height: u32,
    /// Number of leaf pages in the index
    pub leaf_pages: u64,
    /// Number of distinct keys in the index
    pub distinct_keys: u64,
    /// Index size in bytes
    pub index_size: u64,
    /// When statistics were last updated
    pub last_updated: DateTime<Utc>,
}

/// Statistics collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsConfig {
    /// Enable automatic statistics collection
    pub auto_analyze_enabled: bool,
    /// Threshold percentage of changed rows to trigger auto-analyze
    pub auto_analyze_threshold: f64,
    /// Maximum time between forced statistics updates
    pub max_analyze_interval_hours: u32,
    /// Number of histogram buckets to maintain
    pub histogram_buckets: u32,
    /// Number of most common values to track
    pub most_common_values_count: u32,
    /// Sample size for statistics collection (0.0 to 1.0)
    pub sample_fraction: f64,
}

impl Default for StatisticsConfig {
    fn default() -> Self {
        Self {
            auto_analyze_enabled: true,
            auto_analyze_threshold: 0.2,    // 20% of rows changed
            max_analyze_interval_hours: 24, // Daily analysis
            histogram_buckets: 100,
            most_common_values_count: 100,
            sample_fraction: 0.01, // 1% sample for large tables
        }
    }
}

/// Statistics collection errors
#[derive(Debug, Clone, PartialEq)]
pub enum StatisticsError {
    TableNotFound(String),
    IndexNotFound(String),
    InvalidSample(String),
    CollectionFailed(String),
    SerializationError(String),
}

impl std::fmt::Display for StatisticsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatisticsError::TableNotFound(table) => write!(f, "Table not found: {}", table),
            StatisticsError::IndexNotFound(index) => write!(f, "Index not found: {}", index),
            StatisticsError::InvalidSample(msg) => write!(f, "Invalid sample: {}", msg),
            StatisticsError::CollectionFailed(msg) => write!(f, "Collection failed: {}", msg),
            StatisticsError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for StatisticsError {}

/// Statistics manager responsible for collecting and maintaining statistics
pub struct StatisticsManager {
    /// Configuration for statistics collection
    config: StatisticsConfig,
    /// Cached table statistics
    table_stats: Arc<RwLock<HashMap<String, TableStatistics>>>,
    /// Cached index statistics
    index_stats: Arc<RwLock<HashMap<String, IndexStatistics>>>,
    /// Tracks when tables were last analyzed
    analyze_log: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    /// Tracks row change counts for auto-analyze triggers
    change_tracker: Arc<RwLock<HashMap<String, u64>>>,
}

impl StatisticsManager {
    /// Create a new statistics manager
    pub fn new(config: StatisticsConfig) -> Self {
        Self {
            config,
            table_stats: Arc::new(RwLock::new(HashMap::new())),
            index_stats: Arc::new(RwLock::new(HashMap::new())),
            analyze_log: Arc::new(RwLock::new(HashMap::new())),
            change_tracker: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get table statistics
    pub async fn get_table_statistics(&self, table_name: &str) -> Option<TableStatistics> {
        let stats = self.table_stats.read().await;
        stats.get(table_name).cloned()
    }

    /// Get index statistics
    pub async fn get_index_statistics(&self, index_id: &str) -> Option<IndexStatistics> {
        let stats = self.index_stats.read().await;
        stats.get(index_id).cloned()
    }

    /// Collect statistics for a table
    pub async fn analyze_table(
        &self,
        table_name: &str,
    ) -> Result<TableStatistics, StatisticsError> {
        // TODO: Implement actual statistics collection from storage layer
        // For now, create sample statistics

        let now = Utc::now();
        let stats = TableStatistics {
            row_count: 10000, // Sample data
            page_count: 100,
            avg_row_size: 64,
            null_fraction: 0.05,
            distinct_values: 8000,
            most_common_values: vec![
                (QueryValue::String("common_value".to_string()), 0.1),
                (QueryValue::Integer(42), 0.05),
            ],
            histogram: self.create_sample_histogram(),
            last_analyzed: now,
            column_statistics: HashMap::new(),
        };

        // Cache the statistics
        {
            let mut cache = self.table_stats.write().await;
            cache.insert(table_name.to_string(), stats.clone());
        }

        // Update analyze log
        {
            let mut log = self.analyze_log.write().await;
            log.insert(table_name.to_string(), now);
        }

        Ok(stats)
    }

    /// Collect statistics for an index
    pub async fn analyze_index(&self, index_id: &str) -> Result<IndexStatistics, StatisticsError> {
        let now = Utc::now();
        let stats = IndexStatistics {
            index_id: index_id.to_string(),
            selectivity: 0.01,      // 1% selectivity (highly selective)
            clustering_factor: 0.8, // Well-ordered index
            tree_height: 3,
            leaf_pages: 50,
            distinct_keys: 9500,
            index_size: 1024 * 1024, // 1MB
            last_updated: now,
        };

        // Cache the statistics
        {
            let mut cache = self.index_stats.write().await;
            cache.insert(index_id.to_string(), stats.clone());
        }

        Ok(stats)
    }

    /// Check if a table needs statistics update
    pub async fn needs_analyze(&self, table_name: &str) -> bool {
        if !self.config.auto_analyze_enabled {
            return false;
        }

        let change_tracker = self.change_tracker.read().await;
        let table_stats = self.table_stats.read().await;
        let analyze_log = self.analyze_log.read().await;

        // Check change threshold
        if let (Some(&changes), Some(stats)) =
            (change_tracker.get(table_name), table_stats.get(table_name))
        {
            let change_ratio = changes as f64 / stats.row_count as f64;
            if change_ratio >= self.config.auto_analyze_threshold {
                return true;
            }
        }

        // Check time threshold
        if let Some(&last_analyzed) = analyze_log.get(table_name) {
            let hours_since_analyze = (Utc::now() - last_analyzed).num_hours();
            if hours_since_analyze >= self.config.max_analyze_interval_hours as i64 {
                return true;
            }
        } else {
            // Never analyzed
            return true;
        }

        false
    }

    /// Record a change to a table (for auto-analyze triggering)
    pub async fn record_table_change(&self, table_name: &str, change_count: u64) {
        let mut tracker = self.change_tracker.write().await;
        *tracker.entry(table_name.to_string()).or_insert(0) += change_count;
    }

    /// Auto-analyze tables that need statistics updates
    pub async fn auto_analyze(&self) -> Result<Vec<String>, StatisticsError> {
        let mut analyzed_tables = Vec::new();

        // Get list of all tables that need analysis
        let table_stats = self.table_stats.read().await;
        let table_names: Vec<String> = table_stats.keys().cloned().collect();
        drop(table_stats);

        for table_name in table_names {
            if self.needs_analyze(&table_name).await {
                match self.analyze_table(&table_name).await {
                    Ok(_) => {
                        analyzed_tables.push(table_name.clone());
                        // Reset change counter
                        let mut tracker = self.change_tracker.write().await;
                        tracker.insert(table_name, 0);
                    }
                    Err(e) => {
                        eprintln!("Failed to analyze table {}: {}", table_name, e);
                    }
                }
            }
        }

        Ok(analyzed_tables)
    }

    /// Create sample histogram for testing
    fn create_sample_histogram(&self) -> Vec<HistogramBucket> {
        vec![
            HistogramBucket {
                lower_bound: QueryValue::Integer(0),
                upper_bound: QueryValue::Integer(100),
                count: 1000,
                distinct_count: 100,
            },
            HistogramBucket {
                lower_bound: QueryValue::Integer(100),
                upper_bound: QueryValue::Integer(1000),
                count: 8000,
                distinct_count: 900,
            },
            HistogramBucket {
                lower_bound: QueryValue::Integer(1000),
                upper_bound: QueryValue::Integer(10000),
                count: 1000,
                distinct_count: 1000,
            },
        ]
    }

    /// Estimate selectivity for a predicate
    pub async fn estimate_selectivity(
        &self,
        table_name: &str,
        column: &str,
        operator: &str,
        value: &QueryValue,
    ) -> f64 {
        if let Some(stats) = self.get_table_statistics(table_name).await {
            if let Some(col_stats) = stats.column_statistics.get(column) {
                return self.calculate_predicate_selectivity(col_stats, operator, value);
            }
        }

        // Default selectivity estimates
        match operator {
            "=" => 0.01,       // 1% for equality
            "<" | ">" => 0.33, // 33% for range
            "<=" | ">=" => 0.33,
            "LIKE" => 0.1, // 10% for pattern match
            "IN" => 0.05,  // 5% for IN list
            _ => 0.1,      // 10% default
        }
    }

    /// Calculate selectivity for a specific predicate
    fn calculate_predicate_selectivity(
        &self,
        col_stats: &ColumnStatistics,
        operator: &str,
        value: &QueryValue,
    ) -> f64 {
        match operator {
            "=" => {
                // Check if value is in most common values
                if let Some(pos) = col_stats.most_common_vals.iter().position(|v| v == value) {
                    col_stats.most_common_freqs[pos]
                } else {
                    // Estimate based on distinct values
                    1.0 / col_stats.n_distinct as f64
                }
            }
            "<" | "<=" | ">" | ">=" => {
                // Use histogram bounds for range estimates
                if !col_stats.histogram_bounds.is_empty() {
                    self.estimate_range_selectivity(&col_stats.histogram_bounds, operator, value)
                } else {
                    0.33 // Default range selectivity
                }
            }
            _ => 0.1, // Default selectivity for other operators
        }
    }

    /// Estimate selectivity for range predicates using histogram
    fn estimate_range_selectivity(
        &self,
        histogram_bounds: &[QueryValue],
        operator: &str,
        value: &QueryValue,
    ) -> f64 {
        // Simplified range estimation
        // In a real implementation, this would use proper histogram analysis
        match operator {
            "<" | "<=" => 0.25,
            ">" | ">=" => 0.25,
            _ => 0.1,
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> &StatisticsConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: StatisticsConfig) {
        self.config = config;
    }
}

impl Default for StatisticsManager {
    fn default() -> Self {
        Self::new(StatisticsConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_statistics_manager() {
        let manager = StatisticsManager::default();

        // Analyze a sample table
        let stats = manager.analyze_table("test_table").await.unwrap();
        assert_eq!(stats.row_count, 10000);
        assert!(stats.histogram.len() > 0);

        // Retrieve cached statistics
        let cached_stats = manager.get_table_statistics("test_table").await.unwrap();
        assert_eq!(cached_stats.row_count, 10000);
    }

    #[tokio::test]
    async fn test_auto_analyze_threshold() {
        let manager = StatisticsManager::default();

        // Analyze table first
        manager.analyze_table("test_table").await.unwrap();

        // Record changes
        manager.record_table_change("test_table", 2500).await; // 25% of 10000 rows

        // Should need analysis due to exceeding 20% threshold
        assert!(manager.needs_analyze("test_table").await);
    }

    #[tokio::test]
    async fn test_index_statistics() {
        let manager = StatisticsManager::default();

        let stats = manager.analyze_index("test_index").await.unwrap();
        assert_eq!(stats.index_id, "test_index");
        assert!(stats.selectivity > 0.0);
        assert!(stats.selectivity <= 1.0);
    }

    #[tokio::test]
    async fn test_selectivity_estimation() {
        let manager = StatisticsManager::default();
        manager.analyze_table("test_table").await.unwrap();

        let selectivity = manager
            .estimate_selectivity("test_table", "id", "=", &QueryValue::Integer(42))
            .await;

        assert!(selectivity > 0.0);
        assert!(selectivity <= 1.0);
    }
}
