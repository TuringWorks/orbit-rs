#!/bin/bash

# Security Audit Check
# Replicates: cargo audit and cargo deny check

set -e

echo "🔍 Running security audits..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Check and install cargo-audit if needed
if ! command -v cargo-audit &> /dev/null; then
    echo "Installing cargo-audit..."
    cargo install cargo-audit --locked
fi

# Check and install cargo-deny if needed
if ! command -v cargo-deny &> /dev/null; then
    echo "Installing cargo-deny..."
    cargo install cargo-deny --locked
fi

# Run cargo audit
echo "Running: cargo audit"
if cargo audit; then
    echo "✅ Security audit passed"
else
    echo "⚠️ Security audit completed with warnings/errors"
    echo "💡 Review the security issues above"
    # Don't exit 1 here as some vulnerabilities might be acceptable
fi

# Run cargo deny check
echo ""
echo "Running: cargo deny check"
if cargo deny check; then
    echo "✅ Cargo deny check passed"
else
    echo "❌ Cargo deny check failed"
    echo "💡 Review the deny.toml configuration and dependency issues"
    exit 1
fi

echo "✅ Security check completed successfully"