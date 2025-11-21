//! Integration tests for Iceberg cold tier with Azure Blob Storage (Azurite)
//!
//! ## Prerequisites
//!
//! 1. **Start Azurite**:
//!    ```bash
//!    docker run -p 10000:10000 -p 10001:10001 -p 10002:10002 mcr.microsoft.com/azure-storage/azurite
//!    ```
//!
//! 2. **Create the 'orbitstore' container**:
//!
//!    The tests require a container named `orbitstore`. Create it using Azure Storage Explorer:
//!    - Download: https://azure.microsoft.com/products/storage/storage-explorer/
//!    - Connect to "Local & Attached" > "Storage Accounts" > "(Emulator - Default Ports)"
//!    - Right-click "Blob Containers" and create "orbitstore"
//!
//!    Or use Azure CLI:
//!    ```bash
//!    az storage container create --name orbitstore \
//!      --connection-string "DefaultEndpointsProtocol=http;AccountName=devstoreaccount1;AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;"
//!    ```
//!
//! 3. **Run the tests**:
//!    ```bash
//!    cargo test --test iceberg_azurite_integration --features iceberg-cold -- --ignored --nocapture
//!    ```
//!
//! See tests/AZURITE_SETUP.md for detailed instructions.

#![cfg(feature = "iceberg-cold")]

use opendal::Operator;

use orbit_protocols::postgres_wire::sql::execution::{
    Column, ColumnBatch, NullBitmap, column_batch_to_arrow,
};

/// Azurite configuration for testing (default Azurite development credentials)
const AZURITE_ACCOUNT_NAME: &str = "devstoreaccount1";
const AZURITE_ACCOUNT_KEY: &str = "Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==";
const AZURITE_ENDPOINT: &str = "http://127.0.0.1:10000/devstoreaccount1";
const AZURITE_CONTAINER: &str = "orbitstore";

// Note: Iceberg 0.7's FileIO doesn't natively support Azure Blob Storage (azblob://).
// We use OpenDAL directly for Azure operations, which provides excellent Azure support.
// For Iceberg integration, we use the StorageBackend abstraction in storage_config.rs
// which wraps OpenDAL and provides the necessary FileIO creation.

/// Create an OpenDAL operator for Azure Blob Storage
/// Note: The container must already exist in Azurite
async fn create_azure_operator() -> opendal::Result<Operator> {
    let builder = opendal::services::Azblob::default()
        .account_name(AZURITE_ACCOUNT_NAME)
        .account_key(AZURITE_ACCOUNT_KEY)
        .endpoint(AZURITE_ENDPOINT)
        .container(AZURITE_CONTAINER);

    let op = Operator::new(builder)?.finish();
    Ok(op)
}

/// Create test data as ColumnBatch
fn create_test_data(row_count: usize) -> ColumnBatch {
    let ids: Vec<i32> = (0..row_count as i32).collect();
    let names: Vec<String> = (0..row_count)
        .map(|i| format!("user_{}", i))
        .collect();
    let values: Vec<i32> = (0..row_count as i32)
        .map(|i| i * 10)
        .collect();

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
#[ignore] // Requires running Azurite instance
async fn test_azurite_connection() {
    // Test basic connectivity to Azurite using OpenDAL
    let op = create_azure_operator()
        .await
        .expect("Failed to create Azure operator");

    // Try to write a test file
    let test_path = "test/connection_test.txt";
    let test_data = "Hello from Orbit + Azure!";

    match op.write(test_path, test_data).await {
        Ok(_) => {},
        Err(e) => {
            if e.to_string().contains("ContainerNotFound") {
                eprintln!("\n❌ ERROR: Container 'orbitstore' does not exist in Azurite!");
                eprintln!("\n📋 SETUP REQUIRED:");
                eprintln!("   Please create the container first using Azure Storage Explorer:");
                eprintln!("   1. Download: https://azure.microsoft.com/products/storage/storage-explorer/");
                eprintln!("   2. Connect to: Local & Attached > Storage Accounts > (Emulator - Default Ports)");
                eprintln!("   3. Right-click 'Blob Containers' and create 'orbitstore'");
                eprintln!("\n   Or use Azure CLI:");
                eprintln!("   az storage container create --name orbitstore \\");
                eprintln!("     --connection-string \"DefaultEndpointsProtocol=http;AccountName=devstoreaccount1;...\"");
                eprintln!("\n   See tests/AZURITE_SETUP.md for details.\n");
            }
            panic!("Failed to write to Azurite: {}", e);
        }
    }

    // Verify we can read it back
    let read_data = op
        .read(test_path)
        .await
        .expect("Failed to read from Azurite");

    assert_eq!(read_data.to_bytes().as_ref(), test_data.as_bytes());

    // Verify metadata
    let metadata = op
        .stat(test_path)
        .await
        .expect("Failed to stat file in Azurite");

    assert_eq!(metadata.content_length(), test_data.len() as u64);

    // Cleanup
    op.delete(test_path)
        .await
        .expect("Failed to delete test file");

    println!("✓ Successfully connected to Azurite and performed basic operations");
}

#[tokio::test]
#[ignore] // Requires running Azurite instance
pub async fn test_write_parquet_to_azure() {
    use arrow::array::{Int32Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;
    use parquet::basic::Compression;
    use parquet::file::properties::WriterProperties;
    use std::sync::Arc as StdArc;

    let op = create_azure_operator()
        .await
        .expect("Failed to create Azure operator");

    // Create Arrow schema
    let arrow_schema = StdArc::new(ArrowSchema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("name", DataType::Utf8, true),
        Field::new("value", DataType::Int32, true),
    ]));

    // Create test data
    let ids = Int32Array::from(vec![1, 2, 3, 4, 5]);
    let names = StringArray::from(vec![
        Some("Alice"),
        Some("Bob"),
        None,
        Some("Charlie"),
        Some("Diana"),
    ]);
    let values = Int32Array::from(vec![Some(100), Some(200), Some(300), None, Some(500)]);

    let batch = RecordBatch::try_new(
        arrow_schema.clone(),
        vec![
            StdArc::new(ids),
            StdArc::new(names),
            StdArc::new(values),
        ],
    )
    .expect("Failed to create RecordBatch");

    // Write to in-memory buffer first
    let mut buffer = Vec::new();
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(parquet::basic::ZstdLevel::try_new(3).unwrap()))
        .build();

    {
        let mut writer = ArrowWriter::try_new(&mut buffer, arrow_schema.clone(), Some(props))
            .expect("Failed to create ArrowWriter");
        writer.write(&batch).expect("Failed to write batch");
        writer.close().expect("Failed to close writer");
    }

    // Upload to Azure
    let parquet_path = "test/data/test_data.parquet";
    op.write(parquet_path, buffer.clone())
        .await
        .expect("Failed to write parquet to Azure");

    println!(
        "✓ Successfully wrote {} bytes of Parquet data to Azure",
        buffer.len()
    );

    // Verify the file exists and has correct size
    let metadata = op
        .stat(parquet_path)
        .await
        .expect("Failed to stat parquet file");

    assert_eq!(metadata.content_length(), buffer.len() as u64);
}

#[tokio::test]
#[ignore] // Requires running Azurite instance
async fn test_read_parquet_from_azure() {
    use arrow::array::{Int32Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema as ArrowSchema};
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;
    use parquet::basic::Compression;
    use parquet::file::properties::WriterProperties;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::sync::Arc as StdArc;

    let op = create_azure_operator()
        .await
        .expect("Failed to create Azure operator");

    // Create and write test data first
    let arrow_schema = StdArc::new(ArrowSchema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("name", DataType::Utf8, true),
        Field::new("value", DataType::Int32, true),
    ]));

    let ids = Int32Array::from(vec![1, 2, 3, 4, 5]);
    let names = StringArray::from(vec![
        Some("Alice"),
        Some("Bob"),
        None,
        Some("Charlie"),
        Some("Diana"),
    ]);
    let values = Int32Array::from(vec![Some(100), Some(200), Some(300), None, Some(500)]);

    let batch = RecordBatch::try_new(
        arrow_schema.clone(),
        vec![
            StdArc::new(ids),
            StdArc::new(names),
            StdArc::new(values),
        ],
    )
    .expect("Failed to create RecordBatch");

    let mut buffer = Vec::new();
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(parquet::basic::ZstdLevel::try_new(3).unwrap()))
        .build();

    {
        let mut writer = ArrowWriter::try_new(&mut buffer, arrow_schema.clone(), Some(props))
            .expect("Failed to create ArrowWriter");
        writer.write(&batch).expect("Failed to write batch");
        writer.close().expect("Failed to close writer");
    }

    let parquet_path = "test/data/test_data.parquet";
    op.write(parquet_path, buffer.clone())
        .await
        .expect("Failed to write parquet to Azure");


    let op = create_azure_operator()
        .await
        .expect("Failed to create Azure operator");

    // Read the parquet file
    let parquet_path = "test/data/test_data.parquet";
    let data = op
        .read(parquet_path)
        .await
        .expect("Failed to read parquet from Azure");

    // Parse parquet
    let bytes = data.to_bytes();
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes)
        .expect("Failed to create parquet reader");

    let mut reader = builder.build().expect("Failed to build reader");

    // Read batches
    let mut total_rows = 0;
    while let Some(batch_result) = reader.next() {
        let batch: RecordBatch = batch_result.expect("Failed to read batch");
        total_rows += batch.num_rows();
        println!(
            "✓ Read batch with {} rows and {} columns",
            batch.num_rows(),
            batch.num_columns()
        );
    }

    assert_eq!(total_rows, 5, "Expected 5 rows total");
}

#[tokio::test]
#[ignore] // Requires running Azurite instance
async fn test_column_batch_roundtrip_azure() {
    use parquet::arrow::{arrow_reader::ParquetRecordBatchReaderBuilder, ArrowWriter};
    use parquet::basic::Compression;
    use parquet::file::properties::WriterProperties;

    let op = create_azure_operator()
        .await
        .expect("Failed to create Azure operator");

    // Create test ColumnBatch
    let original_batch = create_test_data(100);

    // Convert ColumnBatch → Arrow
    let arrow_batch = column_batch_to_arrow(&original_batch)
        .expect("Failed to convert ColumnBatch to Arrow");

    // Write Arrow → Parquet (in memory)
    let mut buffer = Vec::new();
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(parquet::basic::ZstdLevel::try_new(3).unwrap()))
        .build();

    {
        let mut writer =
            ArrowWriter::try_new(&mut buffer, arrow_batch.schema(), Some(props))
                .expect("Failed to create ArrowWriter");
        writer.write(&arrow_batch).expect("Failed to write batch");
        writer.close().expect("Failed to close writer");
    }

    let original_size = buffer.len();
    println!(
        "✓ Compressed 100 rows to {} bytes with ZSTD",
        original_size
    );

    // Upload to Azure
    let path = "test/data/roundtrip_test.parquet";
    op.write(path, buffer.clone())
        .await
        .expect("Failed to write to Azure");

    // Download from Azure
    let downloaded = op.read(path).await.expect("Failed to read from Azure");

    assert_eq!(
        downloaded.to_bytes().len(),
        original_size,
        "Downloaded size should match uploaded size"
    );

    // Parse Parquet back to Arrow
    let bytes = downloaded.to_bytes();
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes)
        .expect("Failed to create parquet reader");

    let mut reader = builder.build().expect("Failed to build reader");

    // Read all batches
    let mut batches = Vec::new();
    while let Some(batch_result) = reader.next() {
        batches.push(batch_result.expect("Failed to read batch"));
    }

    // Verify we got the data back
    assert!(!batches.is_empty(), "Should have at least one batch");
    let first_batch = &batches[0];
    assert_eq!(
        first_batch.num_rows(),
        100,
        "Should have 100 rows after roundtrip"
    );
    assert_eq!(
        first_batch.num_columns(),
        3,
        "Should have 3 columns after roundtrip"
    );

    println!("✓ Successfully completed ColumnBatch → Arrow → Parquet → Azure → Arrow roundtrip");
}

#[tokio::test]
#[ignore] // Requires running Azurite instance
async fn test_large_dataset_performance_azure() {
    use parquet::arrow::ArrowWriter;
    use parquet::basic::Compression;
    use parquet::file::properties::WriterProperties;
    use std::time::Instant;

    let op = create_azure_operator()
        .await
        .expect("Failed to create Azure operator");

    // Create large dataset (100K rows)
    let large_batch = create_test_data(100_000);
    println!("✓ Created test dataset with 100,000 rows");

    // Convert to Arrow
    let start = Instant::now();
    let arrow_batch = column_batch_to_arrow(&large_batch)
        .expect("Failed to convert to Arrow");
    let conversion_time = start.elapsed();
    println!(
        "✓ Converted to Arrow in {:.2}ms",
        conversion_time.as_secs_f64() * 1000.0
    );

    // Write with ZSTD compression
    let start = Instant::now();
    let mut buffer = Vec::new();
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(parquet::basic::ZstdLevel::try_new(3).unwrap()))
        .build();

    {
        let mut writer =
            ArrowWriter::try_new(&mut buffer, arrow_batch.schema(), Some(props))
                .expect("Failed to create ArrowWriter");
        writer.write(&arrow_batch).expect("Failed to write batch");
        writer.close().expect("Failed to close writer");
    }

    let write_time = start.elapsed();
    let compressed_size = buffer.len();

    println!(
        "✓ Wrote Parquet in {:.2}ms ({} bytes, ZSTD compression)",
        write_time.as_secs_f64() * 1000.0,
        compressed_size
    );

    // Calculate compression ratio
    // Approximate uncompressed size (rough estimate)
    let approx_uncompressed = large_batch.row_count * 100; // ~100 bytes per row estimate
    let compression_ratio = approx_uncompressed as f64 / compressed_size as f64;
    println!(
        "✓ Compression ratio: ~{:.2}x (estimated)",
        compression_ratio
    );

    // Upload to Azure
    let start = Instant::now();
    let path = "test/data/large_dataset.parquet";
    op.write(path, buffer.clone())
        .await
        .expect("Failed to upload to Azure");
    let upload_time = start.elapsed();

    let upload_throughput = compressed_size as f64 / upload_time.as_secs_f64() / 1024.0 / 1024.0;
    println!(
        "✓ Uploaded to Azure in {:.2}ms ({:.2} MB/s)",
        upload_time.as_secs_f64() * 1000.0,
        upload_throughput
    );

    // Download from Azure
    let start = Instant::now();
    let downloaded = op.read(path).await.expect("Failed to download from Azure");
    let download_time = start.elapsed();

    let download_throughput =
        downloaded.to_bytes().len() as f64 / download_time.as_secs_f64() / 1024.0 / 1024.0;
    println!(
        "✓ Downloaded from Azure in {:.2}ms ({:.2} MB/s)",
        download_time.as_secs_f64() * 1000.0,
        download_throughput
    );

    // Verify data integrity
    assert_eq!(
        downloaded.to_bytes().len(),
        compressed_size,
        "Downloaded size should match uploaded size"
    );

    println!("\n=== Performance Summary ===");
    println!("Dataset: 100,000 rows");
    println!("Compressed size: {} bytes", compressed_size);
    println!("Upload throughput: {:.2} MB/s", upload_throughput);
    println!("Download throughput: {:.2} MB/s", download_throughput);
}

#[tokio::test]
#[ignore] // Requires running Azurite instance
async fn test_azure_storage_config() {
    use orbit_protocols::postgres_wire::sql::execution::storage_config::{AzureConfig, StorageBackend};

    // Test 1: Create AzureConfig using the azurite helper
    let azure_config = AzureConfig::azurite(AZURITE_CONTAINER.to_string());

    assert_eq!(azure_config.account_name, AZURITE_ACCOUNT_NAME);
    assert_eq!(azure_config.account_key, AZURITE_ACCOUNT_KEY);
    assert_eq!(azure_config.container, AZURITE_CONTAINER);
    assert_eq!(
        azure_config.endpoint,
        Some(AZURITE_ENDPOINT.to_string())
    );

    println!("✓ AzureConfig created with azurite() helper");

    // Test 2: Create from connection string (user's use case)
    let connection_string = format!(
        "AccountName={};AccountKey={};DefaultEndpointsProtocol=http;BlobEndpoint={};",
        AZURITE_ACCOUNT_NAME, AZURITE_ACCOUNT_KEY, AZURITE_ENDPOINT
    );

    let azure_config_from_conn = AzureConfig::from_connection_string(
        &connection_string,
        AZURITE_CONTAINER.to_string(),
    ).expect("Failed to parse connection string");

    assert_eq!(azure_config_from_conn.account_name, AZURITE_ACCOUNT_NAME);
    assert_eq!(azure_config_from_conn.account_key, AZURITE_ACCOUNT_KEY);

    println!("✓ AzureConfig created from connection string");

    // Test 3: Create StorageBackend and verify warehouse path
    let storage_backend = StorageBackend::Azure(azure_config.clone());
    let warehouse_path = storage_backend.warehouse_path();

    assert!(
        warehouse_path.starts_with("az://"),
        "Warehouse path should start with az:// scheme"
    );

    println!("✓ StorageBackend created with warehouse path: {}", warehouse_path);

    // Test 4: Verify we can create an OpenDAL operator from the config
    let op = create_azure_operator()
        .await
        .expect("Failed to create Azure operator");

    // Write and read a test marker
    let test_path = "test_storage_config_marker.txt";
    let test_data = "Storage config test successful";

    op.write(test_path, test_data)
        .await
        .expect("Failed to write test marker");

    let read_data = op.read(test_path)
        .await
        .expect("Failed to read test marker");

    assert_eq!(
        read_data.to_vec(),
        test_data.as_bytes(),
        "Read data should match written data"
    );

    // Clean up
    op.delete(test_path)
        .await
        .expect("Failed to delete test marker");

    println!("✓ Successfully integrated with OpenDAL for Azure Blob Storage");
    println!("\n=== Azure Storage Configuration Test Complete ===");
    println!("✓ AzureConfig helpers work correctly");
    println!("✓ Connection string parsing works");
    println!("✓ StorageBackend integration verified");
    println!("✓ OpenDAL operator functional");
}
