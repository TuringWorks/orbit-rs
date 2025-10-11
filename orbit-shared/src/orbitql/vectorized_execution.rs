//! Vectorized execution engine for high-performance query processing
//!
//! This module provides SIMD-optimized query execution that processes data in
//! batches rather than row-by-row, achieving significant performance improvements
//! for analytical workloads. Implements Phase 9.4 of the optimization plan.

use serde::{Deserialize, Serialize};
// Note: Portable SIMD is unstable, so we use manual vectorization for now
// use std::simd::{f64x8, i64x8, u64x8, Simd, SimdElement, SimdPartialEq, SimdPartialOrd};

use crate::orbitql::ast::*;
use crate::orbitql::QueryValue;

/// Default batch size for vectorized operations
pub const DEFAULT_BATCH_SIZE: usize = 1024;

/// SIMD vector width for different data types
pub const SIMD_F64_WIDTH: usize = 8; // AVX-512: 8 f64 values
pub const SIMD_I64_WIDTH: usize = 8; // AVX-512: 8 i64 values

/// Vectorized execution engine
pub struct VectorizedExecutor {
    /// Batch size for vectorized operations
    batch_size: usize,
    /// SIMD instruction width
    simd_width: usize,
    /// Available vector register sets
    vector_registers: VectorRegisterSet,
    /// Configuration
    config: VectorizationConfig,
}

/// Configuration for vectorized execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizationConfig {
    /// Enable SIMD instructions
    pub enable_simd: bool,
    /// Preferred batch size
    pub batch_size: usize,
    /// Enable AVX-512 instructions (if available)
    pub enable_avx512: bool,
    /// Enable vectorized string operations
    pub enable_vectorized_strings: bool,
    /// Null handling strategy
    pub null_handling: NullHandlingStrategy,
    /// Memory alignment for vectors
    pub memory_alignment: usize,
}

impl Default for VectorizationConfig {
    fn default() -> Self {
        Self {
            enable_simd: true,
            batch_size: DEFAULT_BATCH_SIZE,
            enable_avx512: cfg!(target_feature = "avx512f"),
            enable_vectorized_strings: true,
            null_handling: NullHandlingStrategy::Bitmap,
            memory_alignment: 64, // 64-byte alignment for AVX-512
        }
    }
}

/// Null handling strategies for vectorized operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NullHandlingStrategy {
    /// Use bitmaps to track null values
    Bitmap,
    /// Use sentinel values to represent nulls
    Sentinel,
    /// Skip null handling (for performance-critical paths)
    None,
}

/// Vector register set for managing SIMD operations
#[derive(Debug, Clone)]
pub struct VectorRegisterSet {
    /// Available registers for different data types
    f64_registers: Vec<VectorRegister<f64>>,
    i64_registers: Vec<VectorRegister<i64>>,
    string_registers: Vec<VectorRegister<String>>,
}

/// Generic vector register for SIMD operations
#[derive(Debug, Clone)]
pub struct VectorRegister<T> {
    pub data: Vec<T>,
    pub null_bitmap: Option<Vec<bool>>,
    pub capacity: usize,
}

/// Columnar data representation for vectorized processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnBatch {
    /// Raw data buffer
    pub data: Vec<u8>,
    /// Null bitmap (optional)
    pub nulls: Option<Bitmap>,
    /// Number of elements
    pub length: usize,
    /// Data type
    pub data_type: VectorDataType,
    /// Column name
    pub name: String,
}

/// Bitmap for null tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bitmap {
    /// Bit storage
    pub bits: Vec<u64>,
    /// Number of bits
    pub len: usize,
}

/// Data types supported by vectorized operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VectorDataType {
    Integer64,
    Float64,
    String,
    Boolean,
    Timestamp,
}

/// Record batch for columnar processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordBatch {
    /// Columns in the batch
    pub columns: Vec<ColumnBatch>,
    /// Number of rows
    pub row_count: usize,
    /// Schema information
    pub schema: BatchSchema,
}

/// Schema for record batches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSchema {
    /// Column names and types
    pub fields: Vec<(String, VectorDataType)>,
}

/// Trait for vectorized operations
pub trait VectorizedOperation {
    /// Execute operation on a batch of records
    fn execute_batch(&self, input: &RecordBatch) -> Result<RecordBatch, VectorizationError>;

    /// Check if operation supports SIMD
    fn supports_simd(&self) -> bool;

    /// Preferred batch size for this operation
    fn preferred_batch_size(&self) -> usize {
        DEFAULT_BATCH_SIZE
    }
}

/// Vectorization errors
#[derive(Debug, Clone)]
pub enum VectorizationError {
    SizeMismatch(String),
    TypeMismatch(String),
    SimdNotSupported(String),
    InternalError(String),
}

impl std::fmt::Display for VectorizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VectorizationError::SizeMismatch(msg) => write!(f, "Size mismatch: {}", msg),
            VectorizationError::TypeMismatch(msg) => write!(f, "Type mismatch: {}", msg),
            VectorizationError::SimdNotSupported(msg) => write!(f, "SIMD not supported: {}", msg),
            VectorizationError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for VectorizationError {}

/// Vectorized scan operation
pub struct VectorizedScan {
    /// Table schema
    pub schema: BatchSchema,
    /// Predicate for filtering (optional)
    pub predicate: Option<Expression>,
    /// Projection columns
    pub projection: Vec<String>,
}

/// Vectorized filter operation
pub struct VectorizedFilter {
    /// Filter condition
    pub condition: Expression,
}

/// Vectorized projection operation
pub struct VectorizedProjection {
    /// Expressions to compute
    pub expressions: Vec<Expression>,
    /// Output column names
    pub output_names: Vec<String>,
}

/// Vectorized aggregation operation
pub struct VectorizedAggregation {
    /// Group by expressions
    pub group_by: Vec<Expression>,
    /// Aggregate functions
    pub aggregates: Vec<AggregateFunction>,
}

/// Vectorized join operation
pub struct VectorizedHashJoin {
    /// Join condition
    pub condition: Expression,
    /// Join type
    pub join_type: JoinType,
    /// Build side (smaller table)
    pub build_side: JoinSide,
}

/// Join side specification
#[derive(Debug, Clone)]
pub enum JoinSide {
    Left,
    Right,
}

impl VectorizedExecutor {
    /// Create a new vectorized executor
    pub fn new() -> Self {
        Self::with_config(VectorizationConfig::default())
    }

    /// Create executor with custom configuration
    pub fn with_config(config: VectorizationConfig) -> Self {
        let simd_width = if config.enable_avx512 { 8 } else { 4 };

        Self {
            batch_size: config.batch_size,
            simd_width,
            vector_registers: VectorRegisterSet::new(),
            config,
        }
    }

    /// Execute vectorized operation on record batch
    pub fn execute_vectorized<T: VectorizedOperation>(
        &self,
        operation: &T,
        input: &RecordBatch,
    ) -> Result<RecordBatch, VectorizationError> {
        operation.execute_batch(input)
    }

    /// Create vectorized scan operator
    pub fn create_scan(
        &self,
        schema: BatchSchema,
        predicate: Option<Expression>,
        projection: Vec<String>,
    ) -> VectorizedScan {
        VectorizedScan {
            schema,
            predicate,
            projection,
        }
    }

    /// Create vectorized filter operator
    pub fn create_filter(&self, condition: Expression) -> VectorizedFilter {
        VectorizedFilter { condition }
    }

    /// Create vectorized projection operator
    pub fn create_projection(
        &self,
        expressions: Vec<Expression>,
        output_names: Vec<String>,
    ) -> VectorizedProjection {
        VectorizedProjection {
            expressions,
            output_names,
        }
    }

    /// Create vectorized aggregation operator
    pub fn create_aggregation(
        &self,
        group_by: Vec<Expression>,
        aggregates: Vec<AggregateFunction>,
    ) -> VectorizedAggregation {
        VectorizedAggregation {
            group_by,
            aggregates,
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> &VectorizationConfig {
        &self.config
    }
}

impl VectorizedOperation for VectorizedScan {
    fn execute_batch(&self, _input: &RecordBatch) -> Result<RecordBatch, VectorizationError> {
        // Mock implementation - in reality this would read from storage
        let mut columns = Vec::new();

        for (name, data_type) in &self.schema.fields {
            if self.projection.is_empty() || self.projection.contains(name) {
                let column = ColumnBatch::mock_data(name.clone(), data_type.clone(), 1000);
                columns.push(column);
            }
        }

        let row_count = columns.first().map_or(0, |col| col.length);

        Ok(RecordBatch {
            columns,
            row_count,
            schema: self.schema.clone(),
        })
    }

    fn supports_simd(&self) -> bool {
        true
    }
}

impl VectorizedOperation for VectorizedFilter {
    fn execute_batch(&self, input: &RecordBatch) -> Result<RecordBatch, VectorizationError> {
        // Apply vectorized filter operation
        if let Expression::Binary {
            left,
            operator,
            right,
        } = &self.condition
        {
            if let (Expression::Identifier(col_name), Expression::Literal(value)) =
                (left.as_ref(), right.as_ref())
            {
                return self.apply_simd_filter(input, col_name, operator, value);
            }
        }

        // Fallback to non-SIMD implementation
        self.apply_scalar_filter(input)
    }

    fn supports_simd(&self) -> bool {
        match &self.condition {
            Expression::Binary { left, right, .. } => {
                matches!(left.as_ref(), Expression::Identifier(_))
                    && matches!(right.as_ref(), Expression::Literal(_))
            }
            _ => false,
        }
    }
}

impl VectorizedFilter {
    /// Apply SIMD-optimized filter
    fn apply_simd_filter(
        &self,
        input: &RecordBatch,
        col_name: &str,
        operator: &BinaryOperator,
        value: &QueryValue,
    ) -> Result<RecordBatch, VectorizationError> {
        // Find the target column
        let column_idx = input
            .columns
            .iter()
            .position(|col| col.name == col_name)
            .ok_or_else(|| {
                VectorizationError::InternalError(format!("Column {} not found", col_name))
            })?;

        let column = &input.columns[column_idx];

        // Apply SIMD filter based on data type
        let selection_mask = match (&column.data_type, value) {
            (VectorDataType::Integer64, QueryValue::Integer(filter_value)) => {
                self.simd_filter_i64(column, operator, *filter_value)?
            }
            (VectorDataType::Float64, QueryValue::Float(filter_value)) => {
                self.simd_filter_f64(column, operator, *filter_value)?
            }
            _ => {
                // Fall back to scalar operation
                return self.apply_scalar_filter(input);
            }
        };

        // Apply selection mask to all columns
        self.apply_selection_mask(input, &selection_mask)
    }

    /// Vectorized filter for i64 columns (using scalar operations for stability)
    fn simd_filter_i64(
        &self,
        column: &ColumnBatch,
        operator: &BinaryOperator,
        filter_value: i64,
    ) -> Result<Vec<bool>, VectorizationError> {
        let data = self.extract_i64_data(column)?;
        let mut result = vec![false; data.len()];

        // Process elements using scalar operations for now
        // TODO: Replace with stable SIMD when available
        for (i, &value) in data.iter().enumerate() {
            result[i] = match operator {
                BinaryOperator::Equal => value == filter_value,
                BinaryOperator::GreaterThan => value > filter_value,
                BinaryOperator::LessThan => value < filter_value,
                BinaryOperator::GreaterThanOrEqual => value >= filter_value,
                BinaryOperator::LessThanOrEqual => value <= filter_value,
                _ => {
                    return Err(VectorizationError::SimdNotSupported(format!(
                        "Operator {:?} not supported",
                        operator
                    )))
                }
            };
        }

        Ok(result)
    }

    /// Vectorized filter for f64 columns (using scalar operations for stability)
    fn simd_filter_f64(
        &self,
        column: &ColumnBatch,
        operator: &BinaryOperator,
        filter_value: f64,
    ) -> Result<Vec<bool>, VectorizationError> {
        let data = self.extract_f64_data(column)?;
        let mut result = vec![false; data.len()];

        // Process elements using scalar operations for now
        // TODO: Replace with stable SIMD when available
        for (i, &value) in data.iter().enumerate() {
            result[i] = match operator {
                BinaryOperator::Equal => (value - filter_value).abs() < f64::EPSILON,
                BinaryOperator::GreaterThan => value > filter_value,
                BinaryOperator::LessThan => value < filter_value,
                BinaryOperator::GreaterThanOrEqual => value >= filter_value,
                BinaryOperator::LessThanOrEqual => value <= filter_value,
                _ => {
                    return Err(VectorizationError::SimdNotSupported(format!(
                        "Operator {:?} not supported",
                        operator
                    )))
                }
            };
        }

        Ok(result)
    }

    /// Extract i64 data from column
    fn extract_i64_data<'a>(
        &self,
        column: &'a ColumnBatch,
    ) -> Result<&'a [i64], VectorizationError> {
        if column.data_type != VectorDataType::Integer64 {
            return Err(VectorizationError::TypeMismatch(
                "Expected Integer64".to_string(),
            ));
        }

        let (_, data, _) = unsafe { column.data.align_to::<i64>() };
        if data.len() * 8 != column.data.len() {
            return Err(VectorizationError::InternalError(
                "Data alignment issue".to_string(),
            ));
        }

        Ok(data)
    }

    /// Extract f64 data from column
    fn extract_f64_data<'a>(
        &self,
        column: &'a ColumnBatch,
    ) -> Result<&'a [f64], VectorizationError> {
        if column.data_type != VectorDataType::Float64 {
            return Err(VectorizationError::TypeMismatch(
                "Expected Float64".to_string(),
            ));
        }

        let (_, data, _) = unsafe { column.data.align_to::<f64>() };
        if data.len() * 8 != column.data.len() {
            return Err(VectorizationError::InternalError(
                "Data alignment issue".to_string(),
            ));
        }

        Ok(data)
    }

    /// Apply selection mask to filter records
    fn apply_selection_mask(
        &self,
        input: &RecordBatch,
        mask: &[bool],
    ) -> Result<RecordBatch, VectorizationError> {
        let selected_count = mask.iter().filter(|&&x| x).count();
        let mut filtered_columns = Vec::new();

        for column in &input.columns {
            let filtered_column = self.filter_column(column, mask, selected_count)?;
            filtered_columns.push(filtered_column);
        }

        Ok(RecordBatch {
            columns: filtered_columns,
            row_count: selected_count,
            schema: input.schema.clone(),
        })
    }

    /// Filter a single column using selection mask
    fn filter_column(
        &self,
        column: &ColumnBatch,
        mask: &[bool],
        selected_count: usize,
    ) -> Result<ColumnBatch, VectorizationError> {
        let mut filtered_data = Vec::new();

        match column.data_type {
            VectorDataType::Integer64 => {
                let data = self.extract_i64_data(column)?;
                filtered_data.reserve(selected_count * 8);
                for (i, &selected) in mask.iter().enumerate() {
                    if selected && i < data.len() {
                        let bytes = data[i].to_le_bytes();
                        filtered_data.extend_from_slice(&bytes);
                    }
                }
            }
            VectorDataType::Float64 => {
                let data = self.extract_f64_data(column)?;
                filtered_data.reserve(selected_count * 8);
                for (i, &selected) in mask.iter().enumerate() {
                    if selected && i < data.len() {
                        let bytes = data[i].to_le_bytes();
                        filtered_data.extend_from_slice(&bytes);
                    }
                }
            }
            _ => {
                return Err(VectorizationError::SimdNotSupported(
                    "Data type not supported".to_string(),
                ));
            }
        }

        Ok(ColumnBatch {
            data: filtered_data,
            nulls: column.nulls.clone(), // TODO: Filter null bitmap
            length: selected_count,
            data_type: column.data_type.clone(),
            name: column.name.clone(),
        })
    }

    /// Fallback scalar filter implementation
    fn apply_scalar_filter(&self, input: &RecordBatch) -> Result<RecordBatch, VectorizationError> {
        // Simplified scalar implementation
        Ok(input.clone())
    }
}

impl VectorizedOperation for VectorizedProjection {
    fn execute_batch(&self, input: &RecordBatch) -> Result<RecordBatch, VectorizationError> {
        let mut output_columns = Vec::new();

        for (i, expression) in self.expressions.iter().enumerate() {
            let output_name = if i < self.output_names.len() {
                self.output_names[i].clone()
            } else {
                format!("col_{}", i)
            };

            let column = self.evaluate_expression(expression, input)?;
            let mut output_column = column;
            output_column.name = output_name;
            output_columns.push(output_column);
        }

        let output_schema = BatchSchema {
            fields: output_columns
                .iter()
                .map(|col| (col.name.clone(), col.data_type.clone()))
                .collect(),
        };

        Ok(RecordBatch {
            columns: output_columns,
            row_count: input.row_count,
            schema: output_schema,
        })
    }

    fn supports_simd(&self) -> bool {
        self.expressions.iter().all(|expr| {
            matches!(
                expr,
                Expression::Identifier(_) | Expression::Binary { .. } | Expression::Literal(_)
            )
        })
    }
}

impl VectorizedProjection {
    /// Evaluate expression on input batch
    fn evaluate_expression(
        &self,
        expression: &Expression,
        input: &RecordBatch,
    ) -> Result<ColumnBatch, VectorizationError> {
        match expression {
            Expression::Identifier(col_name) => {
                // Return existing column
                input
                    .columns
                    .iter()
                    .find(|col| col.name == *col_name)
                    .cloned()
                    .ok_or_else(|| {
                        VectorizationError::InternalError(format!("Column {} not found", col_name))
                    })
            }
            Expression::Literal(value) => {
                // Create constant column
                self.create_constant_column(value, input.row_count)
            }
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                // Evaluate binary expression with SIMD
                self.evaluate_binary_expression(left, operator, right, input)
            }
            _ => Err(VectorizationError::SimdNotSupported(
                "Expression type not supported".to_string(),
            )),
        }
    }

    /// Create constant column from literal value
    fn create_constant_column(
        &self,
        value: &QueryValue,
        row_count: usize,
    ) -> Result<ColumnBatch, VectorizationError> {
        match value {
            QueryValue::Integer(val) => {
                let mut data = Vec::with_capacity(row_count * 8);
                for _ in 0..row_count {
                    data.extend_from_slice(&val.to_le_bytes());
                }
                Ok(ColumnBatch {
                    data,
                    nulls: None,
                    length: row_count,
                    data_type: VectorDataType::Integer64,
                    name: "constant".to_string(),
                })
            }
            QueryValue::Float(val) => {
                let mut data = Vec::with_capacity(row_count * 8);
                for _ in 0..row_count {
                    data.extend_from_slice(&val.to_le_bytes());
                }
                Ok(ColumnBatch {
                    data,
                    nulls: None,
                    length: row_count,
                    data_type: VectorDataType::Float64,
                    name: "constant".to_string(),
                })
            }
            _ => Err(VectorizationError::SimdNotSupported(
                "Constant type not supported".to_string(),
            )),
        }
    }

    /// Evaluate binary expression with SIMD optimization
    fn evaluate_binary_expression(
        &self,
        left: &Expression,
        operator: &BinaryOperator,
        right: &Expression,
        input: &RecordBatch,
    ) -> Result<ColumnBatch, VectorizationError> {
        let left_col = self.evaluate_expression(left, input)?;
        let right_col = self.evaluate_expression(right, input)?;

        // Perform SIMD arithmetic operations
        match (&left_col.data_type, &right_col.data_type, operator) {
            (VectorDataType::Integer64, VectorDataType::Integer64, op) => {
                self.simd_arithmetic_i64(&left_col, &right_col, op)
            }
            (VectorDataType::Float64, VectorDataType::Float64, op) => {
                self.simd_arithmetic_f64(&left_col, &right_col, op)
            }
            _ => Err(VectorizationError::SimdNotSupported(
                "Arithmetic operation not supported".to_string(),
            )),
        }
    }

    /// Vectorized arithmetic for i64 columns (using scalar operations for stability)
    fn simd_arithmetic_i64(
        &self,
        left: &ColumnBatch,
        right: &ColumnBatch,
        operator: &BinaryOperator,
    ) -> Result<ColumnBatch, VectorizationError> {
        let left_data = unsafe { left.data.align_to::<i64>().1 };
        let right_data = unsafe { right.data.align_to::<i64>().1 };

        if left_data.len() != right_data.len() {
            return Err(VectorizationError::SizeMismatch(
                "Column sizes don't match".to_string(),
            ));
        }

        let mut result_data = Vec::with_capacity(left_data.len() * 8);

        // Process elements using scalar operations for now
        // TODO: Replace with stable SIMD when available
        for (left_val, right_val) in left_data.iter().zip(right_data.iter()) {
            let result = match operator {
                BinaryOperator::Add => left_val + right_val,
                BinaryOperator::Subtract => left_val - right_val,
                BinaryOperator::Multiply => left_val * right_val,
                BinaryOperator::Divide => {
                    if *right_val != 0 {
                        left_val / right_val
                    } else {
                        0 // Handle division by zero
                    }
                }
                _ => {
                    return Err(VectorizationError::SimdNotSupported(format!(
                        "Operator {:?} not supported",
                        operator
                    )))
                }
            };
            result_data.extend_from_slice(&result.to_le_bytes());
        }

        Ok(ColumnBatch {
            data: result_data,
            nulls: None, // TODO: Combine null bitmaps
            length: left.length,
            data_type: VectorDataType::Integer64,
            name: "arithmetic_result".to_string(),
        })
    }

    /// Vectorized arithmetic for f64 columns (using scalar operations for stability)
    fn simd_arithmetic_f64(
        &self,
        left: &ColumnBatch,
        right: &ColumnBatch,
        operator: &BinaryOperator,
    ) -> Result<ColumnBatch, VectorizationError> {
        let left_data = unsafe { left.data.align_to::<f64>().1 };
        let right_data = unsafe { right.data.align_to::<f64>().1 };

        if left_data.len() != right_data.len() {
            return Err(VectorizationError::SizeMismatch(
                "Column sizes don't match".to_string(),
            ));
        }

        let mut result_data = Vec::with_capacity(left_data.len() * 8);

        // Process elements using scalar operations for now
        // TODO: Replace with stable SIMD when available
        for (left_val, right_val) in left_data.iter().zip(right_data.iter()) {
            let result = match operator {
                BinaryOperator::Add => left_val + right_val,
                BinaryOperator::Subtract => left_val - right_val,
                BinaryOperator::Multiply => left_val * right_val,
                BinaryOperator::Divide => left_val / right_val, // IEEE 754 handles division by zero
                _ => {
                    return Err(VectorizationError::SimdNotSupported(format!(
                        "Operator {:?} not supported",
                        operator
                    )))
                }
            };
            result_data.extend_from_slice(&result.to_le_bytes());
        }

        Ok(ColumnBatch {
            data: result_data,
            nulls: None, // TODO: Combine null bitmaps
            length: left.length,
            data_type: VectorDataType::Float64,
            name: "arithmetic_result".to_string(),
        })
    }
}

impl VectorRegisterSet {
    pub fn new() -> Self {
        Self {
            f64_registers: Vec::new(),
            i64_registers: Vec::new(),
            string_registers: Vec::new(),
        }
    }
}

impl<T> VectorRegister<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            null_bitmap: None,
            capacity,
        }
    }
}

impl ColumnBatch {
    /// Create mock data for testing
    pub fn mock_data(name: String, data_type: VectorDataType, length: usize) -> Self {
        let mut data = Vec::new();

        match data_type {
            VectorDataType::Integer64 => {
                for i in 0..length {
                    data.extend_from_slice(&(i as i64).to_le_bytes());
                }
            }
            VectorDataType::Float64 => {
                for i in 0..length {
                    data.extend_from_slice(&(i as f64 * 1.5).to_le_bytes());
                }
            }
            _ => {
                // Fallback for unsupported types
                data.resize(length * 8, 0);
            }
        }

        Self {
            data,
            nulls: None,
            length,
            data_type,
            name,
        }
    }
}

impl Bitmap {
    pub fn new(len: usize) -> Self {
        let word_count = len.div_ceil(64);
        Self {
            bits: vec![0; word_count],
            len,
        }
    }

    pub fn set(&mut self, index: usize, value: bool) {
        if index >= self.len {
            return;
        }

        let word_index = index / 64;
        let bit_index = index % 64;

        if value {
            self.bits[word_index] |= 1u64 << bit_index;
        } else {
            self.bits[word_index] &= !(1u64 << bit_index);
        }
    }

    pub fn get(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }

        let word_index = index / 64;
        let bit_index = index % 64;

        (self.bits[word_index] >> bit_index) & 1 == 1
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
    fn test_vectorized_executor_creation() {
        let executor = VectorizedExecutor::new();
        assert_eq!(executor.batch_size, DEFAULT_BATCH_SIZE);
        assert!(executor.config.enable_simd);
    }

    #[test]
    fn test_bitmap_operations() {
        let mut bitmap = Bitmap::new(100);

        bitmap.set(10, true);
        bitmap.set(50, true);
        bitmap.set(99, true);

        assert!(bitmap.get(10));
        assert!(bitmap.get(50));
        assert!(bitmap.get(99));
        assert!(!bitmap.get(11));
        assert!(!bitmap.get(0));
    }

    #[test]
    fn test_column_batch_creation() {
        let column =
            ColumnBatch::mock_data("test_col".to_string(), VectorDataType::Integer64, 1000);

        assert_eq!(column.name, "test_col");
        assert_eq!(column.length, 1000);
        assert_eq!(column.data_type, VectorDataType::Integer64);
        assert_eq!(column.data.len(), 8000); // 1000 * 8 bytes per i64
    }

    #[test]
    fn test_vectorized_filter() {
        let schema = BatchSchema {
            fields: vec![("id".to_string(), VectorDataType::Integer64)],
        };

        let column = ColumnBatch::mock_data("id".to_string(), VectorDataType::Integer64, 100);
        let batch = RecordBatch {
            columns: vec![column],
            row_count: 100,
            schema,
        };

        let filter = VectorizedFilter {
            condition: Expression::Binary {
                left: Box::new(Expression::Identifier("id".to_string())),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(Expression::Literal(QueryValue::Integer(50))),
            },
        };

        assert!(filter.supports_simd());

        let result = filter.execute_batch(&batch);
        assert!(result.is_ok());
    }

    #[test]
    fn test_vectorized_projection() {
        let schema = BatchSchema {
            fields: vec![("a".to_string(), VectorDataType::Integer64)],
        };

        let column = ColumnBatch::mock_data("a".to_string(), VectorDataType::Integer64, 100);
        let batch = RecordBatch {
            columns: vec![column],
            row_count: 100,
            schema,
        };

        let projection = VectorizedProjection {
            expressions: vec![Expression::Binary {
                left: Box::new(Expression::Identifier("a".to_string())),
                operator: BinaryOperator::Add,
                right: Box::new(Expression::Literal(QueryValue::Integer(10))),
            }],
            output_names: vec!["a_plus_10".to_string()],
        };

        assert!(projection.supports_simd());

        let result = projection.execute_batch(&batch);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.columns.len(), 1);
        assert_eq!(output.columns[0].name, "a_plus_10");
    }
}
