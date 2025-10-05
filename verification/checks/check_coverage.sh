#!/bin/bash

# Code Coverage Check
# Replicates: cargo tarpaulin --verbose --features="resp,postgres-wire,cypher,rest" --workspace --timeout 120 --out Xml --output-dir ./coverage

set -e

echo "🔍 Generating code coverage report..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Check and install cargo-tarpaulin if needed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Create coverage directory
mkdir -p coverage

FEATURES="resp,postgres-wire,cypher,rest"

echo "Running: cargo tarpaulin --verbose --features=\"$FEATURES\" --workspace --timeout 120 --out Xml --output-dir ./coverage"

if cargo tarpaulin --verbose --features="$FEATURES" --workspace --timeout 120 --out Xml --output-dir ./coverage; then
    echo "✅ Code coverage generation successful"
    
    # Also generate HTML report for local viewing
    echo "Generating HTML coverage report..."
    if cargo tarpaulin --verbose --features="$FEATURES" --workspace --timeout 120 --out Html --output-dir ./coverage; then
        echo "✅ HTML coverage report generated at coverage/tarpaulin-report.html"
    fi
    
    # Display basic coverage info if available
    if [ -f "coverage/cobertura.xml" ]; then
        echo ""
        echo "📊 Coverage report generated:"
        echo "   XML: coverage/cobertura.xml"
        echo "   HTML: coverage/tarpaulin-report.html"
    fi
else
    echo "❌ Code coverage generation failed"
    exit 1
fi

echo "✅ Coverage check completed successfully"