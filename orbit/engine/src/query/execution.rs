//! Vectorized Query Execution Engine
//!
//! Implements batch-based query execution using columnar storage and SIMD operations.
//! Processes data in batches of 1024 rows for optimal cache utilization and SIMD efficiency.
//!
//! ## Design Patterns
//!
//! - **Strategy Pattern**: Pluggable execution strategies (vectorized vs row-based)
//! - **Pipeline Pattern**: Streaming data through operators
//! - **Visitor Pattern**: Walking execution plans
//! - **Builder Pattern**: Fluent API for executor configuration

use std::collections::HashSet;

use crate::error::{EngineError, EngineResult};
use crate::storage::{SqlValue, Column, ColumnBatch, NullBitmap, DEFAULT_BATCH_SIZE};
use super::simd::{SimdCapability, SimdFilter, SimdAggregate, simd_capability};
use super::simd::filters::{SimdFilterI32, SimdFilterI64, SimdFilterF64};
use super::simd::aggregates::{SimdAggregateI32, SimdAggregateI64, SimdAggregateF64};

/// Plan node types that break pipeline execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanNodeType {
    TableScan,
    Filter,
    Projection,
    Aggregation,
    Sort,
    Join,
    Limit,
    Offset,
}

impl PlanNodeType {
    /// Check if this operation breaks the execution pipeline
    pub fn is_pipeline_breaker(&self) -> bool {
        matches!(self, PlanNodeType::Sort | PlanNodeType::Join)
    }
}

/// Vectorized executor configuration
#[derive(Debug, Clone)]
pub struct VectorizedExecutorConfig {
    /// Batch size (number of rows per batch)
    pub batch_size: usize,
    /// Enable SIMD optimizations
    pub use_simd: bool,
    /// Pipeline breakers (operations that require materialization)
    pub pipeline_breakers: HashSet<PlanNodeType>,
    /// Enable parallel execution
    pub enable_parallel: bool,
    /// Detected SIMD capability
    pub simd_capability: SimdCapability,
}

impl Default for VectorizedExecutorConfig {
    fn default() -> Self {
        let mut pipeline_breakers = HashSet::new();
        pipeline_breakers.insert(PlanNodeType::Sort);
        pipeline_breakers.insert(PlanNodeType::Join);

        Self {
            batch_size: DEFAULT_BATCH_SIZE,
            use_simd: true,
            pipeline_breakers,
            enable_parallel: true,
            simd_capability: simd_capability(),
        }
    }
}

/// Builder for VectorizedExecutorConfig
pub struct VectorizedExecutorConfigBuilder {
    config: VectorizedExecutorConfig,
}

impl VectorizedExecutorConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: VectorizedExecutorConfig::default(),
        }
    }

    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size.max(1);
        self
    }

    pub fn use_simd(mut self, enabled: bool) -> Self {
        self.config.use_simd = enabled;
        self
    }

    pub fn enable_parallel(mut self, enabled: bool) -> Self {
        self.config.enable_parallel = enabled;
        self
    }

    pub fn add_pipeline_breaker(mut self, node_type: PlanNodeType) -> Self {
        self.config.pipeline_breakers.insert(node_type);
        self
    }

    pub fn build(self) -> VectorizedExecutorConfig {
        self.config
    }
}

impl Default for VectorizedExecutorConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Comparison operator for filter operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// Aggregate function type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunction {
    Sum,
    Min,
    Max,
    Count,
    Avg,
}

/// Vectorized query executor
pub struct VectorizedExecutor {
    config: VectorizedExecutorConfig,
    // SIMD filters
    simd_filter_i32: SimdFilterI32,
    simd_filter_i64: SimdFilterI64,
    simd_filter_f64: SimdFilterF64,
    // SIMD aggregates
    simd_aggregate_i32: SimdAggregateI32,
    simd_aggregate_i64: SimdAggregateI64,
    simd_aggregate_f64: SimdAggregateF64,
}

impl VectorizedExecutor {
    /// Create a new vectorized executor with default configuration
    pub fn new() -> Self {
        Self::with_config(VectorizedExecutorConfig::default())
    }

    /// Create a new vectorized executor with custom configuration
    pub fn with_config(config: VectorizedExecutorConfig) -> Self {
        Self {
            config,
            simd_filter_i32: SimdFilterI32::new(),
            simd_filter_i64: SimdFilterI64::new(),
            simd_filter_f64: SimdFilterF64::new(),
            simd_aggregate_i32: SimdAggregateI32::new(),
            simd_aggregate_i64: SimdAggregateI64::new(),
            simd_aggregate_f64: SimdAggregateF64::new(),
        }
    }

    /// Get executor configuration
    pub fn config(&self) -> &VectorizedExecutorConfig {
        &self.config
    }

    /// Execute a vectorized table scan
    ///
    /// Reads data from storage in columnar format, batch by batch.
    pub fn execute_table_scan(
        &self,
        columns: Vec<Column>,
        null_bitmaps: Vec<NullBitmap>,
        column_names: Option<Vec<String>>,
    ) -> EngineResult<ColumnBatch> {
        // Validate input
        if columns.is_empty() {
            return Err(EngineError::storage(
                "Table scan requires at least one column".to_string(),
            ));
        }

        if columns.len() != null_bitmaps.len() {
            return Err(EngineError::storage(
                "Column count must match null bitmap count".to_string(),
            ));
        }

        // Determine row count from first column
        let row_count = columns[0].len();

        // Verify all columns have same length
        for (idx, col) in columns.iter().enumerate() {
            if col.len() != row_count {
                return Err(EngineError::storage(
                    format!("Column {} has mismatched length: expected {}, got {}",
                            idx, row_count, col.len()),
                ));
            }
        }

        Ok(ColumnBatch {
            columns,
            null_bitmaps,
            row_count,
            column_names,
        })
    }

    /// Execute a vectorized filter operation
    ///
    /// Uses SIMD-optimized comparison when available, falls back to scalar.
    pub fn execute_filter(
        &self,
        batch: &ColumnBatch,
        column_index: usize,
        op: ComparisonOp,
        value: SqlValue,
    ) -> EngineResult<ColumnBatch> {
        if column_index >= batch.columns.len() {
            return Err(EngineError::storage(
                format!("Column index {} out of bounds", column_index),
            ));
        }

        let column = &batch.columns[column_index];
        let null_bitmap = &batch.null_bitmaps[column_index];

        // Get matching row indices using SIMD when possible
        let mut matching_indices = Vec::new();

        match (column, &value) {
            (Column::Int32(values), SqlValue::Int32(target)) => {
                self.filter_i32(values, *target, op, &mut matching_indices)?;
            }
            (Column::Int64(values), SqlValue::Int64(target)) => {
                self.filter_i64(values, *target, op, &mut matching_indices)?;
            }
            (Column::Float64(values), SqlValue::Float64(target)) => {
                self.filter_f64(values, *target, op, &mut matching_indices)?;
            }
            _ => {
                // Fallback to scalar comparison for other types
                self.filter_scalar(column, &value, op, &mut matching_indices)?;
            }
        }

        // Filter out nulls
        matching_indices.retain(|&idx| null_bitmap.is_valid(idx));

        // Build result batch with only matching rows
        self.select_rows(batch, &matching_indices)
    }

    /// Execute vectorized projection (select specific columns)
    pub fn execute_projection(
        &self,
        batch: &ColumnBatch,
        column_indices: &[usize],
    ) -> EngineResult<ColumnBatch> {
        let mut new_columns = Vec::new();
        let mut new_null_bitmaps = Vec::new();
        let mut new_column_names = Vec::new();

        for &idx in column_indices {
            if idx >= batch.columns.len() {
                return Err(EngineError::storage(
                    format!("Column index {} out of bounds", idx),
                ));
            }

            new_columns.push(batch.columns[idx].clone());
            new_null_bitmaps.push(batch.null_bitmaps[idx].clone());

            if let Some(ref names) = batch.column_names {
                new_column_names.push(names[idx].clone());
            }
        }

        let column_names = if new_column_names.is_empty() {
            None
        } else {
            Some(new_column_names)
        };

        Ok(ColumnBatch {
            columns: new_columns,
            null_bitmaps: new_null_bitmaps,
            row_count: batch.row_count,
            column_names,
        })
    }

    /// Execute vectorized aggregation
    pub fn execute_aggregation(
        &self,
        batch: &ColumnBatch,
        column_index: usize,
        function: AggregateFunction,
    ) -> EngineResult<SqlValue> {
        if column_index >= batch.columns.len() {
            return Err(EngineError::storage(
                format!("Column index {} out of bounds", column_index),
            ));
        }

        let column = &batch.columns[column_index];
        let null_bitmap = &batch.null_bitmaps[column_index];

        match (column, function) {
            (Column::Int32(values), AggregateFunction::Sum) => {
                let result = self.simd_aggregate_i32.sum(values, null_bitmap);
                Ok(result.map(SqlValue::Int32).unwrap_or(SqlValue::Null))
            }
            (Column::Int32(values), AggregateFunction::Min) => {
                let result = self.simd_aggregate_i32.min(values, null_bitmap);
                Ok(result.map(SqlValue::Int32).unwrap_or(SqlValue::Null))
            }
            (Column::Int32(values), AggregateFunction::Max) => {
                let result = self.simd_aggregate_i32.max(values, null_bitmap);
                Ok(result.map(SqlValue::Int32).unwrap_or(SqlValue::Null))
            }
            (Column::Int32(_), AggregateFunction::Count) => {
                let count = self.simd_aggregate_i32.count(null_bitmap);
                Ok(SqlValue::Int64(count as i64))
            }
            (Column::Int64(values), AggregateFunction::Sum) => {
                let result = self.simd_aggregate_i64.sum(values, null_bitmap);
                Ok(result.map(SqlValue::Int64).unwrap_or(SqlValue::Null))
            }
            (Column::Int64(values), AggregateFunction::Min) => {
                let result = self.simd_aggregate_i64.min(values, null_bitmap);
                Ok(result.map(SqlValue::Int64).unwrap_or(SqlValue::Null))
            }
            (Column::Int64(values), AggregateFunction::Max) => {
                let result = self.simd_aggregate_i64.max(values, null_bitmap);
                Ok(result.map(SqlValue::Int64).unwrap_or(SqlValue::Null))
            }
            (Column::Int64(_), AggregateFunction::Count) => {
                let count = self.simd_aggregate_i64.count(null_bitmap);
                Ok(SqlValue::Int64(count as i64))
            }
            (Column::Float64(values), AggregateFunction::Sum) => {
                let result = self.simd_aggregate_f64.sum(values, null_bitmap);
                Ok(result.map(SqlValue::Float64).unwrap_or(SqlValue::Null))
            }
            (Column::Float64(values), AggregateFunction::Min) => {
                let result = self.simd_aggregate_f64.min(values, null_bitmap);
                Ok(result.map(SqlValue::Float64).unwrap_or(SqlValue::Null))
            }
            (Column::Float64(values), AggregateFunction::Max) => {
                let result = self.simd_aggregate_f64.max(values, null_bitmap);
                Ok(result.map(SqlValue::Float64).unwrap_or(SqlValue::Null))
            }
            (Column::Float64(_), AggregateFunction::Count) => {
                let count = self.simd_aggregate_f64.count(null_bitmap);
                Ok(SqlValue::Int64(count as i64))
            }
            (_, AggregateFunction::Avg) => {
                // Average requires both sum and count
                Err(EngineError::storage(
                    "AVG aggregation not yet implemented".to_string(),
                ))
            }
            _ => Err(EngineError::storage(
                format!("Unsupported column type for {:?}", function),
            )),
        }
    }

    /// Execute LIMIT operation
    pub fn execute_limit(
        &self,
        batch: &ColumnBatch,
        limit: usize,
    ) -> EngineResult<ColumnBatch> {
        if limit == 0 || limit >= batch.row_count {
            return Ok(batch.clone());
        }

        let indices: Vec<usize> = (0..limit).collect();
        self.select_rows(batch, &indices)
    }

    /// Execute OFFSET operation
    pub fn execute_offset(
        &self,
        batch: &ColumnBatch,
        offset: usize,
    ) -> EngineResult<ColumnBatch> {
        if offset >= batch.row_count {
            // Return empty batch
            return self.select_rows(batch, &[]);
        }

        let indices: Vec<usize> = (offset..batch.row_count).collect();
        self.select_rows(batch, &indices)
    }

    // Private helper methods

    /// Filter i32 values using SIMD
    fn filter_i32(
        &self,
        values: &[i32],
        target: i32,
        op: ComparisonOp,
        result: &mut Vec<usize>,
    ) -> EngineResult<()> {
        if self.config.use_simd {
            match op {
                ComparisonOp::Equal => self.simd_filter_i32.filter_eq(values, target, result),
                ComparisonOp::NotEqual => self.simd_filter_i32.filter_ne(values, target, result),
                ComparisonOp::LessThan => self.simd_filter_i32.filter_lt(values, target, result),
                ComparisonOp::LessThanOrEqual => self.simd_filter_i32.filter_le(values, target, result),
                ComparisonOp::GreaterThan => self.simd_filter_i32.filter_gt(values, target, result),
                ComparisonOp::GreaterThanOrEqual => self.simd_filter_i32.filter_ge(values, target, result),
            }
        } else {
            self.filter_scalar_generic(values, target, op, result);
        }
        Ok(())
    }

    /// Filter i64 values
    fn filter_i64(
        &self,
        values: &[i64],
        target: i64,
        op: ComparisonOp,
        result: &mut Vec<usize>,
    ) -> EngineResult<()> {
        match op {
            ComparisonOp::Equal => self.simd_filter_i64.filter_eq(values, target, result),
            ComparisonOp::NotEqual => self.simd_filter_i64.filter_ne(values, target, result),
            ComparisonOp::LessThan => self.simd_filter_i64.filter_lt(values, target, result),
            ComparisonOp::LessThanOrEqual => self.simd_filter_i64.filter_le(values, target, result),
            ComparisonOp::GreaterThan => self.simd_filter_i64.filter_gt(values, target, result),
            ComparisonOp::GreaterThanOrEqual => self.simd_filter_i64.filter_ge(values, target, result),
        }
        Ok(())
    }

    /// Filter f64 values
    fn filter_f64(
        &self,
        values: &[f64],
        target: f64,
        op: ComparisonOp,
        result: &mut Vec<usize>,
    ) -> EngineResult<()> {
        match op {
            ComparisonOp::Equal => self.simd_filter_f64.filter_eq(values, target, result),
            ComparisonOp::NotEqual => self.simd_filter_f64.filter_ne(values, target, result),
            ComparisonOp::LessThan => self.simd_filter_f64.filter_lt(values, target, result),
            ComparisonOp::LessThanOrEqual => self.simd_filter_f64.filter_le(values, target, result),
            ComparisonOp::GreaterThan => self.simd_filter_f64.filter_gt(values, target, result),
            ComparisonOp::GreaterThanOrEqual => self.simd_filter_f64.filter_ge(values, target, result),
        }
        Ok(())
    }

    /// Generic scalar filter for any PartialOrd + PartialEq type
    fn filter_scalar_generic<T: PartialOrd + PartialEq>(
        &self,
        values: &[T],
        target: T,
        op: ComparisonOp,
        result: &mut Vec<usize>,
    ) {
        result.clear();
        for (idx, value) in values.iter().enumerate() {
            let matches = match op {
                ComparisonOp::Equal => *value == target,
                ComparisonOp::NotEqual => *value != target,
                ComparisonOp::LessThan => *value < target,
                ComparisonOp::LessThanOrEqual => *value <= target,
                ComparisonOp::GreaterThan => *value > target,
                ComparisonOp::GreaterThanOrEqual => *value >= target,
            };
            if matches {
                result.push(idx);
            }
        }
    }

    /// Scalar filter for arbitrary column types
    fn filter_scalar(
        &self,
        column: &Column,
        target: &SqlValue,
        op: ComparisonOp,
        result: &mut Vec<usize>,
    ) -> EngineResult<()> {
        result.clear();

        match (column, target) {
            (Column::Bool(values), SqlValue::Boolean(target_val)) => {
                for (idx, &value) in values.iter().enumerate() {
                    let matches = match op {
                        ComparisonOp::Equal => value == *target_val,
                        ComparisonOp::NotEqual => value != *target_val,
                        _ => false,
                    };
                    if matches {
                        result.push(idx);
                    }
                }
            }
            (Column::String(values), SqlValue::String(target_str)) => {
                for (idx, value) in values.iter().enumerate() {
                    let matches = match op {
                        ComparisonOp::Equal => value == target_str,
                        ComparisonOp::NotEqual => value != target_str,
                        ComparisonOp::LessThan => value < target_str,
                        ComparisonOp::LessThanOrEqual => value <= target_str,
                        ComparisonOp::GreaterThan => value > target_str,
                        ComparisonOp::GreaterThanOrEqual => value >= target_str,
                    };
                    if matches {
                        result.push(idx);
                    }
                }
            }
            _ => {
                return Err(EngineError::storage(
                    "Unsupported column/value type combination for filter".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Select specific rows from a batch
    fn select_rows(
        &self,
        batch: &ColumnBatch,
        indices: &[usize],
    ) -> EngineResult<ColumnBatch> {
        let mut new_columns = Vec::new();
        let mut new_null_bitmaps = Vec::new();

        for (col_idx, column) in batch.columns.iter().enumerate() {
            let new_column = self.select_column_rows(column, indices)?;
            let new_null_bitmap = self.select_null_bitmap_rows(
                &batch.null_bitmaps[col_idx],
                indices,
            );

            new_columns.push(new_column);
            new_null_bitmaps.push(new_null_bitmap);
        }

        Ok(ColumnBatch {
            columns: new_columns,
            null_bitmaps: new_null_bitmaps,
            row_count: indices.len(),
            column_names: batch.column_names.clone(),
        })
    }

    /// Select specific rows from a column
    fn select_column_rows(
        &self,
        column: &Column,
        indices: &[usize],
    ) -> EngineResult<Column> {
        let result = match column {
            Column::Bool(values) => {
                let selected: Vec<bool> = indices.iter().map(|&i| values[i]).collect();
                Column::Bool(selected)
            }
            Column::Int16(values) => {
                let selected: Vec<i16> = indices.iter().map(|&i| values[i]).collect();
                Column::Int16(selected)
            }
            Column::Int32(values) => {
                let selected: Vec<i32> = indices.iter().map(|&i| values[i]).collect();
                Column::Int32(selected)
            }
            Column::Int64(values) => {
                let selected: Vec<i64> = indices.iter().map(|&i| values[i]).collect();
                Column::Int64(selected)
            }
            Column::Float32(values) => {
                let selected: Vec<f32> = indices.iter().map(|&i| values[i]).collect();
                Column::Float32(selected)
            }
            Column::Float64(values) => {
                let selected: Vec<f64> = indices.iter().map(|&i| values[i]).collect();
                Column::Float64(selected)
            }
            Column::String(values) => {
                let selected: Vec<String> = indices.iter().map(|&i| values[i].clone()).collect();
                Column::String(selected)
            }
            Column::Binary(values) => {
                let selected: Vec<Vec<u8>> = indices.iter().map(|&i| values[i].clone()).collect();
                Column::Binary(selected)
            }
        };

        Ok(result)
    }

    /// Select specific rows from a null bitmap
    fn select_null_bitmap_rows(
        &self,
        null_bitmap: &NullBitmap,
        indices: &[usize],
    ) -> NullBitmap {
        let mut new_bitmap = NullBitmap::new_all_valid(indices.len());

        for (new_idx, &old_idx) in indices.iter().enumerate() {
            if null_bitmap.is_null(old_idx) {
                new_bitmap.set_null(new_idx);
            }
        }

        new_bitmap
    }
}

impl Default for VectorizedExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let executor = VectorizedExecutor::new();
        assert_eq!(executor.config().batch_size, DEFAULT_BATCH_SIZE);
        assert!(executor.config().use_simd);
    }

    #[test]
    fn test_executor_config_builder() {
        let config = VectorizedExecutorConfigBuilder::new()
            .batch_size(2048)
            .use_simd(false)
            .enable_parallel(false)
            .build();

        assert_eq!(config.batch_size, 2048);
        assert!(!config.use_simd);
        assert!(!config.enable_parallel);
    }

    #[test]
    fn test_table_scan() {
        let executor = VectorizedExecutor::new();

        let columns = vec![
            Column::Int32(vec![1, 2, 3, 4, 5]),
            Column::String(vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string()]),
        ];

        let null_bitmaps = vec![
            NullBitmap::new_all_valid(5),
            NullBitmap::new_all_valid(5),
        ];

        let result = executor.execute_table_scan(
            columns,
            null_bitmaps,
            Some(vec!["id".to_string(), "name".to_string()]),
        );

        assert!(result.is_ok());
        let batch = result.unwrap();
        assert_eq!(batch.row_count, 5);
        assert_eq!(batch.columns.len(), 2);
    }

    #[test]
    fn test_filter_i32_equal() {
        let executor = VectorizedExecutor::new();

        let batch = ColumnBatch {
            columns: vec![Column::Int32(vec![1, 5, 3, 5, 2, 5, 7])],
            null_bitmaps: vec![NullBitmap::new_all_valid(7)],
            row_count: 7,
            column_names: Some(vec!["value".to_string()]),
        };

        let result = executor.execute_filter(&batch, 0, ComparisonOp::Equal, SqlValue::Int32(5));

        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.row_count, 3); // Three 5s in the data

        if let Column::Int32(values) = &filtered.columns[0] {
            assert_eq!(values, &vec![5, 5, 5]);
        } else {
            panic!("Expected Int32 column");
        }
    }

    #[test]
    fn test_filter_i32_greater_than() {
        let executor = VectorizedExecutor::new();

        let batch = ColumnBatch {
            columns: vec![Column::Int32(vec![1, 5, 3, 5, 2, 5, 7])],
            null_bitmaps: vec![NullBitmap::new_all_valid(7)],
            row_count: 7,
            column_names: Some(vec!["value".to_string()]),
        };

        let result = executor.execute_filter(&batch, 0, ComparisonOp::GreaterThan, SqlValue::Int32(4));

        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.row_count, 4); // 5, 5, 5, 7

        if let Column::Int32(values) = &filtered.columns[0] {
            assert_eq!(values, &vec![5, 5, 5, 7]);
        } else {
            panic!("Expected Int32 column");
        }
    }

    #[test]
    fn test_projection() {
        let executor = VectorizedExecutor::new();

        let batch = ColumnBatch {
            columns: vec![
                Column::Int32(vec![1, 2, 3]),
                Column::String(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
                Column::Float64(vec![1.1, 2.2, 3.3]),
            ],
            null_bitmaps: vec![
                NullBitmap::new_all_valid(3),
                NullBitmap::new_all_valid(3),
                NullBitmap::new_all_valid(3),
            ],
            row_count: 3,
            column_names: Some(vec!["id".to_string(), "name".to_string(), "score".to_string()]),
        };

        // Select columns 0 and 2 (id and score)
        let result = executor.execute_projection(&batch, &[0, 2]);

        assert!(result.is_ok());
        let projected = result.unwrap();
        assert_eq!(projected.columns.len(), 2);
        assert_eq!(projected.row_count, 3);

        if let Some(names) = projected.column_names {
            assert_eq!(names, vec!["id", "score"]);
        } else {
            panic!("Expected column names");
        }
    }

    #[test]
    fn test_aggregation_sum() {
        let executor = VectorizedExecutor::new();

        let batch = ColumnBatch {
            columns: vec![Column::Int32(vec![1, 2, 3, 4, 5])],
            null_bitmaps: vec![NullBitmap::new_all_valid(5)],
            row_count: 5,
            column_names: None,
        };

        let result = executor.execute_aggregation(&batch, 0, AggregateFunction::Sum);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SqlValue::Int32(15));
    }

    #[test]
    fn test_aggregation_with_nulls() {
        let executor = VectorizedExecutor::new();

        let mut null_bitmap = NullBitmap::new_all_valid(5);
        null_bitmap.set_null(1); // Exclude 2
        null_bitmap.set_null(3); // Exclude 4

        let batch = ColumnBatch {
            columns: vec![Column::Int32(vec![1, 2, 3, 4, 5])],
            null_bitmaps: vec![null_bitmap],
            row_count: 5,
            column_names: None,
        };

        let result = executor.execute_aggregation(&batch, 0, AggregateFunction::Sum);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), SqlValue::Int32(9)); // 1 + 3 + 5
    }

    #[test]
    fn test_limit() {
        let executor = VectorizedExecutor::new();

        let batch = ColumnBatch {
            columns: vec![Column::Int32(vec![1, 2, 3, 4, 5])],
            null_bitmaps: vec![NullBitmap::new_all_valid(5)],
            row_count: 5,
            column_names: None,
        };

        let result = executor.execute_limit(&batch, 3);

        assert!(result.is_ok());
        let limited = result.unwrap();
        assert_eq!(limited.row_count, 3);

        if let Column::Int32(values) = &limited.columns[0] {
            assert_eq!(values, &vec![1, 2, 3]);
        } else {
            panic!("Expected Int32 column");
        }
    }

    #[test]
    fn test_offset() {
        let executor = VectorizedExecutor::new();

        let batch = ColumnBatch {
            columns: vec![Column::Int32(vec![1, 2, 3, 4, 5])],
            null_bitmaps: vec![NullBitmap::new_all_valid(5)],
            row_count: 5,
            column_names: None,
        };

        let result = executor.execute_offset(&batch, 2);

        assert!(result.is_ok());
        let offset_batch = result.unwrap();
        assert_eq!(offset_batch.row_count, 3);

        if let Column::Int32(values) = &offset_batch.columns[0] {
            assert_eq!(values, &vec![3, 4, 5]);
        } else {
            panic!("Expected Int32 column");
        }
    }

    #[test]
    fn test_pipeline_breakers() {
        assert!(PlanNodeType::Sort.is_pipeline_breaker());
        assert!(PlanNodeType::Join.is_pipeline_breaker());
        assert!(!PlanNodeType::Filter.is_pipeline_breaker());
        assert!(!PlanNodeType::Projection.is_pipeline_breaker());
    }
}
