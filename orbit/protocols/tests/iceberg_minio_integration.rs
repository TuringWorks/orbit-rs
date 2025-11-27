//! Integration tests for Iceberg cold tier with MinIO
//!
//! These tests require a running MinIO instance at localhost:63884
//! with credentials: minioadmin/minioadmin
//!
//! To run these tests:
//! ```bash
//! cargo test --test iceberg_minio_integration --features iceberg-cold -- --ignored
//! ```

#![cfg(feature = "iceberg-cold")]

use std::sync::Arc;

use iceberg::io::{FileIO, FileIOBuilder};
use iceberg::spec::{NestedField, PrimitiveType, Schema, Type};
use opendal::Operator;

use orbit_protocols::postgres_wire::sql::execution::{
    column_batch_to_arrow, Column, ColumnBatch, NullBitmap,
};

/// MinIO configuration for testing
/// Note: Port 9000 is the API port, port 63884 is the console UI
const MINIO_ENDPOINT: &str = "http://localhost:9000";
const MINIO_BUCKET: &str = "orbitstore";
const MINIO_ACCESS_KEY: &str = "minioadmin";
const MINIO_SECRET_KEY: &str = "minioadmin";
const WAREHOUSE_PATH: &str = "s3://orbitstore/warehouse";

/// Create a test FileIO configured for MinIO
fn create_minio_file_io() -> FileIO {
    let mut builder = FileIOBuilder::new("s3");

    // S3 configuration for MinIO
    builder = builder
        .with_prop("s3.endpoint", MINIO_ENDPOINT)
        .with_prop("s3.access_key_id", MINIO_ACCESS_KEY)
        .with_prop("s3.secret_access_key", MINIO_SECRET_KEY)
        .with_prop("s3.region", "us-east-1")
        .with_prop("s3.path_style_access", "true"); // MinIO requires path-style

    builder.build().expect("Failed to create FileIO")
}

/// Create a simple test schema (id: int, name: string, value: int)
fn create_test_schema() -> Schema {
    Schema::builder()
        .with_fields(vec![
            Arc::new(NestedField::required(
                1,
                "id",
                Type::Primitive(PrimitiveType::Int),
            )),
            Arc::new(NestedField::optional(
                2,
                "name",
                Type::Primitive(PrimitiveType::String),
            )),
            Arc::new(NestedField::optional(
                3,
                "value",
                Type::Primitive(PrimitiveType::Int),
            )),
        ])
        .build()
        .expect("Failed to create schema")
}

/// Create test data as ColumnBatch
fn create_test_data(row_count: usize) -> ColumnBatch {
    let ids: Vec<i32> = (0..row_count as i32).collect();
    let names: Vec<String> = (0..row_count).map(|i| format!("user_{}", i)).collect();
    let values: Vec<i32> = (0..row_count as i32).map(|i| i * 10).collect();

    ColumnBatch {
        columns: vec![
            Column::Int32(ids),
            Column::String(names),
            Column::Int32(values),
        ],
        null_bitmaps: vec![
            NullBitmap::new_all_valid(row_count),
            NullBitmap::new_all_valid(row_count),
            NullBitmap::new_all_valid(row_count),
        ],
        row_count,
        column_names: Some(vec![
            "id".to_string(),
            "name".to_string(),
            "value".to_string(),
        ]),
    }
}

#[tokio::test]
#[ignore] // Requires MinIO running
async fn test_minio_connection() {
    // Test basic MinIO connectivity with opendal
    let builder = opendal::services::S3::default()
        .endpoint(MINIO_ENDPOINT)
        .access_key_id(MINIO_ACCESS_KEY)
        .secret_access_key(MINIO_SECRET_KEY)
        .bucket(MINIO_BUCKET)
        .region("us-east-1");

    let op = Operator::new(builder)
        .expect("Failed to create operator")
        .finish();

    // Try to list the bucket
    let result = op.list("/").await;
    assert!(
        result.is_ok(),
        "Failed to connect to MinIO: {:?}",
        result.err()
    );

    println!("✅ MinIO connection successful");
}

#[tokio::test]
#[ignore] // Requires MinIO running
async fn test_write_parquet_to_minio() {
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;
    use std::io::Cursor;

    // Create test data
    let batch = create_test_data(1000);

    // Convert to Arrow
    let arrow_batch = column_batch_to_arrow(&batch).expect("Failed to convert to Arrow");

    println!(
        "✅ Created Arrow batch with {} rows",
        arrow_batch.num_rows()
    );

    // Write to Parquet in memory
    let mut buffer = Vec::new();
    {
        let props = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::ZSTD(
                parquet::basic::ZstdLevel::default(),
            ))
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, arrow_batch.schema(), Some(props))
            .expect("Failed to create Parquet writer");

        writer.write(&arrow_batch).expect("Failed to write batch");

        writer.close().expect("Failed to close writer");
    }

    println!("✅ Wrote Parquet file ({} bytes)", buffer.len());

    // Upload to MinIO
    let builder = opendal::services::S3::default()
        .endpoint(MINIO_ENDPOINT)
        .access_key_id(MINIO_ACCESS_KEY)
        .secret_access_key(MINIO_SECRET_KEY)
        .bucket(MINIO_BUCKET)
        .region("us-east-1");

    let op = Operator::new(builder)
        .expect("Failed to create operator")
        .finish();

    let path = "test/data_1000rows.parquet";
    op.write(path, buffer)
        .await
        .expect("Failed to upload to MinIO");

    println!("✅ Uploaded to MinIO: s3://{}/{}", MINIO_BUCKET, path);

    // Verify file exists
    let metadata = op.stat(path).await.expect("Failed to stat file");
    println!(
        "✅ Verified file exists (size: {} bytes)",
        metadata.content_length()
    );
}

#[tokio::test]
#[ignore] // Requires MinIO running
async fn test_read_parquet_from_minio() {
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    // Setup MinIO operator
    let builder = opendal::services::S3::default()
        .endpoint(MINIO_ENDPOINT)
        .access_key_id(MINIO_ACCESS_KEY)
        .secret_access_key(MINIO_SECRET_KEY)
        .bucket(MINIO_BUCKET)
        .region("us-east-1");

    let op = Operator::new(builder)
        .expect("Failed to create operator")
        .finish();

    // First, write test data
    let batch = create_test_data(500);
    let arrow_batch = column_batch_to_arrow(&batch).expect("Failed to convert to Arrow");

    let mut buffer = Vec::new();
    {
        use parquet::arrow::ArrowWriter;
        let mut writer = ArrowWriter::try_new(&mut buffer, arrow_batch.schema(), None)
            .expect("Failed to create writer");
        writer.write(&arrow_batch).expect("Failed to write");
        writer.close().expect("Failed to close");
    }

    let path = "test/read_test.parquet";
    op.write(path, buffer.clone())
        .await
        .expect("Failed to upload");

    println!("✅ Uploaded {} bytes to MinIO", buffer.len());

    // Now read it back
    let downloaded: opendal::Buffer = op.read(path).await.expect("Failed to download from MinIO");

    println!("✅ Downloaded {} bytes from MinIO", downloaded.len());

    // Convert to Bytes for Parquet reader
    let bytes = Bytes::from(downloaded.to_vec());
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create Parquet reader");

    let mut reader = builder.build().expect("Failed to build reader");

    // Read batch
    let read_batch = reader
        .next()
        .expect("No batches found")
        .expect("Failed to read batch");

    println!("✅ Read {} rows from Parquet", read_batch.num_rows());
    assert_eq!(read_batch.num_rows(), 500);
    assert_eq!(read_batch.num_columns(), 3);
}

#[tokio::test]
#[ignore] // Requires MinIO running
async fn test_column_batch_roundtrip() {
    use bytes::Bytes;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use parquet::arrow::ArrowWriter;

    // Original data
    let original_batch = create_test_data(100);
    println!(
        "Original batch: {} rows, {} columns",
        original_batch.row_count,
        original_batch.columns.len()
    );

    // Convert to Arrow
    let arrow_batch = column_batch_to_arrow(&original_batch).expect("Failed to convert to Arrow");

    // Write to Parquet (in memory)
    let mut buffer = Vec::new();
    {
        let mut writer = ArrowWriter::try_new(&mut buffer, arrow_batch.schema(), None)
            .expect("Failed to create writer");
        writer.write(&arrow_batch).expect("Failed to write");
        writer.close().expect("Failed to close");
    }

    // Read back from Parquet
    let bytes = Bytes::from(buffer);
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes).expect("Failed to create reader");
    let mut reader = builder.build().expect("Failed to build reader");
    let read_batch = reader.next().unwrap().expect("Failed to read");

    // Verify round-trip
    assert_eq!(read_batch.num_rows(), original_batch.row_count);
    assert_eq!(read_batch.num_columns(), original_batch.columns.len());

    // Verify first column (IDs)
    let id_array = read_batch
        .column(0)
        .as_any()
        .downcast_ref::<arrow::array::Int32Array>()
        .expect("Wrong type for ID column");

    if let Column::Int32(ref ids) = original_batch.columns[0] {
        for i in 0..10 {
            // Check first 10 values
            assert_eq!(id_array.value(i), ids[i], "ID mismatch at row {}", i);
        }
    }

    println!("✅ Column batch round-trip successful");
}

#[tokio::test]
#[ignore] // Requires MinIO running
async fn test_large_dataset_performance() {
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;
    use std::time::Instant;

    // Create larger dataset (100K rows)
    let start = Instant::now();
    let batch = create_test_data(100_000);
    println!("✅ Created 100K rows in {:?}", start.elapsed());

    // Convert to Arrow
    let start = Instant::now();
    let arrow_batch = column_batch_to_arrow(&batch).expect("Failed to convert");
    println!("✅ Converted to Arrow in {:?}", start.elapsed());

    // Write to Parquet with compression
    let start = Instant::now();
    let mut buffer = Vec::new();
    {
        let props = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::ZSTD(
                parquet::basic::ZstdLevel::try_new(3).unwrap(),
            ))
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, arrow_batch.schema(), Some(props))
            .expect("Failed to create writer");
        writer.write(&arrow_batch).expect("Failed to write");
        writer.close().expect("Failed to close");
    }
    let write_time = start.elapsed();

    println!(
        "✅ Wrote Parquet in {:?} ({} bytes, {:.2} MB/s)",
        write_time,
        buffer.len(),
        (buffer.len() as f64 / 1024.0 / 1024.0) / write_time.as_secs_f64()
    );

    // Upload to MinIO
    let builder = opendal::services::S3::default()
        .endpoint(MINIO_ENDPOINT)
        .access_key_id(MINIO_ACCESS_KEY)
        .secret_access_key(MINIO_SECRET_KEY)
        .bucket(MINIO_BUCKET)
        .region("us-east-1");

    let op = Operator::new(builder)
        .expect("Failed to create operator")
        .finish();

    let path = "test/large_100k_rows.parquet";
    let start = Instant::now();
    op.write(path, buffer.clone())
        .await
        .expect("Failed to upload");

    println!(
        "✅ Uploaded to MinIO in {:?} ({:.2} MB/s)",
        start.elapsed(),
        (buffer.len() as f64 / 1024.0 / 1024.0) / start.elapsed().as_secs_f64()
    );

    // Compression ratio
    let uncompressed_size = 100_000 * (4 + 20 + 4); // id + name (avg) + value
    let compression_ratio = uncompressed_size as f64 / buffer.len() as f64;
    println!(
        "✅ Compression ratio: {:.2}x ({} → {} bytes)",
        compression_ratio,
        uncompressed_size,
        buffer.len()
    );
}
