#!/bin/bash

# Manual CI/CD Script for Orbit-RS
# Purpose: Run comprehensive CI/CD checks locally on macOS
# Replaces automated GitHub Actions workflows for manual execution

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_TYPE="${BUILD_TYPE:-both}"  # debug, release, or both
JOBS="${JOBS:-2}"                 # Parallel jobs for compilation
SKIP_TESTS="${SKIP_TESTS:-false}" # Skip running tests if set to true
SKIP_EXAMPLES="${SKIP_EXAMPLES:-false}" # Skip building examples if set to true

# Print colored output
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

# Print banner
print_banner() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════════════════════════════════════════════╗"
    echo "║                          Orbit-RS Manual CI/CD                              ║"
    echo "║                         Local Quality & Build Checks                        ║"
    echo "╚══════════════════════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
    echo "Project Root: ${PROJECT_ROOT}"
    echo "Build Type: ${BUILD_TYPE}"
    echo "Jobs: ${JOBS}"
    echo "Skip Tests: ${SKIP_TESTS}"
    echo "Skip Examples: ${SKIP_EXAMPLES}"
    echo ""
}

# Check prerequisites
check_prerequisites() {
    print_step "Checking prerequisites..."
    
    # Check if we're in the correct directory
    if [[ ! -f "${PROJECT_ROOT}/Cargo.toml" ]]; then
        print_error "Not in Orbit-RS project root. Please run from project root."
        exit 1
    fi
    
    # Check for required tools
    local missing_tools=()
    
    if ! command -v cargo >/dev/null 2>&1; then
        missing_tools+=("cargo (Rust)")
    fi
    
    if ! command -v protoc >/dev/null 2>&1; then
        missing_tools+=("protoc (Protocol Buffers)")
    fi
    
    if ! command -v jq >/dev/null 2>&1; then
        missing_tools+=("jq (JSON processor)")
    fi
    
    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        print_error "Missing required tools:"
        for tool in "${missing_tools[@]}"; do
            echo "  - ${tool}"
        done
        echo ""
        echo "Install missing tools:"
        echo "  brew install protobuf jq"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    
    # Check Rust toolchain components
    if ! cargo fmt --version >/dev/null 2>&1; then
        print_warning "rustfmt not available, installing..."
        rustup component add rustfmt
    fi
    
    if ! cargo clippy --version >/dev/null 2>&1; then
        print_warning "clippy not available, installing..."
        rustup component add clippy
    fi
    
    print_success "Prerequisites check passed"
}

# Format check
check_formatting() {
    print_step "Checking code formatting..."
    cd "${PROJECT_ROOT}"
    
    if cargo fmt --all -- --check; then
        print_success "Code formatting is correct"
    else
        print_error "Code formatting issues found"
        print_warning "Run 'cargo fmt --all' to fix formatting"
        return 1
    fi
}

# Apply formatting
apply_formatting() {
    print_step "Applying code formatting..."
    cd "${PROJECT_ROOT}"
    
    cargo fmt --all
    print_success "Code formatting applied"
}

# Linting with Clippy
run_clippy() {
    print_step "Running Clippy lints..."
    cd "${PROJECT_ROOT}"
    
    if cargo clippy --all-targets --workspace -- -D warnings; then
        print_success "Clippy lints passed"
    else
        print_error "Clippy found issues"
        return 1
    fi
}

# Security audit
run_security_audit() {
    print_step "Running security audit..."
    cd "${PROJECT_ROOT}"
    
    # Install cargo-audit if not present
    if ! cargo audit --version >/dev/null 2>&1; then
        print_warning "Installing cargo-audit..."
        cargo install cargo-audit --locked
    fi
    
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security audit found issues (may be acceptable)"
        return 0  # Don't fail build for security warnings
    fi
}

# Build function
build_project() {
    local build_mode="$1"
    print_step "Building project (${build_mode})..."
    cd "${PROJECT_ROOT}"
    
    local flags=""
    if [[ "${build_mode}" == "release" ]]; then
        flags="--release"
    fi
    
    # Set memory-optimized linker flags for macOS
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-dead_strip"
    
    # Build libraries first
    if cargo build ${flags} --workspace --jobs "${JOBS}" --lib; then
        print_success "Libraries built successfully (${build_mode})"
    else
        print_error "Failed to build libraries (${build_mode})"
        return 1
    fi
    
    # Build binaries
    if cargo build ${flags} --workspace --jobs "${JOBS}" --bins; then
        print_success "Binaries built successfully (${build_mode})"
    else
        print_error "Failed to build binaries (${build_mode})"
        return 1
    fi
}

# Run tests
run_tests() {
    local build_mode="$1"
    print_step "Running tests (${build_mode})..."
    cd "${PROJECT_ROOT}"
    
    local flags=""
    if [[ "${build_mode}" == "release" ]]; then
        flags="--release"
    fi
    
    # Set memory-optimized linker flags
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-dead_strip"
    
    if cargo test ${flags} --workspace --verbose --jobs "${JOBS}"; then
        print_success "Tests passed (${build_mode})"
    else
        print_error "Tests failed (${build_mode})"
        return 1
    fi
}

# Build examples
build_examples() {
    if [[ "${SKIP_EXAMPLES}" == "true" ]]; then
        print_warning "Skipping examples build"
        return 0
    fi
    
    print_step "Building examples..."
    cd "${PROJECT_ROOT}"
    
    local examples=(
        "hello-world"
        "distributed-transactions-example"
        "distributed-counter"
        "saga-example"
    )
    
    for example in "${examples[@]}"; do
        print_step "Building example: ${example}"
        if cargo build --example "${example}" --jobs "${JOBS}"; then
            print_success "Built example: ${example}"
        else
            print_warning "Failed to build example: ${example} (continuing...)"
        fi
    done
}

# Generate documentation
generate_docs() {
    print_step "Generating documentation..."
    cd "${PROJECT_ROOT}"
    
    if cargo doc --no-deps --workspace; then
        print_success "Documentation generated successfully"
        echo "  Documentation available at: target/doc/index.html"
    else
        print_error "Failed to generate documentation"
        return 1
    fi
}

# Generate code coverage (optional)
generate_coverage() {
    print_step "Generating code coverage..."
    cd "${PROJECT_ROOT}"
    
    # Install cargo-tarpaulin if not present
    if ! cargo tarpaulin --version >/dev/null 2>&1; then
        print_warning "Installing cargo-tarpaulin (this may take a while)..."
        cargo install cargo-tarpaulin --locked
    fi
    
    # Check if tarpaulin config exists
    if [[ -f "${PROJECT_ROOT}/tarpaulin.toml" ]]; then
        print_step "Using tarpaulin configuration file..."
        if cargo tarpaulin --config tarpaulin.toml --verbose --timeout 300 --out Html --output-dir ./coverage; then
            print_success "Code coverage generated"
            echo "  Coverage report available at: coverage/tarpaulin-report.html"
        else
            print_warning "Code coverage generation failed"
            return 0  # Don't fail build
        fi
    else
        print_warning "No tarpaulin.toml found, skipping code coverage"
    fi
}

# Create build artifacts
create_artifacts() {
    local build_mode="$1"
    print_step "Creating build artifacts (${build_mode})..."
    cd "${PROJECT_ROOT}"
    
    local version
    version=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')
    local commit_sha
    commit_sha=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    local target="$(rustc -vV | grep host | cut -d' ' -f2)"
    
    local artifact_dir="orbit-rs-${version}-${target}-${build_mode}"
    local artifact_path="${PROJECT_ROOT}/target/artifacts/${artifact_dir}"
    
    # Create artifact directory
    mkdir -p "${artifact_path}"
    
    # Copy binaries
    local target_dir="target/${build_mode}"
    if [[ "${build_mode}" == "release" ]]; then
        target_dir="target/release"
    else
        target_dir="target/debug"
    fi
    
    # Copy available binaries
    for binary in orbit-server orbit-client orbit-operator; do
        if [[ -f "${target_dir}/${binary}" ]]; then
            cp "${target_dir}/${binary}" "${artifact_path}/"
            print_success "Copied binary: ${binary}"
        else
            print_warning "Binary not found: ${binary}"
        fi
    done
    
    # Copy additional files
    cp README.md "${artifact_path}/" 2>/dev/null || print_warning "README.md not found"
    cp LICENSE* "${artifact_path}/" 2>/dev/null || print_warning "LICENSE files not found"
    
    # Copy configuration files
    if [[ -d "config" ]]; then
        cp -r config "${artifact_path}/"
        print_success "Copied configuration files"
    fi
    
    # Copy Helm charts
    if [[ -d "helm" ]]; then
        cp -r helm "${artifact_path}/"
        print_success "Copied Helm charts"
    fi
    
    # Copy Kubernetes manifests
    if [[ -d "k8s" ]]; then
        cp -r k8s "${artifact_path}/"
        print_success "Copied Kubernetes manifests"
    fi
    
    # Create version info file
    cat > "${artifact_path}/VERSION.txt" << EOF
Orbit-RS Version: ${version}
Target: ${target}
Profile: ${build_mode}
Commit: ${commit_sha}
Build Date: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
Built by: Manual CI/CD Script
Built on: $(uname -s) $(uname -r)
EOF
    
    # Create archive
    cd "${PROJECT_ROOT}/target/artifacts"
    if tar -czf "${artifact_dir}.tar.gz" "${artifact_dir}"; then
        print_success "Created artifact: target/artifacts/${artifact_dir}.tar.gz"
    else
        print_error "Failed to create artifact archive"
        return 1
    fi
}

# Generate checksums
generate_checksums() {
    print_step "Generating checksums..."
    cd "${PROJECT_ROOT}/target/artifacts"
    
    if [[ -n "$(ls *.tar.gz 2>/dev/null)" ]]; then
        shasum -a 256 *.tar.gz > checksums.txt
        print_success "Generated checksums"
        cat checksums.txt
    else
        print_warning "No artifacts found for checksum generation"
    fi
}

# Main execution
main() {
    print_banner
    
    # Navigate to project root
    cd "${PROJECT_ROOT}"
    
    # Initialize error tracking
    local errors=0
    
    # Phase 1: Prerequisites and Basic Checks
    check_prerequisites || ((errors++))
    
    # Phase 2: Code Quality Checks
    if ! check_formatting; then
        print_warning "Auto-fixing formatting issues..."
        apply_formatting
    fi
    
    run_clippy || ((errors++))
    run_security_audit  # Don't count as error
    
    # Phase 3: Build Phase
    case "${BUILD_TYPE}" in
        "debug")
            build_project "debug" || ((errors++))
            if [[ "${SKIP_TESTS}" != "true" ]]; then
                run_tests "debug" || ((errors++))
            fi
            create_artifacts "debug" || ((errors++))
            ;;
        "release")
            build_project "release" || ((errors++))
            if [[ "${SKIP_TESTS}" != "true" ]]; then
                run_tests "release" || ((errors++))
            fi
            create_artifacts "release" || ((errors++))
            ;;
        "both")
            build_project "debug" || ((errors++))
            build_project "release" || ((errors++))
            if [[ "${SKIP_TESTS}" != "true" ]]; then
                run_tests "debug" || ((errors++))
                run_tests "release" || ((errors++))
            fi
            create_artifacts "debug" || ((errors++))
            create_artifacts "release" || ((errors++))
            ;;
        *)
            print_error "Invalid build type: ${BUILD_TYPE}"
            echo "Valid options: debug, release, both"
            exit 1
            ;;
    esac
    
    # Phase 4: Additional Tasks
    build_examples
    generate_docs || ((errors++))
    
    # Optional: Generate code coverage (comment out if not needed)
    # generate_coverage
    
    # Phase 5: Finalization
    generate_checksums
    
    # Summary
    echo ""
    print_step "Build Summary"
    if [[ ${errors} -eq 0 ]]; then
        print_success "All CI/CD checks completed successfully! 🎉"
        echo ""
        echo "Artifacts created in: target/artifacts/"
        echo "Documentation available at: target/doc/index.html"
        echo ""
        echo "Next steps:"
        echo "  1. Review build artifacts"
        echo "  2. Test binaries manually if needed"
        echo "  3. Commit and push changes if satisfied"
    else
        print_error "CI/CD completed with ${errors} error(s) ❌"
        echo ""
        echo "Please review and fix the issues before proceeding."
        exit 1
    fi
}

# Handle script arguments
case "${1:-}" in
    "--help"|"-h")
        echo "Usage: $0 [OPTIONS]"
        echo ""
        echo "Environment variables:"
        echo "  BUILD_TYPE    Build type: debug, release, both (default: both)"
        echo "  JOBS          Number of parallel jobs (default: 2)"
        echo "  SKIP_TESTS    Skip running tests (default: false)"
        echo "  SKIP_EXAMPLES Skip building examples (default: false)"
        echo ""
        echo "Examples:"
        echo "  $0                          # Run full CI/CD"
        echo "  BUILD_TYPE=debug $0         # Build debug only"
        echo "  SKIP_TESTS=true $0          # Skip tests"
        echo "  JOBS=4 $0                   # Use 4 parallel jobs"
        exit 0
        ;;
    "")
        main
        ;;
    *)
        print_error "Unknown option: $1"
        echo "Use $0 --help for usage information"
        exit 1
        ;;
esac