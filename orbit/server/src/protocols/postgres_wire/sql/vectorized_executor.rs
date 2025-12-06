//! Vectorized SQL Executor
//!
//! This module provides SIMD-accelerated and GPU-accelerated execution of SQL queries.
//! It bridges the SQL execution layer with orbit-compute for high-performance operations.
//!
//! ## Features
//! - Automatic conversion from row-oriented to columnar format
//! - SIMD-accelerated filtering (WHERE clauses)
//! - SIMD/GPU-accelerated aggregations (SUM, AVG, MIN, MAX, COUNT)
//! - Cost-based routing between CPU SIMD and GPU execution
//! - Statistics collection for query optimization

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::ast::{
    BinaryOperator, Expression, OrderByItem, SelectItem, SelectStatement, SortDirection,
};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "gpu-acceleration")]
#[allow(unused_imports)]
use orbit_compute::columnar_analytics::{
    AggregateFunction, ColumnarAnalyticsConfig, GPUColumnarAnalytics,
};
#[cfg(feature = "gpu-acceleration")]
use orbit_compute::cpu::engine::CPUEngine;
#[cfg(feature = "gpu-acceleration")]
use orbit_compute::cpu::simd::NullBitmap;

/// Vectorized execution configuration
#[derive(Debug, Clone)]
pub struct VectorizedConfig {
    /// Minimum rows to use vectorized execution
    pub min_rows_for_vectorized: usize,
    /// Minimum rows to prefer GPU over CPU SIMD
    pub min_rows_for_gpu: usize,
    /// Enable SIMD acceleration
    pub enable_simd: bool,
    /// Enable GPU acceleration
    pub enable_gpu: bool,
    /// Collect execution statistics
    pub collect_statistics: bool,
}

impl Default for VectorizedConfig {
    fn default() -> Self {
        Self {
            min_rows_for_vectorized: 1000,
            min_rows_for_gpu: 10000,
            enable_simd: true,
            enable_gpu: true,
            collect_statistics: true,
        }
    }
}

/// Statistics from vectorized execution
#[derive(Debug, Clone, Default)]
pub struct VectorizedStats {
    /// Total execution time in microseconds
    pub execution_time_us: u64,
    /// Number of rows scanned
    pub rows_scanned: usize,
    /// Number of rows after filtering
    pub rows_after_filter: usize,
    /// Whether SIMD was used
    pub used_simd: bool,
    /// Whether GPU was used
    pub used_gpu: bool,
    /// Filter execution time in microseconds
    pub filter_time_us: u64,
    /// Aggregation execution time in microseconds
    pub aggregation_time_us: u64,
}

/// Columnar data representation for vectorized operations
#[derive(Debug, Clone)]
pub struct ColumnarData {
    /// Column names
    pub column_names: Vec<String>,
    /// Integer columns (i64)
    pub int_columns: HashMap<String, Vec<i64>>,
    /// Float columns (f64)
    pub float_columns: HashMap<String, Vec<f64>>,
    /// String columns
    pub string_columns: HashMap<String, Vec<Option<String>>>,
    /// Boolean columns
    pub bool_columns: HashMap<String, Vec<bool>>,
    /// Null bitmaps per column
    pub null_bitmaps: HashMap<String, Vec<bool>>,
    /// Row count
    pub row_count: usize,
}

impl ColumnarData {
    /// Create empty columnar data
    pub fn new() -> Self {
        Self {
            column_names: Vec::new(),
            int_columns: HashMap::new(),
            float_columns: HashMap::new(),
            string_columns: HashMap::new(),
            bool_columns: HashMap::new(),
            null_bitmaps: HashMap::new(),
            row_count: 0,
        }
    }

    /// Convert from row-oriented format to columnar
    pub fn from_rows(rows: &[HashMap<String, SqlValue>], column_order: &[String]) -> Self {
        let mut result = Self::new();
        result.row_count = rows.len();
        result.column_names = column_order.to_vec();

        if rows.is_empty() {
            return result;
        }

        // Initialize columns based on first row types
        for col_name in column_order {
            if let Some(first_row) = rows.first() {
                if let Some(value) = first_row.get(col_name) {
                    match value {
                        SqlValue::Integer(_) | SqlValue::BigInt(_) | SqlValue::SmallInt(_) => {
                            result
                                .int_columns
                                .insert(col_name.clone(), Vec::with_capacity(rows.len()));
                        }
                        SqlValue::Real(_) | SqlValue::DoublePrecision(_) | SqlValue::Decimal(_) => {
                            result
                                .float_columns
                                .insert(col_name.clone(), Vec::with_capacity(rows.len()));
                        }
                        SqlValue::Boolean(_) => {
                            result
                                .bool_columns
                                .insert(col_name.clone(), Vec::with_capacity(rows.len()));
                        }
                        _ => {
                            result
                                .string_columns
                                .insert(col_name.clone(), Vec::with_capacity(rows.len()));
                        }
                    }
                }
            }
            result
                .null_bitmaps
                .insert(col_name.clone(), Vec::with_capacity(rows.len()));
        }

        // Populate columns
        for row in rows {
            for col_name in column_order {
                let value = row.get(col_name);
                let is_null = value.map_or(true, |v| matches!(v, SqlValue::Null));

                if let Some(nulls) = result.null_bitmaps.get_mut(col_name) {
                    nulls.push(is_null);
                }

                if let Some(int_col) = result.int_columns.get_mut(col_name) {
                    let int_val = match value {
                        Some(SqlValue::Integer(i)) => *i as i64,
                        Some(SqlValue::BigInt(i)) => *i,
                        Some(SqlValue::SmallInt(i)) => *i as i64,
                        _ => 0,
                    };
                    int_col.push(int_val);
                } else if let Some(float_col) = result.float_columns.get_mut(col_name) {
                    let float_val = match value {
                        Some(SqlValue::Real(f)) => *f as f64,
                        Some(SqlValue::DoublePrecision(f)) => *f,
                        Some(SqlValue::Decimal(d)) => d.to_string().parse().unwrap_or(0.0),
                        Some(SqlValue::Integer(i)) => *i as f64,
                        Some(SqlValue::BigInt(i)) => *i as f64,
                        _ => 0.0,
                    };
                    float_col.push(float_val);
                } else if let Some(bool_col) = result.bool_columns.get_mut(col_name) {
                    let bool_val = match value {
                        Some(SqlValue::Boolean(b)) => *b,
                        _ => false,
                    };
                    bool_col.push(bool_val);
                } else if let Some(str_col) = result.string_columns.get_mut(col_name) {
                    let str_val = value.map(|v| v.to_postgres_string());
                    str_col.push(str_val);
                }
            }
        }

        result
    }

    /// Convert back to row-oriented format with given indices
    pub fn to_rows(&self, indices: &[usize]) -> Vec<HashMap<String, SqlValue>> {
        let mut rows = Vec::with_capacity(indices.len());

        for &idx in indices {
            if idx >= self.row_count {
                continue;
            }

            let mut row = HashMap::new();

            for col_name in &self.column_names {
                let is_null = self
                    .null_bitmaps
                    .get(col_name)
                    .and_then(|nulls| nulls.get(idx))
                    .copied()
                    .unwrap_or(true);

                if is_null {
                    row.insert(col_name.clone(), SqlValue::Null);
                    continue;
                }

                if let Some(int_col) = self.int_columns.get(col_name) {
                    if let Some(&val) = int_col.get(idx) {
                        row.insert(col_name.clone(), SqlValue::BigInt(val));
                    }
                } else if let Some(float_col) = self.float_columns.get(col_name) {
                    if let Some(&val) = float_col.get(idx) {
                        row.insert(col_name.clone(), SqlValue::DoublePrecision(val));
                    }
                } else if let Some(bool_col) = self.bool_columns.get(col_name) {
                    if let Some(&val) = bool_col.get(idx) {
                        row.insert(col_name.clone(), SqlValue::Boolean(val));
                    }
                } else if let Some(str_col) = self.string_columns.get(col_name) {
                    if let Some(val) = str_col.get(idx) {
                        match val {
                            Some(s) => row.insert(col_name.clone(), SqlValue::Text(s.clone())),
                            None => row.insert(col_name.clone(), SqlValue::Null),
                        };
                    }
                }
            }

            rows.push(row);
        }

        rows
    }

    /// Get all row indices (0..row_count)
    pub fn all_indices(&self) -> Vec<usize> {
        (0..self.row_count).collect()
    }
}

impl Default for ColumnarData {
    fn default() -> Self {
        Self::new()
    }
}

/// Vectorized SQL executor using SIMD and GPU acceleration
pub struct VectorizedExecutor {
    config: VectorizedConfig,
    #[cfg(feature = "gpu-acceleration")]
    cpu_engine: CPUEngine,
    #[cfg(feature = "gpu-acceleration")]
    #[allow(dead_code)] // Reserved for future GPU aggregation path
    gpu_analytics: Option<GPUColumnarAnalytics>,
    last_stats: VectorizedStats,
}

impl VectorizedExecutor {
    /// Create a new vectorized executor
    pub async fn new(config: VectorizedConfig) -> ProtocolResult<Self> {
        #[cfg(feature = "gpu-acceleration")]
        {
            let cpu_engine = CPUEngine::new();

            let gpu_analytics = if config.enable_gpu {
                let gpu_config = ColumnarAnalyticsConfig {
                    enable_gpu: true,
                    gpu_min_rows: config.min_rows_for_gpu,
                    use_cpu_parallel: true,
                };
                match GPUColumnarAnalytics::new(gpu_config).await {
                    Ok(analytics) => Some(analytics),
                    Err(e) => {
                        tracing::warn!("GPU analytics unavailable: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            Ok(Self {
                config,
                cpu_engine,
                gpu_analytics,
                last_stats: VectorizedStats::default(),
            })
        }

        #[cfg(not(feature = "gpu-acceleration"))]
        {
            Ok(Self {
                config,
                last_stats: VectorizedStats::default(),
            })
        }
    }

    /// Create with default config
    pub async fn new_default() -> ProtocolResult<Self> {
        Self::new(VectorizedConfig::default()).await
    }

    /// Get statistics from last execution
    pub fn last_stats(&self) -> &VectorizedStats {
        &self.last_stats
    }

    /// Check if vectorized execution should be used
    pub fn should_use_vectorized(&self, row_count: usize) -> bool {
        row_count >= self.config.min_rows_for_vectorized && self.config.enable_simd
    }

    /// Execute a filter operation using SIMD
    #[cfg(feature = "gpu-acceleration")]
    pub fn execute_filter_simd(
        &self,
        columnar_data: &ColumnarData,
        column_name: &str,
        op: FilterOp,
        value: &SqlValue,
    ) -> ProtocolResult<Vec<usize>> {
        // Try integer column first
        if let Some(int_col) = columnar_data.int_columns.get(column_name) {
            let target = match value {
                SqlValue::Integer(i) => *i as i64,
                SqlValue::BigInt(i) => *i,
                SqlValue::SmallInt(i) => *i as i64,
                _ => {
                    return Err(ProtocolError::PostgresError(format!(
                        "Cannot compare integer column to {:?}",
                        value
                    )))
                }
            };

            let result = match op {
                FilterOp::Eq => self.cpu_engine.filter_eq_i64(int_col, target),
                FilterOp::Ne => {
                    // Ne = all indices not in Eq result
                    let eq_indices: std::collections::HashSet<_> = self
                        .cpu_engine
                        .filter_eq_i64(int_col, target)
                        .into_iter()
                        .collect();
                    (0..int_col.len())
                        .filter(|i| !eq_indices.contains(i))
                        .collect()
                }
                FilterOp::Lt => self.cpu_engine.filter_lt_i64(int_col, target),
                FilterOp::Le => {
                    let mut result = self.cpu_engine.filter_lt_i64(int_col, target);
                    result.extend(self.cpu_engine.filter_eq_i64(int_col, target));
                    result.sort_unstable();
                    result.dedup();
                    result
                }
                FilterOp::Gt => {
                    let le_indices: std::collections::HashSet<_> = {
                        let mut le = self.cpu_engine.filter_lt_i64(int_col, target);
                        le.extend(self.cpu_engine.filter_eq_i64(int_col, target));
                        le.into_iter().collect()
                    };
                    (0..int_col.len())
                        .filter(|i| !le_indices.contains(i))
                        .collect()
                }
                FilterOp::Ge => {
                    let lt_indices: std::collections::HashSet<_> = self
                        .cpu_engine
                        .filter_lt_i64(int_col, target)
                        .into_iter()
                        .collect();
                    (0..int_col.len())
                        .filter(|i| !lt_indices.contains(i))
                        .collect()
                }
            };

            return Ok(result);
        }

        // Try float column
        if let Some(float_col) = columnar_data.float_columns.get(column_name) {
            let target = match value {
                SqlValue::Real(f) => *f as f64,
                SqlValue::DoublePrecision(f) => *f,
                SqlValue::Integer(i) => *i as f64,
                SqlValue::BigInt(i) => *i as f64,
                _ => {
                    return Err(ProtocolError::PostgresError(format!(
                        "Cannot compare float column to {:?}",
                        value
                    )))
                }
            };

            let result = match op {
                FilterOp::Eq => self.cpu_engine.filter_eq_f64(float_col, target),
                FilterOp::Lt => self.cpu_engine.filter_lt_f64(float_col, target),
                _ => {
                    // Fallback to scalar for other ops
                    float_col
                        .iter()
                        .enumerate()
                        .filter(|(_, &v)| match op {
                            FilterOp::Ne => (v - target).abs() > f64::EPSILON,
                            FilterOp::Le => v <= target,
                            FilterOp::Gt => v > target,
                            FilterOp::Ge => v >= target,
                            _ => false,
                        })
                        .map(|(i, _)| i)
                        .collect()
                }
            };

            return Ok(result);
        }

        // Fallback to scalar for string columns
        if let Some(str_col) = columnar_data.string_columns.get(column_name) {
            let target = value.to_postgres_string();
            let result: Vec<usize> = str_col
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    let v_str = v.as_deref().unwrap_or("");
                    match op {
                        FilterOp::Eq => v_str == target,
                        FilterOp::Ne => v_str != target,
                        FilterOp::Lt => v_str < target.as_str(),
                        FilterOp::Le => v_str <= target.as_str(),
                        FilterOp::Gt => v_str > target.as_str(),
                        FilterOp::Ge => v_str >= target.as_str(),
                    }
                })
                .map(|(i, _)| i)
                .collect();
            return Ok(result);
        }

        Err(ProtocolError::PostgresError(format!(
            "Column '{}' not found for filtering",
            column_name
        )))
    }

    /// Execute aggregation using SIMD/GPU
    #[cfg(feature = "gpu-acceleration")]
    pub async fn execute_aggregate(
        &self,
        columnar_data: &ColumnarData,
        column_name: &str,
        agg_func: AggregateType,
        indices: Option<&[usize]>,
    ) -> ProtocolResult<SqlValue> {
        let start = Instant::now();

        // Get the column data filtered by indices
        let result = if let Some(int_col) = columnar_data.int_columns.get(column_name) {
            let values: Vec<i64> = match indices {
                Some(idx) => idx
                    .iter()
                    .filter_map(|&i| int_col.get(i).copied())
                    .collect(),
                None => int_col.clone(),
            };

            let null_bitmap = columnar_data.null_bitmaps.get(column_name);
            let nulls: Vec<bool> = match (indices, null_bitmap) {
                (Some(idx), Some(nb)) => idx.iter().filter_map(|&i| nb.get(i).copied()).collect(),
                (None, Some(nb)) => nb.clone(),
                _ => vec![false; values.len()],
            };

            // Convert to NullBitmap format
            let null_bm = NullBitmap::from_bools(&nulls);

            match agg_func {
                AggregateType::Sum => {
                    let sum = self.cpu_engine.sum_i64(&values, &null_bm);
                    sum.map(SqlValue::BigInt).unwrap_or(SqlValue::Null)
                }
                AggregateType::Avg => {
                    let sum = self.cpu_engine.sum_i64(&values, &null_bm);
                    let count = null_bm.non_null_count();
                    match (sum, count) {
                        (Some(s), c) if c > 0 => SqlValue::DoublePrecision(s as f64 / c as f64),
                        _ => SqlValue::Null,
                    }
                }
                AggregateType::Min => {
                    let min = self.cpu_engine.min_i64(&values, &null_bm);
                    min.map(SqlValue::BigInt).unwrap_or(SqlValue::Null)
                }
                AggregateType::Max => {
                    let max = self.cpu_engine.max_i64(&values, &null_bm);
                    max.map(SqlValue::BigInt).unwrap_or(SqlValue::Null)
                }
                AggregateType::Count => {
                    let count = null_bm.non_null_count();
                    SqlValue::BigInt(count as i64)
                }
            }
        } else if let Some(float_col) = columnar_data.float_columns.get(column_name) {
            let values: Vec<f64> = match indices {
                Some(idx) => idx
                    .iter()
                    .filter_map(|&i| float_col.get(i).copied())
                    .collect(),
                None => float_col.clone(),
            };

            let null_bitmap = columnar_data.null_bitmaps.get(column_name);
            let nulls: Vec<bool> = match (indices, null_bitmap) {
                (Some(idx), Some(nb)) => idx.iter().filter_map(|&i| nb.get(i).copied()).collect(),
                (None, Some(nb)) => nb.clone(),
                _ => vec![false; values.len()],
            };

            let null_bm = NullBitmap::from_bools(&nulls);

            match agg_func {
                AggregateType::Sum => {
                    let sum = self.cpu_engine.sum_f64(&values, &null_bm);
                    sum.map(SqlValue::DoublePrecision).unwrap_or(SqlValue::Null)
                }
                AggregateType::Avg => {
                    let sum = self.cpu_engine.sum_f64(&values, &null_bm);
                    let count = null_bm.non_null_count();
                    match (sum, count) {
                        (Some(s), c) if c > 0 => SqlValue::DoublePrecision(s / c as f64),
                        _ => SqlValue::Null,
                    }
                }
                AggregateType::Min => {
                    let min = self.cpu_engine.min_f64(&values, &null_bm);
                    min.map(SqlValue::DoublePrecision).unwrap_or(SqlValue::Null)
                }
                AggregateType::Max => {
                    let max = self.cpu_engine.max_f64(&values, &null_bm);
                    max.map(SqlValue::DoublePrecision).unwrap_or(SqlValue::Null)
                }
                AggregateType::Count => {
                    let count = null_bm.non_null_count();
                    SqlValue::BigInt(count as i64)
                }
            }
        } else {
            // String columns - only COUNT supported
            match agg_func {
                AggregateType::Count => {
                    if let Some(str_col) = columnar_data.string_columns.get(column_name) {
                        let count = match indices {
                            Some(idx) => idx
                                .iter()
                                .filter(|&&i| str_col.get(i).map_or(false, |v| v.is_some()))
                                .count(),
                            None => str_col.iter().filter(|v| v.is_some()).count(),
                        };
                        SqlValue::BigInt(count as i64)
                    } else {
                        SqlValue::Null
                    }
                }
                _ => {
                    return Err(ProtocolError::PostgresError(format!(
                        "Aggregation {:?} not supported for string column '{}'",
                        agg_func, column_name
                    )))
                }
            }
        };

        let _elapsed = start.elapsed();
        Ok(result)
    }

    /// Fallback scalar filter when SIMD not available
    #[cfg(not(feature = "gpu-acceleration"))]
    pub fn execute_filter_simd(
        &self,
        columnar_data: &ColumnarData,
        column_name: &str,
        op: FilterOp,
        value: &SqlValue,
    ) -> ProtocolResult<Vec<usize>> {
        self.execute_filter_scalar(columnar_data, column_name, op, value)
    }

    /// Scalar filter implementation (fallback)
    pub fn execute_filter_scalar(
        &self,
        columnar_data: &ColumnarData,
        column_name: &str,
        op: FilterOp,
        value: &SqlValue,
    ) -> ProtocolResult<Vec<usize>> {
        if let Some(int_col) = columnar_data.int_columns.get(column_name) {
            let target = match value {
                SqlValue::Integer(i) => *i as i64,
                SqlValue::BigInt(i) => *i,
                SqlValue::SmallInt(i) => *i as i64,
                _ => {
                    return Err(ProtocolError::PostgresError(format!(
                        "Cannot compare integer column to {:?}",
                        value
                    )))
                }
            };

            let result: Vec<usize> = int_col
                .iter()
                .enumerate()
                .filter(|(_, &v)| match op {
                    FilterOp::Eq => v == target,
                    FilterOp::Ne => v != target,
                    FilterOp::Lt => v < target,
                    FilterOp::Le => v <= target,
                    FilterOp::Gt => v > target,
                    FilterOp::Ge => v >= target,
                })
                .map(|(i, _)| i)
                .collect();

            return Ok(result);
        }

        if let Some(float_col) = columnar_data.float_columns.get(column_name) {
            let target = match value {
                SqlValue::Real(f) => *f as f64,
                SqlValue::DoublePrecision(f) => *f,
                SqlValue::Integer(i) => *i as f64,
                _ => {
                    return Err(ProtocolError::PostgresError(format!(
                        "Cannot compare float column to {:?}",
                        value
                    )))
                }
            };

            let result: Vec<usize> = float_col
                .iter()
                .enumerate()
                .filter(|(_, &v)| match op {
                    FilterOp::Eq => (v - target).abs() < f64::EPSILON,
                    FilterOp::Ne => (v - target).abs() >= f64::EPSILON,
                    FilterOp::Lt => v < target,
                    FilterOp::Le => v <= target,
                    FilterOp::Gt => v > target,
                    FilterOp::Ge => v >= target,
                })
                .map(|(i, _)| i)
                .collect();

            return Ok(result);
        }

        Err(ProtocolError::PostgresError(format!(
            "Column '{}' not found",
            column_name
        )))
    }

    /// Fallback scalar aggregation
    #[cfg(not(feature = "gpu-acceleration"))]
    pub async fn execute_aggregate(
        &self,
        columnar_data: &ColumnarData,
        column_name: &str,
        agg_func: AggregateType,
        indices: Option<&[usize]>,
    ) -> ProtocolResult<SqlValue> {
        self.execute_aggregate_scalar(columnar_data, column_name, agg_func, indices)
    }

    /// Scalar aggregation implementation (fallback)
    pub fn execute_aggregate_scalar(
        &self,
        columnar_data: &ColumnarData,
        column_name: &str,
        agg_func: AggregateType,
        indices: Option<&[usize]>,
    ) -> ProtocolResult<SqlValue> {
        if let Some(int_col) = columnar_data.int_columns.get(column_name) {
            let values: Vec<i64> = match indices {
                Some(idx) => idx
                    .iter()
                    .filter_map(|&i| int_col.get(i).copied())
                    .collect(),
                None => int_col.clone(),
            };

            if values.is_empty() {
                return Ok(SqlValue::Null);
            }

            let result = match agg_func {
                AggregateType::Sum => SqlValue::BigInt(values.iter().sum()),
                AggregateType::Avg => {
                    let sum: i64 = values.iter().sum();
                    SqlValue::DoublePrecision(sum as f64 / values.len() as f64)
                }
                AggregateType::Min => SqlValue::BigInt(*values.iter().min().unwrap()),
                AggregateType::Max => SqlValue::BigInt(*values.iter().max().unwrap()),
                AggregateType::Count => SqlValue::BigInt(values.len() as i64),
            };

            return Ok(result);
        }

        if let Some(float_col) = columnar_data.float_columns.get(column_name) {
            let values: Vec<f64> = match indices {
                Some(idx) => idx
                    .iter()
                    .filter_map(|&i| float_col.get(i).copied())
                    .collect(),
                None => float_col.clone(),
            };

            if values.is_empty() {
                return Ok(SqlValue::Null);
            }

            let result = match agg_func {
                AggregateType::Sum => SqlValue::DoublePrecision(values.iter().sum()),
                AggregateType::Avg => {
                    let sum: f64 = values.iter().sum();
                    SqlValue::DoublePrecision(sum / values.len() as f64)
                }
                AggregateType::Min => {
                    SqlValue::DoublePrecision(values.iter().cloned().reduce(f64::min).unwrap())
                }
                AggregateType::Max => {
                    SqlValue::DoublePrecision(values.iter().cloned().reduce(f64::max).unwrap())
                }
                AggregateType::Count => SqlValue::BigInt(values.len() as i64),
            };

            return Ok(result);
        }

        Ok(SqlValue::Null)
    }

    /// Execute vectorized sorting on columnar data
    /// Returns sorted row indices
    pub fn execute_sort(
        &self,
        columnar_data: &ColumnarData,
        sort_keys: &[SortKey],
        indices: Option<&[usize]>,
    ) -> ProtocolResult<Vec<usize>> {
        if sort_keys.is_empty() {
            return Ok(indices
                .map(|i| i.to_vec())
                .unwrap_or_else(|| columnar_data.all_indices()));
        }

        let mut sorted_indices: Vec<usize> = indices
            .map(|i| i.to_vec())
            .unwrap_or_else(|| columnar_data.all_indices());

        // Sort using the first key, with tiebreakers from subsequent keys
        sorted_indices.sort_by(|&a, &b| {
            for key in sort_keys {
                let cmp = self.compare_rows(columnar_data, a, b, key);
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        });

        Ok(sorted_indices)
    }

    /// Compare two rows by a sort key
    fn compare_rows(
        &self,
        columnar_data: &ColumnarData,
        a: usize,
        b: usize,
        key: &SortKey,
    ) -> std::cmp::Ordering {
        // Check nulls
        let a_null = columnar_data
            .null_bitmaps
            .get(&key.column)
            .and_then(|nb| nb.get(a))
            .copied()
            .unwrap_or(true);
        let b_null = columnar_data
            .null_bitmaps
            .get(&key.column)
            .and_then(|nb| nb.get(b))
            .copied()
            .unwrap_or(true);

        match (a_null, b_null) {
            (true, true) => return std::cmp::Ordering::Equal,
            (true, false) => {
                return if key.nulls_first {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
            (false, true) => {
                return if key.nulls_first {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                };
            }
            (false, false) => {}
        }

        // Compare values
        let ordering = if let Some(int_col) = columnar_data.int_columns.get(&key.column) {
            let val_a = int_col.get(a).copied().unwrap_or(0);
            let val_b = int_col.get(b).copied().unwrap_or(0);
            val_a.cmp(&val_b)
        } else if let Some(float_col) = columnar_data.float_columns.get(&key.column) {
            let val_a = float_col.get(a).copied().unwrap_or(0.0);
            let val_b = float_col.get(b).copied().unwrap_or(0.0);
            val_a
                .partial_cmp(&val_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else if let Some(str_col) = columnar_data.string_columns.get(&key.column) {
            let val_a = str_col.get(a).and_then(|v| v.as_ref());
            let val_b = str_col.get(b).and_then(|v| v.as_ref());
            val_a.cmp(&val_b)
        } else if let Some(bool_col) = columnar_data.bool_columns.get(&key.column) {
            let val_a = bool_col.get(a).copied().unwrap_or(false);
            let val_b = bool_col.get(b).copied().unwrap_or(false);
            val_a.cmp(&val_b)
        } else {
            std::cmp::Ordering::Equal
        };

        if key.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    }

    /// Apply multiple filter conditions with AND semantics
    pub fn apply_filters(
        &self,
        columnar_data: &ColumnarData,
        analysis: &WhereAnalysis,
    ) -> ProtocolResult<Vec<usize>> {
        if analysis.filters.is_empty() {
            return Ok(columnar_data.all_indices());
        }

        // Start with all indices
        let mut result_set: std::collections::HashSet<usize> =
            columnar_data.all_indices().into_iter().collect();

        // Apply each filter with AND semantics
        for filter in &analysis.filters {
            let filter_result =
                self.execute_filter_simd(columnar_data, &filter.column, filter.op, &filter.value)?;
            let filter_set: std::collections::HashSet<usize> = filter_result.into_iter().collect();
            result_set = result_set.intersection(&filter_set).copied().collect();
        }

        let mut result: Vec<usize> = result_set.into_iter().collect();
        result.sort_unstable();
        Ok(result)
    }

    /// Execute a complete SELECT with SIMD optimization
    /// Returns (columns, rows) suitable for ExecutionResult
    pub async fn execute_select_vectorized(
        &mut self,
        select: &SelectStatement,
        source_rows: Vec<HashMap<String, SqlValue>>,
    ) -> ProtocolResult<(Vec<String>, Vec<Vec<Option<String>>>)> {
        let start = Instant::now();

        // Determine column order from SELECT list
        let column_order: Vec<String> = select
            .select_list
            .iter()
            .filter_map(|item| match item {
                SelectItem::Expression { expr, alias } => alias.clone().or_else(|| {
                    if let Expression::Column(col_ref) = expr {
                        Some(col_ref.name.clone())
                    } else {
                        None
                    }
                }),
                SelectItem::Wildcard => None,
                SelectItem::QualifiedWildcard { .. } => None,
            })
            .collect();

        // If SELECT *, get all column names from first row
        let column_order = if column_order.is_empty() && !source_rows.is_empty() {
            source_rows[0].keys().cloned().collect()
        } else {
            column_order
        };

        // Convert to columnar format
        let columnar_data = ColumnarData::from_rows(&source_rows, &column_order);
        self.last_stats.rows_scanned = columnar_data.row_count;

        // Apply WHERE clause filters
        let filter_start = Instant::now();
        let filtered_indices = if let Some(where_clause) = &select.where_clause {
            let analysis = WhereAnalysis::analyze(where_clause);
            if analysis.too_complex {
                // Fall back to all rows, let standard executor handle complex WHERE
                columnar_data.all_indices()
            } else {
                self.apply_filters(&columnar_data, &analysis)?
            }
        } else {
            columnar_data.all_indices()
        };
        self.last_stats.filter_time_us = filter_start.elapsed().as_micros() as u64;
        self.last_stats.rows_after_filter = filtered_indices.len();

        // Apply ORDER BY
        let sorted_indices = if let Some(order_by) = &select.order_by {
            let sort_keys = SortKey::from_order_by(order_by);
            self.execute_sort(&columnar_data, &sort_keys, Some(&filtered_indices))?
        } else {
            filtered_indices
        };

        // Apply LIMIT/OFFSET
        let final_indices = if let Some(limit_clause) = &select.limit {
            let offset = select.offset.unwrap_or(0) as usize;

            let limit_val = limit_clause
                .count
                .as_ref()
                .and_then(|expr| match expr {
                    Expression::Literal(SqlValue::Integer(n)) => Some(*n as usize),
                    Expression::Literal(SqlValue::BigInt(n)) => Some(*n as usize),
                    _ => None,
                })
                .unwrap_or(sorted_indices.len());

            sorted_indices
                .into_iter()
                .skip(offset)
                .take(limit_val)
                .collect()
        } else {
            sorted_indices
        };

        // Convert back to rows
        let result_rows = columnar_data.to_rows(&final_indices);

        // Format as Vec<Vec<Option<String>>> for ExecutionResult
        let formatted_rows: Vec<Vec<Option<String>>> = result_rows
            .iter()
            .map(|row| {
                column_order
                    .iter()
                    .map(|col| row.get(col).map(|v| v.to_postgres_string()))
                    .collect()
            })
            .collect();

        self.last_stats.execution_time_us = start.elapsed().as_micros() as u64;
        self.last_stats.used_simd = self.config.enable_simd;

        Ok((column_order, formatted_rows))
    }
}

/// Filter operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl FilterOp {
    /// Convert from AST BinaryOperator to FilterOp
    pub fn from_binary_operator(op: &BinaryOperator) -> Option<Self> {
        match op {
            BinaryOperator::Equal => Some(FilterOp::Eq),
            BinaryOperator::NotEqual => Some(FilterOp::Ne),
            BinaryOperator::LessThan => Some(FilterOp::Lt),
            BinaryOperator::LessThanOrEqual => Some(FilterOp::Le),
            BinaryOperator::GreaterThan => Some(FilterOp::Gt),
            BinaryOperator::GreaterThanOrEqual => Some(FilterOp::Ge),
            _ => None,
        }
    }
}

/// Extracted filter condition from an expression
#[derive(Debug, Clone)]
pub struct FilterCondition {
    pub column: String,
    pub op: FilterOp,
    pub value: SqlValue,
}

/// Result of analyzing a WHERE clause
#[derive(Debug, Clone, Default)]
pub struct WhereAnalysis {
    /// Simple filter conditions (column op value)
    pub filters: Vec<FilterCondition>,
    /// AND conditions between filters
    pub and_conditions: Vec<(usize, usize)>,
    /// Whether the expression is too complex for vectorized execution
    pub too_complex: bool,
}

impl WhereAnalysis {
    /// Analyze a WHERE expression and extract filter conditions
    pub fn analyze(expr: &Expression) -> Self {
        let mut result = WhereAnalysis::default();
        result.extract_conditions(expr);
        result
    }

    fn extract_conditions(&mut self, expr: &Expression) {
        match expr {
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                // Check for AND - combine conditions
                if *operator == BinaryOperator::And {
                    self.extract_conditions(left);
                    let left_idx = self.filters.len().saturating_sub(1);
                    self.extract_conditions(right);
                    let right_idx = self.filters.len().saturating_sub(1);
                    if left_idx != right_idx {
                        self.and_conditions.push((left_idx, right_idx));
                    }
                    return;
                }

                // Check for simple comparison: column op literal
                if let Some(filter_op) = FilterOp::from_binary_operator(operator) {
                    // Try column on left, literal on right
                    if let (Expression::Column(col_ref), Expression::Literal(value)) =
                        (left.as_ref(), right.as_ref())
                    {
                        self.filters.push(FilterCondition {
                            column: col_ref.name.clone(),
                            op: filter_op,
                            value: value.clone(),
                        });
                        return;
                    }

                    // Try literal on left, column on right (flip operator)
                    if let (Expression::Literal(value), Expression::Column(col_ref)) =
                        (left.as_ref(), right.as_ref())
                    {
                        let flipped_op = match filter_op {
                            FilterOp::Lt => FilterOp::Gt,
                            FilterOp::Le => FilterOp::Ge,
                            FilterOp::Gt => FilterOp::Lt,
                            FilterOp::Ge => FilterOp::Le,
                            other => other,
                        };
                        self.filters.push(FilterCondition {
                            column: col_ref.name.clone(),
                            op: flipped_op,
                            value: value.clone(),
                        });
                        return;
                    }
                }

                // Complex expression
                self.too_complex = true;
            }
            Expression::IsNull { expr, negated } => {
                // Can handle IS NULL / IS NOT NULL for some cases
                if let Expression::Column(_col_ref) = expr.as_ref() {
                    // We could add special handling, but mark as complex for now
                    let _ = negated;
                    self.too_complex = true;
                }
            }
            _ => {
                self.too_complex = true;
            }
        }
    }
}

/// Sort key extracted from ORDER BY
#[derive(Debug, Clone)]
pub struct SortKey {
    pub column: String,
    pub ascending: bool,
    pub nulls_first: bool,
}

impl SortKey {
    /// Extract sort keys from ORDER BY items
    pub fn from_order_by(order_by: &[OrderByItem]) -> Vec<Self> {
        order_by
            .iter()
            .filter_map(|item| {
                // Only support simple column references
                if let Expression::Column(col_ref) = &item.expression {
                    let ascending = !matches!(item.direction, Some(SortDirection::Descending));
                    let nulls_first = match &item.nulls {
                        Some(crate::protocols::postgres_wire::sql::ast::NullsOrder::First) => true,
                        Some(crate::protocols::postgres_wire::sql::ast::NullsOrder::Last) => false,
                        None => !ascending, // Default: NULLS LAST for ASC, NULLS FIRST for DESC
                    };
                    Some(SortKey {
                        column: col_ref.name.clone(),
                        ascending,
                        nulls_first,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Aggregate operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateType {
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

impl std::fmt::Display for FilterOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterOp::Eq => write!(f, "="),
            FilterOp::Ne => write!(f, "!="),
            FilterOp::Lt => write!(f, "<"),
            FilterOp::Le => write!(f, "<="),
            FilterOp::Gt => write!(f, ">"),
            FilterOp::Ge => write!(f, ">="),
        }
    }
}

impl std::fmt::Display for AggregateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AggregateType::Sum => write!(f, "SUM"),
            AggregateType::Avg => write!(f, "AVG"),
            AggregateType::Min => write!(f, "MIN"),
            AggregateType::Max => write!(f, "MAX"),
            AggregateType::Count => write!(f, "COUNT"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_columnar_data_from_rows() {
        let mut row1 = HashMap::new();
        row1.insert("id".to_string(), SqlValue::Integer(1));
        row1.insert("value".to_string(), SqlValue::DoublePrecision(10.5));
        row1.insert("name".to_string(), SqlValue::Text("Alice".to_string()));

        let mut row2 = HashMap::new();
        row2.insert("id".to_string(), SqlValue::Integer(2));
        row2.insert("value".to_string(), SqlValue::DoublePrecision(20.5));
        row2.insert("name".to_string(), SqlValue::Text("Bob".to_string()));

        let rows = vec![row1, row2];
        let columns = vec!["id".to_string(), "value".to_string(), "name".to_string()];

        let columnar = ColumnarData::from_rows(&rows, &columns);

        assert_eq!(columnar.row_count, 2);
        assert!(columnar.int_columns.contains_key("id"));
        assert!(columnar.float_columns.contains_key("value"));
        assert!(columnar.string_columns.contains_key("name"));

        assert_eq!(columnar.int_columns.get("id").unwrap(), &vec![1i64, 2i64]);
        assert_eq!(
            columnar.float_columns.get("value").unwrap(),
            &vec![10.5, 20.5]
        );
    }

    #[test]
    fn test_columnar_to_rows() {
        let mut columnar = ColumnarData::new();
        columnar.column_names = vec!["id".to_string(), "value".to_string()];
        columnar.row_count = 3;
        columnar.int_columns.insert("id".to_string(), vec![1, 2, 3]);
        columnar
            .float_columns
            .insert("value".to_string(), vec![10.0, 20.0, 30.0]);
        columnar
            .null_bitmaps
            .insert("id".to_string(), vec![false, false, false]);
        columnar
            .null_bitmaps
            .insert("value".to_string(), vec![false, false, false]);

        let rows = columnar.to_rows(&[0, 2]);
        assert_eq!(rows.len(), 2);

        assert_eq!(rows[0].get("id"), Some(&SqlValue::BigInt(1)));
        assert_eq!(rows[1].get("id"), Some(&SqlValue::BigInt(3)));
    }

    #[test]
    fn test_scalar_filter() {
        let mut columnar = ColumnarData::new();
        columnar.column_names = vec!["age".to_string()];
        columnar.row_count = 5;
        columnar
            .int_columns
            .insert("age".to_string(), vec![25, 30, 35, 40, 45]);

        let executor = VectorizedExecutor {
            config: VectorizedConfig::default(),
            #[cfg(feature = "gpu-acceleration")]
            cpu_engine: CPUEngine::new(),
            #[cfg(feature = "gpu-acceleration")]
            gpu_analytics: None,
            last_stats: VectorizedStats::default(),
        };

        let result = executor
            .execute_filter_scalar(&columnar, "age", FilterOp::Gt, &SqlValue::Integer(30))
            .unwrap();

        assert_eq!(result, vec![2, 3, 4]); // indices where age > 30
    }

    #[test]
    fn test_scalar_aggregate() {
        let mut columnar = ColumnarData::new();
        columnar.column_names = vec!["value".to_string()];
        columnar.row_count = 4;
        columnar
            .int_columns
            .insert("value".to_string(), vec![10, 20, 30, 40]);

        let executor = VectorizedExecutor {
            config: VectorizedConfig::default(),
            #[cfg(feature = "gpu-acceleration")]
            cpu_engine: CPUEngine::new(),
            #[cfg(feature = "gpu-acceleration")]
            gpu_analytics: None,
            last_stats: VectorizedStats::default(),
        };

        let sum = executor
            .execute_aggregate_scalar(&columnar, "value", AggregateType::Sum, None)
            .unwrap();

        assert_eq!(sum, SqlValue::BigInt(100));

        let avg = executor
            .execute_aggregate_scalar(&columnar, "value", AggregateType::Avg, None)
            .unwrap();

        assert_eq!(avg, SqlValue::DoublePrecision(25.0));
    }
}
