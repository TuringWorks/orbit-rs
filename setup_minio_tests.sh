#!/usr/bin/env bash
# Setup script for MinIO tiered storage tests

set -e

echo "Setting up MinIO for tiered storage tests..."

# Check if MinIO is running
if ! curl -s http://localhost:9000/minio/health/live \u003e /dev/null 2\u003e\u00261; then
    echo "Error: MinIO is not running at localhost:9000"
    echo "Please start MinIO first"
    exit 1
fi

echo "✓ MinIO is running"

# Create bucket using a simple Rust test
echo "Creating test bucket 'orbit-cold-storage'..."

cargo test --test tiered_storage_minio_tests create_test_bucket -- --exact --nocapture --ignored 2\u003e\u00261 || {
    echo "Note: Bucket creation test not found, bucket may already exist"
}

echo "✓ Setup complete"
echo ""
echo "Run tests with:"
echo "  cargo test --test tiered_storage_minio_tests -- --nocapture --ignored"
