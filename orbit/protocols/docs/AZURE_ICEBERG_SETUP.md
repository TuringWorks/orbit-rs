# Azure Blob Storage Integration with Iceberg Cold Tier

This guide explains how to configure and use Azure Blob Storage (or Azurite emulator) as the storage backend for Orbit's Iceberg cold tier.

## Overview

Orbit's Iceberg cold tier now supports multiple storage backends:
- **S3-compatible storage** (MinIO, AWS S3)
- **Azure Blob Storage** (Azurite, Azure Storage) ✨ NEW

## Prerequisites

### For Local Development (Azurite)

1. **Start Azurite emulator**:
   ```bash
   docker run -p 10000:10000 -p 10001:10001 -p 10002:10002 \
     mcr.microsoft.com/azure-storage/azurite
   ```

2. **Start Iceberg REST Catalog**:
   ```bash
   docker run -p 8181:8181 \
     -e AWS_REGION=us-east-1 \
     tabulario/iceberg-rest:latest
   ```

### For Production (Azure Storage)

1. Create an Azure Storage account
2. Create a container (e.g., `orbitstore`)
3. Get your account name and access key

## Configuration

### Method 1: Using Azurite (Local Development)

```rust
use orbit_protocols::postgres_wire::sql::execution::{
    AzureConfig, StorageBackend, create_rest_catalog_with_storage
};

// Create Azurite configuration with default development credentials
let azure_config = AzureConfig::azurite("orbitstore".to_string());
let storage_backend = StorageBackend::Azure(azure_config);

// Create REST catalog with Azure backend
let catalog = create_rest_catalog_with_storage(
    "http://localhost:8181",  // REST catalog URI
    storage_backend,
).await?;
```

### Method 2: Using Production Azure Storage

```rust
let azure_config = AzureConfig::azure_storage(
    "mystorageaccount".to_string(),  // Account name
    "your-account-key".to_string(),  // Account key
    "orbitstore".to_string(),        // Container name
);

let storage_backend = StorageBackend::Azure(azure_config);
let catalog = create_rest_catalog_with_storage(
    "http://localhost:8181",
    storage_backend,
).await?;
```

### Method 3: Using Connection String

This is the connection string format you provided:

```rust
let connection_string = "AccountName=devstoreaccount1;\
    AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;\
    DefaultEndpointsProtocol=http;\
    BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;\
    QueueEndpoint=http://127.0.0.1:10001/devstoreaccount1;\
    TableEndpoint=http://127.0.0.1:10002/devstoreaccount1;";

let azure_config = AzureConfig::from_connection_string(
    connection_string,
    "orbitstore".to_string(),
)?;

let storage_backend = StorageBackend::Azure(azure_config);
```

## Complete Usage Example

```rust
use std::sync::Arc;
use orbit_protocols::postgres_wire::sql::execution::{
    AzureConfig, StorageBackend, IcebergColdStore,
    create_rest_catalog_with_storage,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure Azure storage using connection string
    let connection_string = "AccountName=devstoreaccount1;\
        AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;\
        DefaultEndpointsProtocol=http;\
        BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;";

    let azure_config = AzureConfig::from_connection_string(
        connection_string,
        "orbitstore".to_string(),
    )?;

    // 2. Create storage backend
    let storage_backend = StorageBackend::Azure(azure_config);

    // 3. Create REST catalog with Azure backend
    let catalog = create_rest_catalog_with_storage(
        "http://localhost:8181",
        storage_backend,
    ).await?;

    // 4. Create Iceberg cold store
    let cold_store = IcebergColdStore::new(
        catalog,
        "default",      // namespace
        "user_events",  // table name
    ).await?;

    // 5. Query data
    let batches = cold_store.scan(None).await?;
    println!("Retrieved {} batches", batches.len());

    // 6. Process results
    for batch in batches {
        println!("Batch: {} rows × {} columns",
            batch.num_rows(),
            batch.num_columns()
        );
    }

    Ok(())
}
```

## Testing

### Running Integration Tests

```bash
# Start Azurite
docker run -p 10000:10000 -p 10001:10001 -p 10002:10002 \
  mcr.microsoft.com/azure-storage/azurite

# Run tests
cargo test --test iceberg_azurite_integration --features iceberg-cold -- --ignored
```

### Available Tests

- `test_azurite_connection` - Basic connectivity test
- `test_write_parquet_to_azure` - Write Parquet files to Azure
- `test_read_parquet_from_azure` - Read Parquet files from Azure
- `test_column_batch_roundtrip_azure` - Full data roundtrip test
- `test_large_dataset_performance_azure` - Performance benchmark (100K rows)

## Environment Variables

For production deployments, you can use environment variables:

```bash
export AZURE_STORAGE_ACCOUNT=mystorageaccount
export AZURE_STORAGE_KEY=your-account-key-here
export AZURE_CONTAINER=orbitstore
export ICEBERG_CATALOG_URI=http://localhost:8181
```

Then in your code:

```rust
let account_name = std::env::var("AZURE_STORAGE_ACCOUNT")?;
let account_key = std::env::var("AZURE_STORAGE_KEY")?;
let container = std::env::var("AZURE_CONTAINER")?;

let azure_config = AzureConfig::azure_storage(
    account_name,
    account_key,
    container,
);
```

## Storage Backend Comparison

| Feature | S3/MinIO | Azure Blob Storage |
|---------|----------|-------------------|
| Local Development | MinIO | Azurite |
| Connection String Support | ❌ | ✅ |
| Path-Style Access | ✅ (required for MinIO) | N/A |
| Endpoint Override | ✅ | ✅ |
| Default Credentials | MinIO defaults | Azurite defaults |

## Architecture

```
┌─────────────────────────────────────────────────┐
│            Orbit Hybrid Storage                 │
├─────────────────────────────────────────────────┤
│  Hot Tier (Row-based) → Warm Tier → Cold Tier  │
└──────────────────────┬──────────────────────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │ Iceberg Cold Store   │
            └──────────┬───────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │   REST Catalog       │
            │  (Table Metadata)    │
            └──────────┬───────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │  Storage Backend     │
            │  (Configurable)      │
            └──────────┬───────────┘
                       │
          ┌────────────┴────────────┐
          ▼                         ▼
    ┌─────────┐            ┌──────────────┐
    │   S3    │            │    Azure     │
    │  MinIO  │            │ Blob Storage │
    └─────────┘            └──────────────┘
```

## Data Flow

1. **Write Path** (planned for Phase 3):
   ```
   ColumnBatch → Arrow → Parquet → Azure Blob Storage
   ```

2. **Read Path** (currently implemented):
   ```
   Azure Blob Storage → Parquet → Arrow → ColumnBatch
   ```

## Performance Considerations

Based on integration tests with 100K rows:

- **Compression**: ZSTD level 3 provides excellent compression ratios (~10-20x)
- **Upload/Download**: Performance depends on network and Azure endpoint
- **Local (Azurite)**: ~50-100 MB/s throughput
- **Production (Azure)**: Varies by region and network

## Troubleshooting

### Connection Errors

```
Error: Failed to create Azure FileIO
```

**Solution**: Verify Azurite is running and accessible:
```bash
curl http://127.0.0.1:10000/devstoreaccount1?restype=account&comp=properties
```

### Authentication Errors

```
Error: Azure authentication failed
```

**Solution**: Check your account name and key are correct. For Azurite, ensure you're using the default credentials.

### Container Not Found

```
Error: Container 'orbitstore' not found
```

**Solution**: Create the container first using Azure Storage Explorer or:
```bash
az storage container create --name orbitstore --account-name devstoreaccount1
```

## Additional Resources

- [Apache Iceberg Documentation](https://iceberg.apache.org/)
- [Azurite GitHub](https://github.com/Azure/Azurite)
- [Azure Blob Storage Documentation](https://docs.microsoft.com/en-us/azure/storage/blobs/)
- [Orbit Iceberg Integration](../src/postgres_wire/sql/execution/iceberg_cold.rs)

## Examples

Run the complete example:

```bash
cargo run --example azure_iceberg_example --features iceberg-cold
```

See also:
- `protocols/tests/iceberg_azurite_integration.rs` - Integration tests
- `protocols/examples/azure_iceberg_example.rs` - Usage examples
- `protocols/src/postgres_wire/sql/execution/storage_config.rs` - Configuration API
