#!/bin/bash

# Comprehensive CI/CD Verification Script
# Replicates all checks from GitHub Actions workflows locally

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STAGES_DIR="$SCRIPT_DIR/stages"

echo "🎯 Orbit-RS Local CI/CD Verification"
echo "====================================="
echo ""
echo "This script replicates all checks from the GitHub Actions workflows:"
echo "  - .github/workflows/ci.yml"
echo "  - .github/workflows/ci-cd.yml"
echo ""

# Parse command line arguments
SKIP_OPTIONAL=""
STAGE=""
VERBOSE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-optional)
            SKIP_OPTIONAL="true"
            shift
            ;;
        --stage)
            STAGE="$2"
            shift 2
            ;;
        --verbose|-v)
            VERBOSE="true"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --skip-optional    Skip optional checks (coverage, benchmarks, docker, helm)"
            echo "  --stage STAGE      Run only specific stage: rust|quality|infrastructure"
            echo "  --verbose, -v      Enable verbose output"
            echo "  --help, -h         Show this help message"
            echo ""
            echo "Stages:"
            echo "  rust              Formatting, linting, building, testing, security"
            echo "  quality           Documentation, coverage, benchmarks"
            echo "  infrastructure    Docker build, Helm charts"
            echo ""
            echo "Examples:"
            echo "  $0                          # Run all checks"
            echo "  $0 --skip-optional          # Skip slow/optional checks"
            echo "  $0 --stage rust             # Run only Rust checks"
            echo "  $0 --stage quality --verbose # Run quality checks with verbose output"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Set environment variables for verbose mode
if [ "$VERBOSE" = "true" ]; then
    export CARGO_TERM_COLOR=always
    export RUST_BACKTRACE=1
    echo "🔍 Verbose mode enabled"
    echo ""
fi

# Check system requirements
echo "🔧 Checking system requirements..."

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install Rust: https://rustup.rs/"
    exit 1
fi

RUST_VERSION=$(rustc --version)
echo "✅ Rust found: $RUST_VERSION"

# Check for required system dependencies (macOS specific)
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "📱 Detected macOS environment"
    
    # Check for protobuf compiler
    if ! command -v protoc &> /dev/null; then
        echo "⚠️ Protocol Buffers compiler not found. Install with: brew install protobuf"
        echo "💡 This is required for building the project"
        exit 1
    fi
else
    echo "🐧 Detected non-macOS environment"
    echo "💡 Make sure you have: pkg-config, libssl-dev, libsqlite3-dev, protobuf-compiler"
fi

echo ""

# Track overall failures
FAILED_STAGES=()

# Function to run a stage and track failures
run_stage() {
    local stage_name="$1"
    local stage_script="$2"
    
    echo "🏗️ Starting $stage_name Stage..."
    echo "$(date '+%Y-%m-%d %H:%M:%S') - $stage_name stage started"
    
    START_TIME=$(date +%s)
    
    if bash "$STAGES_DIR/$stage_script"; then
        END_TIME=$(date +%s)
        DURATION=$((END_TIME - START_TIME))
        echo "✅ $stage_name Stage completed successfully in ${DURATION}s"
    else
        END_TIME=$(date +%s)
        DURATION=$((END_TIME - START_TIME))
        echo "❌ $stage_name Stage failed after ${DURATION}s"
        FAILED_STAGES+=("$stage_name")
    fi
    echo ""
}

# Run stages based on parameters
if [ -n "$STAGE" ]; then
    case "$STAGE" in
        rust)
            run_stage "Rust Checks" "rust_checks.sh"
            ;;
        quality)
            run_stage "Quality Checks" "quality_checks.sh"
            ;;
        infrastructure)
            run_stage "Infrastructure Checks" "infrastructure_checks.sh"
            ;;
        *)
            echo "❌ Unknown stage: $STAGE"
            echo "Valid stages: rust, quality, infrastructure"
            exit 1
            ;;
    esac
else
    # Run all stages
    run_stage "Rust Checks" "rust_checks.sh"
    
    if [ "$SKIP_OPTIONAL" != "true" ]; then
        run_stage "Quality Checks" "quality_checks.sh"
        run_stage "Infrastructure Checks" "infrastructure_checks.sh"
    else
        echo "⏩ Skipping optional stages (quality and infrastructure)"
        echo ""
    fi
fi

# Final summary
echo "========================================="
echo "🏁 CI/CD Verification Complete"
echo "========================================="

if [ ${#FAILED_STAGES[@]} -eq 0 ]; then
    echo "🎉 All verification stages passed!"
    echo ""
    echo "✅ Your code is ready for CI/CD pipeline"
    echo "💡 The GitHub Actions workflows should pass with these changes"
    exit 0
else
    echo "❌ The following stages failed:"
    for stage in "${FAILED_STAGES[@]}"; do
        echo "   - $stage"
    done
    echo ""
    echo "💡 Fix the issues above before pushing to trigger CI/CD"
    exit 1
fi