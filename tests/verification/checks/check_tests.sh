#!/bin/bash

# Rust Test Execution
# Replicates: cargo test --workspace --verbose and cargo test --verbose --features="resp,postgres-wire,cypher,rest"

set -e

echo "🔍 Running Rust tests..."

# Set Cargo color output and backtrace
export CARGO_TERM_COLOR=always
export RUST_BACKTRACE=1

TEST_TYPE="${1:-workspace}"
FEATURES="resp,postgres-wire,cypher,rest"

if [ "$TEST_TYPE" = "workspace" ]; then
    echo "Running: cargo test --workspace --verbose"
    if cargo test --workspace --verbose; then
        echo "✅ Workspace tests passed"
    else
        echo "❌ Workspace tests failed"
        exit 1
    fi
elif [ "$TEST_TYPE" = "features" ]; then
    echo "Running: cargo test --verbose --features=\"$FEATURES\""
    if cargo test --verbose --features="$FEATURES"; then
        echo "✅ Feature tests passed"
    else
        echo "❌ Feature tests failed"
        exit 1
    fi
elif [ "$TEST_TYPE" = "all" ]; then
    echo "Running: cargo test --workspace --verbose"
    if ! cargo test --workspace --verbose; then
        echo "❌ Workspace tests failed"
        exit 1
    fi
    
    echo "Running: cargo test --verbose --features=\"$FEATURES\""
    if ! cargo test --verbose --features="$FEATURES"; then
        echo "❌ Feature tests failed"
        exit 1
    fi
    
    echo "✅ All tests passed"
else
    echo "❌ Unknown test type: $TEST_TYPE"
    echo "Usage: $0 [workspace|features|all]"
    exit 1
fi

echo "✅ Test check completed successfully"