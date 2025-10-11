#!/bin/bash

# Code Coverage Check
# Supports both cargo-tarpaulin and cargo-llvm-cov for comprehensive coverage analysis

set -e

# Source utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../utils.sh"

# Configuration
TIMEOUT_SECONDS=${COVERAGE_TIMEOUT:-300}  # 5 minutes default
FEATURES="resp,postgres-wire,cypher,rest"
COVERAGE_METHOD=${COVERAGE_METHOD:-"tarpaulin"}  # tarpaulin, llvm-cov, or both
SKIP_SLOW=${SKIP_SLOW:-"false"}

print_step "Generating code coverage report..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Create coverage directory
mkdir -p coverage

# Function to run tarpaulin coverage
run_tarpaulin_coverage() {
    print_info "Running cargo-tarpaulin coverage analysis..."
    
    # Check and install cargo-tarpaulin if needed
    install_cargo_tool "cargo-tarpaulin"
    
    print_info "Running: cargo tarpaulin --verbose --features=\"$FEATURES\" --workspace --timeout 120 --out Xml --output-dir ./coverage"
    
    if run_with_timeout "$TIMEOUT_SECONDS" cargo tarpaulin --verbose --features="$FEATURES" --workspace --timeout 120 --out Xml --output-dir ./coverage; then
        print_success "Tarpaulin XML coverage generation successful"
        
        # Also generate HTML report for local viewing if not in fast mode
        if [ "$SKIP_SLOW" != "true" ]; then
            print_info "Generating HTML coverage report..."
            if run_with_timeout "$TIMEOUT_SECONDS" cargo tarpaulin --verbose --features="$FEATURES" --workspace --timeout 120 --out Html --output-dir ./coverage; then
                print_success "HTML coverage report generated at coverage/tarpaulin-report.html"
            fi
        fi
        
        return 0
    else
        print_error "Tarpaulin coverage generation failed"
        return 1
    fi
}

# Function to run llvm-cov coverage
run_llvm_coverage() {
    print_info "Running cargo-llvm-cov coverage analysis..."
    
    # Check and install cargo-llvm-cov if needed
    install_cargo_tool "cargo-llvm-cov"
    
    print_info "Running: cargo llvm-cov --all-features --workspace --html --output-dir coverage/llvm"
    
    # Create llvm coverage subdirectory
    mkdir -p coverage/llvm
    
    if run_with_timeout "$TIMEOUT_SECONDS" cargo llvm-cov --all-features --workspace --html --output-dir coverage/llvm; then
        print_success "LLVM coverage generation successful"
        
        # Generate text summary
        if run_with_timeout "$TIMEOUT_SECONDS" cargo llvm-cov --all-features --workspace > coverage/llvm-summary.txt 2>&1; then
            print_info "LLVM coverage summary saved to coverage/llvm-summary.txt"
        fi
        
        return 0
    else
        print_error "LLVM coverage generation failed"
        return 1
    fi
}

# Main coverage execution logic
SUCCESS=false

case "$COVERAGE_METHOD" in
    "tarpaulin")
        if run_tarpaulin_coverage; then
            SUCCESS=true
        fi
        ;;
    "llvm-cov")
        if run_llvm_coverage; then
            SUCCESS=true
        fi
        ;;
    "both")
        TARPAULIN_SUCCESS=false
        LLVM_SUCCESS=false
        
        if run_tarpaulin_coverage; then
            TARPAULIN_SUCCESS=true
        fi
        
        echo ""
        
        if run_llvm_coverage; then
            LLVM_SUCCESS=true
        fi
        
        if [ "$TARPAULIN_SUCCESS" = "true" ] || [ "$LLVM_SUCCESS" = "true" ]; then
            SUCCESS=true
        fi
        ;;
    *)
        print_error "Unknown coverage method: $COVERAGE_METHOD"
        print_info "Valid options: tarpaulin, llvm-cov, both"
        exit 1
        ;;
esac

# Display results summary
if [ "$SUCCESS" = "true" ]; then
    echo ""
    print_success "📊 Coverage analysis completed successfully!"
    echo ""
    
    print_info "Generated coverage reports:"
    if [ -f "coverage/cobertura.xml" ]; then
        echo "   📄 XML (tarpaulin): coverage/cobertura.xml"
    fi
    if [ -f "coverage/tarpaulin-report.html" ]; then
        echo "   🌐 HTML (tarpaulin): coverage/tarpaulin-report.html"
    fi
    if [ -d "coverage/llvm" ]; then
        echo "   🌐 HTML (llvm-cov): coverage/llvm/index.html"
    fi
    if [ -f "coverage/llvm-summary.txt" ]; then
        echo "   📄 Summary (llvm-cov): coverage/llvm-summary.txt"
    fi
    
    echo ""
    print_success "Coverage check completed successfully"
else
    print_error "Coverage analysis failed"
    exit 1
fi
