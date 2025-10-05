#!/bin/bash

# Orbit-RS Quick Pre-commit Validation Script
# Runs only the essential checks that must pass for CI/CD to succeed
# This is a fast version that skips slow operations like coverage and benchmarks

set -e  # Exit on any error

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

# Change to project root directory
cd "$(dirname "$0")/.."

echo "Starting Orbit-RS Quick Pre-commit Validation..."
echo "Running essential checks only (use checkin_validations.sh for full validation)"
echo ""

# 1. Check code formatting
print_step "1/8 Checking code formatting..."
if cargo fmt --all -- --check; then
    print_success "Code formatting is correct"
else
    print_error "Code formatting issues found. Run 'cargo fmt --all' to fix."
    exit 1
fi
echo ""

# 2. Run Clippy lints
print_step "2/8 Running Clippy lints..."
if cargo clippy --all-targets --features="resp,postgres-wire,cypher,rest" -- -D warnings; then
    print_success "Clippy checks passed"
else
    print_error "Clippy found issues that need to be fixed"
    exit 1
fi
echo ""

# 3. Build with features (development build)
print_step "3/8 Building with features..."
if cargo build --features="resp,postgres-wire,cypher,rest"; then
    print_success "Development build successful"
else
    print_error "Development build failed"
    exit 1
fi
echo ""

# 4. Build release version
print_step "4/8 Building release version..."
if cargo build --release --workspace; then
    print_success "Release build successful"
else
    print_error "Release build failed"
    exit 1
fi
echo ""

# 5. Run tests
print_step "5/8 Running tests..."
if cargo test --workspace; then
    print_success "Tests passed"
else
    print_error "Tests failed"
    exit 1
fi
echo ""

# 6. Generate documentation
print_step "6/8 Building documentation..."
if cargo doc --no-deps --features="resp,postgres-wire,cypher,rest" --quiet; then
    print_success "Documentation built successfully"
else
    print_error "Documentation build failed"
    exit 1
fi
echo ""

# 7. Security audit (quick version)
print_step "7/8 Running security audit..."
if command -v cargo-audit &> /dev/null; then
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security audit found issues (check if acceptable)"
    fi
else
    print_warning "cargo-audit not installed - install with: cargo install cargo-audit"
fi
echo ""

# 8. Vulnerability check (quick version)
print_step "8/8 Running vulnerability check..."
if command -v cargo-deny &> /dev/null; then
    if cargo deny check --hide-inclusion-graph; then
        print_success "Vulnerability check passed"
    else
        print_warning "Vulnerability check found issues - run 'cargo deny check' for details"
    fi
else
    print_warning "cargo-deny not installed - install with: cargo install cargo-deny --locked"
fi
echo ""

echo "=========================================="
print_success "🚀 Quick validation completed!"
echo "=========================================="
echo ""
print_success "Essential checks passed - your code should pass CI/CD."
echo ""
echo "Next steps:"
echo "  1. git add ."
echo "  2. git commit -m 'Your commit message'"
echo "  3. git push"
echo ""
print_warning "For complete validation including coverage and benchmarks, run:"
print_warning "  ./validations/checkin_validations.sh"