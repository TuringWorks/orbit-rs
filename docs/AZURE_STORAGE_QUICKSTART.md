# Azure Storage with Iceberg - Quick Start

## What Was Added

Azure Blob Storage support has been added to Orbit's Iceberg cold tier integration. You can now use Azure Blob Storage (or Azurite emulator) alongside S3/MinIO for cold tier storage.

## Your Connection String

The implementation supports parsing Azure connection strings exactly as you provided:

```text
AccountName=devstoreaccount1;
AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;
DefaultEndpointsProtocol=http;
BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;
QueueEndpoint=http://127.0.0.1:10001/devstoreaccount1;
TableEndpoint=http://127.0.0.1:10002/devstoreaccount1;
```

## Quick Usage

```rust
use orbit_protocols::postgres_wire::sql::execution::{
    AzureConfig, StorageBackend, IcebergColdStore,
    create_rest_catalog_with_storage,
};

// Parse your connection string
let azure_config = AzureConfig::from_connection_string(
    "AccountName=devstoreaccount1;AccountKey=...;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;",
    "orbitstore".to_string(),  // container name
)?;

// Create storage backend
let storage_backend = StorageBackend::Azure(azure_config);

// Create REST catalog with Azure backend
let catalog = create_rest_catalog_with_storage(
    "http://localhost:8181",  // REST catalog URI
    storage_backend,
).await?;

// Create Iceberg cold store
let cold_store = IcebergColdStore::new(catalog, "default", "table_name").await?;

// Query data
let batches = cold_store.scan(None).await?;
```

## Files to Check

1. **Documentation**: `protocols/docs/AZURE_ICEBERG_SETUP.md`
2. **Implementation Summary**: `protocols/docs/AZURE_IMPLEMENTATION_SUMMARY.md`
3. **Integration Tests**: `protocols/tests/iceberg_azurite_integration.rs`
4. **Example Application**: `protocols/examples/azure_iceberg_example.rs`
5. **Storage Config API**: `protocols/src/postgres_wire/sql/execution/storage_config.rs`

## Running Tests

```bash
# Start Azurite emulator
docker run -p 10000:10000 -p 10001:10001 -p 10002:10002 \
  mcr.microsoft.com/azure-storage/azurite

# Run integration tests
cd protocols
cargo test --test iceberg_azurite_integration --features iceberg-cold -- --ignored
```

## Running Example

```bash
cd protocols
cargo run --example azure_iceberg_example --features iceberg-cold
```

## What's Included

 **Full Iceberg Catalog Support**: Complete REST catalog integration with Azure
 **Connection String Parsing**: Parses Azure connection strings
 **Azurite Support**: Pre-configured defaults for local development
 **Production Azure**: Support for Azure Storage accounts
 **Integration Tests**: Full test suite with 100K row performance tests
 **OpenDAL Integration**: Uses OpenDAL's Azblob service
 **Storage Abstraction**: Unified API for S3 and Azure
 **Documentation**: Complete setup and usage guides

## Key Components

### 1. AzureConfig

Three ways to create it:

```rust
// From connection string (your use case)
let config = AzureConfig::from_connection_string(conn_str, container)?;

// Azurite defaults
let config = AzureConfig::azurite("orbitstore".to_string());

// Production Azure
let config = AzureConfig::azure_storage(account_name, account_key, container);
```

### 2. StorageBackend

Supports both S3 and Azure:

```rust
let backend = StorageBackend::Azure(azure_config);
// or
let backend = StorageBackend::S3(s3_config);
```

### 3. REST Catalog Creation

One function creates a fully configured catalog for any backend:

```rust
let catalog = create_rest_catalog_with_storage(catalog_uri, backend).await?;
```

### 4. IcebergColdStore

Use the catalog to create a cold store for table operations:

```rust
let cold_store = IcebergColdStore::new(catalog, "namespace", "table").await?;
let batches = cold_store.scan(None).await?;
```

## Prerequisites

To use the full Iceberg integration, you need:

1. **Azurite** (for local development):

   ```bash
   docker run -p 10000:10000 mcr.microsoft.com/azure-storage/azurite
   ```

2. **Iceberg REST Catalog**:

   ```bash
   docker run -p 8181:8181 tabulario/iceberg-rest:latest
   ```

## Next Steps

The implementation is complete! You can now:

1. Start the required services (Azurite + REST catalog)
2. Use `create_rest_catalog_with_storage()` to create a catalog with Azure backend
3. Create an `IcebergColdStore` for table operations
4. Enjoy schema evolution, time travel, and all Iceberg features with Azure storage!

---

For detailed information, see the comprehensive documentation in:

- `protocols/docs/AZURE_ICEBERG_SETUP.md` - Full setup guide
- `protocols/docs/AZURE_IMPLEMENTATION_SUMMARY.md` - Implementation details
