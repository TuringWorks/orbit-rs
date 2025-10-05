# Local CI/CD Verification System

This directory contains scripts that replicate all the checks from the GitHub Actions workflows locally, allowing you to validate your code before pushing to the repository.

## 🎯 Overview

The verification system mirrors the following GitHub Actions workflows:
- `.github/workflows/ci.yml` - Continuous Integration
- `.github/workflows/ci-cd.yml` - CI/CD Pipeline

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
│   ├── check_docker.sh        # Docker build
│   └── check_helm.sh          # Helm chart validation
└── stages/                # Grouped stage scripts
    ├── rust_checks.sh         # All Rust-related checks
    ├── quality_checks.sh      # Coverage, benchmarks, docs
    └── infrastructure_checks.sh # Docker and Helm checks
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

### Run Only Essential Checks (Skip Slow Ones)
```bash
./verification/verify_all.sh --skip-optional
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

### Optional Tools
- **Docker**: For container build checks
- **Helm**: For Kubernetes chart validation
- **cargo-tarpaulin**: For code coverage (auto-installed)
- **cargo-audit**: For security audits (auto-installed)
- **cargo-deny**: For dependency validation (auto-installed)

## 📊 Check Details

### Rust Checks (Stage 1)
| Check | Command | Purpose |
|-------|---------|---------|
| Formatting | `cargo fmt --all -- --check` | Code style consistency |
| Clippy | `cargo clippy --all-targets --features="..." -- -D warnings` | Linting and best practices |
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
| Docker | `docker build -t orbit-rs:local-test .` | Container image build |
| Helm | `helm lint && helm template` | Kubernetes deployment validation |

## 💡 Usage Examples

### Before Committing
```bash
# Fastest check during development
./verification/quick_check.sh

# Quick validation of Rust code
./verification/verify_all.sh --stage rust

# If Rust checks pass, run all checks
./verification/verify_all.sh
```

### CI/CD Troubleshooting
```bash
# Run only the failing stage with verbose output
./verification/verify_all.sh --stage rust --verbose
```

### Individual Check
```bash
# Run just formatting check
./verification/checks/check_formatting.sh

# Run just Docker build
./verification/checks/check_docker.sh
```

### Before Release
```bash
# Run complete validation including optional checks
./verification/verify_all.sh
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

**Docker checks fail**
- Make sure Docker Desktop is running
- Check if you have sufficient disk space

**Helm checks fail**
- Install Helm: [helm.sh/docs/intro/install](https://helm.sh/docs/intro/install/)
- Install chart-testing plugin: `helm plugin install https://github.com/helm/chart-testing`

## 📈 Performance Tips

- Use `--skip-optional` for faster feedback during development
- Run `--stage rust` first to catch basic issues quickly
- Use `--verbose` only when debugging specific failures
- Coverage and benchmarks are the slowest checks - consider running them separately

## 🔄 Continuous Integration Mapping

| Local Script | GitHub Action | Workflow File |
|--------------|---------------|---------------|
| `rust_checks.sh` | `rust-checks` job | `ci-cd.yml` |
| `check_tests.sh` | `test` job | `ci.yml` |
| `check_coverage.sh` | `coverage` job | `ci.yml` |
| `check_benchmarks.sh` | `benchmark` job | `ci.yml` |
| `check_docs.sh` | `docs` job | `ci.yml` |
| `check_docker.sh` | `docker-build` job | `ci-cd.yml` |
| `check_helm.sh` | `helm-checks` job | `ci-cd.yml` |

## 📝 Notes

- Scripts are designed to be idempotent and safe to run multiple times
- Optional checks (coverage, benchmarks, Docker, Helm) won't fail the overall verification if they're not available
- All scripts include helpful error messages and suggestions for fixing issues
- The verification system supports both development and CI-like environments