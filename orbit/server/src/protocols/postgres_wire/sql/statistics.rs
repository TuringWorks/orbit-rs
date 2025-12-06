//! Table and Column Statistics for Query Optimization
//!
//! This module provides statistics collection and storage for use in:
//! - Cardinality estimation
//! - Cost-based query optimization
//! - Deciding between CPU SIMD vs GPU execution
//! - Index selection
//!
//! ## Statistics Collected
//! - Row counts
//! - Distinct value counts (NDV)
//! - Min/max values for numeric columns
//! - Null counts
//! - Histograms for value distribution

use crate::protocols::postgres_wire::sql::types::SqlValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Configuration for statistics collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsConfig {
    /// Enable automatic statistics collection
    pub auto_collect: bool,
    /// Collect statistics after this many rows are modified
    pub collect_threshold: usize,
    /// Maximum number of histogram buckets
    pub max_histogram_buckets: usize,
    /// Sample ratio for large tables (0.0-1.0)
    pub sample_ratio: f64,
    /// Statistics cache TTL
    pub cache_ttl_seconds: u64,
}

impl Default for StatisticsConfig {
    fn default() -> Self {
        Self {
            auto_collect: true,
            collect_threshold: 1000,
            max_histogram_buckets: 100,
            sample_ratio: 0.1,
            cache_ttl_seconds: 300, // 5 minutes
        }
    }
}

/// Statistics for a single column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    /// Column name
    pub column_name: String,
    /// Number of distinct values (estimated)
    pub distinct_count: usize,
    /// Number of null values
    pub null_count: usize,
    /// Minimum value (for numeric columns)
    pub min_value: Option<SqlValue>,
    /// Maximum value (for numeric columns)
    pub max_value: Option<SqlValue>,
    /// Average value (for numeric columns)
    pub avg_value: Option<f64>,
    /// Value histogram (bucket boundaries and counts)
    pub histogram: Option<Histogram>,
    /// Most common values and their frequencies
    pub most_common_values: Vec<(SqlValue, usize)>,
    /// Column data type
    pub data_type: String,
    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl ColumnStatistics {
    /// Create empty statistics for a column
    pub fn new(column_name: &str, data_type: &str) -> Self {
        Self {
            column_name: column_name.to_string(),
            distinct_count: 0,
            null_count: 0,
            min_value: None,
            max_value: None,
            avg_value: None,
            histogram: None,
            most_common_values: Vec::new(),
            data_type: data_type.to_string(),
            last_updated: chrono::Utc::now(),
        }
    }

    /// Estimate selectivity for equality predicate
    pub fn selectivity_eq(&self, total_rows: usize) -> f64 {
        if total_rows == 0 || self.distinct_count == 0 {
            return 1.0;
        }
        1.0 / self.distinct_count as f64
    }

    /// Estimate selectivity for range predicate (assuming uniform distribution)
    pub fn selectivity_range(&self, min: Option<&SqlValue>, max: Option<&SqlValue>) -> f64 {
        // If we have histogram, use it; otherwise assume uniform distribution
        if let Some(histogram) = &self.histogram {
            histogram.selectivity_range(min, max)
        } else {
            // Without histogram, assume 1/3 selectivity for range queries
            0.33
        }
    }

    /// Estimate selectivity for IS NULL predicate
    pub fn selectivity_null(&self, total_rows: usize) -> f64 {
        if total_rows == 0 {
            return 0.0;
        }
        self.null_count as f64 / total_rows as f64
    }
}

/// Statistics for a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStatistics {
    /// Table name
    pub table_name: String,
    /// Total number of rows
    pub row_count: usize,
    /// Total size in bytes (estimated)
    pub size_bytes: usize,
    /// Statistics per column
    pub columns: HashMap<String, ColumnStatistics>,
    /// Number of modifications since last statistics collection
    pub modifications_since_analyze: usize,
    /// Last full analyze timestamp
    pub last_analyzed: chrono::DateTime<chrono::Utc>,
    /// Whether statistics are considered stale
    pub is_stale: bool,
}

impl TableStatistics {
    /// Create empty statistics for a table
    pub fn new(table_name: &str) -> Self {
        Self {
            table_name: table_name.to_string(),
            row_count: 0,
            size_bytes: 0,
            columns: HashMap::new(),
            modifications_since_analyze: 0,
            last_analyzed: chrono::Utc::now(),
            is_stale: true,
        }
    }

    /// Update row count
    pub fn set_row_count(&mut self, count: usize) {
        self.row_count = count;
        self.is_stale = false;
        self.last_analyzed = chrono::Utc::now();
    }

    /// Add column statistics
    pub fn add_column_stats(&mut self, stats: ColumnStatistics) {
        self.columns.insert(stats.column_name.clone(), stats);
    }

    /// Get column statistics
    pub fn get_column_stats(&self, column_name: &str) -> Option<&ColumnStatistics> {
        self.columns.get(column_name)
    }

    /// Estimate cardinality for a simple filter
    pub fn estimate_cardinality(&self, _column: &str, selectivity: f64) -> usize {
        ((self.row_count as f64) * selectivity).max(1.0) as usize
    }

    /// Record a modification (insert/update/delete)
    pub fn record_modification(&mut self, count: usize) {
        self.modifications_since_analyze += count;
    }

    /// Check if statistics should be refreshed
    pub fn needs_refresh(&self, threshold: usize) -> bool {
        self.is_stale || self.modifications_since_analyze >= threshold
    }
}

/// Histogram for value distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    /// Bucket boundaries (n+1 values for n buckets)
    pub boundaries: Vec<f64>,
    /// Count of values in each bucket
    pub counts: Vec<usize>,
    /// Total values in histogram
    pub total_count: usize,
}

impl Histogram {
    /// Create a new histogram from values
    pub fn from_values(values: &[f64], num_buckets: usize) -> Self {
        if values.is_empty() {
            return Self {
                boundaries: vec![],
                counts: vec![],
                total_count: 0,
            };
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let range = max - min;

        if range == 0.0 {
            return Self {
                boundaries: vec![min, max],
                counts: vec![sorted.len()],
                total_count: sorted.len(),
            };
        }

        let bucket_width = range / num_buckets as f64;
        let mut boundaries = Vec::with_capacity(num_buckets + 1);
        let mut counts = vec![0; num_buckets];

        for i in 0..=num_buckets {
            boundaries.push(min + bucket_width * i as f64);
        }

        for &value in &sorted {
            let bucket_idx = ((value - min) / bucket_width).floor() as usize;
            let bucket_idx = bucket_idx.min(num_buckets - 1);
            counts[bucket_idx] += 1;
        }

        Self {
            boundaries,
            counts,
            total_count: sorted.len(),
        }
    }

    /// Estimate selectivity for range query
    pub fn selectivity_range(&self, min: Option<&SqlValue>, max: Option<&SqlValue>) -> f64 {
        if self.total_count == 0 || self.boundaries.is_empty() {
            return 0.33; // Default estimate
        }

        let range_min = min.and_then(|v| v.to_f64()).unwrap_or(self.boundaries[0]);
        let range_max = max
            .and_then(|v| v.to_f64())
            .unwrap_or(*self.boundaries.last().unwrap());

        let mut matching_count = 0usize;

        for i in 0..self.counts.len() {
            let bucket_min = self.boundaries[i];
            let bucket_max = self.boundaries[i + 1];

            // Check if bucket overlaps with range
            if bucket_max >= range_min && bucket_min <= range_max {
                // Partial overlap estimation
                let overlap_start = bucket_min.max(range_min);
                let overlap_end = bucket_max.min(range_max);
                let bucket_width = bucket_max - bucket_min;

                if bucket_width > 0.0 {
                    let overlap_ratio = (overlap_end - overlap_start) / bucket_width;
                    matching_count += (self.counts[i] as f64 * overlap_ratio) as usize;
                } else {
                    matching_count += self.counts[i];
                }
            }
        }

        matching_count as f64 / self.total_count as f64
    }
}

/// Statistics manager for caching and managing table statistics
pub struct StatisticsManager {
    /// Cached statistics per table
    cache: Arc<RwLock<HashMap<String, (TableStatistics, Instant)>>>,
    /// Configuration
    config: StatisticsConfig,
}

impl StatisticsManager {
    /// Create a new statistics manager
    pub fn new(config: StatisticsConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create with default config
    pub fn new_default() -> Self {
        Self::new(StatisticsConfig::default())
    }

    /// Get statistics for a table
    pub async fn get_table_stats(&self, table_name: &str) -> Option<TableStatistics> {
        let cache = self.cache.read().await;
        if let Some((stats, cached_at)) = cache.get(table_name) {
            let ttl = Duration::from_secs(self.config.cache_ttl_seconds);
            if cached_at.elapsed() < ttl {
                return Some(stats.clone());
            }
        }
        None
    }

    /// Store statistics for a table
    pub async fn store_table_stats(&self, stats: TableStatistics) {
        let mut cache = self.cache.write().await;
        cache.insert(stats.table_name.clone(), (stats, Instant::now()));
    }

    /// Invalidate statistics for a table
    pub async fn invalidate(&self, table_name: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(table_name);
    }

    /// Record modifications and potentially trigger re-analyze
    pub async fn record_modifications(&self, table_name: &str, count: usize) -> bool {
        let mut cache = self.cache.write().await;
        if let Some((stats, _)) = cache.get_mut(table_name) {
            stats.record_modification(count);
            return stats.needs_refresh(self.config.collect_threshold);
        }
        false
    }

    /// Analyze a table and collect statistics
    pub async fn analyze_table(
        &self,
        table_name: &str,
        rows: &[HashMap<String, SqlValue>],
        column_types: &HashMap<String, String>,
    ) -> TableStatistics {
        let mut stats = TableStatistics::new(table_name);
        stats.set_row_count(rows.len());

        // Calculate column statistics
        for (col_name, col_type) in column_types {
            let col_stats = self.analyze_column(col_name, col_type, rows);
            stats.add_column_stats(col_stats);
        }

        // Store in cache
        self.store_table_stats(stats.clone()).await;

        stats
    }

    /// Analyze a single column
    fn analyze_column(
        &self,
        column_name: &str,
        data_type: &str,
        rows: &[HashMap<String, SqlValue>],
    ) -> ColumnStatistics {
        let mut stats = ColumnStatistics::new(column_name, data_type);

        if rows.is_empty() {
            return stats;
        }

        // Collect values
        let mut distinct_values: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut null_count = 0usize;
        let mut numeric_values: Vec<f64> = Vec::new();
        let mut value_counts: HashMap<String, usize> = HashMap::new();

        for row in rows {
            let value = row.get(column_name);

            match value {
                None | Some(SqlValue::Null) => {
                    null_count += 1;
                }
                Some(v) => {
                    let str_repr = v.to_postgres_string();
                    distinct_values.insert(str_repr.clone());
                    *value_counts.entry(str_repr).or_insert(0) += 1;

                    // For numeric columns, track min/max/avg
                    if let Some(f) = v.to_f64() {
                        numeric_values.push(f);
                    }
                }
            }
        }

        stats.null_count = null_count;
        stats.distinct_count = distinct_values.len();

        // Numeric statistics
        if !numeric_values.is_empty() {
            let min = numeric_values.iter().cloned().reduce(f64::min).unwrap();
            let max = numeric_values.iter().cloned().reduce(f64::max).unwrap();
            let sum: f64 = numeric_values.iter().sum();
            let avg = sum / numeric_values.len() as f64;

            stats.min_value = Some(SqlValue::DoublePrecision(min));
            stats.max_value = Some(SqlValue::DoublePrecision(max));
            stats.avg_value = Some(avg);

            // Build histogram
            if numeric_values.len() >= 10 {
                stats.histogram = Some(Histogram::from_values(
                    &numeric_values,
                    self.config
                        .max_histogram_buckets
                        .min(numeric_values.len() / 2),
                ));
            }
        }

        // Most common values (top 10)
        let mut value_freq: Vec<_> = value_counts.into_iter().collect();
        value_freq.sort_by(|a, b| b.1.cmp(&a.1));
        stats.most_common_values = value_freq
            .into_iter()
            .take(10)
            .map(|(v, c)| (SqlValue::Text(v), c))
            .collect();

        stats.last_updated = chrono::Utc::now();
        stats
    }

    /// Get cardinality estimate for a query
    pub async fn estimate_cardinality(
        &self,
        table_name: &str,
        predicates: &[Predicate],
    ) -> Option<usize> {
        let stats = self.get_table_stats(table_name).await?;

        if predicates.is_empty() {
            return Some(stats.row_count);
        }

        // Combine selectivities (assuming independence)
        let mut combined_selectivity = 1.0f64;

        for predicate in predicates {
            let selectivity = match predicate {
                Predicate::Eq { column, .. } => stats
                    .get_column_stats(column)
                    .map(|cs| cs.selectivity_eq(stats.row_count))
                    .unwrap_or(0.1),
                Predicate::Range { column, min, max } => stats
                    .get_column_stats(column)
                    .map(|cs| cs.selectivity_range(min.as_ref(), max.as_ref()))
                    .unwrap_or(0.33),
                Predicate::IsNull { column } => stats
                    .get_column_stats(column)
                    .map(|cs| cs.selectivity_null(stats.row_count))
                    .unwrap_or(0.01),
                Predicate::IsNotNull { column } => stats
                    .get_column_stats(column)
                    .map(|cs| 1.0 - cs.selectivity_null(stats.row_count))
                    .unwrap_or(0.99),
            };
            combined_selectivity *= selectivity;
        }

        Some(stats.estimate_cardinality("", combined_selectivity))
    }
}

impl Default for StatisticsManager {
    fn default() -> Self {
        Self::new_default()
    }
}

/// Predicate types for cardinality estimation
#[derive(Debug, Clone)]
pub enum Predicate {
    Eq {
        column: String,
        value: SqlValue,
    },
    Range {
        column: String,
        min: Option<SqlValue>,
        max: Option<SqlValue>,
    },
    IsNull {
        column: String,
    },
    IsNotNull {
        column: String,
    },
}

/// Extension trait for SqlValue to get f64 representation
trait SqlValueExt {
    fn to_f64(&self) -> Option<f64>;
}

impl SqlValueExt for SqlValue {
    fn to_f64(&self) -> Option<f64> {
        match self {
            SqlValue::SmallInt(i) => Some(*i as f64),
            SqlValue::Integer(i) => Some(*i as f64),
            SqlValue::BigInt(i) => Some(*i as f64),
            SqlValue::Real(f) => Some(*f as f64),
            SqlValue::DoublePrecision(f) => Some(*f),
            SqlValue::Decimal(d) => d.to_string().parse().ok(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_creation() {
        let values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let histogram = Histogram::from_values(&values, 10);

        assert_eq!(histogram.boundaries.len(), 11); // n+1 boundaries for n buckets
        assert_eq!(histogram.counts.len(), 10);
        assert_eq!(histogram.total_count, 100);
    }

    #[test]
    fn test_histogram_selectivity() {
        let values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let histogram = Histogram::from_values(&values, 10);

        // Full range should be ~100%
        let full_sel = histogram.selectivity_range(None, None);
        assert!(full_sel > 0.9);

        // Half range should be ~50%
        let half_sel = histogram.selectivity_range(
            Some(&SqlValue::DoublePrecision(1.0)),
            Some(&SqlValue::DoublePrecision(50.0)),
        );
        assert!(half_sel > 0.4 && half_sel < 0.6);
    }

    #[test]
    fn test_column_selectivity() {
        let mut stats = ColumnStatistics::new("id", "integer");
        stats.distinct_count = 100;

        // Equality selectivity should be 1/NDV
        let sel = stats.selectivity_eq(1000);
        assert!((sel - 0.01).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_statistics_manager() {
        let manager = StatisticsManager::new_default();

        let mut rows = Vec::new();
        for i in 0..100 {
            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Integer(i));
            row.insert(
                "value".to_string(),
                SqlValue::DoublePrecision(i as f64 * 1.5),
            );
            rows.push(row);
        }

        let mut col_types = HashMap::new();
        col_types.insert("id".to_string(), "integer".to_string());
        col_types.insert("value".to_string(), "double precision".to_string());

        let stats = manager.analyze_table("test_table", &rows, &col_types).await;

        assert_eq!(stats.row_count, 100);
        assert!(stats.columns.contains_key("id"));
        assert!(stats.columns.contains_key("value"));

        let id_stats = stats.get_column_stats("id").unwrap();
        assert_eq!(id_stats.distinct_count, 100);
        assert_eq!(id_stats.null_count, 0);
    }

    #[tokio::test]
    async fn test_cardinality_estimation() {
        let manager = StatisticsManager::new_default();

        let mut rows = Vec::new();
        for i in 0..1000 {
            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Integer(i % 100)); // 100 distinct values
            row.insert(
                "category".to_string(),
                SqlValue::Text(format!("cat_{}", i % 10)),
            ); // 10 distinct
            rows.push(row);
        }

        let mut col_types = HashMap::new();
        col_types.insert("id".to_string(), "integer".to_string());
        col_types.insert("category".to_string(), "text".to_string());

        manager.analyze_table("test_table", &rows, &col_types).await;

        // Estimate for equality predicate
        let predicates = vec![Predicate::Eq {
            column: "id".to_string(),
            value: SqlValue::Integer(5),
        }];

        let estimate = manager
            .estimate_cardinality("test_table", &predicates)
            .await;
        assert!(estimate.is_some());
        // Should be around 10 (1000 rows / 100 distinct values)
        let est = estimate.unwrap();
        assert!(est >= 5 && est <= 20);
    }
}
