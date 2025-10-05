#!/bin/bash

# Rust Build Check
# Replicates: cargo build --release --workspace and cargo build --verbose --features="resp,postgres-wire,cypher,rest"

set -e

echo "🔍 Building Rust project..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

BUILD_TYPE="${1:-release}"
FEATURES="resp,postgres-wire,cypher,rest"

if [ "$BUILD_TYPE" = "release" ]; then
    echo "Running: cargo build --release --workspace"
    if cargo build --release --workspace; then
        echo "✅ Release build successful"
    else
        echo "❌ Release build failed"
        exit 1
    fi
elif [ "$BUILD_TYPE" = "dev" ]; then
    echo "Running: cargo build --verbose --features=\"$FEATURES\""
    if cargo build --verbose --features="$FEATURES"; then
        echo "✅ Development build with features successful"
    else
        echo "❌ Development build failed"
        exit 1
    fi
else
    echo "❌ Unknown build type: $BUILD_TYPE"
    echo "Usage: $0 [release|dev]"
    exit 1
fi

echo "✅ Build check completed successfully"