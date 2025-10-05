#!/bin/bash

# Quick Check Script
# Runs the most essential checks quickly for development workflow

set -e

echo "⚡ Orbit-RS Quick Check"
echo "======================"
echo "Running essential checks for development workflow..."
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECKS_DIR="$SCRIPT_DIR/checks"

# Track failures
FAILED_CHECKS=()

# Function to run a check and track failures
run_check() {
    local check_name="$1"
    local check_script="$2"
    
    echo "🔄 $check_name..."
    
    if bash "$CHECKS_DIR/$check_script" > /dev/null 2>&1; then
        echo "✅ $check_name"
    else
        echo "❌ $check_name"
        FAILED_CHECKS+=("$check_name")
    fi
}

# Run essential checks
run_check "Code Formatting" "check_formatting.sh"
run_check "Clippy Lints" "check_clippy.sh"
run_check "Build" "check_build.sh dev"
run_check "Tests" "check_tests.sh workspace"

echo ""
echo "======================"

if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    echo "🎉 All quick checks passed!"
    echo ""
    echo "💡 Run './verification/verify_all.sh' for complete validation"
    exit 0
else
    echo "❌ The following checks failed:"
    for check in "${FAILED_CHECKS[@]}"; do
        echo "   - $check"
    done
    echo ""
    echo "🔧 Fix the issues above, or run individual checks with:"
    echo "   ./verification/checks/check_<name>.sh"
    exit 1
fi