//! Statistics Collection for Cost-Based Optimization
//!
//! This module maintains statistics about tables, columns, and indexes
//! that are used by the cost-based optimizer to make informed decisions.

use std::collections::HashMap;

/// Table-level statistics
#[derive(Debug, Clone)]
pub struct TableStatistics {
    /// Total number of rows in the table
    pub row_count: usize,
    /// Average number of rows per page
    pub rows_per_page: usize,
    /// Average row size in bytes
    pub average_row_size: usize,
    /// Fraction of null values across all columns
    pub null_frac: f64,
    /// Total number of distinct values across key columns
    pub distinct_values: usize,
}

impl Default for TableStatistics {
    fn default() -> Self {
        Self {
            row_count: 1000,
            rows_per_page: 100,
            average_row_size: 100,
            null_frac: 0.1,
            distinct_values: 100,
        }
    }
}

/// Column-level statistics
#[derive(Debug, Clone)]
pub struct ColumnStatistics {
    /// Number of distinct values in this column
    pub distinct_count: usize,
    /// Fraction of null values
    pub null_fraction: f64,
    /// Most common values and their frequencies
    pub most_common_values: Vec<(String, f64)>,
    /// Histogram of value distribution
    pub histogram: Vec<HistogramBucket>,
    /// Average width of column values in bytes
    pub avg_width: usize,
    /// Correlation with physical row order (-1 to 1)
    pub correlation: f64,
}

impl Default for ColumnStatistics {
    fn default() -> Self {
        Self {
            distinct_count: 100,
            null_fraction: 0.1,
            most_common_values: Vec::new(),
            histogram: Vec::new(),
            avg_width: 50,
            correlation: 0.0,
        }
    }
}

/// Histogram bucket for value distribution
#[derive(Debug, Clone)]
pub struct HistogramBucket {
    /// Lower bound of the bucket
    pub lower_bound: String,
    /// Upper bound of the bucket
    pub upper_bound: String,
    /// Frequency of values in this bucket
    pub frequency: f64,
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStatistics {
    /// Number of pages in the index
    pub page_count: usize,
    /// Number of tuples in the index
    pub tuple_count: usize,
    /// Index selectivity (0.0 to 1.0)
    pub selectivity: f64,
    /// Average number of tuples per page
    pub tuples_per_page: usize,
    /// Index correlation with table order
    pub correlation: f64,
}

impl Default for IndexStatistics {
    fn default() -> Self {
        Self {
            page_count: 10,
            tuple_count: 1000,
            selectivity: 0.1,
            tuples_per_page: 100,
            correlation: 1.0,
        }
    }
}

/// Main statistics collector
pub struct StatisticsCollector {
    /// Table statistics by table name
    table_stats: HashMap<String, TableStatistics>,
    /// Column statistics by table.column
    column_stats: HashMap<String, ColumnStatistics>,
    /// Index statistics by index name
    index_stats: HashMap<String, IndexStatistics>,
}

impl StatisticsCollector {
    /// Create a new statistics collector
    pub fn new() -> Self {
        Self {
            table_stats: HashMap::new(),
            column_stats: HashMap::new(),
            index_stats: HashMap::new(),
        }
    }

    /// Update statistics for a table
    pub fn update_table_stats(&mut self, table_name: &str, stats: TableStatistics) {
        self.table_stats.insert(table_name.to_string(), stats);
    }

    /// Get statistics for a table
    pub fn get_table_stats(&self, table_name: &str) -> Option<&TableStatistics> {
        self.table_stats.get(table_name)
    }

    /// Update statistics for a column
    pub fn update_column_stats(
        &mut self,
        table_name: &str,
        column_name: &str,
        stats: ColumnStatistics,
    ) {
        let key = format!("{}.{}", table_name, column_name);
        self.column_stats.insert(key, stats);
    }

    /// Get statistics for a column
    pub fn get_column_stats(
        &self,
        table_name: &str,
        column_name: &str,
    ) -> Option<&ColumnStatistics> {
        let key = format!("{}.{}", table_name, column_name);
        self.column_stats.get(&key)
    }

    /// Update statistics for an index
    pub fn update_index_stats(&mut self, index_name: &str, stats: IndexStatistics) {
        self.index_stats.insert(index_name.to_string(), stats);
    }

    /// Get statistics for an index
    pub fn get_index_stats(&self, index_name: &str) -> Option<&IndexStatistics> {
        self.index_stats.get(index_name)
    }

    /// Estimate the selectivity of a column equality predicate
    pub fn estimate_equality_selectivity(&self, table_name: &str, column_name: &str) -> f64 {
        if let Some(col_stats) = self.get_column_stats(table_name, column_name) {
            if col_stats.distinct_count > 0 {
                1.0 / col_stats.distinct_count as f64
            } else {
                0.1 // Default selectivity
            }
        } else {
            0.1 // Default selectivity when no stats available
        }
    }

    /// Estimate the selectivity of a range predicate (column > value, column < value, etc.)
    pub fn estimate_range_selectivity(
        &self,
        table_name: &str,
        column_name: &str,
        _operator: &str,
        _value: &str,
    ) -> f64 {
        if let Some(_col_stats) = self.get_column_stats(table_name, column_name) {
            // In a full implementation, this would use histogram data
            // to estimate how many rows fall within the range
            0.33 // Default estimate for range predicates
        } else {
            0.33
        }
    }

    /// Estimate join selectivity between two columns
    pub fn estimate_join_selectivity(
        &self,
        left_table: &str,
        left_column: &str,
        right_table: &str,
        right_column: &str,
    ) -> f64 {
        let left_stats = self.get_column_stats(left_table, left_column);
        let right_stats = self.get_column_stats(right_table, right_column);

        match (left_stats, right_stats) {
            (Some(left), Some(right)) => {
                // Estimate based on the maximum distinct values
                let max_distinct = left.distinct_count.max(right.distinct_count);
                if max_distinct > 0 {
                    1.0 / max_distinct as f64
                } else {
                    0.1
                }
            }
            _ => 0.1, // Default join selectivity
        }
    }

    /// Collect statistics from actual data (placeholder implementation)
    pub fn collect_table_statistics(&mut self, table_name: &str) -> Result<(), String> {
        // In a real implementation, this would:
        // 1. Scan the table to count rows
        // 2. Sample data to estimate column distributions
        // 3. Update the statistics structures

        // For now, just insert default statistics
        self.update_table_stats(table_name, TableStatistics::default());

        // Add some sample column statistics
        self.update_column_stats(
            table_name,
            "id",
            ColumnStatistics {
                distinct_count: 1000,
                null_fraction: 0.0,
                correlation: 1.0,
                ..Default::default()
            },
        );

        Ok(())
    }

    /// Auto-update statistics based on data changes
    pub fn auto_update_stats(
        &mut self,
        table_name: &str,
        rows_inserted: usize,
        _rows_updated: usize,
        rows_deleted: usize,
    ) {
        if let Some(stats) = self.table_stats.get_mut(table_name) {
            // Update row count
            stats.row_count = stats.row_count + rows_inserted - rows_deleted;

            // In a real implementation, would also update column statistics
            // based on the nature of the changes
        }
    }

    /// Get all table names with statistics
    pub fn get_tables_with_stats(&self) -> Vec<&String> {
        self.table_stats.keys().collect()
    }

    /// Get memory usage estimate for statistics
    pub fn memory_usage_bytes(&self) -> usize {
        // Rough estimate
        self.table_stats.len() * 200 +  // Table stats
        self.column_stats.len() * 500 + // Column stats (larger due to histograms)
        self.index_stats.len() * 100 // Index stats
    }

    /// Clear all statistics
    pub fn clear_all_stats(&mut self) {
        self.table_stats.clear();
        self.column_stats.clear();
        self.index_stats.clear();
    }

    /// Export statistics to a format suitable for persistence
    pub fn export_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut export = HashMap::new();

        // Export table stats
        for (table_name, stats) in &self.table_stats {
            export.insert(
                format!("table:{}", table_name),
                serde_json::json!({
                    "row_count": stats.row_count,
                    "rows_per_page": stats.rows_per_page,
                    "average_row_size": stats.average_row_size,
                    "null_frac": stats.null_frac,
                    "distinct_values": stats.distinct_values,
                }),
            );
        }

        export
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
    fn test_statistics_collector_creation() {
        let collector = StatisticsCollector::new();
        assert_eq!(collector.table_stats.len(), 0);
        assert_eq!(collector.column_stats.len(), 0);
        assert_eq!(collector.index_stats.len(), 0);
    }

    #[test]
    fn test_table_statistics_update() {
        let mut collector = StatisticsCollector::new();

        let stats = TableStatistics {
            row_count: 10000,
            rows_per_page: 100,
            average_row_size: 200,
            null_frac: 0.05,
            distinct_values: 5000,
        };

        collector.update_table_stats("test_table", stats.clone());

        let retrieved = collector.get_table_stats("test_table").unwrap();
        assert_eq!(retrieved.row_count, 10000);
        assert_eq!(retrieved.average_row_size, 200);
    }

    #[test]
    fn test_column_statistics_update() {
        let mut collector = StatisticsCollector::new();

        let stats = ColumnStatistics {
            distinct_count: 500,
            null_fraction: 0.02,
            avg_width: 50,
            correlation: 0.8,
            ..Default::default()
        };

        collector.update_column_stats("test_table", "test_column", stats);

        let retrieved = collector
            .get_column_stats("test_table", "test_column")
            .unwrap();
        assert_eq!(retrieved.distinct_count, 500);
        assert_eq!(retrieved.correlation, 0.8);
    }

    #[test]
    fn test_selectivity_estimation() {
        let mut collector = StatisticsCollector::new();

        // Add column with known distinct count
        collector.update_column_stats(
            "users",
            "id",
            ColumnStatistics {
                distinct_count: 1000,
                ..Default::default()
            },
        );

        let selectivity = collector.estimate_equality_selectivity("users", "id");
        assert_eq!(selectivity, 0.001); // 1/1000

        // Test with unknown column
        let selectivity = collector.estimate_equality_selectivity("unknown", "column");
        assert_eq!(selectivity, 0.1); // Default
    }

    #[test]
    fn test_join_selectivity_estimation() {
        let mut collector = StatisticsCollector::new();

        collector.update_column_stats(
            "orders",
            "user_id",
            ColumnStatistics {
                distinct_count: 500,
                ..Default::default()
            },
        );

        collector.update_column_stats(
            "users",
            "id",
            ColumnStatistics {
                distinct_count: 1000,
                ..Default::default()
            },
        );

        let selectivity = collector.estimate_join_selectivity("orders", "user_id", "users", "id");
        assert_eq!(selectivity, 0.001); // 1/max(500, 1000) = 1/1000
    }

    #[test]
    fn test_auto_update_stats() {
        let mut collector = StatisticsCollector::new();

        collector.update_table_stats(
            "test_table",
            TableStatistics {
                row_count: 1000,
                ..Default::default()
            },
        );

        // Simulate some data changes
        collector.auto_update_stats("test_table", 100, 50, 25);

        let stats = collector.get_table_stats("test_table").unwrap();
        assert_eq!(stats.row_count, 1075); // 1000 + 100 - 25
    }

    #[test]
    fn test_memory_usage_estimation() {
        let mut collector = StatisticsCollector::new();

        collector.update_table_stats("table1", TableStatistics::default());
        collector.update_column_stats("table1", "col1", ColumnStatistics::default());
        collector.update_index_stats("idx1", IndexStatistics::default());

        let usage = collector.memory_usage_bytes();
        assert!(usage > 0);
        assert!(usage <= 1000); // Should be reasonable for small dataset
    }

    #[test]
    fn test_stats_export() {
        let mut collector = StatisticsCollector::new();

        collector.update_table_stats(
            "test_table",
            TableStatistics {
                row_count: 5000,
                average_row_size: 150,
                ..Default::default()
            },
        );

        let export = collector.export_stats();
        assert!(export.contains_key("table:test_table"));

        if let Some(table_data) = export.get("table:test_table") {
            assert_eq!(table_data["row_count"], 5000);
            assert_eq!(table_data["average_row_size"], 150);
        }
    }
}
