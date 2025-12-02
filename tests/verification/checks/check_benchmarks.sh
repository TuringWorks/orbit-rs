#!/bin/bash

# Benchmarks Check
# Replicates: cargo bench --workspace

set -e

echo "🔍 Running benchmarks..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

echo "Running: cargo bench --workspace"

if cargo bench --workspace; then
    echo "✅ Benchmarks completed successfully"
else
    echo "❌ Benchmarks failed"
    echo "💡 Check the benchmark code and dependencies"
    exit 1
fi

echo "✅ Benchmark check completed successfully"