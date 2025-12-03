#!/bin/bash

# Orbit-RS Test Runner
# Runs tests for the workspace

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "orbit" ]; then
    print_error "Please run this script from the orbit-rs root directory"
    exit 1
fi

# Main execution
main() {
    print_status "Orbit-RS Test Runner"
    print_status "===================="
    echo ""

    case "${1:-workspace}" in
        "workspace"|"all")
            print_status "Running all workspace tests..."
            cargo test --workspace
            print_success "All workspace tests completed!"
            ;;
        "server")
            print_status "Running orbit-server tests..."
            cargo test -p orbit-server
            print_success "Server tests completed!"
            ;;
        "client")
            print_status "Running orbit-client tests..."
            cargo test -p orbit-client
            print_success "Client tests completed!"
            ;;
        "protocols")
            print_status "Running orbit-protocols tests..."
            cargo test -p orbit-protocols
            print_success "Protocols tests completed!"
            ;;
        "engine")
            print_status "Running orbit-engine tests..."
            cargo test -p orbit-engine
            print_success "Engine tests completed!"
            ;;
        "ml")
            print_status "Running orbit-ml tests..."
            cargo test -p orbit-ml
            print_success "ML tests completed!"
            ;;
        "compute")
            print_status "Running orbit-compute tests..."
            cargo test -p orbit-compute
            print_success "Compute tests completed!"
            ;;
        "time-series")
            print_status "Running time series tests..."
            cargo test -p orbit-server time_series::
            print_success "Time series tests completed!"
            ;;
        "ignored")
            print_status "Running ignored (slow) tests..."
            cargo test --workspace -- --ignored
            print_success "Ignored tests completed!"
            ;;
        "verbose")
            print_status "Running all tests with verbose output..."
            cargo test --workspace -- --nocapture
            print_success "Verbose tests completed!"
            ;;
        "quick")
            print_status "Running quick check (no tests, just compile)..."
            cargo check --workspace
            print_success "Quick check completed!"
            ;;
        "help"|"-h"|"--help")
            echo "Usage: $0 [test-type]"
            echo ""
            echo "Test types:"
            echo "  workspace   - Run all workspace tests (default)"
            echo "  all         - Same as workspace"
            echo "  server      - Run orbit-server tests only"
            echo "  client      - Run orbit-client tests only"
            echo "  protocols   - Run orbit-protocols tests only"
            echo "  engine      - Run orbit-engine tests only"
            echo "  ml          - Run orbit-ml tests only"
            echo "  compute     - Run orbit-compute tests only"
            echo "  time-series - Run time series command tests"
            echo "  ignored     - Run slow/ignored integration tests"
            echo "  verbose     - Run all tests with output"
            echo "  quick       - Just compile, no tests"
            echo "  help        - Show this help message"
            ;;
        *)
            print_error "Unknown test type: $1"
            print_status "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
}

main "$@"
