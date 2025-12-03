# MinIO Tiered Storage Test Setup

## Summary of Changes

### 1. Fixed Compilation Errors
- **Issue**: Missing `UnifiedStorageError` import in `tiered.rs`
- **Fix**: Added `UnifiedStorageError` to imports in `orbit/engine/src/unified/tiered.rs`

### 2. Fixed S3 Path-Style Access Bug
- **Issue**: S3Backend was incorrectly using virtual-host-style URLs (`http://bucket.localhost:9000`) instead of path-style URLs (`http://localhost:9000/bucket`) when `path_style_access` was set to `true`
- **Location**: `orbit/engine/src/unified/s3_backend.rs` line 141-143
- **Fix**: Inverted the logic - now only enables virtual-host-style when `path_style_access` is `false`
- **Impact**: This was preventing all MinIO operations from working

### 3. Added Bucket Creation Logic
- **Location**: `orbit/engine/src/unified/s3_backend.rs` in `initialize()` method
- **Behavior**: Attempts to detect if bucket doesn't exist and provides clear error message
- **Note**: MinIO doesn't support auto-bucket-creation via S3 API, so manual creation is required

## Prerequisites

### MinIO Must Be Running
```bash
# Check if MinIO is accessible
curl http://localhost:9000/minio/health/live
```

### Create the Test Bucket

The bucket `orbit-cold-storage` must be created before running tests. Here are several methods:

#### Method 1: Using MinIO Client (mc)
```bash
# If mc is installed
mc alias set local http://localhost:9000 minioadmin minioadmin
mc mb local/orbit-cold-storage
mc ls local/
```

#### Method 2: Using MinIO Web Console
1. Open http://localhost:9001 in your browser
2. Login with minioadmin/minioadmin
3. Click "Buckets" → "Create Bucket"
4. Enter name: `orbit-cold-storage`
5. Click "Create"

#### Method 3: Using AWS CLI (if configured for MinIO)
```bash
aws --endpoint-url http://localhost:9000 s3 mb s3://orbit-cold-storage
```

#### Method 4: Using Python (if minio package is installed)
```bash
pip install minio
python3 create_bucket.py  # Script provided in repo root
```

## Running the Tests

The MinIO tests have been moved to the `orbit-integration-tests` package and require the `minio-tests` feature flag to run.

### Run Setup Test First
```bash
cargo test -p orbit-integration-tests --features minio-tests --test tiered_storage_minio_tests setup_minio_bucket -- --exact --nocapture --ignored
```

This will verify the bucket exists and is accessible.

### Run Individual Tests
```bash
# Basic S3 operations
cargo test -p orbit-integration-tests --features minio-tests --test tiered_storage_minio_tests test_s3_backend_basic_operations -- --exact --nocapture --ignored

# Scan prefix functionality
cargo test -p orbit-integration-tests --features minio-tests --test tiered_storage_minio_tests test_s3_backend_scan_prefix -- --exact --nocapture --ignored

# Hot to warm tier propagation
cargo test -p orbit-integration-tests --features minio-tests --test tiered_storage_minio_tests test_hot_to_warm_propagation -- --exact --nocapture --ignored
```

### Run All MinIO Tests
```bash
cargo test -p orbit-integration-tests --features minio-tests --test tiered_storage_minio_tests -- --nocapture --test-threads=1 --ignored
```

**Note**: Use `--test-threads=1` to avoid race conditions when multiple tests access the same bucket.

### Why Feature Flag?
The MinIO tests require an external MinIO server to be running, so they are gated behind a feature flag to prevent them from cluttering regular test output. Without `--features minio-tests`, these tests won't even compile.

## Test Coverage

The test suite includes:

1. **S3 Backend Tests**
   - `test_s3_backend_basic_operations` - PUT, GET, DELETE, EXISTS operations
   - `test_s3_backend_scan_prefix` - Prefix scanning and filtering

2. **Tiered Storage Tests**
   - `test_hot_to_warm_propagation` - LRU eviction from hot to warm tier
   - `test_warm_to_cold_archival` - Manual archival to cold storage
   - `test_cold_tier_recall` - Reading data from cold tier
   - `test_tier_promotion_on_access` - Promotion based on access patterns
   - `test_multi_tier_consistency` - Data consistency across tiers
   - `test_batch_operations_across_tiers` - Batch PUT/DELETE operations
   - `test_scan_across_tiers` - Scanning across all storage tiers
   - `test_tiered_metrics_accuracy` - Metrics tracking verification
   - `test_graceful_shutdown` - Shutdown with pending operations

## Known Issues

1. **Bucket Auto-Creation**: MinIO does not support automatic bucket creation via the S3 API. The bucket must be created manually before running tests.

2. **Path-Style Access**: MinIO requires path-style S3 access. Virtual-host-style will not work with localhost.

## Next Steps

1. Create the `orbit-cold-storage` bucket using one of the methods above
2. Run the test suite to verify tiered storage functionality
3. Consider adding RocksDB-backed warm tier (currently using memory)
4. Implement actual S3 cold tier backend (currently using memory mock)
