#!/bin/bash

# Orbit-RS Test Runner
# This script runs tests based on feature flags and implementation readiness

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
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

# Function to check if a feature is implemented
check_implementation_ready() {
    local feature=$1
    local implementation_file=$2
    
    if [ -f "$implementation_file" ]; then
        return 0
    else
        return 1
    fi
}

# Function to run tests for a specific phase
run_phase_tests() {
    local phase=$1
    local feature_flag=$2
    local description=$3
    
    print_status "Running tests for $phase: $description"
    
    case $phase in
        "phase-9")
            if check_implementation_ready "query-optimization" "orbit-server/src/query_optimization.rs"; then
                print_success "Query optimization implementation found, running tests..."
                cargo test --features "$feature_flag" --test query_optimization_tests
            else
                print_warning "Query optimization not implemented yet, skipping tests"
            fi
            ;;
        "phase-10")
            if check_implementation_ready "production-readiness" "orbit-server/src/monitoring.rs"; then
                print_success "Production readiness implementation found, running tests..."
                cargo test --features "$feature_flag" --test production_tests
            else
                print_warning "Production features not implemented yet, skipping tests"
            fi
            ;;
        "phase-13")
            if check_implementation_ready "neo4j-features" "orbit-protocols/src/neo4j/mod.rs"; then
                print_success "Neo4j implementation found, running tests..."
                cd tests && cargo test --features "$feature_flag" neo4j_integration
            else
                print_warning "Neo4j features not implemented yet, skipping tests"
            fi
            ;;
        "phase-15")
            if check_implementation_ready "arangodb-features" "orbit-protocols/src/arangodb/mod.rs"; then
                print_success "ArangoDB implementation found, running tests..."
                cd tests && cargo test --features "$feature_flag" arangodb_integration
            else
                print_warning "ArangoDB features not implemented yet, skipping tests"
            fi
            ;;
        "phase-16")
            if check_implementation_ready "graphml-features" "orbit-protocols/src/graphml/mod.rs"; then
                print_success "GraphML implementation found, running tests..."
                cd tests && cargo test --features "$feature_flag" graphml_integration
            else
                print_warning "GraphML features not implemented yet, skipping tests"
            fi
            ;;
        *)
            print_warning "Phase $phase not configured for testing yet"
            ;;
    esac
}

# Function to run mock tests (always available)
run_mock_tests() {
    print_status "Running mock tests (always available)..."
    
    # Neo4j mock tests
    print_status "Running Neo4j mock tests..."
    cd tests && cargo test --features "neo4j-features" --lib neo4j::

    # GraphML mock tests  
    print_status "Running GraphML mock tests..."
    cd tests && cargo test --features "graphml-features" --lib graphml::

    # ArangoDB mock tests
    print_status "Running ArangoDB mock tests..."
    cd tests && cargo test --features "arangodb-features" --lib arangodb::

    print_success "All mock tests completed"
}

# Function to run BDD feature tests
run_bdd_tests() {
    print_status "Running BDD feature tests..."
    
    # Check if Cucumber features exist
    if [ -d "tests/features" ]; then
        # Run Neo4j Cypher BDD tests
        if [ -f "tests/features/neo4j_cypher.feature" ]; then
            print_status "Running Neo4j Cypher BDD tests..."
            cd tests && cargo test --features "neo4j-features" --test neo4j_cypher_bdd
        fi
        
        # Run ArangoDB AQL BDD tests
        if [ -f "tests/features/arangodb_aql.feature" ]; then
            print_status "Running ArangoDB AQL BDD tests..."
            cd tests && cargo test --features "arangodb-features" --test arangodb_aql_bdd
        fi
        
        # Run GraphML BDD tests
        if [ -f "tests/features/graphml.feature" ]; then
            print_status "Running GraphML BDD tests..."
            cd tests && cargo test --features "graphml-features" --test graphml_bdd
        fi
    else
        print_warning "No BDD feature files found"
    fi
}

# Function to run property-based tests
run_property_tests() {
    print_status "Running property-based tests..."
    
    cd tests && cargo test --features "property-tests" --lib -- --ignored
    
    print_success "Property-based tests completed"
}

# Function to run performance tests
run_performance_tests() {
    print_status "Running performance tests..."
    
    cd tests && cargo test --features "performance-tests" --release --lib performance_
    
    print_success "Performance tests completed"
}

# Main execution
main() {
    print_status "Orbit-RS Test Runner"
    print_status "===================="
    
    # Parse command line arguments
    case "${1:-all}" in
        "mock")
            run_mock_tests
            ;;
        "bdd")
            run_bdd_tests
            ;;
        "property")
            run_property_tests
            ;;
        "performance")
            run_performance_tests
            ;;
        "phase-9")
            run_phase_tests "phase-9" "phase-9-features" "Query Optimization & Performance"
            ;;
        "phase-10")
            run_phase_tests "phase-10" "phase-10-features" "Production Readiness"
            ;;
        "phase-13")
            run_phase_tests "phase-13" "phase-13-features" "Neo4j Bolt Protocol Compatibility"
            ;;
        "phase-15")
            run_phase_tests "phase-15" "phase-15-features" "ArangoDB Multi-Model Database"
            ;;
        "phase-16")
            run_phase_tests "phase-16" "phase-16-features" "GraphML, GraphRAG & Graph Analytics"
            ;;
        "all")
            print_status "Running all available tests..."
            run_mock_tests
            run_bdd_tests
            
            # Run phase tests only if implementations are ready
            run_phase_tests "phase-9" "phase-9-features" "Query Optimization & Performance"
            run_phase_tests "phase-10" "phase-10-features" "Production Readiness"  
            run_phase_tests "phase-13" "phase-13-features" "Neo4j Bolt Protocol Compatibility"
            run_phase_tests "phase-15" "phase-15-features" "ArangoDB Multi-Model Database"
            run_phase_tests "phase-16" "phase-16-features" "GraphML, GraphRAG & Graph Analytics"
            
            print_success "All tests completed!"
            ;;
        "help"|"-h"|"--help")
            echo "Usage: $0 [test-type]"
            echo ""
            echo "Test types:"
            echo "  mock        - Run mock tests (always available)"
            echo "  bdd         - Run BDD/Cucumber tests"
            echo "  property    - Run property-based tests"
            echo "  performance - Run performance benchmarks"
            echo "  phase-9     - Run Phase 9 tests (Query Optimization)"
            echo "  phase-10    - Run Phase 10 tests (Production Readiness)"
            echo "  phase-13    - Run Phase 13 tests (Neo4j Compatibility)"
            echo "  phase-15    - Run Phase 15 tests (ArangoDB Multi-Model)"
            echo "  phase-16    - Run Phase 16 tests (GraphML/GraphRAG)"
            echo "  all         - Run all available tests (default)"
            echo "  help        - Show this help message"
            ;;
        *)
            print_error "Unknown test type: $1"
            print_status "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "tests" ]; then
    print_error "Please run this script from the orbit-rs root directory"
    exit 1
fi

# Run main function with all arguments
main "$@"