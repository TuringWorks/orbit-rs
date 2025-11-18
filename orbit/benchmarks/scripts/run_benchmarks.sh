#!/bin/bash

# Orbit-RS Benchmark Runner Script
# Consolidated script for running all types of benchmarks
# This script replaces the old verification/checks/check_benchmarks.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default configuration
BENCHMARK_TYPE="all"
DURATION="5m"
VERBOSE=false
GENERATE_REPORT=true
OUTPUT_DIR="./results"
TIMEOUT_ENABLED=true

# Function to print colored output
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to show usage
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Consolidated benchmark runner for Orbit-RS

OPTIONS:
    -t, --type TYPE         Benchmark type: all, actor, leader, persistence, compute, performance
                           (default: all)
    -d, --duration TIME     Timeout duration (default: 5m)
    -o, --output DIR        Output directory for results (default: ./results)
    -v, --verbose           Enable verbose output
    -n, --no-report        Disable report generation
    -T, --no-timeout       Disable timeout protection
    -h, --help             Show this help message

BENCHMARK TYPES:
    all             Run all available benchmarks (safe ones)
    actor           Actor system performance benchmarks
    leader          Leader election benchmarks
    persistence     Storage persistence benchmarks (⚠️  may hang)
    compute         Heterogeneous compute benchmarks (if available)
    performance     Transaction performance benchmarks
    safe            Run only safe benchmarks (actor + leader)

EXAMPLES:
    $0                          # Run all safe benchmarks
    $0 -t actor -v              # Run only actor benchmarks with verbose output
    $0 -t persistence -d 2m     # Run persistence benchmarks with 2 minute timeout
    $0 -t safe -o ./my_results  # Run safe benchmarks, save to custom directory
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--type)
            BENCHMARK_TYPE="$2"
            shift 2
            ;;
        -d|--duration)
            DURATION="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -n|--no-report)
            GENERATE_REPORT=false
            shift
            ;;
        -T|--no-timeout)
            TIMEOUT_ENABLED=false
            shift
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Validate benchmark type
case $BENCHMARK_TYPE in
    all|actor|leader|persistence|compute|performance|safe)
        ;;
    *)
        print_error "Invalid benchmark type: $BENCHMARK_TYPE"
        show_usage
        exit 1
        ;;
esac

# Set environment variables
export CARGO_TERM_COLOR=always
export RUST_BACKTRACE=1

# Ensure we're in the orbit-benchmarks directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCHMARK_DIR="$(dirname "$SCRIPT_DIR")"

if [[ ! -f "$BENCHMARK_DIR/Cargo.toml" ]]; then
    print_error "Must be run from orbit-benchmarks directory or its scripts/ subdirectory"
    exit 1
fi

cd "$BENCHMARK_DIR"

print_info "Starting Orbit-RS Benchmark Suite"
print_info "Benchmark Type: $BENCHMARK_TYPE"
print_info "Timeout: $DURATION"
print_info "Output Directory: $OUTPUT_DIR"
print_info "Working Directory: $(pwd)"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Build benchmarks first
print_info "Building benchmark dependencies..."
if [[ "$VERBOSE" == "true" ]]; then
    cargo build --release
else
    cargo build --release > /dev/null 2>&1
fi

if [[ $? -ne 0 ]]; then
    print_error "Failed to build benchmark dependencies"
    exit 1
fi

print_success "Build completed successfully"

# Function to run a specific benchmark with timeout protection
run_benchmark() {
    local bench_name="$1"
    local description="$2"
    local output_file="$OUTPUT_DIR/${bench_name}_results.json"
    local command=""

    print_info "Running $description..."

    if [[ "$TIMEOUT_ENABLED" == "true" ]]; then
        command="timeout $DURATION cargo bench --bench $bench_name"
    else
        command="cargo bench --bench $bench_name"
    fi

    # Add JSON output if generating reports
    if [[ "$GENERATE_REPORT" == "true" ]]; then
        command="$command -- --output-format json"
    fi

    # Execute the benchmark
    if [[ "$VERBOSE" == "true" ]]; then
        eval "$command" 2>&1 | tee "${OUTPUT_DIR}/${bench_name}_output.log"
        local exit_code=${PIPESTATUS[0]}
    else
        eval "$command" > "${OUTPUT_DIR}/${bench_name}_output.log" 2>&1
        local exit_code=$?
    fi

    # Handle results
    case $exit_code in
        0)
            print_success "$description completed successfully"
            if [[ "$GENERATE_REPORT" == "true" && -f "$output_file" ]]; then
                print_info "Results saved to $output_file"
            fi
            return 0
            ;;
        124)
            print_warning "$description timed out after $DURATION (this may be expected for persistence benchmarks)"
            return 1
            ;;
        *)
            print_error "$description failed with exit code $exit_code"
            if [[ "$VERBOSE" == "false" ]]; then
                print_info "Run with -v for detailed output"
            fi
            return 1
            ;;
    esac
}

# Run benchmarks based on type
FAILED_BENCHMARKS=0
TOTAL_BENCHMARKS=0
START_TIME=$(date +%s)

case $BENCHMARK_TYPE in
    "actor"|"all"|"safe")
        print_info "🚀 Running Actor Benchmarks"
        run_benchmark "actor_benchmarks" "Actor Performance Benchmarks"
        TOTAL_BENCHMARKS=$((TOTAL_BENCHMARKS + 1))
        if [[ $? -ne 0 ]]; then
            FAILED_BENCHMARKS=$((FAILED_BENCHMARKS + 1))
        fi
        ;;
esac

case $BENCHMARK_TYPE in
    "leader"|"all"|"safe")
        print_info "🗳️ Running Leader Election Benchmarks"
        run_benchmark "leader_election_benchmarks" "Leader Election Benchmarks"
        TOTAL_BENCHMARKS=$((TOTAL_BENCHMARKS + 1))
        if [[ $? -ne 0 ]]; then
            FAILED_BENCHMARKS=$((FAILED_BENCHMARKS + 1))
        fi
        ;;
esac

case $BENCHMARK_TYPE in
    "persistence"|"all")
        print_warning "💾 Running Persistence Comparison Benchmarks (may hang due to WAL replay issues)"
        print_info "Using timeout protection: $DURATION"
        run_benchmark "persistence_comparison" "Persistence Comparison Benchmarks"
        TOTAL_BENCHMARKS=$((TOTAL_BENCHMARKS + 1))
        if [[ $? -ne 0 ]]; then
            FAILED_BENCHMARKS=$((FAILED_BENCHMARKS + 1))
        fi
        ;;
esac

# Note: Compute and performance benchmarks would be integrated here when moved

END_TIME=$(date +%s)
DURATION_SEC=$((END_TIME - START_TIME))

# Generate summary report
if [[ "$GENERATE_REPORT" == "true" ]]; then
    REPORT_FILE="$OUTPUT_DIR/benchmark_summary.md"
    
    cat > "$REPORT_FILE" << EOF
# Orbit-RS Benchmark Results

**Date**: $(date -u +"%Y-%m-%d %H:%M:%S UTC")  
**Duration**: ${DURATION_SEC} seconds  
**Benchmark Type**: $BENCHMARK_TYPE  
**Timeout**: $DURATION  
**Working Directory**: $(pwd)  

## Summary

- **Total Benchmarks**: $TOTAL_BENCHMARKS
- **Failed Benchmarks**: $FAILED_BENCHMARKS
- **Success Rate**: $(( (TOTAL_BENCHMARKS - FAILED_BENCHMARKS) * 100 / TOTAL_BENCHMARKS ))%

## Results

EOF

    # Add individual benchmark results
    for result_file in "$OUTPUT_DIR"/*_results.json; do
        if [[ -f "$result_file" ]]; then
            bench_name=$(basename "$result_file" "_results.json")
            echo "### $bench_name" >> "$REPORT_FILE"
            echo "Results available in: \`$result_file\`" >> "$REPORT_FILE"
            echo "" >> "$REPORT_FILE"
        fi
    done

    cat >> "$REPORT_FILE" << EOF

## Log Files

EOF

    for log_file in "$OUTPUT_DIR"/*_output.log; do
        if [[ -f "$log_file" ]]; then
            log_name=$(basename "$log_file")
            echo "- \`$log_name\`" >> "$REPORT_FILE"
        fi
    done

    print_info "Summary report generated: $REPORT_FILE"
fi

# Final summary
print_info "Benchmark Summary:"
print_info "  Total Benchmarks: $TOTAL_BENCHMARKS"
if [[ $FAILED_BENCHMARKS -eq 0 ]]; then
    print_success "  All benchmarks completed successfully!"
else
    print_warning "  Failed Benchmarks: $FAILED_BENCHMARKS"
fi

print_info "  Total Duration: ${DURATION_SEC} seconds"

if [[ "$GENERATE_REPORT" == "true" ]]; then
    print_info "  Results saved to: $OUTPUT_DIR/"
fi

# Exit with appropriate code
if [[ $FAILED_BENCHMARKS -eq 0 ]]; then
    print_success "🎉 Benchmark suite completed successfully!"
    exit 0
else
    print_error "💥 Some benchmarks failed. Check the logs for details."
    exit 1
fi