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
use orbit_compute::cpu::{CPUEngine, SimdLevel};
use orbit_compute::cpu::simd::{SimdFilter, SimdAggregate, SimdCapability, simd_capability};
use orbit_compute::cpu::simd::filters::{SimdFilterI32, SimdFilterI64, SimdFilterF64};
use orbit_compute::cpu::simd::aggregates::{SimdAggregateI32, SimdAggregateI64, SimdAggregateF64};

/// Plan node types that break pipeline execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanNodeType {
    /// Table scan operation
    TableScan,
    /// Filter/WHERE operation
    Filter,
    /// Projection/SELECT operation
    Projection,
    /// Aggregation operation (GROUP BY)
    Aggregation,
    /// Sort/ORDER BY operation
    Sort,
    /// Join operation
    Join,
    /// Limit operation (LIMIT clause)
    Limit,
    /// Offset operation (OFFSET clause)
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
    /// The configuration being built
    config: VectorizedExecutorConfig,
}

impl VectorizedExecutorConfigBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: VectorizedExecutorConfig::default(),
        }
    }

    /// Set the batch size for vectorized operations
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size.max(1);
        self
    }

    /// Enable or disable SIMD optimizations
    pub fn use_simd(mut self, enabled: bool) -> Self {
        self.config.use_simd = enabled;
        self
    }

    /// Enable or disable parallel execution
    pub fn enable_parallel(mut self, enabled: bool) -> Self {
        self.config.enable_parallel = enabled;
        self
    }

    /// Add a pipeline-breaking node type
    pub fn add_pipeline_breaker(mut self, node_type: PlanNodeType) -> Self {
        self.config.pipeline_breakers.insert(node_type);
        self
    }

    /// Build and return the final configuration
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
    /// Equality comparison operator (=)
    Equal,
    /// Not equal comparison operator (!=)
    NotEqual,
    /// Less than comparison operator (<)
    LessThan,
    /// Less than or equal comparison operator (<=)
    LessThanOrEqual,
    /// Greater than comparison operator (>)
    GreaterThan,
    /// Greater than or equal comparison operator (>=)
    GreaterThanOrEqual,
}

/// Aggregate function type for column operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunction {
    /// Sum aggregation function (SUM)
    Sum,
    /// Minimum value aggregation function (MIN)
    Min,
    /// Maximum value aggregation function (MAX)
    Max,
    /// Count aggregation function (COUNT)
    Count,
    /// Average aggregation function (AVG)
    Avg,
}

/// Vectorized query executor
pub struct VectorizedExecutor {
    /// Executor configuration
    config: VectorizedExecutorConfig,
    /// CPU compute engine for SIMD operations
    cpu_engine: CPUEngine,
    /// SIMD filter for i32 values
    simd_filter_i32: SimdFilterI32,
    /// SIMD filter for i64 values
    simd_filter_i64: SimdFilterI64,
    /// SIMD filter for f64 values
    simd_filter_f64: SimdFilterF64,
    /// SIMD aggregate for i32 values
    simd_aggregate_i32: SimdAggregateI32,
    /// SIMD aggregate for i64 values
    simd_aggregate_i64: SimdAggregateI64,
    /// SIMD aggregate for f64 values
    simd_aggregate_f64: SimdAggregateF64,
    /// Cached GPU device manager (lazy-initialized, checked once)
    gpu_device_manager: std::sync::OnceLock<orbit_compute::gpu_backend::GpuDeviceManager>,
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
            cpu_engine: CPUEngine::new(),
            simd_filter_i32: SimdFilterI32::new(),
            simd_filter_i64: SimdFilterI64::new(),
            simd_filter_f64: SimdFilterF64::new(),
            simd_aggregate_i32: SimdAggregateI32::new(),
            simd_aggregate_i64: SimdAggregateI64::new(),
            simd_aggregate_f64: SimdAggregateF64::new(),
            gpu_device_manager: std::sync::OnceLock::new(),
        }
    }

    /// Get or initialize the GPU device manager (cached, only checked once)
    /// 
    /// This method uses OnceLock to ensure GPU detection only happens once
    /// per executor instance, avoiding repeated checks on every query.
    fn get_gpu_device_manager(&self) -> &orbit_compute::gpu_backend::GpuDeviceManager {
        self.gpu_device_manager.get_or_init(|| {
            orbit_compute::gpu_backend::GpuDeviceManager::new()
        })
    }

    /// Get executor configuration
    pub fn config(&self) -> &VectorizedExecutorConfig {
        &self.config
    }

    /// Get CPU engine reference
    pub fn cpu_engine(&self) -> &CPUEngine {
        &self.cpu_engine
    }

    /// Get detected SIMD level
    pub fn simd_level(&self) -> SimdLevel {
        self.cpu_engine.simd_level()
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

    /// Filter i32 values using SIMD when available, fallback to scalar
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

    /// Filter i64 values using SIMD
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

    /// Filter f64 values using SIMD
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

    /// Scalar filter implementation for any comparable type
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

    /// Scalar filter for arbitrary column types without SIMD optimization
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

    /// Extract specific rows from a batch by index
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

    /// Extract specific rows from a single column by index
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

    /// Extract specific rows from a null bitmap by index
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

    /// Execute query with acceleration strategy routing
    ///
    /// This method analyzes the ExecutionPlan's acceleration strategy and routes
    /// execution to the appropriate compute backend (CPU SIMD, GPU, Neural Engine, etc.).
    ///
    /// # Arguments
    ///
    /// * `plan` - Execution plan with acceleration strategy
    /// * `query` - Query to execute
    /// * `data` - Input data batch
    ///
    /// # Returns
    ///
    /// Result containing the query execution output
    ///
    /// # Example
    ///
    /// ```
    /// use orbit_engine::query::execution::VectorizedExecutor;
    /// use orbit_engine::query::{ExecutionPlan, Query};
    /// use orbit_engine::storage::columnar::ColumnBatch;
    /// use orbit_compute::AccelerationStrategy;
    ///
    /// # tokio_test::block_on(async {
    /// let executor = VectorizedExecutor::new();
    /// let query = Query {
    ///     table: "test".to_string(),
    ///     projection: Some(vec!["id".to_string()]),
    ///     filter: None,
    ///     limit: None,
    ///     offset: None,
    /// };
    /// let plan = ExecutionPlan {
    ///     nodes: vec![],
    ///     estimated_cost: 1.0,
    ///     uses_simd: false,
    ///     acceleration_strategy: Some(AccelerationStrategy::None),
    ///     query_analysis: None,
    /// };
    /// let data = ColumnBatch::new(0, 0);
    /// let _result = executor.execute_with_acceleration(&plan, &query, &data).await;
    /// // Note: Result may be an error if query plan is invalid/incomplete
    /// # });
    /// ```
    pub async fn execute_with_acceleration(
        &self,
        plan: &crate::query::ExecutionPlan,
        query: &crate::query::Query,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        use orbit_compute::AccelerationStrategy;

        // Get the recommended acceleration strategy from the plan
        let strategy = plan.acceleration_strategy.unwrap_or(AccelerationStrategy::None);

        tracing::debug!(
            "Executing query with strategy: {:?}, complexity: {:?}",
            strategy,
            plan.query_analysis.as_ref().map(|a| a.complexity_score)
        );

        // Route to appropriate execution backend
        match strategy {
            AccelerationStrategy::CpuSimd => {
                // Use CPU SIMD execution path with full query context
                self.execute_cpu_simd_with_query(plan, query, data).await
            }
            AccelerationStrategy::Gpu => {
                // GPU execution path with fallback chain: CUDA -> ROCm -> Vulkan -> CPU
                // Use cached GpuDeviceManager to try backends in priority order
                // (GPU detection only happens once, cached for subsequent calls)
                let device_manager = self.get_gpu_device_manager();
                let backends = device_manager.available_backends();
                
                if backends.is_empty() {
                    tracing::info!("No GPU backends available, using CPU SIMD");
                    return self.execute_cpu_simd_with_query(plan, query, data).await;
                }
                
                // Try each backend in order until one succeeds
                for backend in backends {
                    match self.execute_gpu_with_backend(plan, query, data, *backend).await {
                        Ok(result) => {
                            tracing::info!("GPU execution succeeded with backend: {:?}", backend);
                            return Ok(result);
                        }
                        Err(e) => {
                            tracing::warn!("GPU backend {:?} failed: {}, trying next backend", backend, e);
                            continue;
                        }
                    }
                }
                
                // All GPU backends failed, fall back to CPU
                tracing::warn!("All GPU backends failed, falling back to CPU SIMD");
                self.execute_cpu_simd_with_query(plan, query, data).await
            }
            AccelerationStrategy::NeuralEngine => {
                // Neural Engine execution path (future implementation)
                tracing::info!("Neural Engine acceleration requested but not yet implemented, falling back to CPU SIMD");
                self.execute_cpu_simd_with_query(plan, query, data).await
            }
            AccelerationStrategy::Hybrid => {
                // Hybrid execution path (future implementation)
                tracing::info!("Hybrid acceleration requested but not yet implemented, falling back to CPU SIMD");
                self.execute_cpu_simd_with_query(plan, query, data).await
            }
            AccelerationStrategy::None => {
                // Standard execution with query context
                self.execute_cpu_simd_with_query(plan, query, data).await
            }
        }
    }

    /// Execute query using CPU SIMD acceleration
    ///
    /// This method uses the existing SIMD-optimized operations for filters,
    /// projections, and aggregations.
    #[allow(dead_code)]
    async fn execute_cpu_simd(
        &self,
        _plan: &crate::query::ExecutionPlan,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        // For now, return the data as-is since we need Query details
        // to execute filters/projections. This will be enhanced when
        // we add a full query execution pipeline.
        tracing::debug!(
            "CPU SIMD execution path (returning input data, needs Query context for full execution)"
        );
        Ok(data.clone())
    }

    /// Execute query using CPU SIMD acceleration with full Query context
    ///
    /// This method connects the optimizer's ExecutionPlan with the Query details
    /// to execute SIMD-accelerated operations.
    async fn execute_cpu_simd_with_query(
        &self,
        _plan: &crate::query::ExecutionPlan,
        query: &crate::query::Query,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        let mut result = data.clone();

        tracing::debug!(
            "Executing query with CPU SIMD: table={}, rows={}",
            query.table,
            result.row_count
        );

        // Apply filter if present
        if let Some(ref filter) = query.filter {
            result = self.execute_filter_predicate(&result, filter)?;
            tracing::debug!("After filter: {} rows", result.row_count);
        }

        // Apply offset BEFORE limit (standard SQL order)
        if let Some(offset) = query.offset {
            if offset < result.row_count {
                result = self.execute_offset(&result, offset)?;
                tracing::debug!("After offset: {} rows", result.row_count);
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            if result.row_count > limit {
                result = self.execute_limit(&result, limit)?;
                tracing::debug!("After limit: {} rows", result.row_count);
            }
        }

        // Apply projection LAST to ensure column names are available for earlier operations
        if let Some(ref projection) = query.projection {
            result = self.execute_projection_by_names(&result, projection)?;
            tracing::debug!(
                "After projection: {} columns",
                projection.len()
            );
        }

        Ok(result)
    }

    /// Execute query using GPU Metal acceleration (macOS only)
    ///
    /// This method uses the Metal GPU to accelerate filter operations.
    /// Falls back to CPU SIMD on error.
    #[cfg(target_os = "macos")]
    async fn execute_gpu_metal_with_query(
        &self,
        _plan: &crate::query::ExecutionPlan,
        query: &crate::query::Query,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        
        use orbit_compute::gpu_metal::MetalDevice;

        tracing::info!(
            "Executing query with Metal GPU: table={}, rows={}",
            query.table,
            data.row_count
        );

        // Initialize Metal device (cached in production)
        let device = MetalDevice::new().map_err(|e| {
            EngineError::Internal(format!("Failed to initialize Metal device: {}", e))
        })?;

        let mut result = data.clone();

        // Apply filter if present (GPU accelerated)
        if let Some(ref filter) = query.filter {
            result = self.execute_filter_predicate_gpu(&device, &result, filter)?;
            tracing::info!("GPU filter complete: {} rows remaining", result.row_count);
        }

        // Apply offset BEFORE limit (standard SQL order)
        if let Some(offset) = query.offset {
            if offset < result.row_count {
                result = self.execute_offset(&result, offset)?;
                tracing::debug!("After offset: {} rows", result.row_count);
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            if result.row_count > limit {
                result = self.execute_limit(&result, limit)?;
                tracing::debug!("After limit: {} rows", result.row_count);
            }
        }

        // Apply projection LAST
        if let Some(ref projection) = query.projection {
            result = self.execute_projection_by_names(&result, projection)?;
            tracing::debug!("After projection: {} columns", projection.len());
        }

        Ok(result)
    }

    /// Execute query using GPU CUDA acceleration (Linux/Windows)
    ///
    /// This method uses CUDA to accelerate filter operations.
    /// Falls back to CPU SIMD on error.
    #[cfg(all(not(target_os = "macos"), feature = "gpu-cuda"))]
    async fn execute_gpu_cuda_with_query(
        &self,
        _plan: &crate::query::ExecutionPlan,
        query: &crate::query::Query,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        use orbit_compute::gpu_cuda::CudaDevice;

        tracing::info!(
            "Executing query with CUDA GPU: table={}, rows={}",
            query.table,
            data.row_count
        );

        // Initialize CUDA device (cached in production)
        let device = CudaDevice::new().map_err(|e| {
            EngineError::Internal(format!("Failed to initialize CUDA device: {}", e))
        })?;

        let mut result = data.clone();

        // Apply filter if present (GPU accelerated)
        if let Some(ref filter) = query.filter {
            result = self.execute_filter_predicate_gpu_cuda(&device, &result, filter)?;
            tracing::info!("CUDA filter complete: {} rows remaining", result.row_count);
        }

        // Apply offset BEFORE limit (standard SQL order)
        if let Some(offset) = query.offset {
            if offset < result.row_count {
                result = self.execute_offset(&result, offset)?;
                tracing::debug!("After offset: {} rows", result.row_count);
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            if result.row_count > limit {
                result = self.execute_limit(&result, limit)?;
                tracing::debug!("After limit: {} rows", result.row_count);
            }
        }

        // Apply projection LAST
        if let Some(ref projection) = query.projection {
            result = self.execute_projection_by_names(&result, projection)?;
            tracing::debug!("After projection: {} columns", projection.len());
        }

        Ok(result)
    }

    /// Execute query using GPU with a specific backend
    /// 
    /// This method routes to the appropriate GPU backend implementation.
    /// Falls back to CPU SIMD if the backend fails.
    async fn execute_gpu_with_backend(
        &self,
        plan: &crate::query::ExecutionPlan,
        query: &crate::query::Query,
        data: &ColumnBatch,
        backend: orbit_compute::gpu_backend::GpuBackendType,
    ) -> EngineResult<ColumnBatch> {
        use orbit_compute::gpu_backend::GpuBackendType;
        
        match backend {
            #[cfg(target_os = "macos")]
            GpuBackendType::Metal => {
                self.execute_gpu_metal_with_query(plan, query, data).await
            }
            
            #[cfg(all(not(target_os = "macos"), feature = "gpu-cuda"))]
            GpuBackendType::Cuda => {
                self.execute_gpu_cuda_with_query(plan, query, data).await
            }
            
            #[cfg(all(unix, not(target_os = "macos"), feature = "gpu-rocm"))]
            GpuBackendType::Rocm => {
                self.execute_gpu_rocm_with_query(plan, query, data).await
            }
            
            #[cfg(feature = "gpu-vulkan")]
            GpuBackendType::Vulkan => {
                self.execute_gpu_vulkan_with_query(plan, query, data).await
            }
            
            _ => {
                Err(EngineError::Internal(format!(
                    "GPU backend {:?} not supported or not compiled in",
                    backend
                )))
            }
        }
    }

    /// Execute query using GPU ROCm acceleration (Linux only)
    ///
    /// This method uses ROCm to accelerate filter operations.
    /// Falls back to CPU SIMD on error.
    #[cfg(all(unix, not(target_os = "macos"), feature = "gpu-rocm"))]
    async fn execute_gpu_rocm_with_query(
        &self,
        _plan: &crate::query::ExecutionPlan,
        query: &crate::query::Query,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        use orbit_compute::gpu_rocm::RocmDevice;

        tracing::info!(
            "Executing query with ROCm GPU: table={}, rows={}",
            query.table,
            data.row_count
        );

        // Initialize ROCm device (cached in production)
        let device = RocmDevice::new().map_err(|e| {
            EngineError::Internal(format!("Failed to initialize ROCm device: {}", e))
        })?;

        let mut result = data.clone();

        // Apply filter if present (GPU accelerated)
        if let Some(ref filter) = query.filter {
            result = self.execute_filter_predicate_gpu_rocm(&device, &result, filter)?;
            tracing::info!("ROCm filter complete: {} rows remaining", result.row_count);
        }

        // Apply offset BEFORE limit (standard SQL order)
        if let Some(offset) = query.offset {
            if offset < result.row_count {
                result = self.execute_offset(&result, offset)?;
                tracing::debug!("After offset: {} rows", result.row_count);
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            if result.row_count > limit {
                result = self.execute_limit(&result, limit)?;
                tracing::debug!("After limit: {} rows", result.row_count);
            }
        }

        // Apply projection LAST
        if let Some(ref projection) = query.projection {
            result = self.execute_projection_by_names(&result, projection)?;
            tracing::debug!("After projection: {} columns", projection.len());
        }

        Ok(result)
    }

    /// Execute query using GPU Vulkan acceleration (cross-platform)
    ///
    /// This method uses Vulkan to accelerate filter operations.
    /// Falls back to CPU SIMD on error.
    #[cfg(feature = "gpu-vulkan")]
    async fn execute_gpu_vulkan_with_query(
        &self,
        _plan: &crate::query::ExecutionPlan,
        query: &crate::query::Query,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        use orbit_compute::gpu_vulkan::VulkanDevice;

        tracing::info!(
            "Executing query with Vulkan GPU: table={}, rows={}",
            query.table,
            data.row_count
        );

        // Initialize Vulkan device (cached in production)
        let device = VulkanDevice::new().map_err(|e| {
            EngineError::Internal(format!("Failed to initialize Vulkan device: {}", e))
        })?;

        let mut result = data.clone();

        // Apply filter if present (GPU accelerated)
        if let Some(ref filter) = query.filter {
            result = self.execute_filter_predicate_gpu_vulkan(&device, &result, filter)?;
            tracing::info!("Vulkan filter complete: {} rows remaining", result.row_count);
        }

        // Apply offset BEFORE limit (standard SQL order)
        if let Some(offset) = query.offset {
            if offset < result.row_count {
                result = self.execute_offset(&result, offset)?;
                tracing::debug!("After offset: {} rows", result.row_count);
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            if result.row_count > limit {
                result = self.execute_limit(&result, limit)?;
                tracing::debug!("After limit: {} rows", result.row_count);
            }
        }

        // Apply projection LAST
        if let Some(ref projection) = query.projection {
            result = self.execute_projection_by_names(&result, projection)?;
            tracing::debug!("After projection: {} columns", projection.len());
        }

        Ok(result)
    }

    /// Execute filter predicate on GPU
    #[cfg(target_os = "macos")]
    fn execute_filter_predicate_gpu(
        &self,
        device: &orbit_compute::gpu_metal::MetalDevice,
        batch: &ColumnBatch,
        predicate: &crate::storage::FilterPredicate,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::FilterPredicate;
        use orbit_compute::gpu_backend::FilterOp;

        match predicate {
            FilterPredicate::Eq(column, value) => {
                self.execute_filter_by_column_gpu(device, batch, column, FilterOp::Equal, value)
            }
            FilterPredicate::Ne(column, value) => {
                self.execute_filter_by_column_gpu(device, batch, column, FilterOp::NotEqual, value)
            }
            FilterPredicate::Lt(column, value) => {
                self.execute_filter_by_column_gpu(device, batch, column, FilterOp::LessThan, value)
            }
            FilterPredicate::Le(column, value) => {
                self.execute_filter_by_column_gpu(device, batch, column, FilterOp::LessOrEqual, value)
            }
            FilterPredicate::Gt(column, value) => {
                self.execute_filter_by_column_gpu(device, batch, column, FilterOp::GreaterThan, value)
            }
            FilterPredicate::Ge(column, value) => {
                self.execute_filter_by_column_gpu(device, batch, column, FilterOp::GreaterOrEqual, value)
            }
            FilterPredicate::And(predicates) => {
                // Apply each predicate and combine with AND using GPU bitmap operations
                if predicates.is_empty() {
                    return Ok(batch.clone());
                }

                // Execute first predicate
                let mut result = self.execute_filter_predicate_gpu(device, batch, &predicates[0])?;

                // Apply remaining predicates sequentially (simplified approach)
                // Full GPU AND would execute all predicates in parallel and combine masks
                for pred in &predicates[1..] {
                    result = self.execute_filter_predicate_gpu(device, &result, pred)?;
                }

                Ok(result)
            }
            FilterPredicate::Or(_predicates) => {
                // For OR with GPU, we would:
                // 1. Execute each predicate to get separate masks
                // 2. Use bitmap_or to combine masks
                // 3. Compact results
                // For now, fall back to CPU
                tracing::warn!("GPU OR predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
            FilterPredicate::Not(_predicate) => {
                // For NOT with GPU, we would use bitmap_not
                // For now, fall back to CPU
                tracing::warn!("GPU NOT predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
        }
    }

    /// Execute filter by column name using GPU
    #[cfg(target_os = "macos")]
    fn execute_filter_by_column_gpu(
        &self,
        device: &orbit_compute::gpu_metal::MetalDevice,
        batch: &ColumnBatch,
        column_name: &str,
        op: orbit_compute::gpu_backend::FilterOp,
        value: &crate::storage::SqlValue,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::{Column, SqlValue};

        // Find column index
        let column_index = if let Some(ref names) = batch.column_names {
            names
                .iter()
                .position(|name| name == column_name)
                .ok_or_else(|| {
                    EngineError::storage(format!("Column '{}' not found", column_name))
                })?
        } else {
            return Err(EngineError::storage(
                "Cannot filter by name: batch has no column names".to_string(),
            ));
        };

        // Execute GPU filter based on column type
        let mask = match (&batch.columns[column_index], value) {
            (Column::Int32(data), SqlValue::Int32(val)) => {
                device.execute_filter_i32(data, *val, op).map_err(|e| {
                    EngineError::Internal(format!("GPU filter i32 failed: {}", e))
                })?
            }
            (Column::Int64(data), SqlValue::Int64(val)) => {
                device.execute_filter_i64(data, *val, op).map_err(|e| {
                    EngineError::Internal(format!("GPU filter i64 failed: {}", e))
                })?
            }
            (Column::Float64(data), SqlValue::Float64(val)) => {
                device.execute_filter_f64(data, *val, op).map_err(|e| {
                    EngineError::Internal(format!("GPU filter f64 failed: {}", e))
                })?
            }
            _ => {
                return Err(EngineError::storage(format!(
                    "Unsupported column type or value mismatch for GPU filtering"
                )));
            }
        };

        // Count selected rows
        let selected_count = mask.iter().filter(|&&v| v != 0).count();

        tracing::debug!(
            "GPU filter on column '{}': {}/{} rows selected",
            column_name,
            selected_count,
            batch.row_count
        );

        // Build output indices for compaction
        let mut output_indices: Vec<usize> = Vec::with_capacity(selected_count);
        for (i, &m) in mask.iter().enumerate() {
            if m != 0 {
                output_indices.push(i);
            }
        }

        // Compact all columns based on mask
        let new_columns: Vec<Column> = batch
            .columns
            .iter()
            .map(|col| match col {
                Column::Int32(data) => {
                    Column::Int32(output_indices.iter().map(|&i| data[i]).collect())
                }
                Column::Int64(data) => {
                    Column::Int64(output_indices.iter().map(|&i| data[i]).collect())
                }
                Column::Float64(data) => {
                    Column::Float64(output_indices.iter().map(|&i| data[i]).collect())
                }
                Column::String(data) => {
                    Column::String(output_indices.iter().map(|&i| data[i].clone()).collect())
                }
                Column::Bool(data) => {
                    Column::Bool(output_indices.iter().map(|&i| data[i]).collect())
                }
                Column::Int16(data) => {
                    Column::Int16(output_indices.iter().map(|&i| data[i]).collect())
                }
                Column::Float32(data) => {
                    Column::Float32(output_indices.iter().map(|&i| data[i]).collect())
                }
                Column::Binary(data) => {
                    Column::Binary(output_indices.iter().map(|&i| data[i].clone()).collect())
                }
            })
            .collect();

        // Compact null bitmaps
        let new_null_bitmaps: Vec<NullBitmap> = batch
            .null_bitmaps
            .iter()
            .map(|bitmap| {
                let mut new_bitmap = NullBitmap::new_all_valid(selected_count);
                for (new_idx, &old_idx) in output_indices.iter().enumerate() {
                    if bitmap.is_null(old_idx) {
                        new_bitmap.set_null(new_idx);
                    }
                }
                new_bitmap
            })
            .collect();

        Ok(ColumnBatch {
            columns: new_columns,
            null_bitmaps: new_null_bitmaps,
            row_count: selected_count,
            column_names: batch.column_names.clone(),
        })
    }

    /// Execute filter predicate on GPU using CUDA
    #[cfg(all(not(target_os = "macos"), feature = "gpu-cuda"))]
    fn execute_filter_predicate_gpu_cuda(
        &self,
        device: &orbit_compute::gpu_cuda::CudaDevice,
        batch: &ColumnBatch,
        predicate: &crate::storage::FilterPredicate,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::FilterPredicate;
        use orbit_compute::gpu_backend::FilterOp;

        match predicate {
            FilterPredicate::Eq(column, value) => {
                self.execute_filter_by_column_gpu_cuda(device, batch, column, FilterOp::Equal, value)
            }
            FilterPredicate::Ne(column, value) => {
                self.execute_filter_by_column_gpu_cuda(device, batch, column, FilterOp::NotEqual, value)
            }
            FilterPredicate::Lt(column, value) => {
                self.execute_filter_by_column_gpu_cuda(device, batch, column, FilterOp::LessThan, value)
            }
            FilterPredicate::Le(column, value) => {
                self.execute_filter_by_column_gpu_cuda(device, batch, column, FilterOp::LessOrEqual, value)
            }
            FilterPredicate::Gt(column, value) => {
                self.execute_filter_by_column_gpu_cuda(device, batch, column, FilterOp::GreaterThan, value)
            }
            FilterPredicate::Ge(column, value) => {
                self.execute_filter_by_column_gpu_cuda(device, batch, column, FilterOp::GreaterOrEqual, value)
            }
            FilterPredicate::And(predicates) => {
                if predicates.is_empty() {
                    return Ok(batch.clone());
                }
                let mut result = self.execute_filter_predicate_gpu_cuda(device, batch, &predicates[0])?;
                for pred in &predicates[1..] {
                    result = self.execute_filter_predicate_gpu_cuda(device, &result, pred)?;
                }
                Ok(result)
            }
            FilterPredicate::Or(_predicates) => {
                tracing::warn!("CUDA OR predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
            FilterPredicate::Not(_predicate) => {
                tracing::warn!("CUDA NOT predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
        }
    }

    /// Execute filter by column name using CUDA GPU
    #[cfg(all(not(target_os = "macos"), feature = "gpu-cuda"))]
    fn execute_filter_by_column_gpu_cuda(
        &self,
        device: &orbit_compute::gpu_cuda::CudaDevice,
        batch: &ColumnBatch,
        column_name: &str,
        op: orbit_compute::gpu_backend::FilterOp,
        value: &crate::storage::SqlValue,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::{Column, SqlValue};

        // Find column index
        let column_index = if let Some(ref names) = batch.column_names {
            names
                .iter()
                .position(|name| name == column_name)
                .ok_or_else(|| {
                    EngineError::storage(format!("Column '{}' not found", column_name))
                })?
        } else {
            return Err(EngineError::storage(
                "Cannot filter by name: batch has no column names".to_string(),
            ));
        };

        // Execute CUDA filter based on column type
        let mask = match (&batch.columns[column_index], value) {
            (Column::Int32(data), SqlValue::Int32(v)) => {
                device.execute_filter_i32(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("CUDA filter failed: {}", e))
                })?
            }
            (Column::Int64(data), SqlValue::Int64(v)) => {
                device.execute_filter_i64(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("CUDA filter failed: {}", e))
                })?
            }
            (Column::Float64(data), SqlValue::Float64(v)) => {
                device.execute_filter_f64(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("CUDA filter failed: {}", e))
                })?
            }
            _ => {
                // Fall back to CPU for unsupported types
                tracing::warn!("CUDA filter not supported for this column type, using CPU fallback");
                let comparison_op = match op {
                    orbit_compute::gpu_backend::FilterOp::Equal => ComparisonOp::Equal,
                    orbit_compute::gpu_backend::FilterOp::NotEqual => ComparisonOp::NotEqual,
                    orbit_compute::gpu_backend::FilterOp::LessThan => ComparisonOp::LessThan,
                    orbit_compute::gpu_backend::FilterOp::LessOrEqual => ComparisonOp::LessThanOrEqual,
                    orbit_compute::gpu_backend::FilterOp::GreaterThan => ComparisonOp::GreaterThan,
                    orbit_compute::gpu_backend::FilterOp::GreaterOrEqual => ComparisonOp::GreaterThanOrEqual,
                };
                return self.execute_filter_by_column(batch, column_name, comparison_op, value);
            }
        };

        // Compact results using mask (similar to Metal implementation)
        self.compact_batch_with_mask(batch, &mask)
    }

    /// Execute filter predicate on GPU using ROCm
    #[cfg(all(unix, not(target_os = "macos"), feature = "gpu-rocm"))]
    fn execute_filter_predicate_gpu_rocm(
        &self,
        device: &orbit_compute::gpu_rocm::RocmDevice,
        batch: &ColumnBatch,
        predicate: &crate::storage::FilterPredicate,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::FilterPredicate;
        use orbit_compute::gpu_backend::FilterOp;

        match predicate {
            FilterPredicate::Eq(column, value) => {
                self.execute_filter_by_column_gpu_rocm(device, batch, column, FilterOp::Equal, value)
            }
            FilterPredicate::Ne(column, value) => {
                self.execute_filter_by_column_gpu_rocm(device, batch, column, FilterOp::NotEqual, value)
            }
            FilterPredicate::Lt(column, value) => {
                self.execute_filter_by_column_gpu_rocm(device, batch, column, FilterOp::LessThan, value)
            }
            FilterPredicate::Le(column, value) => {
                self.execute_filter_by_column_gpu_rocm(device, batch, column, FilterOp::LessOrEqual, value)
            }
            FilterPredicate::Gt(column, value) => {
                self.execute_filter_by_column_gpu_rocm(device, batch, column, FilterOp::GreaterThan, value)
            }
            FilterPredicate::Ge(column, value) => {
                self.execute_filter_by_column_gpu_rocm(device, batch, column, FilterOp::GreaterOrEqual, value)
            }
            FilterPredicate::And(predicates) => {
                if predicates.is_empty() {
                    return Ok(batch.clone());
                }
                let mut result = self.execute_filter_predicate_gpu_rocm(device, batch, &predicates[0])?;
                for pred in &predicates[1..] {
                    result = self.execute_filter_predicate_gpu_rocm(device, &result, pred)?;
                }
                Ok(result)
            }
            FilterPredicate::Or(_predicates) => {
                tracing::warn!("ROCm OR predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
            FilterPredicate::Not(_predicate) => {
                tracing::warn!("ROCm NOT predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
        }
    }

    /// Execute filter by column name using ROCm GPU
    #[cfg(all(unix, not(target_os = "macos"), feature = "gpu-rocm"))]
    fn execute_filter_by_column_gpu_rocm(
        &self,
        device: &orbit_compute::gpu_rocm::RocmDevice,
        batch: &ColumnBatch,
        column_name: &str,
        op: orbit_compute::gpu_backend::FilterOp,
        value: &crate::storage::SqlValue,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::{Column, SqlValue};

        // Find column index
        let column_index = if let Some(ref names) = batch.column_names {
            names
                .iter()
                .position(|name| name == column_name)
                .ok_or_else(|| {
                    EngineError::storage(format!("Column '{}' not found", column_name))
                })?
        } else {
            return Err(EngineError::storage(
                "Cannot filter by name: batch has no column names".to_string(),
            ));
        };

        // Execute ROCm filter based on column type
        let mask = match (&batch.columns[column_index], value) {
            (Column::Int32(data), SqlValue::Int32(v)) => {
                device.execute_filter_i32(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("ROCm filter failed: {}", e))
                })?
            }
            (Column::Int64(data), SqlValue::Int64(v)) => {
                device.execute_filter_i64(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("ROCm filter failed: {}", e))
                })?
            }
            (Column::Float64(data), SqlValue::Float64(v)) => {
                device.execute_filter_f64(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("ROCm filter failed: {}", e))
                })?
            }
            _ => {
                // Fall back to CPU for unsupported types
                tracing::warn!("ROCm filter not supported for this column type, using CPU fallback");
                let comparison_op = match op {
                    orbit_compute::gpu_backend::FilterOp::Equal => ComparisonOp::Equal,
                    orbit_compute::gpu_backend::FilterOp::NotEqual => ComparisonOp::NotEqual,
                    orbit_compute::gpu_backend::FilterOp::LessThan => ComparisonOp::LessThan,
                    orbit_compute::gpu_backend::FilterOp::LessOrEqual => ComparisonOp::LessThanOrEqual,
                    orbit_compute::gpu_backend::FilterOp::GreaterThan => ComparisonOp::GreaterThan,
                    orbit_compute::gpu_backend::FilterOp::GreaterOrEqual => ComparisonOp::GreaterThanOrEqual,
                };
                return self.execute_filter_by_column(batch, column_name, comparison_op, value);
            }
        };

        // Compact results using mask
        self.compact_batch_with_mask(batch, &mask)
    }

    /// Execute filter predicate on GPU using Vulkan
    #[cfg(feature = "gpu-vulkan")]
    fn execute_filter_predicate_gpu_vulkan(
        &self,
        device: &orbit_compute::gpu_vulkan::VulkanDevice,
        batch: &ColumnBatch,
        predicate: &crate::storage::FilterPredicate,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::FilterPredicate;
        use orbit_compute::gpu_backend::FilterOp;

        match predicate {
            FilterPredicate::Eq(column, value) => {
                self.execute_filter_by_column_gpu_vulkan(device, batch, column, FilterOp::Equal, value)
            }
            FilterPredicate::Ne(column, value) => {
                self.execute_filter_by_column_gpu_vulkan(device, batch, column, FilterOp::NotEqual, value)
            }
            FilterPredicate::Lt(column, value) => {
                self.execute_filter_by_column_gpu_vulkan(device, batch, column, FilterOp::LessThan, value)
            }
            FilterPredicate::Le(column, value) => {
                self.execute_filter_by_column_gpu_vulkan(device, batch, column, FilterOp::LessOrEqual, value)
            }
            FilterPredicate::Gt(column, value) => {
                self.execute_filter_by_column_gpu_vulkan(device, batch, column, FilterOp::GreaterThan, value)
            }
            FilterPredicate::Ge(column, value) => {
                self.execute_filter_by_column_gpu_vulkan(device, batch, column, FilterOp::GreaterOrEqual, value)
            }
            FilterPredicate::And(predicates) => {
                if predicates.is_empty() {
                    return Ok(batch.clone());
                }
                let mut result = self.execute_filter_predicate_gpu_vulkan(device, batch, &predicates[0])?;
                for pred in &predicates[1..] {
                    result = self.execute_filter_predicate_gpu_vulkan(device, &result, pred)?;
                }
                Ok(result)
            }
            FilterPredicate::Or(_predicates) => {
                tracing::warn!("Vulkan OR predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
            FilterPredicate::Not(_predicate) => {
                tracing::warn!("Vulkan NOT predicate not fully implemented, using CPU fallback");
                self.execute_filter_predicate(batch, predicate)
            }
        }
    }

    /// Execute filter by column name using Vulkan GPU
    #[cfg(feature = "gpu-vulkan")]
    fn execute_filter_by_column_gpu_vulkan(
        &self,
        device: &orbit_compute::gpu_vulkan::VulkanDevice,
        batch: &ColumnBatch,
        column_name: &str,
        op: orbit_compute::gpu_backend::FilterOp,
        value: &crate::storage::SqlValue,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::{Column, SqlValue};

        // Find column index
        let column_index = if let Some(ref names) = batch.column_names {
            names
                .iter()
                .position(|name| name == column_name)
                .ok_or_else(|| {
                    EngineError::storage(format!("Column '{}' not found", column_name))
                })?
        } else {
            return Err(EngineError::storage(
                "Cannot filter by name: batch has no column names".to_string(),
            ));
        };

        // Execute Vulkan filter based on column type
        let mask = match (&batch.columns[column_index], value) {
            (Column::Int32(data), SqlValue::Int32(v)) => {
                device.execute_filter_i32(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("Vulkan filter failed: {}", e))
                })?
            }
            (Column::Int64(data), SqlValue::Int64(v)) => {
                device.execute_filter_i64(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("Vulkan filter failed: {}", e))
                })?
            }
            (Column::Float64(data), SqlValue::Float64(v)) => {
                device.execute_filter_f64(data, *v, op).map_err(|e| {
                    EngineError::Internal(format!("Vulkan filter failed: {}", e))
                })?
            }
            _ => {
                // Fall back to CPU for unsupported types
                tracing::warn!("Vulkan filter not supported for this column type, using CPU fallback");
                let comparison_op = match op {
                    orbit_compute::gpu_backend::FilterOp::Equal => ComparisonOp::Equal,
                    orbit_compute::gpu_backend::FilterOp::NotEqual => ComparisonOp::NotEqual,
                    orbit_compute::gpu_backend::FilterOp::LessThan => ComparisonOp::LessThan,
                    orbit_compute::gpu_backend::FilterOp::LessOrEqual => ComparisonOp::LessThanOrEqual,
                    orbit_compute::gpu_backend::FilterOp::GreaterThan => ComparisonOp::GreaterThan,
                    orbit_compute::gpu_backend::FilterOp::GreaterOrEqual => ComparisonOp::GreaterThanOrEqual,
                };
                return self.execute_filter_by_column(batch, column_name, comparison_op, value);
            }
        };

        // Compact results using mask
        self.compact_batch_with_mask(batch, &mask)
    }

    /// Execute a filter predicate recursively
    fn execute_filter_predicate(
        &self,
        batch: &ColumnBatch,
        predicate: &crate::storage::FilterPredicate,
    ) -> EngineResult<ColumnBatch> {
        use crate::storage::FilterPredicate;

        match predicate {
            FilterPredicate::Eq(column, value) => {
                self.execute_filter_by_column(batch, column, ComparisonOp::Equal, value)
            }
            FilterPredicate::Ne(column, value) => {
                self.execute_filter_by_column(batch, column, ComparisonOp::NotEqual, value)
            }
            FilterPredicate::Lt(column, value) => {
                self.execute_filter_by_column(batch, column, ComparisonOp::LessThan, value)
            }
            FilterPredicate::Le(column, value) => {
                self.execute_filter_by_column(batch, column, ComparisonOp::LessThanOrEqual, value)
            }
            FilterPredicate::Gt(column, value) => {
                self.execute_filter_by_column(batch, column, ComparisonOp::GreaterThan, value)
            }
            FilterPredicate::Ge(column, value) => {
                self.execute_filter_by_column(batch, column, ComparisonOp::GreaterThanOrEqual, value)
            }
            FilterPredicate::And(predicates) => {
                // Apply filters sequentially (AND logic)
                let mut result = batch.clone();
                for pred in predicates {
                    result = self.execute_filter_predicate(&result, pred)?;
                }
                Ok(result)
            }
            FilterPredicate::Or(predicates) => {
                // For OR, we need to collect results from each predicate and merge
                // For now, just apply the first predicate (simplified implementation)
                // Full OR implementation would require collecting and merging row indices
                if let Some(first_pred) = predicates.first() {
                    self.execute_filter_predicate(batch, first_pred)
                } else {
                    Ok(batch.clone())
                }
            }
            FilterPredicate::Not(predicate) => {
                // For NOT, we would need to invert the selection
                // For now, just pass through (simplified implementation)
                tracing::warn!("NOT predicate not fully implemented, passing through");
                self.execute_filter_predicate(batch, predicate)
            }
        }
    }

    /// Execute filter by column name (helper to map column name to index)
    fn execute_filter_by_column(
        &self,
        batch: &ColumnBatch,
        column_name: &str,
        op: ComparisonOp,
        value: &SqlValue,
    ) -> EngineResult<ColumnBatch> {
        // Find column index by name
        let column_index = if let Some(ref names) = batch.column_names {
            names.iter().position(|name| name == column_name).ok_or_else(|| {
                EngineError::storage(format!("Column '{}' not found", column_name))
            })?
        } else {
            return Err(EngineError::storage(
                "Cannot filter by name: batch has no column names".to_string(),
            ));
        };

        // Execute filter using existing SIMD-optimized method
        self.execute_filter(batch, column_index, op, value.clone())
    }

    /// Execute projection by column names
    fn execute_projection_by_names(
        &self,
        batch: &ColumnBatch,
        column_names: &[String],
    ) -> EngineResult<ColumnBatch> {
        if let Some(ref names) = batch.column_names {
            // Map column names to indices
            let mut indices = Vec::new();
            for col_name in column_names {
                let idx = names.iter().position(|name| name == col_name).ok_or_else(|| {
                    EngineError::storage(format!("Column '{}' not found", col_name))
                })?;
                indices.push(idx);
            }

            // Execute projection using existing method
            self.execute_projection(batch, &indices)
        } else {
            // If no column names, assume sequential indices
            let indices: Vec<usize> = (0..column_names.len()).collect();
            self.execute_projection(batch, &indices)
        }
    }

    /// Execute query using standard (non-accelerated) execution
    ///
    /// This method provides a baseline execution path without hardware acceleration.
    #[allow(dead_code)]
    async fn execute_standard(
        &self,
        plan: &crate::query::ExecutionPlan,
        data: &ColumnBatch,
    ) -> EngineResult<ColumnBatch> {
        // For now, standard execution is the same as SIMD execution
        // but without SIMD optimizations enabled
        tracing::debug!("Executing with standard (non-accelerated) path");
        self.execute_cpu_simd(plan, data).await
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
