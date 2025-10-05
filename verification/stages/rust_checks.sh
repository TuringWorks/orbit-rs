#!/bin/bash

# Rust Checks Stage
# Combines all Rust-related checks from CI/CD

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECKS_DIR="$(dirname "$SCRIPT_DIR")/checks"

echo "🚀 Running Rust Checks Stage"
echo "============================"

# Track failures
FAILED_CHECKS=()

# Function to run a check and track failures
run_check() {
    local check_name="$1"
    shift
    local check_script_and_args=($@)
    
    echo ""
    echo "🔄 Running $check_name..."
    
    if bash "$CHECKS_DIR/${check_script_and_args[0]}" "${check_script_and_args[@]:1}"; then
        echo "✅ $check_name completed successfully"
    else
        echo "❌ $check_name failed"
        FAILED_CHECKS+=("$check_name")
    fi
}

# Run all Rust checks
run_check "Code Formatting" "check_formatting.sh"
run_check "Clippy Linting" "check_clippy.sh" 
run_check "Release Build" "check_build.sh" "release"
run_check "Tests" "check_tests.sh" "all"
run_check "Security Audit" "check_security.sh"
run_check "Example Builds" "check_examples.sh"

echo ""
echo "=============================="
echo "🏁 Rust Checks Stage Complete"

if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    echo "✅ All Rust checks passed!"
    exit 0
else
    echo "❌ The following checks failed:"
    for check in "${FAILED_CHECKS[@]}"; do
        echo "   - $check"
    done
    exit 1
fi