//! Column Statistics for Query Optimization
//!
//! This module provides column-level statistics that enable cost-based query optimization,
//! including min/max values, null counts, distinct counts, and histograms.

use crate::protocols::postgres_wire::sql::execution::Column;
use std::collections::HashSet;

/// Column statistics for query optimization
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnStatistics {
    /// Column name
    pub column_name: String,
    /// Minimum value (if applicable)
    pub min_value: Option<StatisticValue>,
    /// Maximum value (if applicable)
    pub max_value: Option<StatisticValue>,
    /// Number of null values
    pub null_count: usize,
    /// Total number of values
    pub total_count: usize,
    /// Number of distinct values (approximate for large datasets)
    pub distinct_count: Option<usize>,
    /// Average value (for numeric columns)
    pub average_value: Option<f64>,
    /// Histogram buckets for value distribution
    pub histogram: Option<Histogram>,
}

/// Value type for statistics
#[derive(Debug, Clone, PartialEq)]
pub enum StatisticValue {
    Integer(i64),
    Float(f64),
    Text(String),
    Boolean(bool),
}

// Custom Hash implementation for StatisticValue (handles f64 NaN)
impl std::hash::Hash for StatisticValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            StatisticValue::Integer(i) => i.hash(state),
            StatisticValue::Float(f) => {
                // Handle NaN by using a special value
                if f.is_nan() {
                    state.write_u64(0x7FF8000000000000); // NaN bit pattern
                } else {
                    f.to_bits().hash(state);
                }
            }
            StatisticValue::Text(s) => s.hash(state),
            StatisticValue::Boolean(b) => b.hash(state),
        }
    }
}

// Custom Eq implementation for StatisticValue (handles f64 NaN)
impl Eq for StatisticValue {}

impl PartialOrd for StatisticValue {
    fn partial_cmp(&self, other: &StatisticValue) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (StatisticValue::Integer(a), StatisticValue::Integer(b)) => Some(a.cmp(b)),
            (StatisticValue::Float(a), StatisticValue::Float(b)) => a.partial_cmp(b),
            (StatisticValue::Text(a), StatisticValue::Text(b)) => Some(a.cmp(b)),
            (StatisticValue::Boolean(a), StatisticValue::Boolean(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

impl StatisticValue {
    /// Compare two statistic values
    pub fn compare(&self, other: &StatisticValue) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (StatisticValue::Integer(a), StatisticValue::Integer(b)) => Some(a.cmp(b)),
            (StatisticValue::Float(a), StatisticValue::Float(b)) => {
                a.partial_cmp(b)
            }
            (StatisticValue::Text(a), StatisticValue::Text(b)) => Some(a.cmp(b)),
            (StatisticValue::Boolean(a), StatisticValue::Boolean(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

/// Histogram for value distribution
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    /// Number of buckets
    pub bucket_count: usize,
    /// Bucket boundaries
    pub buckets: Vec<HistogramBucket>,
}

/// Histogram bucket
#[derive(Debug, Clone, PartialEq)]
pub struct HistogramBucket {
    /// Lower bound (inclusive)
    pub lower_bound: StatisticValue,
    /// Upper bound (exclusive)
    pub upper_bound: StatisticValue,
    /// Number of values in this bucket
    pub count: usize,
    /// Number of distinct values in this bucket
    pub distinct_count: usize,
}

/// Column statistics builder
pub struct ColumnStatisticsBuilder {
    column_name: String,
    min_value: Option<StatisticValue>,
    max_value: Option<StatisticValue>,
    null_count: usize,
    total_count: usize,
    distinct_values: HashSet<StatisticValue>,
    sum: Option<f64>,
    histogram_buckets: Option<Vec<HistogramBucket>>,
}

impl ColumnStatisticsBuilder {
    /// Create a new statistics builder
    pub fn new(column_name: String) -> Self {
        Self {
            column_name,
            min_value: None,
            max_value: None,
            null_count: 0,
            total_count: 0,
            distinct_values: HashSet::new(),
            sum: None,
            histogram_buckets: None,
        }
    }

    /// Add a value to statistics
    pub fn add_value(&mut self, value: Option<&Column>) {
        self.total_count += 1;

        if value.is_none() {
            self.null_count += 1;
            return;
        }

        let col = value.unwrap();
        let stat_value = self.column_to_statistic_value(col, 0);

        if let Some(stat_val) = &stat_value {
            // Update min/max
            match (&self.min_value, stat_val) {
                (None, _) => {
                    self.min_value = Some(stat_val.clone());
                    self.max_value = Some(stat_val.clone());
                }
                (Some(min), new_val) => {
                    if let Some(ordering) = new_val.compare(min) {
                        if ordering == std::cmp::Ordering::Less {
                            self.min_value = Some(stat_val.clone());
                        }
                    }
                }
            }

            match (&self.max_value, stat_val) {
                (None, _) => {
                    self.max_value = Some(stat_val.clone());
                }
                (Some(max), new_val) => {
                    if let Some(ordering) = new_val.compare(max) {
                        if ordering == std::cmp::Ordering::Greater {
                            self.max_value = Some(stat_val.clone());
                        }
                    }
                }
            }

            // Track distinct values
            self.distinct_values.insert(stat_val.clone());

            // Update sum for numeric values
            if let StatisticValue::Float(f) = stat_val {
                *self.sum.get_or_insert(0.0) += f;
            } else if let StatisticValue::Integer(i) = stat_val {
                *self.sum.get_or_insert(0.0) += *i as f64;
            }
        }
    }

    /// Add a column batch to statistics
    pub fn add_column(&mut self, column: &Column, null_bitmap: &crate::protocols::postgres_wire::sql::execution::NullBitmap) {
        let len = column.len();
        self.total_count += len;

        for i in 0..len {
            if null_bitmap.is_null(i) {
                self.null_count += 1;
            } else {
                let stat_value = self.column_to_statistic_value(column, i);
                if let Some(stat_val) = &stat_value {
                    // Update min/max
                    match (&self.min_value, stat_val) {
                        (None, _) => {
                            self.min_value = Some(stat_val.clone());
                            self.max_value = Some(stat_val.clone());
                        }
                        (Some(min), new_val) => {
                            if let Some(ordering) = new_val.compare(min) {
                                if ordering == std::cmp::Ordering::Less {
                                    self.min_value = Some(stat_val.clone());
                                }
                            }
                        }
                    }

                    match (&self.max_value, stat_val) {
                        (None, _) => {
                            self.max_value = Some(stat_val.clone());
                        }
                        (Some(max), new_val) => {
                            if let Some(ordering) = new_val.compare(max) {
                                if ordering == std::cmp::Ordering::Greater {
                                    self.max_value = Some(stat_val.clone());
                                }
                            }
                        }
                    }

                    // Track distinct values
                    self.distinct_values.insert(stat_val.clone());

                    // Update sum for numeric values
                    if let StatisticValue::Float(f) = stat_val {
                        *self.sum.get_or_insert(0.0) += f;
                    } else if let StatisticValue::Integer(i) = stat_val {
                        *self.sum.get_or_insert(0.0) += *i as f64;
                    }
                }
            }
        }
    }

    /// Convert column value to statistic value
    fn column_to_statistic_value(&self, column: &Column, index: usize) -> Option<StatisticValue> {
        match column {
            Column::Bool(v) => v.get(index).map(|&b| StatisticValue::Boolean(b)),
            Column::Int16(v) => v.get(index).map(|&i| StatisticValue::Integer(i as i64)),
            Column::Int32(v) => v.get(index).map(|&i| StatisticValue::Integer(i as i64)),
            Column::Int64(v) => v.get(index).map(|&i| StatisticValue::Integer(i)),
            Column::Float32(v) => v.get(index).map(|&f| StatisticValue::Float(f as f64)),
            Column::Float64(v) => v.get(index).map(|&f| StatisticValue::Float(f)),
            Column::String(v) => v.get(index).map(|s| StatisticValue::Text(s.clone())),
            Column::Binary(_) => None, // Binary not supported for statistics
        }
    }

    /// Build column statistics
    pub fn build(self) -> ColumnStatistics {
        let non_null_count = self.total_count - self.null_count;
        let average_value = if non_null_count > 0 {
            self.sum.map(|s| s / non_null_count as f64)
        } else {
            None
        };

        ColumnStatistics {
            column_name: self.column_name,
            min_value: self.min_value,
            max_value: self.max_value,
            null_count: self.null_count,
            total_count: self.total_count,
            distinct_count: Some(self.distinct_values.len()),
            average_value,
            histogram: self.histogram_buckets.map(|buckets| Histogram {
                bucket_count: buckets.len(),
                buckets,
            }),
        }
    }
}

impl ColumnStatistics {
    /// Estimate selectivity for a range predicate
    pub fn estimate_range_selectivity(&self, min: Option<&StatisticValue>, max: Option<&StatisticValue>) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }

        // Use min/max bounds to estimate selectivity
        let column_min = &self.min_value;
        let column_max = &self.max_value;

        if let (Some(col_min), Some(col_max), Some(pred_min), Some(pred_max)) = 
            (column_min, column_max, min, max) {
            // Check if ranges overlap
            if let (Some(min_cmp), Some(max_cmp)) = 
                (pred_max.compare(col_min), pred_min.compare(col_max)) {
                if min_cmp == std::cmp::Ordering::Less || max_cmp == std::cmp::Ordering::Greater {
                    return 0.0; // No overlap
                }
            }

            // Estimate based on value range (simplified)
            // In production, would use histogram for better accuracy
            if let (StatisticValue::Integer(col_min_val), StatisticValue::Integer(col_max_val),
                    StatisticValue::Integer(pred_min_val), StatisticValue::Integer(pred_max_val)) =
                (col_min, col_max, pred_min, pred_max) {
                let col_range = (col_max_val - col_min_val) as f64;
                let pred_range = (pred_max_val - pred_min_val) as f64;
                if col_range > 0.0 {
                    return (pred_range / col_range).min(1.0);
                }
            }
        }

        // Default selectivity estimate
        0.1
    }

    /// Estimate selectivity for equality predicate
    pub fn estimate_equality_selectivity(&self) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }

        // Use distinct count if available
        if let Some(distinct) = self.distinct_count {
            if distinct > 0 {
                return 1.0 / distinct as f64;
            }
        }

        // Default estimate
        0.01
    }

    /// Get null ratio
    pub fn null_ratio(&self) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }
        self.null_count as f64 / self.total_count as f64
    }

    /// Check if column has useful statistics
    pub fn is_useful(&self) -> bool {
        self.total_count > 0 && (self.min_value.is_some() || self.distinct_count.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::postgres_wire::sql::execution::{Column, NullBitmap};

    #[test]
    fn test_statistics_builder() {
        let mut builder = ColumnStatisticsBuilder::new("test_col".to_string());
        let values = vec![1, 2, 3, 4, 5];
        let column = Column::Int32(values);
        let null_bitmap = NullBitmap::new_all_valid(5);

        builder.add_column(&column, &null_bitmap);
        let stats = builder.build();

        assert_eq!(stats.total_count, 5);
        assert_eq!(stats.null_count, 0);
        assert_eq!(stats.distinct_count, Some(5));
        assert!(stats.min_value.is_some());
        assert!(stats.max_value.is_some());
    }

    #[test]
    fn test_range_selectivity() {
        let stats = ColumnStatistics {
            column_name: "test".to_string(),
            min_value: Some(StatisticValue::Integer(0)),
            max_value: Some(StatisticValue::Integer(100)),
            null_count: 0,
            total_count: 100,
            distinct_count: Some(100),
            average_value: Some(50.0),
            histogram: None,
        };

        let selectivity = stats.estimate_range_selectivity(
            Some(&StatisticValue::Integer(10)),
            Some(&StatisticValue::Integer(20)),
        );
        assert!(selectivity > 0.0 && selectivity <= 1.0);
    }

    #[test]
    fn test_equality_selectivity() {
        let stats = ColumnStatistics {
            column_name: "test".to_string(),
            min_value: Some(StatisticValue::Integer(0)),
            max_value: Some(StatisticValue::Integer(100)),
            null_count: 0,
            total_count: 100,
            distinct_count: Some(100),
            average_value: Some(50.0),
            histogram: None,
        };

        let selectivity = stats.estimate_equality_selectivity();
        assert_eq!(selectivity, 0.01); // 1/100
    }
}

