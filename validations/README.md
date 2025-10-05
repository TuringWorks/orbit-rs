# Orbit-RS Validation Scripts

This directory contains validation scripts that mirror the checks performed in the CI/CD pipeline, allowing developers to validate their changes locally before committing.

## Scripts

### 🚀 quick_check.sh (Recommended)

**Fast validation script that runs essential checks only.**

```bash
./validations/quick_check.sh
```

**What it checks:**
1. Code formatting (`cargo fmt --all -- --check`)
2. Clippy lints (`cargo clippy`)  
3. Development build (`cargo build`)
4. Release build (`cargo build --release`)
5. Tests (`cargo test --workspace`)
6. Documentation build (`cargo doc`)
7. Security audit (`cargo audit`)
8. Vulnerability check (`cargo deny check`)

**Runtime:** ~1-3 minutes  
**Use case:** Quick pre-commit validation

### 🧪 checkin_validations.sh (Comprehensive)

**Full validation script that runs all CI/CD checks including slow operations.**

```bash
./validations/checkin_validations.sh

# Options:
./validations/checkin_validations.sh --fast          # Skip slow checks
./validations/checkin_validations.sh --timeout=600   # Set timeout (default: 300s)
./validations/checkin_validations.sh --help          # Show help
```

**What it checks (15 steps):**
- All checks from quick_check.sh, plus:
- Example builds
- Code coverage (tarpaulin & llvm-cov)
- Benchmarks  
- Helm chart validation

**Runtime:** ~5-15 minutes  
**Use case:** Complete validation before important commits/releases

## CI/CD Pipeline Alignment

These scripts mirror the checks performed in:

- **`.github/workflows/ci.yml`** - Multi-version Rust testing, security, coverage
- **`.github/workflows/ci-cd.yml`** - Production builds, Docker, Kubernetes deployment  
- **`.github/workflows/codeql.yml`** - Static security analysis

### Checks Covered Locally

✅ **Code formatting** - `cargo fmt --all -- --check`  
✅ **Linting** - `cargo clippy --all-targets --features="resp,postgres-wire,cypher,rest" -- -D warnings`  
✅ **Build (dev)** - `cargo build --features="resp,postgres-wire,cypher,rest"`  
✅ **Build (release)** - `cargo build --release --workspace`  
✅ **Tests** - `cargo test --workspace --verbose`  
✅ **Examples** - `cargo build --package <example>`  
✅ **Security audit** - `cargo audit`  
✅ **Vulnerability check** - `cargo deny check`  
✅ **Documentation** - `cargo doc --no-deps`  
✅ **Code coverage** - `cargo tarpaulin` & `cargo llvm-cov`  
✅ **Benchmarks** - `cargo bench --package orbit-benchmarks`  
✅ **Helm charts** - `helm lint` & `helm template`  

### Checks Only in CI/CD

❌ **Docker builds** - Multi-platform container images  
❌ **Container security** - Trivy vulnerability scanning  
❌ **SBOM generation** - Software Bill of Materials  
❌ **Kubernetes deployment** - Staging/production deployments  
❌ **CodeQL analysis** - Advanced static analysis  

## Usage Recommendations

### Pre-commit Hook
Add to `.git/hooks/pre-commit`:
```bash
#!/bin/bash
./validations/quick_check.sh
```

### Development Workflow
```bash
# Quick check during development
./validations/quick_check.sh

# Full validation before pushing
./validations/checkin_validations.sh

# If tests are slow, use fast mode
./validations/checkin_validations.sh --fast
```

### CI/CD Troubleshooting
If CI/CD fails:
1. Run `./validations/quick_check.sh` to catch common issues
2. Run `./validations/checkin_validations.sh` for comprehensive validation
3. Check specific tool versions if there are discrepancies

## Tool Installation

The scripts will automatically install required tools:
- `cargo-audit` - Security vulnerability scanning
- `cargo-deny` - License and dependency checking  
- `cargo-tarpaulin` - Code coverage (comprehensive script only)
- `cargo-llvm-cov` - Alternative coverage (comprehensive script only)

**Note:** On macOS, install GNU coreutils for timeout support:
```bash
brew install coreutils
```

## Configuration

Validation behavior is controlled by:
- `deny.toml` - cargo-deny configuration
- `Cargo.toml` - Package features and dependencies
- CI workflow files - Pipeline definitions

## Exit Codes

- **0** - All checks passed
- **1** - One or more checks failed
- **124/143** - Timeout (comprehensive script only)

## Troubleshooting

### Common Issues

**Timeout errors:**
```bash
./validations/checkin_validations.sh --timeout=600
```

**Formatting issues:**
```bash
cargo fmt --all
```

**Clippy warnings:**
```bash
cargo clippy --fix --all-targets --features="resp,postgres-wire,cypher,rest"
```

**Test failures:**
```bash
cargo test --workspace -- --nocapture
```

**Vulnerability check failures:**
Update `deny.toml` or run:
```bash
cargo deny check
```