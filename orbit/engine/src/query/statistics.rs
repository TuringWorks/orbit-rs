//! Statistics collection for query optimization
//!
//! Collects and maintains statistics about tables, columns, and indexes
//! to enable cost-based query optimization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Table-level statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStatistics {
    /// Table name
    pub table_name: String,
    /// Total number of rows
    pub row_count: u64,
    /// Average row size in bytes
    pub avg_row_size: u64,
    /// Total table size in bytes
    pub table_size: u64,
    /// Column statistics
    pub column_stats: HashMap<String, ColumnStatistics>,
    /// Last update timestamp
    pub last_updated: i64,
}

/// Column-level statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    /// Column name
    pub column_name: String,
    /// Number of distinct values (cardinality)
    pub distinct_count: u64,
    /// Number of null values
    pub null_count: u64,
    /// Minimum value (if applicable)
    pub min_value: Option<String>,
    /// Maximum value (if applicable)
    pub max_value: Option<String>,
    /// Average value length for string columns
    pub avg_length: Option<u64>,
    /// Histogram for value distribution
    pub histogram: Option<Histogram>,
}

/// Histogram for value distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    /// Bucket boundaries
    pub buckets: Vec<HistogramBucket>,
    /// Total number of buckets
    pub bucket_count: usize,
}

/// Individual histogram bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    /// Lower bound (inclusive)
    pub lower_bound: String,
    /// Upper bound (exclusive)
    pub upper_bound: String,
    /// Number of values in this bucket
    pub count: u64,
    /// Number of distinct values in this bucket
    pub distinct_count: u64,
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    /// Index name
    pub index_name: String,
    /// Table name
    pub table_name: String,
    /// Indexed columns
    pub columns: Vec<String>,
    /// Index size in bytes
    pub index_size: u64,
    /// Number of index entries
    pub entry_count: u64,
    /// Index selectivity (0.0 to 1.0)
    pub selectivity: f64,
    /// Average key length
    pub avg_key_length: u64,
}

/// Statistics collector
pub struct StatisticsCollector {
    /// Table statistics cache
    table_stats: HashMap<String, TableStatistics>,
    /// Index statistics cache
    index_stats: HashMap<String, IndexStatistics>,
}

impl StatisticsCollector {
    /// Create a new statistics collector
    pub fn new() -> Self {
        Self {
            table_stats: HashMap::new(),
            index_stats: HashMap::new(),
        }
    }

    /// Get table statistics
    pub fn get_table_stats(&self, table_name: &str) -> Option<&TableStatistics> {
        self.table_stats.get(table_name)
    }

    /// Get index statistics
    pub fn get_index_stats(&self, index_name: &str) -> Option<&IndexStatistics> {
        self.index_stats.get(index_name)
    }

    /// Update table statistics
    pub fn update_table_stats(&mut self, stats: TableStatistics) {
        self.table_stats.insert(stats.table_name.clone(), stats);
    }

    /// Update index statistics
    pub fn update_index_stats(&mut self, stats: IndexStatistics) {
        self.index_stats.insert(stats.index_name.clone(), stats);
    }

    /// Estimate selectivity for a predicate
    pub fn estimate_selectivity(&self, table_name: &str, column_name: &str) -> f64 {
        if let Some(table_stats) = self.table_stats.get(table_name) {
            if let Some(col_stats) = table_stats.column_stats.get(column_name) {
                if table_stats.row_count > 0 {
                    // Selectivity = distinct_count / row_count
                    return col_stats.distinct_count as f64 / table_stats.row_count as f64;
                }
            }
        }
        // Default selectivity if no stats available
        0.1
    }

    /// Estimate row count for a table scan
    pub fn estimate_row_count(&self, table_name: &str) -> u64 {
        self.table_stats
            .get(table_name)
            .map(|stats| stats.row_count)
            .unwrap_or(1000) // Default estimate
    }

    /// Clear all statistics
    pub fn clear(&mut self) {
        self.table_stats.clear();
        self.index_stats.clear();
    }
}

impl Default for StatisticsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_collector() {
        let mut collector = StatisticsCollector::new();

        let table_stats = TableStatistics {
            table_name: "users".to_string(),
            row_count: 1000,
            avg_row_size: 256,
            table_size: 256000,
            column_stats: HashMap::new(),
            last_updated: 0,
        };

        collector.update_table_stats(table_stats);
        assert_eq!(collector.estimate_row_count("users"), 1000);
    }

    #[test]
    fn test_selectivity_estimation() {
        let mut collector = StatisticsCollector::new();

        let mut column_stats = HashMap::new();
        column_stats.insert(
            "id".to_string(),
            ColumnStatistics {
                column_name: "id".to_string(),
                distinct_count: 1000,
                null_count: 0,
                min_value: Some("1".to_string()),
                max_value: Some("1000".to_string()),
                avg_length: None,
                histogram: None,
            },
        );

        let table_stats = TableStatistics {
            table_name: "users".to_string(),
            row_count: 1000,
            avg_row_size: 256,
            table_size: 256000,
            column_stats,
            last_updated: 0,
        };

        collector.update_table_stats(table_stats);

        // Selectivity should be 1.0 for unique column
        let selectivity = collector.estimate_selectivity("users", "id");
        assert!((selectivity - 1.0).abs() < 0.001);
    }
}
