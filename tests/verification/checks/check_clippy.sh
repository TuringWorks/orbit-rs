#!/bin/bash

# Rust Clippy Linting Check
# Replicates: cargo clippy --all-targets --all-features -- -D warnings
# Treats all clippy warnings as errors for strict code quality

set -e

# Source utilities for better output formatting
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../utils.sh"

print_step "Running Clippy linting checks with -D warnings..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Check if clippy is available
if ! cargo clippy --version &> /dev/null; then
    print_warning "clippy not found. Installing..."
    rustup component add clippy
    if cargo clippy --version &> /dev/null; then
        print_success "clippy installed successfully"
    else
        print_error "Failed to install clippy"
        exit 1
    fi
fi

# Get clippy version for debugging
CLIPPY_VERSION=$(cargo clippy --version)
print_info "Using: $CLIPPY_VERSION"

# Define features (keeping backward compatibility)
FEATURES="resp,postgres-wire,cypher,rest"

# Run clippy checks with all targets and treat warnings as errors
print_info "Running: cargo clippy --all-targets --all-features -- -D warnings"
if cargo clippy --all-targets --all-features -- -D warnings; then
    print_success "Clippy linting check passed - no warnings found"
    exit 0
else
    print_error "Clippy linting check failed"
    echo ""
    print_info "All clippy warnings are treated as errors (-D warnings flag)"
    print_info "Fix all linting issues above and try again"
    echo ""
    print_info "Common fixes:"
    print_info "  - Add #[allow(clippy::lint_name)] for intentional code patterns"
    print_info "  - Refactor code to follow clippy suggestions"
    print_info "  - Run 'cargo clippy --fix' for automatic fixes (when safe)"
    exit 1
fi
