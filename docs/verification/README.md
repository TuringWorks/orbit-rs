# Local CI/CD Verification System

This directory contains scripts that replicate all the checks from the GitHub Actions workflows locally, allowing you to validate your code before pushing to the repository.

## 🎯 Overview

The verification system mirrors the following GitHub Actions workflows:
- `.github/workflows/ci.yml` - Basic Continuous Integration
- `.github/workflows/ci-cd-enhanced.yml` - **Enhanced CI/CD Pipeline** (Primary)
- `.github/workflows/ci-cd.yml` - Legacy CI/CD Pipeline (Renamed)
- `.github/workflows/test-release.yml` - Test Release Workflow

## 📁 Structure

```
verification/
├── README.md              # This file
├── verify_all.sh          # Main script - runs all checks
├── quick_check.sh         # Fast essential checks for development
├── checks/                # Individual check scripts
│   ├── check_formatting.sh    # Code formatting (cargo fmt)
│   ├── check_clippy.sh        # Linting (cargo clippy)
│   ├── check_build.sh         # Building (cargo build)
│   ├── check_tests.sh         # Testing (cargo test)
│   ├── check_security.sh      # Security audits (cargo audit, cargo deny)
│   ├── check_examples.sh      # Example builds
│   ├── check_coverage.sh      # Code coverage (cargo tarpaulin)
│   ├── check_benchmarks.sh    # Benchmarks (cargo bench)
│   ├── check_docs.sh          # Documentation (cargo doc)
│   └── check_helm.sh          # Helm chart validation
└── stages/                # Grouped stage scripts
    ├── rust_checks.sh         # All Rust-related checks
    ├── quality_checks.sh      # Coverage, benchmarks, docs
    └── infrastructure_checks.sh # Helm checks (Docker removed)
```

## 🚀 Quick Start

### Run All Checks
```bash
./verification/verify_all.sh
```

### Quick Development Check (Fastest)
```bash
./verification/quick_check.sh
```

### Fast Mode (Skip Slow Checks with Timeouts)
```bash
./verification/verify_all.sh --fast
```

### Run Only Essential Checks (Skip Slow Ones)
```bash
./verification/verify_all.sh --skip-optional
```

### Run with Custom Timeout
```bash
./verification/verify_all.sh --timeout=600  # 10 minutes
```

### Run Specific Stage
```bash
./verification/verify_all.sh --stage rust
./verification/verify_all.sh --stage quality  
./verification/verify_all.sh --stage infrastructure
```

### Run with Verbose Output
```bash
./verification/verify_all.sh --verbose
```

## 🔧 Prerequisites

### Required (macOS)
- **Rust**: Install via [rustup.rs](https://rustup.rs/)
- **Protocol Buffers**: `brew install protobuf`
- **GNU coreutils** (for timeout support): `brew install coreutils`

### Required Rust Components
```bash
rustup component add rustfmt
rustup component add clippy
```

### Optional Tools (Auto-installed)
- **cargo-tarpaulin**: For code coverage (comprehensive mode)
- **cargo-llvm-cov**: Alternative coverage tool (comprehensive mode)
- **cargo-audit**: For security vulnerability scanning
- **cargo-deny**: For license and dependency checking
- **cargo-benchmarks**: For performance testing (if orbit-benchmarks package exists)

### System Tools
- **Helm**: For Kubernetes chart validation (optional)

## 📊 Check Details

### Rust Checks (Stage 1)
| Check | Command | Purpose |
|-------|---------|---------|
| Formatting | `cargo fmt --all -- --check` | Code style consistency |
| Clippy | `cargo clippy --all-targets --features="resp,postgres-wire,cypher,rest" -- -D warnings` | Linting and best practices |
| Build | `cargo build --release --workspace` | Compilation validation |
| Tests | `cargo test --workspace --verbose` | Unit and integration tests |
| Security | `cargo audit && cargo deny check` | Vulnerability scanning |
| Examples | `cargo build --package <example>` | Example project builds |

### Quality Checks (Stage 2)
| Check | Command | Purpose |
|-------|---------|---------|
| Documentation | `cargo doc --no-deps --features="..."` | API documentation |
| Coverage | `cargo tarpaulin --verbose --workspace` | Code coverage analysis |
| Benchmarks | `cargo bench --package orbit-benchmarks` | Performance testing |

### Infrastructure Checks (Stage 3)
| Check | Command | Purpose |
|-------|---------|---------|
| Helm | `helm lint && helm template` | Kubernetes deployment validation |

## 💡 Usage Examples

### Development Workflow
```bash

# Quick check during development (1-3 minutes)
./verification/quick_check.sh

# Fast mode for pre-commit validation
./verification/verify_all.sh --fast

# Quick validation of Rust code only
./verification/verify_all.sh --stage rust

# If Rust checks pass, run all checks
./verification/verify_all.sh
```

### Pre-commit Hook
Add to `.git/hooks/pre-commit`:
```bash

#!/bin/bash
./verification/quick_check.sh
```

### CI/CD Troubleshooting
```bash

# Run only the failing stage with verbose output
./verification/verify_all.sh --stage rust --verbose

# If tests are slow, use fast mode with longer timeout
./verification/verify_all.sh --fast --timeout=600
```

### Coverage Analysis
```bash

# Use tarpaulin (default)
./verification/checks/check_coverage.sh

# Use llvm-cov instead
COVERAGE_METHOD=llvm-cov ./verification/checks/check_coverage.sh

# Use both coverage tools
COVERAGE_METHOD=both ./verification/checks/check_coverage.sh
```

### Individual Check
```bash

# Run just formatting check
./verification/checks/check_formatting.sh

# Run just Docker build
./verification/checks/check_docker.sh

# Run security audit with custom timeout
CHECK_TIMEOUT=600 ./verification/checks/check_security.sh
```

### Before Release
```bash

# Run complete validation including all optional checks
./verification/verify_all.sh

# Run with extended timeout for comprehensive coverage
./verification/verify_all.sh --timeout=900
```

## 🔍 Troubleshooting

### Common Issues

**"rustfmt not found"**
```bash
rustup component add rustfmt
```

**"clippy not found"**  
```bash
rustup component add clippy
```

**"protoc not found" (macOS)**
```bash
brew install protobuf
```

**Timeout errors (macOS)**
Install GNU coreutils for proper timeout support:
```bash
brew install coreutils
```

**Formatting issues**
```bash
cargo fmt --all
```

**Clippy warnings**
```bash
cargo clippy --fix --all-targets --features="resp,postgres-wire,cypher,rest"
```

**Test failures**
```bash
cargo test --workspace -- --nocapture
```

**Vulnerability check failures**
Update `config/deny.toml` or run:
```bash
cargo deny check
```

**Coverage generation fails**
Try alternative coverage tool:
```bash
COVERAGE_METHOD=llvm-cov ./verification/checks/check_coverage.sh
```

**Helm checks fail**
- Install Helm: [helm.sh/docs/intro/install](https://helm.sh/docs/intro/install/)
- Install chart-testing plugin: `helm plugin install https://github.com/helm/chart-testing`

## 📈 Performance Tips

- Use `--fast` mode for quickest feedback during development (skips slow operations)
- Use `--skip-optional` to skip coverage, benchmarks, and Helm
- Run `--stage rust` first to catch basic issues quickly (formatting, linting, building, testing)
- Use `--verbose` only when debugging specific failures
- Coverage and benchmarks are the slowest checks - consider running them separately
- Set `COVERAGE_METHOD=llvm-cov` for faster coverage analysis than tarpaulin
- Use custom timeouts with `--timeout=SECONDS` for slow network environments

## 📧 Exit Codes

- **0** - All checks passed successfully
- **1** - One or more checks failed
- **124/143** - Timeout (when timeout commands are available)

## ⚙️ Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SKIP_SLOW` | `false` | Skip slow operations (coverage HTML, benchmarks) |
| `COVERAGE_METHOD` | `tarpaulin` | Coverage tool: `tarpaulin`, `llvm-cov`, or `both` |
| `COVERAGE_TIMEOUT` | `300` | Timeout for coverage operations (seconds) |
| `CHECK_TIMEOUT` | `300` | Timeout for individual checks (seconds) |
| `CARGO_TERM_COLOR` | `always` | Cargo color output (set by verbose mode) |
| `RUST_BACKTRACE` | `1` | Rust backtrace (set by verbose mode) |

### Configuration Files

Validation behavior is controlled by:
- `config/deny.toml` - cargo-deny configuration for dependency/license checks
- `Cargo.toml` - Package features and dependencies
- `.github/workflows/` - CI workflow definitions (mirrored by verification scripts)

## 🔄 Continuous Integration Mapping

| Local Script | GitHub Action | Workflow File |
|--------------|---------------|---------------|
| `rust_checks.sh` | `rust-checks` job | `ci-cd-enhanced.yml` |
| `check_tests.sh` | `test` job | `ci-cd-enhanced.yml` |
| `check_coverage.sh` | `coverage` job | `ci.yml` |
| `check_benchmarks.sh` | `benchmark` job | `ci.yml` |
| `check_docs.sh` | `docs` job | `ci-cd-enhanced.yml` |
| `check_helm.sh` | `helm-checks` job | `ci-cd-enhanced.yml` |

## 📝 Notes

- Scripts are designed to be idempotent and safe to run multiple times
- Optional checks (coverage, benchmarks, Helm) won't fail the overall verification if they're not available
- All scripts include helpful error messages and suggestions for fixing issues
- The verification system supports both development and CI-like environments
- Docker requirements have been removed from the enhanced CI/CD pipeline
