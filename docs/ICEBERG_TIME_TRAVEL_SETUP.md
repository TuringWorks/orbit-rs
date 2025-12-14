# Iceberg Time Travel Setup Guide

This guide explains how to configure Apache Iceberg for time travel queries in Orbit-RS.

## Overview

Orbit-RS supports SQL time travel queries using Apache Iceberg as the cold tier storage backend. Time travel allows you to query historical versions of your data using familiar SQL syntax.

### Supported Syntax

```sql
-- Snowflake-compatible syntax
SELECT * FROM orders AT(TIMESTAMP => '2025-01-01 00:00:00');
SELECT * FROM orders AT(VERSION => 2583872980615177898);
SELECT * FROM orders AT(SNAPSHOT => 2583872980615177898);

-- SQL:2011 temporal syntax
SELECT * FROM orders FOR SYSTEM_TIME AS OF TIMESTAMP '2025-01-01';

-- With JOINs and aliases
SELECT o.*, c.name
FROM orders AT(TIMESTAMP => '2025-01-01') o
JOIN customers c ON o.customer_id = c.id;

-- Restore dropped tables
UNDROP TABLE deleted_orders;
```

## Prerequisites

1. **Iceberg REST Catalog**: A running Iceberg REST catalog service
2. **Object Storage**: S3, MinIO, Azure Blob Storage, or GCS
3. **Feature Flag**: Build with `--features storage-iceberg`

## Quick Start with MinIO (Local Development)

### 1. Start MinIO (Object Storage)

```bash
docker run -d \
  --name minio \
  -p 9000:9000 \
  -p 9001:9001 \
  -e MINIO_ROOT_USER=minioadmin \
  -e MINIO_ROOT_PASSWORD=minioadmin \
  minio/minio server /data --console-address ":9001"
```

Create a bucket:
```bash
# Using MinIO CLI
mc alias set local http://localhost:9000 minioadmin minioadmin
mc mb local/orbit-warehouse
```

### 2. Start Iceberg REST Catalog

```bash
docker run -d \
  --name iceberg-rest \
  -p 8181:8181 \
  -e CATALOG_WAREHOUSE=s3://orbit-warehouse/ \
  -e CATALOG_IO__IMPL=org.apache.iceberg.aws.s3.S3FileIO \
  -e CATALOG_S3_ENDPOINT=http://host.docker.internal:9000 \
  -e AWS_ACCESS_KEY_ID=minioadmin \
  -e AWS_SECRET_ACCESS_KEY=minioadmin \
  -e AWS_REGION=us-east-1 \
  tabulario/iceberg-rest
```

### 3. Configure Orbit-RS

Edit `config/orbit-server.toml`:

```toml
[unified_storage.cold_tier]
enabled = true
backend = "minio"
data_format = "iceberg"
partition_strategy = "time"
compression = "snappy"

[unified_storage.cold_tier.s3]
bucket = "orbit-warehouse"
region = "us-east-1"
access_key_id = "minioadmin"
secret_access_key = "minioadmin"
endpoint = "http://localhost:9000"

[unified_storage.cold_tier.iceberg]
catalog_uri = "http://localhost:8181"
default_namespace = "orbit"
ssl_enabled = false
timeout_seconds = 30
```

### 4. Build and Run

```bash
cargo build --release --features storage-iceberg
./target/release/orbit-server --config config/orbit-server.toml
```

## Production Setup with AWS S3

### Configuration

```toml
[unified_storage.cold_tier]
enabled = true
backend = "s3"
data_format = "iceberg"
partition_strategy = "time"
compression = "zstd"

[unified_storage.cold_tier.s3]
bucket = "my-company-orbit-warehouse"
region = "us-west-2"
access_key_id = "${AWS_ACCESS_KEY_ID}"      # Use environment variables
secret_access_key = "${AWS_SECRET_ACCESS_KEY}"

[unified_storage.cold_tier.iceberg]
catalog_uri = "https://iceberg-catalog.my-company.com"
warehouse_path = "s3://my-company-orbit-warehouse/iceberg"
default_namespace = "production"
ssl_enabled = true
timeout_seconds = 60
```

### Using AWS Glue as Iceberg Catalog

AWS Glue can serve as an Iceberg catalog. Configure the REST catalog endpoint to point to your Glue-compatible service or use a REST catalog wrapper.

## Production Setup with Azure Blob Storage

```toml
[unified_storage.cold_tier]
enabled = true
backend = "azure"
data_format = "iceberg"
partition_strategy = "time"
compression = "zstd"

[unified_storage.cold_tier.azure]
account_name = "mycompanystorage"
container_name = "orbit-warehouse"
access_key = "${AZURE_STORAGE_KEY}"

[unified_storage.cold_tier.iceberg]
catalog_uri = "https://iceberg-catalog.mycompany.com"
warehouse_path = "abfss://orbit-warehouse@mycompanystorage.dfs.core.windows.net/iceberg"
default_namespace = "production"
ssl_enabled = true
timeout_seconds = 60
```

## Configuration Reference

### `[unified_storage.cold_tier.iceberg]`

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `catalog_uri` | String | Yes | - | REST catalog endpoint URL |
| `warehouse_path` | String | No | Auto-generated | Custom warehouse path for Iceberg tables |
| `default_namespace` | String | No | `"default"` | Default namespace for tables |
| `ssl_enabled` | Boolean | No | `false` | Enable SSL/TLS for catalog communication |
| `timeout_seconds` | Integer | No | `30` | Request timeout for catalog operations |

### Auto-generated Warehouse Path

If `warehouse_path` is not specified, it is automatically generated:
- S3/MinIO: `s3://{bucket}/warehouse`
- Azure: `az://{container}/warehouse`
- GCS: `gs://{bucket}/warehouse`

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Orbit-RS Server                        │
├─────────────────────────────────────────────────────────────┤
│  SQL Parser                                                 │
│  ├─ AT(TIMESTAMP => ...) → TimeTravelClause::Timestamp     │
│  ├─ AT(VERSION => ...)   → TimeTravelClause::Version       │
│  └─ FOR SYSTEM_TIME AS OF → TimeTravelClause::SystemTime   │
├─────────────────────────────────────────────────────────────┤
│  MVCC Executor                                              │
│  └─ Routes time travel queries to IcebergColdStore         │
├─────────────────────────────────────────────────────────────┤
│  IcebergColdStore                                           │
│  ├─ query_as_of(timestamp) → Historical data               │
│  ├─ query_by_snapshot_id(id) → Specific version            │
│  ├─ list_snapshots() → Available versions                  │
│  └─ current_snapshot() → Latest version                    │
├─────────────────────────────────────────────────────────────┤
│  Iceberg REST Catalog                                       │
│  └─ Manages table metadata, snapshots, and schema          │
├─────────────────────────────────────────────────────────────┤
│  Object Storage (S3/Azure/GCS/MinIO)                        │
│  └─ Stores Parquet data files                              │
└─────────────────────────────────────────────────────────────┘
```

## Verifying the Setup

### 1. Check Catalog Connectivity

```bash
curl http://localhost:8181/v1/config
```

Expected response:
```json
{
  "defaults": {},
  "overrides": {}
}
```

### 2. List Namespaces

```bash
curl http://localhost:8181/v1/namespaces
```

### 3. Test Time Travel Query

```sql
-- Connect via psql
psql -h localhost -p 5432 -U postgres

-- Create and populate a table (data will tier to Iceberg)
CREATE TABLE orders (id INT, status TEXT, created_at TIMESTAMP);
INSERT INTO orders VALUES (1, 'pending', NOW());

-- Wait for tier migration or manually trigger
-- Then query historical data
SELECT * FROM orders AT(TIMESTAMP => '2025-01-01 00:00:00');
```

## Troubleshooting

### "No Iceberg catalog configured"

Ensure:
1. `data_format = "iceberg"` is set
2. `[unified_storage.cold_tier.iceberg]` section exists
3. `catalog_uri` is specified

### "Failed to connect to catalog"

Check:
1. Catalog service is running: `curl http://localhost:8181/v1/config`
2. Network connectivity between Orbit-RS and catalog
3. SSL settings match catalog configuration

### "No snapshot found at timestamp"

The table might not have data at the requested timestamp. Use:
```sql
-- List available snapshots (when implemented)
SELECT * FROM iceberg_snapshots('orders');
```

### "Table not found in Iceberg"

Ensure:
1. Table has been migrated to cold tier
2. Correct namespace is configured
3. Table exists in the catalog: `curl http://localhost:8181/v1/namespaces/{namespace}/tables`

## Performance Considerations

1. **Snapshot Retention**: Configure appropriate snapshot retention in Iceberg to balance storage costs and time travel range

2. **Partition Strategy**: Use time-based partitioning for time-series data to optimize time travel queries

3. **Metadata Caching**: The IcebergColdStore caches table metadata to reduce catalog calls

4. **Concurrent Queries**: Time travel queries are read-only and can run concurrently without blocking writes

## Related Documentation

- [Iceberg Official Docs](https://iceberg.apache.org/docs/latest/)
- [Iceberg REST Catalog Spec](https://iceberg.apache.org/docs/latest/rest-catalog/)
- [Orbit-RS Storage Architecture](docs/content/architecture/ORBIT_ARCHITECTURE.md)
- [PRD - Time Travel Features](specifications/PRD.md)
