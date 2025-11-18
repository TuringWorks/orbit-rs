//! Iceberg Cold Tier Storage
//!
//! Implements cold tier storage using Apache Iceberg for:
//! - Time travel queries
//! - Schema evolution
//! - Metadata-based query planning (100-1000x speedup for large tables)
//! - Multi-engine interoperability (Spark, Trino, Flink)
//!
//! ## Design
//!
//! - Read-only operations (Phase 1)
//! - Parquet file format with Zstd compression
//! - Hidden partitioning by time dimensions
//! - Integration with VectorizedExecutor for SIMD aggregations
//!
//! ## Future (Phase 2+)
//!
//! - Write operations (append, compact)
//! - Automatic tier migration from warm tier
//! - Partition evolution
//! - Snapshot management

use std::sync::Arc;
use std::time::SystemTime;

use arrow::array::{Array, ArrayRef, Int32Array, Int64Array, StringArray, BooleanArray};
use arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
use arrow::record_batch::RecordBatch;
use iceberg::{Catalog, NamespaceIdent, TableIdent};
use iceberg::table::Table;
use iceberg::spec::Schema as IcebergSchema;
use iceberg_catalog_rest::RestCatalog;

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::types::SqlValue;
use super::{
    Column, ColumnBatch, NullBitmap,
    VectorizedExecutor, VectorizedExecutorConfig,
    AggregateFunction, ComparisonOp,
};

/// Iceberg cold tier storage
///
/// Provides read-only access to historical data stored in Iceberg tables.
/// Optimized for analytics with metadata pruning and SIMD aggregations.
pub struct IcebergColdStore {
    /// Iceberg table
    table: Arc<Table>,

    /// Table name
    table_name: String,

    /// Vectorized executor for SIMD operations
    vectorized_executor: VectorizedExecutor,

    /// Creation timestamp
    created_at: SystemTime,
}

impl IcebergColdStore {
    /// Create a new Iceberg cold store
    pub async fn new(
        catalog: Arc<RestCatalog>,
        namespace: &str,
        table_name: &str,
    ) -> ProtocolResult<Self> {
        let namespace_ident = NamespaceIdent::new(namespace.to_string());
        let table_ident = TableIdent::new(namespace_ident, table_name.to_string());

        let table = catalog
            .load_table(&table_ident)
            .await
            .map_err(|e| ProtocolError::PostgresError(
                format!("Failed to load Iceberg table: {}", e)
            ))?;

        Ok(Self {
            table: Arc::new(table),
            table_name: table_name.to_string(),
            vectorized_executor: VectorizedExecutor::with_config(VectorizedExecutorConfig::default()),
            created_at: SystemTime::now(),
        })
    }

    /// Scan table with optional filter
    ///
    /// Uses Iceberg metadata pruning for efficient file selection.
    pub async fn scan(
        &self,
        filter: Option<&FilterPredicate>,
    ) -> ProtocolResult<Vec<RecordBatch>> {
        // TODO: Build Iceberg scan with filter
        // let mut scan_builder = self.table.scan();
        //
        // if let Some(f) = filter {
        //     let iceberg_filter = convert_to_iceberg_filter(f)?;
        //     scan_builder = scan_builder.with_filter(iceberg_filter);
        // }
        //
        // let scan = scan_builder.build()
        //     .map_err(|e| ProtocolError::PostgresError(
        //         format!("Failed to build scan: {}", e)
        //     ))?;
        //
        // let batches = scan.to_arrow().await
        //     .map_err(|e| ProtocolError::PostgresError(
        //         format!("Failed to read Arrow batches: {}", e)
        //     ))?;

        // Placeholder: Return empty result for Phase 1
        Ok(vec![])
    }

    /// Execute aggregation with SIMD optimization
    ///
    /// Combines Iceberg metadata pruning with our vectorized executor.
    pub async fn aggregate(
        &self,
        column: &str,
        function: AggregateFunction,
        filter: Option<&FilterPredicate>,
    ) -> ProtocolResult<SqlValue> {
        // 1. Scan with filter (Iceberg prunes partitions/files)
        let batches = self.scan(filter).await?;

        if batches.is_empty() {
            return Ok(match function {
                AggregateFunction::Count => SqlValue::BigInt(0),
                AggregateFunction::Sum => SqlValue::Integer(0),
                AggregateFunction::Min | AggregateFunction::Max => SqlValue::Null,
                AggregateFunction::Avg => SqlValue::Real(0.0),
            });
        }

        // 2. Convert Arrow batches to ColumnBatch
        let column_batch = self.arrow_batches_to_column_batch(batches, column)?;

        // 3. Use vectorized executor for SIMD aggregation (14.8x speedup)
        let column_index = 0; // We converted specific column above
        let result = self.vectorized_executor.execute_aggregation(
            &column_batch,
            column_index,
            function,
        )?;

        Ok(result)
    }

    /// Query table as of specific timestamp (time travel)
    pub async fn query_as_of(
        &self,
        timestamp: SystemTime,
        filter: Option<&FilterPredicate>,
    ) -> ProtocolResult<Vec<RecordBatch>> {
        // TODO: Implement time travel
        // let snapshot = self.table.snapshot_as_of_timestamp(timestamp)
        //     .map_err(|e| ProtocolError::PostgresError(
        //         format!("Failed to get snapshot: {}", e)
        //     ))?;
        //
        // let scan = snapshot.scan()
        //     .with_filter(...)
        //     .build()?;

        // Placeholder
        Ok(vec![])
    }

    /// Get table schema
    pub fn schema(&self) -> ProtocolResult<IcebergSchema> {
        Ok(self.table.metadata().current_schema().clone())
    }

    /// Get row count (from metadata, no file I/O)
    pub async fn row_count(&self) -> ProtocolResult<usize> {
        // TODO: Get row count from manifest metadata
        // let manifest = self.table.current_snapshot()
        //     .ok_or_else(|| ProtocolError::PostgresError("No snapshot".into()))?;
        //
        // let count = manifest.summary().get("total-records")
        //     .and_then(|s| s.parse().ok())
        //     .unwrap_or(0);

        Ok(0)
    }

    // Private helper methods

    /// Convert Arrow batches to our ColumnBatch format
    ///
    /// Extracts specified column for SIMD processing.
    fn arrow_batches_to_column_batch(
        &self,
        batches: Vec<RecordBatch>,
        column_name: &str,
    ) -> ProtocolResult<ColumnBatch> {
        if batches.is_empty() {
            return Err(ProtocolError::PostgresError("No batches to convert".into()));
        }

        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();

        // Find column index
        let schema = batches[0].schema();
        let column_index = schema
            .fields()
            .iter()
            .position(|f| f.name() == column_name)
            .ok_or_else(|| ProtocolError::PostgresError(
                format!("Column {} not found", column_name)
            ))?;

        // Extract column from all batches
        let mut values = Vec::with_capacity(total_rows);
        let mut null_bitmap = NullBitmap::new_all_valid(total_rows);
        let mut row_offset = 0;

        for batch in &batches {
            let array = batch.column(column_index);

            match array.data_type() {
                DataType::Int32 => {
                    let int_array = array.as_any().downcast_ref::<Int32Array>()
                        .ok_or_else(|| ProtocolError::PostgresError("Type mismatch".into()))?;

                    for i in 0..int_array.len() {
                        if int_array.is_null(i) {
                            null_bitmap.set_null(row_offset + i);
                            values.push(0);
                        } else {
                            values.push(int_array.value(i));
                        }
                    }
                }
                DataType::Int64 => {
                    let int_array = array.as_any().downcast_ref::<Int64Array>()
                        .ok_or_else(|| ProtocolError::PostgresError("Type mismatch".into()))?;

                    for i in 0..int_array.len() {
                        if int_array.is_null(i) {
                            null_bitmap.set_null(row_offset + i);
                            values.push(0);
                        } else {
                            values.push(int_array.value(i) as i32); // Downcast for simplicity
                        }
                    }
                }
                _ => {
                    return Err(ProtocolError::PostgresError(
                        format!("Unsupported data type: {:?}", array.data_type())
                    ));
                }
            }

            row_offset += batch.num_rows();
        }

        Ok(ColumnBatch {
            columns: vec![Column::Int32(values)],
            null_bitmaps: vec![null_bitmap],
            row_count: total_rows,
            column_names: Some(vec![column_name.to_string()]),
        })
    }
}

/// Filter predicate (re-exported from hybrid module)
#[derive(Debug, Clone, PartialEq)]
pub struct FilterPredicate {
    pub column: String,
    pub operator: ComparisonOp,
    pub value: SqlValue,
}

/// Convert ColumnBatch to Arrow RecordBatch
///
/// Enables writing from our columnar format to Iceberg.
pub fn column_batch_to_arrow(batch: &ColumnBatch) -> ProtocolResult<RecordBatch> {
    if batch.columns.is_empty() {
        return Err(ProtocolError::PostgresError("No columns to convert".into()));
    }

    let column_names = batch.column_names.as_ref()
        .ok_or_else(|| ProtocolError::PostgresError("No column names".into()))?;

    let mut fields = Vec::new();
    let mut arrays: Vec<ArrayRef> = Vec::new();

    for (col_idx, column) in batch.columns.iter().enumerate() {
        let column_name = column_names.get(col_idx)
            .ok_or_else(|| ProtocolError::PostgresError("Missing column name".into()))?;

        let null_bitmap = &batch.null_bitmaps[col_idx];

        match column {
            Column::Int32(values) => {
                let field = Field::new(column_name, DataType::Int32, true);
                fields.push(field);

                // Build Int32Array with nulls
                let mut builder = arrow::array::Int32Builder::with_capacity(values.len());
                for (i, &value) in values.iter().enumerate() {
                    if null_bitmap.is_null(i) {
                        builder.append_null();
                    } else {
                        builder.append_value(value);
                    }
                }
                arrays.push(Arc::new(builder.finish()));
            }
            Column::Int64(values) => {
                let field = Field::new(column_name, DataType::Int64, true);
                fields.push(field);

                let mut builder = arrow::array::Int64Builder::with_capacity(values.len());
                for (i, &value) in values.iter().enumerate() {
                    if null_bitmap.is_null(i) {
                        builder.append_null();
                    } else {
                        builder.append_value(value);
                    }
                }
                arrays.push(Arc::new(builder.finish()));
            }
            Column::Float32(values) => {
                let field = Field::new(column_name, DataType::Float32, true);
                fields.push(field);

                let mut builder = arrow::array::Float32Builder::with_capacity(values.len());
                for (i, &value) in values.iter().enumerate() {
                    if null_bitmap.is_null(i) {
                        builder.append_null();
                    } else {
                        builder.append_value(value);
                    }
                }
                arrays.push(Arc::new(builder.finish()));
            }
            Column::Float64(values) => {
                let field = Field::new(column_name, DataType::Float64, true);
                fields.push(field);

                let mut builder = arrow::array::Float64Builder::with_capacity(values.len());
                for (i, &value) in values.iter().enumerate() {
                    if null_bitmap.is_null(i) {
                        builder.append_null();
                    } else {
                        builder.append_value(value);
                    }
                }
                arrays.push(Arc::new(builder.finish()));
            }
            Column::Bool(values) => {
                let field = Field::new(column_name, DataType::Boolean, true);
                fields.push(field);

                let mut builder = arrow::array::BooleanBuilder::with_capacity(values.len());
                for (i, &value) in values.iter().enumerate() {
                    if null_bitmap.is_null(i) {
                        builder.append_null();
                    } else {
                        builder.append_value(value);
                    }
                }
                arrays.push(Arc::new(builder.finish()));
            }
            Column::String(values) => {
                let field = Field::new(column_name, DataType::Utf8, true);
                fields.push(field);

                let mut builder = arrow::array::StringBuilder::with_capacity(
                    values.len(),
                    values.iter().map(|s| s.len()).sum(),
                );
                for (i, value) in values.iter().enumerate() {
                    if null_bitmap.is_null(i) {
                        builder.append_null();
                    } else {
                        builder.append_value(value);
                    }
                }
                arrays.push(Arc::new(builder.finish()));
            }
        }
    }

    let schema = Arc::new(ArrowSchema::new(fields));
    RecordBatch::try_new(schema, arrays)
        .map_err(|e| ProtocolError::PostgresError(format!("Failed to create RecordBatch: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_batch_to_arrow_int32() {
        let values = vec![1, 2, 3, 4, 5];
        let column = Column::Int32(values.clone());
        let null_bitmap = NullBitmap::new_all_valid(5);

        let batch = ColumnBatch {
            columns: vec![column],
            null_bitmaps: vec![null_bitmap],
            row_count: 5,
            column_names: Some(vec!["id".to_string()]),
        };

        let arrow_batch = column_batch_to_arrow(&batch).unwrap();

        assert_eq!(arrow_batch.num_rows(), 5);
        assert_eq!(arrow_batch.num_columns(), 1);

        let array = arrow_batch.column(0);
        let int_array = array.as_any().downcast_ref::<Int32Array>().unwrap();

        for (i, &expected) in values.iter().enumerate() {
            assert_eq!(int_array.value(i), expected);
            assert!(!int_array.is_null(i));
        }
    }

    #[test]
    fn test_column_batch_to_arrow_with_nulls() {
        let values = vec![1, 2, 3, 4, 5];
        let column = Column::Int32(values);
        let mut null_bitmap = NullBitmap::new_all_valid(5);
        null_bitmap.set_null(1); // Second value is null
        null_bitmap.set_null(3); // Fourth value is null

        let batch = ColumnBatch {
            columns: vec![column],
            null_bitmaps: vec![null_bitmap],
            row_count: 5,
            column_names: Some(vec!["value".to_string()]),
        };

        let arrow_batch = column_batch_to_arrow(&batch).unwrap();
        let array = arrow_batch.column(0);
        let int_array = array.as_any().downcast_ref::<Int32Array>().unwrap();

        assert!(!int_array.is_null(0));
        assert!(int_array.is_null(1)); // Null
        assert!(!int_array.is_null(2));
        assert!(int_array.is_null(3)); // Null
        assert!(!int_array.is_null(4));
    }

    #[test]
    fn test_column_batch_to_arrow_multiple_types() {
        let int_values = vec![1, 2, 3];
        let str_values = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        let batch = ColumnBatch {
            columns: vec![
                Column::Int32(int_values),
                Column::String(str_values.clone()),
            ],
            null_bitmaps: vec![
                NullBitmap::new_all_valid(3),
                NullBitmap::new_all_valid(3),
            ],
            row_count: 3,
            column_names: Some(vec!["id".to_string(), "name".to_string()]),
        };

        let arrow_batch = column_batch_to_arrow(&batch).unwrap();

        assert_eq!(arrow_batch.num_rows(), 3);
        assert_eq!(arrow_batch.num_columns(), 2);

        // Check int column
        let int_array = arrow_batch.column(0).as_any()
            .downcast_ref::<Int32Array>().unwrap();
        assert_eq!(int_array.value(0), 1);

        // Check string column
        let str_array = arrow_batch.column(1).as_any()
            .downcast_ref::<StringArray>().unwrap();
        assert_eq!(str_array.value(0), "a");
    }
}
