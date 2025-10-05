#!/bin/bash

# Rust Clippy Linting Check
# Replicates: cargo clippy --all-targets --features="resp,postgres-wire,cypher,rest" -- -D warnings

set -e

echo "🔍 Running Clippy linting checks..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Check if clippy is available
if ! cargo clippy --version &> /dev/null; then
    echo "❌ clippy not found. Installing..."
    rustup component add clippy
fi

# Define features
FEATURES="resp,postgres-wire,cypher,rest"

echo "Running: cargo clippy --all-targets --features=\"$FEATURES\" -- -D warnings"
if cargo clippy --all-targets --features="$FEATURES" -- -D warnings; then
    echo "✅ Clippy linting check passed"
    exit 0
else
    echo "❌ Clippy linting check failed"
    echo ""
    echo "💡 Fix the linting issues above and try again"
    exit 1
fi