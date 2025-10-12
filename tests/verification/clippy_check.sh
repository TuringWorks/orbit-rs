#!/bin/bash

# Quick Clippy Check with -D warnings
# Standalone script for rapid clippy validation during development

set -e

echo "⚡ Quick Clippy Check (-D warnings)"
echo "=================================="

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Check if clippy is available
if ! cargo clippy --version &> /dev/null; then
    echo "⚠️ clippy not found. Installing..."
    rustup component add clippy
fi

echo "🔍 Running clippy with all warnings treated as errors..."
echo ""

# Run clippy with strict settings
if cargo clippy --all-targets --all-features -- -D warnings; then
    echo ""
    echo "✅ All clippy checks passed!"
    echo "💡 No warnings found - code is ready for CI/CD"
else
    echo ""
    echo "❌ Clippy found issues that need to be fixed"
    echo ""
    echo "💡 Quick fixes:"
    echo "   cargo clippy --fix                    # Auto-fix safe issues"
    echo "   cargo clippy --fix --allow-dirty      # Allow fixes in dirty repo"
    echo "   cargo clippy --fix --allow-staged     # Allow fixes with staged changes"
    echo ""
    echo "📚 For more help: https://doc.rust-lang.org/clippy/"
    exit 1
fi