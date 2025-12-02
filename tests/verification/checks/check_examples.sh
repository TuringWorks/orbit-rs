#!/bin/bash

# Example Builds Check
# Replicates: cargo build --package hello-world, etc.

set -e

echo "🔍 Building example packages..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# List of example packages to build
# Note: These examples don't currently exist in the project
# Uncomment when they are added
EXAMPLES=(
    # "hello-world"
    # "distributed-transactions-example"
    # "distributed-counter" 
    # "saga-example"
)

# Additional examples that might exist
ADDITIONAL_EXAMPLES=(
    # "resp-server"
    # "vector-store"
    # "pgvector-store"
)

BUILD_FAILED=0

# Build core examples
for example in "${EXAMPLES[@]}"; do
    echo "Running: cargo build --package $example"
    if cargo build --package "$example"; then
        echo "✅ Example '$example' built successfully"
    else
        echo "❌ Example '$example' build failed"
        BUILD_FAILED=1
    fi
    echo ""
done

# Try to build additional examples (don't fail if they don't exist)
for example in "${ADDITIONAL_EXAMPLES[@]}"; do
    echo "Running: cargo build --package $example"
    if cargo build --package "$example" 2>/dev/null; then
        echo "✅ Example '$example' built successfully"
    else
        echo "⚠️ Example '$example' not found or build failed (this might be expected)"
    fi
    echo ""
done

if [ $BUILD_FAILED -eq 1 ]; then
    echo "❌ Some example builds failed"
    exit 1
fi

echo "✅ Example builds completed successfully"