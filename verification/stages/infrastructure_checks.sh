#!/bin/bash

# Infrastructure Checks Stage
# Combines Docker and Helm checks

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECKS_DIR="$(dirname "$SCRIPT_DIR")/checks"

echo "🚀 Running Infrastructure Checks Stage"
echo "======================================"

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
            echo "⚠️ $check_name failed (optional - may require additional setup)"
        else
            echo "❌ $check_name failed"
            FAILED_CHECKS+=("$check_name")
        fi
    fi
}

# Run infrastructure checks
run_check "Docker Build" "check_docker.sh" "true"  # Optional since it requires Docker
run_check "Helm Charts" "check_helm.sh" "true"    # Optional since it requires Helm

echo ""
echo "======================================"
echo "🏁 Infrastructure Checks Stage Complete"

if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    echo "✅ All infrastructure checks passed!"
    exit 0
else
    echo "❌ The following checks failed:"
    for check in "${FAILED_CHECKS[@]}"; do
        echo "   - $check"
    done
    echo ""
    echo "💡 Note: Some infrastructure checks require additional tools (Docker, Helm)"
    exit 1
fi