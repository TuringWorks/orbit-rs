#!/bin/bash
# Pre-commit preparation script for Orbit-RS
# Run this before committing to ensure code quality

set -e  # Exit on error

echo "🚀 Preparing code for commit..."
echo ""

# Step 1: Format code
echo "🔧 Step 1: Formatting code..."
cargo fmt --all
echo "✅ Code formatting complete"
echo ""

# Step 2: Run clippy checks
echo "🔍 Step 2: Running clippy checks..."
cargo clippy --all-targets -- -D warnings
echo "✅ Clippy checks passed"
echo ""

# Step 3: Run tests
echo "🧪 Step 3: Running tests..."
cargo test --workspace --verbose
echo "✅ All tests passed"
echo ""

# Step 4: Build project
echo "🏗️  Step 4: Building project..."
cargo build --workspace
echo "✅ Build successful"
echo ""

echo "🎉 Pre-commit checks completed successfully!"
echo "Your code is ready for commit."
echo ""
echo "Next steps:"
echo "  git add ."
echo "  git commit -m 'your commit message'"