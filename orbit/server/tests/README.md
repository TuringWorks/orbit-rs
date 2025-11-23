# Orbit Server Integration Tests

Comprehensive integration tests for verifying all Orbit Server features are properly initialized and working.

## Overview

These tests verify:
- ✅ **Persistence Layer** - RocksDB initialization
- ✅ **Write-Ahead Log (WAL)** - WAL directory creation and usage
- ✅ **Protocol Adapters** - PostgreSQL (5432), Redis (6379), MySQL (3306), CQL (9042), gRPC (50051)
- ✅ **Prometheus Metrics** - Metrics endpoint on port 9090
- ✅ **MinIO Configuration** - Cold tier cloud storage setup
- ✅ **Automatic Cleanup** - Lingering process cleanup before/after tests

## Automatic Cleanup

**IMPORTANT**: All tests automatically clean up lingering `orbit-server` and `multi-protocol-server` instances before and after each test run. This prevents issues from previous test runs interfering with new tests.

The cleanup function:
1. Kills all `orbit-server` processes
2. Kills all `multi-protocol-server` processes
3. Waits 500ms for graceful termination
4. Verifies no lingering instances remain (assertion)

## Running Tests

### Quick Start (Cleanup Tests Only)

Run the basic cleanup tests to verify no lingering instances:

```bash
cargo test --test integration_test test_cleanup
```

Expected output:
```
running 2 tests
test test_cleanup_after_tests ... ok
test test_cleanup_before_tests ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured
```

### Full Integration Tests

Most tests are marked with `#[ignore]` because they:
- Start the full orbit-server (slow)
- Require ports 3306, 5432, 6379, 9042, 9090, 50051 to be available
- Take 5-15 seconds per test

Run all integration tests with:

```bash
cargo test --test integration_test -- --ignored --nocapture
```

### Individual Feature Tests

Run specific test suites:

**Data Directories:**
```bash
cargo test --test integration_test test_data_directories_created -- --ignored --nocapture
```

**RocksDB Persistence:**
```bash
cargo test --test integration_test test_rocksdb_persistence_initialized -- --ignored --nocapture
```

**All Protocol Ports:**
```bash
cargo test --test integration_test test_all_protocol_ports_listening -- --ignored --nocapture
```

**PostgreSQL Wire Protocol:**
```bash
cargo test --test integration_test test_postgresql_wire_protocol_connection -- --ignored --nocapture
```

**Redis RESP Protocol:**
```bash
cargo test --test integration_test test_redis_resp_protocol_connection -- --ignored --nocapture
```

**Prometheus Metrics:**
```bash
cargo test --test integration_test test_prometheus_metrics_endpoint -- --ignored --nocapture
```

**MinIO Configuration:**
```bash
cargo test --test integration_test test_minio_configuration_loaded -- --ignored --nocapture
```

**WAL Enabled:**
```bash
cargo test --test integration_test test_wal_enabled -- --ignored --nocapture
```

### Stress Tests

**Multiple Restarts (No Lingering Instances):**
```bash
cargo test --test integration_test test_multiple_restarts_no_lingering_instances -- --ignored --nocapture
```

This test:
- Starts and stops the server 5 times
- Verifies cleanup after each iteration
- Ensures no lingering instances after all restarts

**Concurrent Protocol Connections:**
```bash
cargo test --test integration_test test_concurrent_protocol_connections -- --ignored --nocapture
```

This test:
- Starts the server once
- Verifies all 6 ports are listening concurrently
- Tests: PostgreSQL, Redis, MySQL, CQL, gRPC, Metrics

## Test Structure

### Helper Functions

- `cleanup_lingering_instances()` - Kill all server instances and verify cleanup
- `is_port_listening(port)` - Check if a TCP port is in use
- `wait_for_port(port, max_secs)` - Wait for port to start listening (with timeout)
- `data_directory_exists(path)` - Check if directory exists
- `rocksdb_initialized(path)` - Check if RocksDB contains database files
- `wal_directory_exists(path)` - Check if WAL directory exists

### Test Categories

**1. Cleanup Tests** (Always run)
- `test_cleanup_before_tests` - Ensure clean state before test suite
- `test_cleanup_after_tests` - Ensure clean state after test suite

**2. Initialization Tests** (`#[ignore]`)
- `test_data_directories_created` - Verify hot/warm/cold/wal/rocksdb directories
- `test_rocksdb_persistence_initialized` - Verify RocksDB contains files

**3. Protocol Tests** (`#[ignore]`)
- `test_all_protocol_ports_listening` - Verify all 5 protocols start
- `test_postgresql_wire_protocol_connection` - PostgreSQL port 5432
- `test_redis_resp_protocol_connection` - Redis port 6379

**4. Feature Tests** (`#[ignore]`)
- `test_prometheus_metrics_endpoint` - Metrics port 9090
- `test_minio_configuration_loaded` - MinIO environment variables
- `test_wal_enabled` - WAL directory exists

**5. Stress Tests** (`#[ignore]`)
- `test_multiple_restarts_no_lingering_instances` - 5 restart cycles
- `test_concurrent_protocol_connections` - All 6 ports simultaneously

## Expected Behavior

### Successful Test Run

```
running 12 tests
test test_cleanup_before_tests ... ok
test test_data_directories_created ... ok
test test_rocksdb_persistence_initialized ... ok
test test_all_protocol_ports_listening ... ok
test test_postgresql_wire_protocol_connection ... ok
test test_redis_resp_protocol_connection ... ok
test test_prometheus_metrics_endpoint ... ok
test test_minio_configuration_loaded ... ok
test test_wal_enabled ... ok
test test_multiple_restarts_no_lingering_instances ... ok
test test_concurrent_protocol_connections ... ok
test test_cleanup_after_tests ... ok

test result: ok. 12 passed; 0 failed; 0 ignored
```

### Common Issues

**Port Already in Use:**
```
Error: Address already in use (os error 48)
```
**Solution:** Run cleanup test first or manually kill processes:
```bash
killall orbit-server multi-protocol-server
cargo test --test integration_test test_cleanup
```

**Timeout Waiting for Port:**
```
assertion failed: listening
```
**Solution:** Increase timeout in test (currently 15 seconds), or check server logs for startup errors.

**Lingering Instances:**
```
assertion failed: `(left == right)`
  left: `3`,
 right: `0`: Expected 0 lingering instances, found 3
```
**Solution:** Tests should clean up automatically. If this persists, manually kill:
```bash
killall orbit-server multi-protocol-server
```

## Environment Variables

For MinIO cold storage tests:

```bash
export MINIO_ENDPOINT=http://localhost:9000
export MINIO_BUCKET=orbit-cold-tier
export MINIO_ACCESS_KEY=minioadmin
export MINIO_SECRET_KEY=minioadmin
```

## Debugging

Enable verbose output with `--nocapture`:

```bash
cargo test --test integration_test -- --ignored --nocapture
```

View server logs during tests by checking the spawned process output (tests capture stdout/stderr).

## CI/CD Integration

For continuous integration, run cleanup tests on every commit:

```yaml
# GitHub Actions example
- name: Run Integration Tests
  run: |
    # Run cleanup tests (fast, always run)
    cargo test --test integration_test test_cleanup
    
    # Run full integration tests (slower, optional)
    cargo test --test integration_test -- --ignored --nocapture || true
```

For nightly/scheduled builds, run full integration suite:

```yaml
# Nightly build
- name: Full Integration Test Suite
  run: cargo test --test integration_test -- --ignored --nocapture
```

## Adding New Tests

When adding new server features:

1. Add cleanup to `cleanup_lingering_instances()` if needed
2. Create a new test function with `#[tokio::test]` and `#[ignore]`
3. Follow the pattern:
   ```rust
   #[tokio::test]
   #[ignore]
   async fn test_new_feature() {
       cleanup_lingering_instances().await;
       
       // Start server
       let mut child = Command::new("cargo")
           .args(&["run", "-p", "orbit-server", "--"])
           .spawn()
           .expect("Failed to start orbit-server");
       
       // Wait for feature initialization
       sleep(Duration::from_secs(5)).await;
       
       // Verify feature
       assert!(feature_working(), "Feature should work");
       
       // Cleanup
       let _ = child.kill();
       cleanup_lingering_instances().await;
   }
   ```

## Performance Considerations

- **Cleanup tests**: ~0.6 seconds
- **Individual feature tests**: ~5-10 seconds each
- **Stress tests**: ~15-30 seconds each
- **Full suite**: ~2-5 minutes

Run cleanup tests frequently. Run full suite before releases or in CI/CD pipelines.

## License

BSD-3-Clause OR MIT
