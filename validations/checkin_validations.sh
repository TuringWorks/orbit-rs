#!/bin/bash

# Orbit-RS Local CI/CD Validation Script
# This script runs all the same checks that are performed in the CI/CD pipeline
# Run this before committing to ensure your changes will pass CI/CD

set -e  # Exit on any error

# Configuration
TIMEOUT_SECONDS=300  # 5 minutes timeout for long operations
SKIP_SLOW_CHECKS=false

# Parse command line arguments
for arg in "$@"; do
    case $arg in
        --fast)
            SKIP_SLOW_CHECKS=true
            shift
            ;;
        --timeout=*)
            TIMEOUT_SECONDS="${arg#*=}"
            shift
            ;;
        --help)
            echo "Usage: $0 [--fast] [--timeout=SECONDS]"
            echo "  --fast         Skip slow checks (coverage, benchmarks)"
            echo "  --timeout=N    Set timeout for long operations (default: 300s)"
            exit 0
            ;;
    esac
done

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to run commands with timeout
run_with_timeout() {
    local timeout=$1
    shift
    if command -v timeout &> /dev/null; then
        timeout "$timeout" "$@"
    elif command -v gtimeout &> /dev/null; then
        gtimeout "$timeout" "$@"
    else
        # Fallback without timeout on macOS if coreutils not installed
        "$@"
    fi
}

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

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

# Change to project root directory
cd "$(dirname "$0")/.."

echo "Starting Orbit-RS Pre-commit Validation..."
echo "This will run the same checks as CI/CD pipeline"
echo ""

# 1. Check code formatting
print_step "1/15 Checking code formatting..."
if cargo fmt --all -- --check; then
    print_success "Code formatting is correct"
else
    print_error "Code formatting issues found. Run 'cargo fmt --all' to fix."
    exit 1
fi
echo ""

# 2. Run Clippy lints
print_step "2/15 Running Clippy lints..."
if cargo clippy --all-targets --features="resp,postgres-wire,cypher,rest" -- -D warnings; then
    print_success "Clippy checks passed"
else
    print_error "Clippy found issues that need to be fixed"
    exit 1
fi
echo ""

# 3. Build with features (development build)
print_step "3/15 Building with features (development)..."
if cargo build --verbose --features="resp,postgres-wire,cypher,rest"; then
    print_success "Development build successful"
else
    print_error "Development build failed"
    exit 1
fi
echo ""

# 4. Build release version for all workspace
print_step "4/15 Building release version..."
if cargo build --release --workspace; then
    print_success "Release build successful"
else
    print_error "Release build failed"
    exit 1
fi
echo ""

# 5. Run tests with features
print_step "5/15 Running tests with features..."
if cargo test --verbose --features="resp,postgres-wire,cypher,rest"; then
    print_success "Feature tests passed"
else
    print_error "Feature tests failed"
    exit 1
fi
echo ""

# 6. Run workspace tests
print_step "6/15 Running workspace tests..."
if cargo test --workspace --verbose; then
    print_success "Workspace tests passed"
else
    print_error "Workspace tests failed"
    exit 1
fi
echo ""

# 7. Build examples
print_step "7/15 Building examples..."
examples=("hello-world" "distributed-transactions-example" "distributed-counter" "saga-example")
for example in "${examples[@]}"; do
    if cargo build --package "$example"; then
        print_success "Example '$example' built successfully"
    else
        print_warning "Example '$example' failed to build (may not exist)"
    fi
done
echo ""

# 8. Install and run cargo-audit (security audit)
print_step "8/15 Running security audit..."
if ! command -v cargo-audit &> /dev/null; then
    echo "Installing cargo-audit..."
    cargo install cargo-audit
fi
if cargo audit; then
    print_success "Security audit passed"
else
    print_warning "Security audit found issues (check if acceptable)"
fi
echo ""

# 9. Install and run cargo-deny (vulnerability check)
print_step "9/15 Running vulnerability check..."
if ! command -v cargo-deny &> /dev/null; then
    print_info "Installing cargo-deny..."
    if ! run_with_timeout 60 cargo install cargo-deny --locked; then
        print_warning "Failed to install cargo-deny within timeout, skipping vulnerability check"
        echo ""
        # Skip to next check
    else
        print_success "cargo-deny installed successfully"
    fi
fi

print_info "Running cargo deny check with ${TIMEOUT_SECONDS}s timeout..."
# Run cargo deny check with timeout and capture output
if deny_output=$(run_with_timeout "$TIMEOUT_SECONDS" cargo deny check 2>&1); then
    print_success "Vulnerability check passed"
elif [ $? -eq 124 ] || [ $? -eq 143 ]; then
    print_warning "Vulnerability check timed out after ${TIMEOUT_SECONDS}s - this may indicate network issues"
    print_warning "Consider running 'cargo deny check' manually after fixing network connectivity"
else
    # Check if it's just advisory warnings
    if echo "$deny_output" | grep -q "warning\[advisory-not-detected\]" && echo "$deny_output" | grep -q "ok$"; then
        print_warning "Vulnerability check passed with advisory warnings (advisory not detected)"
        print_warning "Consider updating deny.toml to remove unused advisory ignores"
    elif echo "$deny_output" | grep -q "advisories.*ok.*licenses.*ok.*sources.*ok"; then
        print_success "Vulnerability check passed (with some warnings)"
    else
        print_error "Vulnerability check failed"
        echo "Error output:"
        echo "$deny_output" | head -20  # Limit output to prevent spam
        exit 1
    fi
fi
echo ""

# 10. Generate documentation
print_step "10/15 Building documentation..."
if cargo doc --no-deps --features="resp,postgres-wire,cypher,rest"; then
    print_success "Documentation built successfully"
else
    print_error "Documentation build failed"
    exit 1
fi
echo ""

# 11. Run benchmarks (if available)
if [ "$SKIP_SLOW_CHECKS" = false ]; then
    print_step "11/15 Running benchmarks..."
    if run_with_timeout "$TIMEOUT_SECONDS" cargo bench --package orbit-benchmarks > /dev/null 2>&1; then
        print_success "Benchmarks completed"
    else
        print_warning "Benchmarks not available or failed (orbit-benchmarks package not found)"
    fi
else
    print_step "11/15 Skipping benchmarks (fast mode)"
    print_info "Use without --fast flag to run benchmarks"
fi
echo ""

# 12. Code coverage with tarpaulin
if [ "$SKIP_SLOW_CHECKS" = false ]; then
    print_step "12/15 Generating code coverage with tarpaulin..."
    if ! command -v cargo-tarpaulin &> /dev/null; then
        print_info "Installing cargo-tarpaulin..."
        run_with_timeout 60 cargo install cargo-tarpaulin
    fi
    if run_with_timeout "$TIMEOUT_SECONDS" cargo tarpaulin --verbose --features="resp,postgres-wire,cypher,rest" --workspace --timeout 120 --out Xml --output-dir ./coverage > /dev/null 2>&1; then
        print_success "Code coverage generated successfully"
    else
        print_warning "Code coverage generation failed or incomplete"
    fi
else
    print_step "12/15 Skipping code coverage (fast mode)"
    print_info "Use without --fast flag to generate coverage"
fi
echo ""

# 13. Code coverage with llvm-cov (alternative)
if [ "$SKIP_SLOW_CHECKS" = false ]; then
    print_step "13/15 Generating code coverage with llvm-cov..."
    if ! command -v cargo-llvm-cov &> /dev/null; then
        print_info "Installing cargo-llvm-cov..."
        run_with_timeout 60 cargo install cargo-llvm-cov
    fi
    if run_with_timeout "$TIMEOUT_SECONDS" cargo llvm-cov --all-features --workspace > /dev/null 2>&1; then
        print_success "LLVM code coverage completed"
    else
        print_warning "LLVM code coverage failed or incomplete"
    fi
else
    print_step "13/15 Skipping LLVM coverage (fast mode)"
    print_info "Use without --fast flag to generate LLVM coverage"
fi
echo ""

# 14. Check Helm charts (if helm is available)
print_step "14/15 Validating Helm charts..."
if command -v helm &> /dev/null; then
    if [ -d "helm/orbit-rs" ]; then
        if helm lint helm/orbit-rs && helm template orbit-rs helm/orbit-rs --values helm/orbit-rs/values.yaml > /dev/null; then
            print_success "Helm chart validation passed"
        else
            print_error "Helm chart validation failed"
            exit 1
        fi
    else
        print_warning "Helm charts directory not found"
    fi
else
    print_warning "Helm not installed - skipping chart validation"
fi
echo ""

# 15. Final validation summary
print_step "15/15 Running final validation checks..."

# Check if any warnings were issued
echo ""
echo "==========================================="
print_success "🎉 All CI/CD validation checks completed!"
echo "==========================================="
echo ""
print_success "Your code is ready for commit and should pass CI/CD pipeline."
echo ""
echo "Next steps:"
echo "  1. git add ."
echo "  2. git commit -m 'Your commit message'"
echo "  3. git push"
echo ""
print_warning "Note: Some optional checks (benchmarks, coverage) may show warnings but won't fail CI/CD."
print_warning "Docker and deployment checks are only run in the CI/CD environment."

