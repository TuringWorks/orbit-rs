# Azurite Setup for Integration Tests

## Prerequisites

1. **Start Azurite**:
   ```bash
   docker run -p 10000:10000 -p 10001:10001 -p 10002:10002 \
     mcr.microsoft.com/azure-storage/azurite
   ```

   Or install globally:
   ```bash
   npm install -g azurite
   azurite --silent --location /tmp/azurite --blobPort 10000
   ```

2. **Create the test container**:

   The tests require a container named `orbitstore` to exist. You can create it using:

   ### Option 1: Azure Storage Explorer (Recommended)
   1. Download [Azure Storage Explorer](https://azure.microsoft.com/en-us/products/storage/storage-explorer/)
   2. Connect to Azurite (Local & Attached > Storage Accounts > (Emulator - Default Ports))
   3. Right-click on "Blob Containers" and create a new container named `orbitstore`

   ### Option 2: Azure CLI
   ```bash
   az storage container create \
     --name orbitstore \
     --connection-string "DefaultEndpointsProtocol=http;AccountName=devstoreaccount1;AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;"
   ```

   ### Option 3: REST API (curl)
   ```bash
   # This requires proper Azure Shared Key authentication which is complex
   # Use Azure Storage Explorer instead
   ```

   ### Option 4: Python Script
   ```python
   from azure.storage.blob import BlobServiceClient

   connection_string = "DefaultEndpointsProtocol=http;AccountName=devstoreaccount1;AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;"

   blob_service_client = BlobServiceClient.from_connection_string(connection_string)

   try:
       container_client = blob_service_client.create_container("orbitstore")
       print("✓ Container created")
   except Exception as e:
       if "ContainerAlreadyExists" in str(e):
           print("✓ Container already exists")
       else:
           raise
   ```

## Running Tests

Once Azurite is running and the container is created:

```bash
cd protocols
cargo test --test iceberg_azurite_integration --features iceberg-cold -- --ignored --nocapture
```

## Available Tests

- `test_azurite_connection` - Basic connectivity test
- `test_write_parquet_to_azure` - Write Parquet files to Azure
- `test_read_parquet_from_azure` - Read Parquet files from Azure
- `test_column_batch_roundtrip_azure` - Full data roundtrip test
- `test_large_dataset_performance_azure` - Performance benchmark (100K rows)
- `test_azure_file_io_with_iceberg` - Iceberg FileIO creation

## Troubleshooting

### Container Not Found Error
```
AzblobError { code: "ContainerNotFound", message: "The specified container does not exist."
```

**Solution**: Create the `orbitstore` container using one of the methods above.

### Connection Refused
```
Failed to create Azure operator: connect error
```

**Solution**: Ensure Azurite is running on port 10000.

### Authentication Failed
```
AuthorizationFailure: Server failed to authenticate the request
```

**Solution**: This usually means the container doesn't exist. Create it first.

##Default Azurite Credentials

- **Account Name**: `devstoreaccount1`
- **Account Key**: `Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==`
- **Blob Endpoint**: `http://127.0.0.1:10000/devstoreaccount1`
- **Container**: `orbitstore`
