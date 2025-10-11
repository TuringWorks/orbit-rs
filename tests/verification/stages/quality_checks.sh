#!/bin/bash

# Quality Checks Stage
# Combines coverage, benchmarks, and documentation checks

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECKS_DIR="$(dirname "$SCRIPT_DIR")/checks"

echo "🚀 Running Quality Checks Stage"
echo "==============================="

# Track failures
FAILED_CHECKS=()

# Function to run a check and track failures
run_check() {
    local check_name="$1"
    local check_script="$2"
    local optional="${3:-false}"
    
    echo ""
    echo "🔄 Running $check_name..."
    
    if bash "$CHECKS_DIR/$check_script"; then
        echo "✅ $check_name completed successfully"
    else
        if [ "$optional" = "true" ]; then
            echo "⚠️ $check_name failed (optional)"
        else
            echo "❌ $check_name failed"
            FAILED_CHECKS+=("$check_name")
        fi
    fi
}

# Run quality checks
run_check "Documentation Build" "check_docs.sh"
run_check "Code Coverage" "check_coverage.sh" "true"  # Optional since it can be slow
run_check "Benchmarks" "check_benchmarks.sh" "true"  # Optional since it requires specific setup

echo ""
echo "==============================="
echo "🏁 Quality Checks Stage Complete"

if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    echo "✅ All quality checks passed!"
    exit 0
else
    echo "❌ The following checks failed:"
    for check in "${FAILED_CHECKS[@]}"; do
        echo "   - $check"
    done
    exit 1
fi