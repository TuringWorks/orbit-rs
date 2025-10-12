#!/bin/bash

# Rust Code Formatting Check
# Replicates: cargo fmt --all -- --check

set -e

echo "🔍 Checking Rust code formatting..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Check if rustfmt is available
if ! command -v rustfmt &> /dev/null; then
    echo "❌ rustfmt not found. Installing..."
    rustup component add rustfmt
fi

# Run cargo fmt check
echo "Running: cargo fmt --all -- --check"
if cargo fmt --all -- --check; then
    echo "✅ Code formatting check passed"
    exit 0
else
    echo "❌ Code formatting check failed"
    echo ""
    echo "💡 To fix formatting issues, run:"
    echo "   cargo fmt --all"
    exit 1
fi