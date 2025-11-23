//! Columnar Data Structures for Vectorized Execution
//!
//! This module provides cache-friendly columnar storage for intermediate query results.
//! Columnar layout enables:
//! - Better CPU cache utilization
//! - SIMD vectorization opportunities
//! - Efficient null handling with bitmaps
//! - Reduced memory bandwidth requirements
//!
//! ## Design Patterns
//!
//! - **Type-Safe Columns**: Enum with variants for each SQL type
//! - **Builder Pattern**: Fluent API for batch construction
//! - **Iterator Pattern**: Zero-copy iteration over columns
//! - **Visitor Pattern**: Generic column operations

use crate::protocols::postgres_wire::sql::types::SqlValue;
use std::fmt;

/// Default batch size for vectorized operations (optimized for L1 cache)
pub const DEFAULT_BATCH_SIZE: usize = 1024;

/// Null bitmap for efficient null tracking
///
/// Uses bit-packing to minimize memory overhead (1 bit per value)
#[derive(Debug, Clone, PartialEq)]
pub struct NullBitmap {
    bits: Vec<u64>,
    len: usize,
}

impl NullBitmap {
    /// Get the length of the null bitmap
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the bitmap is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Create a new null bitmap with all values non-null
    pub fn new_all_valid(len: usize) -> Self {
        let num_words = (len + 63) / 64;
        Self {
            bits: vec![u64::MAX; num_words],
            len,
        }
    }

    /// Create a new null bitmap with all values null
    pub fn new_all_null(len: usize) -> Self {
        let num_words = (len + 63) / 64;
        Self {
            bits: vec![0; num_words],
            len,
        }
    }

    /// Check if value at index is null
    #[inline]
    pub fn is_null(&self, index: usize) -> bool {
        debug_assert!(index < self.len, "Index out of bounds");
        let word_index = index / 64;
        let bit_index = index % 64;
        (self.bits[word_index] & (1u64 << bit_index)) == 0
    }

    /// Check if value at index is valid (not null)
    #[inline]
    pub fn is_valid(&self, index: usize) -> bool {
        !self.is_null(index)
    }

    /// Set value at index to null
    pub fn set_null(&mut self, index: usize) {
        debug_assert!(index < self.len);
        let word_index = index / 64;
        let bit_index = index % 64;
        self.bits[word_index] &= !(1u64 << bit_index);
    }

    /// Set value at index to valid (not null)
    pub fn set_valid(&mut self, index: usize) {
        debug_assert!(index < self.len);
        let word_index = index / 64;
        let bit_index = index % 64;
        self.bits[word_index] |= 1u64 << bit_index;
    }

    /// Count the number of null values
    pub fn null_count(&self) -> usize {
        // Count only the bits that correspond to actual values (not padding)
        let mut count = 0;
        for index in 0..self.len {
            if self.is_null(index) {
                count += 1;
            }
        }
        count
    }

    /// Iterator over null/valid states
    pub fn iter(&self) -> NullBitmapIter<'_> {
        NullBitmapIter {
            bitmap: self,
            index: 0,
        }
    }
}

/// Iterator over null bitmap
pub struct NullBitmapIter<'a> {
    bitmap: &'a NullBitmap,
    index: usize,
}

impl<'a> Iterator for NullBitmapIter<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.bitmap.len {
            let is_null = self.bitmap.is_null(self.index);
            self.index += 1;
            Some(is_null)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.bitmap.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for NullBitmapIter<'a> {}

/// Typed column storage with type safety
///
/// Each variant stores a specific SQL type in a contiguous vector
/// for cache-friendly access and SIMD operations.
#[derive(Debug, Clone, PartialEq)]
pub enum Column {
    Bool(Vec<bool>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Int64(Vec<i64>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    String(Vec<String>),
    Binary(Vec<Vec<u8>>),
}

impl Column {
    /// Get the number of values in the column
    pub fn len(&self) -> usize {
        match self {
            Column::Bool(v) => v.len(),
            Column::Int16(v) => v.len(),
            Column::Int32(v) => v.len(),
            Column::Int64(v) => v.len(),
            Column::Float32(v) => v.len(),
            Column::Float64(v) => v.len(),
            Column::String(v) => v.len(),
            Column::Binary(v) => v.len(),
        }
    }

    /// Check if column is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get value at index as SqlValue
    pub fn get(&self, index: usize) -> Option<SqlValue> {
        match self {
            Column::Bool(v) => v.get(index).map(|&b| SqlValue::Boolean(b)),
            Column::Int16(v) => v.get(index).map(|&i| SqlValue::SmallInt(i)),
            Column::Int32(v) => v.get(index).map(|&i| SqlValue::Integer(i)),
            Column::Int64(v) => v.get(index).map(|&i| SqlValue::BigInt(i)),
            Column::Float32(v) => v.get(index).map(|&f| SqlValue::Real(f)),
            Column::Float64(v) => v.get(index).map(|&f| SqlValue::DoublePrecision(f)),
            Column::String(v) => v.get(index).map(|s| SqlValue::Text(s.clone())),
            Column::Binary(v) => v.get(index).map(|b| SqlValue::Bytea(b.clone())),
        }
    }

    /// Create column from SqlValues
    pub fn from_sql_values(values: Vec<SqlValue>) -> Result<Self, String> {
        if values.is_empty() {
            return Err("Cannot create column from empty values".to_string());
        }

        // Infer type from first value
        match &values[0] {
            SqlValue::Boolean(_) => {
                let mut bools = Vec::with_capacity(values.len());
                for val in values {
                    match val {
                        SqlValue::Boolean(b) => bools.push(b),
                        _ => return Err("Mixed types in column".to_string()),
                    }
                }
                Ok(Column::Bool(bools))
            }
            SqlValue::Integer(_) => {
                let mut ints = Vec::with_capacity(values.len());
                for val in values {
                    match val {
                        SqlValue::Integer(i) => ints.push(i),
                        _ => return Err("Mixed types in column".to_string()),
                    }
                }
                Ok(Column::Int32(ints))
            }
            SqlValue::BigInt(_) => {
                let mut ints = Vec::with_capacity(values.len());
                for val in values {
                    match val {
                        SqlValue::BigInt(i) => ints.push(i),
                        _ => return Err("Mixed types in column".to_string()),
                    }
                }
                Ok(Column::Int64(ints))
            }
            SqlValue::Real(_) => {
                let mut floats = Vec::with_capacity(values.len());
                for val in values {
                    match val {
                        SqlValue::Real(f) => floats.push(f),
                        _ => return Err("Mixed types in column".to_string()),
                    }
                }
                Ok(Column::Float32(floats))
            }
            SqlValue::DoublePrecision(_) => {
                let mut floats = Vec::with_capacity(values.len());
                for val in values {
                    match val {
                        SqlValue::DoublePrecision(f) => floats.push(f),
                        _ => return Err("Mixed types in column".to_string()),
                    }
                }
                Ok(Column::Float64(floats))
            }
            SqlValue::Text(_) | SqlValue::Varchar(_) => {
                let mut strings = Vec::with_capacity(values.len());
                for val in values {
                    match val {
                        SqlValue::Text(s) | SqlValue::Varchar(s) => strings.push(s),
                        _ => return Err("Mixed types in column".to_string()),
                    }
                }
                Ok(Column::String(strings))
            }
            _ => Err(format!("Unsupported column type: {:?}", values[0])),
        }
    }
}

/// A batch of rows in columnar format
///
/// Stores data in columnar layout for efficient vectorized processing.
/// Each column has an associated null bitmap for tracking NULL values.
#[derive(Debug, Clone)]
pub struct ColumnBatch {
    /// Column data
    pub columns: Vec<Column>,
    /// Null bitmaps (one per column)
    pub null_bitmaps: Vec<NullBitmap>,
    /// Number of rows in the batch
    pub row_count: usize,
    /// Column names (optional, for debugging)
    pub column_names: Option<Vec<String>>,
}

impl ColumnBatch {
    /// Create a new empty column batch
    pub fn new(num_columns: usize, row_count: usize) -> Self {
        Self {
            columns: Vec::with_capacity(num_columns),
            null_bitmaps: Vec::with_capacity(num_columns),
            row_count,
            column_names: None,
        }
    }

    /// Add a column to the batch
    pub fn add_column(&mut self, column: Column, null_bitmap: NullBitmap) -> Result<(), String> {
        if column.len() != self.row_count {
            return Err(format!(
                "Column length {} does not match batch row count {}",
                column.len(),
                self.row_count
            ));
        }
        if null_bitmap.len != self.row_count {
            return Err(format!(
                "Null bitmap length {} does not match batch row count {}",
                null_bitmap.len, self.row_count
            ));
        }
        self.columns.push(column);
        self.null_bitmaps.push(null_bitmap);
        Ok(())
    }

    /// Get number of columns
    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }

    /// Get a row as a vector of SqlValue (row-oriented access)
    pub fn get_row(&self, row_index: usize) -> Option<Vec<Option<SqlValue>>> {
        if row_index >= self.row_count {
            return None;
        }

        let mut row = Vec::with_capacity(self.columns.len());
        for (col_index, column) in self.columns.iter().enumerate() {
            let value = if self.null_bitmaps[col_index].is_null(row_index) {
                None
            } else {
                column.get(row_index)
            };
            row.push(value);
        }
        Some(row)
    }

    /// Get a column by index
    pub fn get_column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// Get null bitmap for a column
    pub fn get_null_bitmap(&self, index: usize) -> Option<&NullBitmap> {
        self.null_bitmaps.get(index)
    }

    /// Set column names for debugging
    pub fn set_column_names(&mut self, names: Vec<String>) {
        self.column_names = Some(names);
    }

    /// Iter over rows (slower, row-oriented access)
    pub fn iter_rows(&self) -> RowIterator<'_> {
        RowIterator {
            batch: self,
            row_index: 0,
        }
    }

    /// Create batch from row-oriented data
    pub fn from_rows(rows: Vec<Vec<Option<SqlValue>>>) -> Result<Self, String> {
        if rows.is_empty() {
            return Err("Cannot create batch from empty rows".to_string());
        }

        let row_count = rows.len();
        let num_columns = rows[0].len();

        // Transpose rows to columns
        let mut column_data: Vec<Vec<Option<SqlValue>>> = vec![Vec::with_capacity(row_count); num_columns];

        for row in rows {
            if row.len() != num_columns {
                return Err("Inconsistent row lengths".to_string());
            }
            for (col_index, value) in row.into_iter().enumerate() {
                column_data[col_index].push(value);
            }
        }

        // Create columns and null bitmaps
        let mut batch = Self::new(num_columns, row_count);

        for col_values in column_data {
            let mut null_bitmap = NullBitmap::new_all_valid(row_count);
            let mut non_null_values = Vec::with_capacity(row_count);

            for (index, value) in col_values.into_iter().enumerate() {
                if let Some(v) = value {
                    non_null_values.push(v);
                } else {
                    null_bitmap.set_null(index);
                    // Push a default value for null positions
                    non_null_values.push(SqlValue::Null);
                }
            }

            // Filter out Null values and create typed column
            let typed_values: Vec<SqlValue> = non_null_values
                .into_iter()
                .filter(|v| !matches!(v, SqlValue::Null))
                .collect();

            if typed_values.is_empty() {
                // All nulls - create empty column of appropriate type
                batch.add_column(Column::String(vec![String::new(); row_count]), null_bitmap)?;
            } else {
                let column = Column::from_sql_values(typed_values)?;
                batch.add_column(column, null_bitmap)?;
            }
        }

        Ok(batch)
    }
}

/// Iterator over rows in a column batch
pub struct RowIterator<'a> {
    batch: &'a ColumnBatch,
    row_index: usize,
}

impl<'a> Iterator for RowIterator<'a> {
    type Item = Vec<Option<SqlValue>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row_index < self.batch.row_count {
            let row = self.batch.get_row(self.row_index);
            self.row_index += 1;
            row
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.batch.row_count - self.row_index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for RowIterator<'a> {}

/// Builder for constructing column batches with fluent API
pub struct ColumnBatchBuilder {
    row_count: usize,
    columns: Vec<Column>,
    null_bitmaps: Vec<NullBitmap>,
    column_names: Option<Vec<String>>,
}

impl ColumnBatchBuilder {
    /// Create a new builder
    pub fn new(row_count: usize) -> Self {
        Self {
            row_count,
            columns: Vec::new(),
            null_bitmaps: Vec::new(),
            column_names: None,
        }
    }

    /// Add a column with explicit null bitmap
    pub fn add_column(mut self, column: Column, null_bitmap: NullBitmap) -> Result<Self, String> {
        if column.len() != self.row_count {
            return Err(format!(
                "Column length {} does not match row count {}",
                column.len(),
                self.row_count
            ));
        }
        self.columns.push(column);
        self.null_bitmaps.push(null_bitmap);
        Ok(self)
    }

    /// Add a column with no nulls
    pub fn add_column_non_null(mut self, column: Column) -> Result<Self, String> {
        if column.len() != self.row_count {
            return Err(format!(
                "Column length {} does not match row count {}",
                column.len(),
                self.row_count
            ));
        }
        self.columns.push(column);
        self.null_bitmaps
            .push(NullBitmap::new_all_valid(self.row_count));
        Ok(self)
    }

    /// Set column names
    pub fn with_column_names(mut self, names: Vec<String>) -> Self {
        self.column_names = Some(names);
        self
    }

    /// Build the column batch
    pub fn build(self) -> ColumnBatch {
        ColumnBatch {
            columns: self.columns,
            null_bitmaps: self.null_bitmaps,
            row_count: self.row_count,
            column_names: self.column_names,
        }
    }
}

impl fmt::Display for ColumnBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ColumnBatch {{ rows: {}, cols: {} }}", self.row_count, self.num_columns())?;

        // Print column names if available
        if let Some(names) = &self.column_names {
            write!(f, "Columns: ")?;
            for (i, name) in names.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", name)?;
            }
            writeln!(f)?;
        }

        // Print first few rows for debugging
        let preview_rows = self.row_count.min(5);
        for row_idx in 0..preview_rows {
            if let Some(row) = self.get_row(row_idx) {
                write!(f, "  Row {}: [", row_idx)?;
                for (col_idx, value) in row.iter().enumerate() {
                    if col_idx > 0 {
                        write!(f, ", ")?;
                    }
                    match value {
                        Some(v) => write!(f, "{:?}", v)?,
                        None => write!(f, "NULL")?,
                    }
                }
                writeln!(f, "]")?;
            }
        }
        if self.row_count > preview_rows {
            writeln!(f, "  ... {} more rows", self.row_count - preview_rows)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_bitmap() {
        let mut bitmap = NullBitmap::new_all_valid(100);
        assert_eq!(bitmap.len, 100);
        assert!(bitmap.is_valid(0));
        assert!(bitmap.is_valid(99));

        bitmap.set_null(10);
        assert!(bitmap.is_null(10));
        assert_eq!(bitmap.null_count(), 1);

        bitmap.set_valid(10);
        assert!(bitmap.is_valid(10));
        assert_eq!(bitmap.null_count(), 0);
    }

    #[test]
    fn test_null_bitmap_iterator() {
        let mut bitmap = NullBitmap::new_all_valid(10);
        bitmap.set_null(3);
        bitmap.set_null(7);

        let nulls: Vec<bool> = bitmap.iter().collect();
        assert_eq!(nulls.len(), 10);
        assert!(!nulls[0]); // is_null returns false for valid
        assert!(nulls[3]); // is_null returns true for null
        assert!(nulls[7]);
    }

    #[test]
    fn test_column_creation() {
        let col = Column::Int32(vec![1, 2, 3, 4, 5]);
        assert_eq!(col.len(), 5);
        assert_eq!(col.get(0), Some(SqlValue::Integer(1)));
        assert_eq!(col.get(4), Some(SqlValue::Integer(5)));
        assert_eq!(col.get(10), None);
    }

    #[test]
    fn test_column_from_sql_values() {
        let values = vec![
            SqlValue::Integer(10),
            SqlValue::Integer(20),
            SqlValue::Integer(30),
        ];
        let col = Column::from_sql_values(values).unwrap();
        assert_eq!(col.len(), 3);
        match col {
            Column::Int32(v) => assert_eq!(v, vec![10, 20, 30]),
            _ => panic!("Wrong column type"),
        }
    }

    #[test]
    fn test_column_batch_builder() {
        let batch = ColumnBatchBuilder::new(3)
            .add_column_non_null(Column::Int32(vec![1, 2, 3]))
            .unwrap()
            .add_column_non_null(Column::String(vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
            ]))
            .unwrap()
            .with_column_names(vec!["id".to_string(), "name".to_string()])
            .build();

        assert_eq!(batch.row_count, 3);
        assert_eq!(batch.num_columns(), 2);
        assert_eq!(batch.column_names.as_ref().unwrap()[0], "id");
    }

    #[test]
    fn test_column_batch_get_row() {
        let mut batch = ColumnBatch::new(2, 2);
        batch
            .add_column(
                Column::Int32(vec![10, 20]),
                NullBitmap::new_all_valid(2),
            )
            .unwrap();
        batch
            .add_column(
                Column::String(vec!["foo".to_string(), "bar".to_string()]),
                NullBitmap::new_all_valid(2),
            )
            .unwrap();

        let row0 = batch.get_row(0).unwrap();
        assert_eq!(row0[0], Some(SqlValue::Integer(10)));
        assert_eq!(row0[1], Some(SqlValue::Text("foo".to_string())));

        let row1 = batch.get_row(1).unwrap();
        assert_eq!(row1[0], Some(SqlValue::Integer(20)));
        assert_eq!(row1[1], Some(SqlValue::Text("bar".to_string())));
    }

    #[test]
    fn test_column_batch_with_nulls() {
        let mut null_bitmap = NullBitmap::new_all_valid(3);
        null_bitmap.set_null(1); // Second value is null

        let mut batch = ColumnBatch::new(1, 3);
        batch
            .add_column(Column::Int32(vec![10, 20, 30]), null_bitmap)
            .unwrap();

        let row1 = batch.get_row(1).unwrap();
        assert_eq!(row1[0], None); // Should be NULL
    }

    #[test]
    fn test_row_iterator() {
        let mut batch = ColumnBatch::new(1, 3);
        batch
            .add_column(
                Column::Int32(vec![1, 2, 3]),
                NullBitmap::new_all_valid(3),
            )
            .unwrap();

        let rows: Vec<_> = batch.iter_rows().collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][0], Some(SqlValue::Integer(1)));
        assert_eq!(rows[2][0], Some(SqlValue::Integer(3)));
    }
}
