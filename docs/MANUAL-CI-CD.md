# Manual CI/CD Setup for Orbit-RS

This document describes how to run CI/CD checks locally after disabling automated GitHub Actions workflows.

## Overview

The automated CI/CD workflows have been **disabled** to prevent automatic runs on push/PR. You now have full control over when to run CI/CD checks using the manual scripts provided.

## Quick Start

### Prerequisites

Before running the manual CI/CD script, ensure you have the required tools installed:

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies (macOS with Homebrew)
brew install protobuf jq

# Verify installations
cargo --version
protoc --version
jq --version
```

### Running CI/CD

#### Option 1: Simple Launcher (Recommended)

```bash
# Run complete CI/CD pipeline (debug + release builds, tests, examples)
./scripts/run-ci.sh

# View help and options
./scripts/run-ci.sh --help
```

#### Option 2: Direct Script Execution

```bash
# Run complete CI/CD pipeline
./scripts/manual-ci-cd.sh

# Run with custom options
BUILD_TYPE=debug ./scripts/manual-ci-cd.sh
SKIP_TESTS=true ./scripts/manual-ci-cd.sh
JOBS=4 ./scripts/manual-ci-cd.sh
```

## Configuration Options

You can customize the CI/CD run using environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `BUILD_TYPE` | `both` | Build type: `debug`, `release`, or `both` |
| `JOBS` | `2` | Number of parallel compilation jobs |
| `SKIP_TESTS` | `false` | Skip running tests (useful for build-only checks) |
| `SKIP_EXAMPLES` | `false` | Skip building examples (saves time/memory) |

### Common Usage Examples

```bash
# Quick debug-only build and test
BUILD_TYPE=debug ./scripts/run-ci.sh

# Release build only, no tests (for packaging)
BUILD_TYPE=release SKIP_TESTS=true ./scripts/run-ci.sh

# Fast CI check (skip examples and tests)
SKIP_TESTS=true SKIP_EXAMPLES=true ./scripts/run-ci.sh

# High-performance build (more parallel jobs)
JOBS=8 ./scripts/run-ci.sh
```

## What the Manual CI/CD Does

The manual CI/CD script performs the same checks as the disabled GitHub Actions:

### 🔍 **Quality Checks**

- ✅ Code formatting (`cargo fmt --all -- --check`)
- ✅ Linting with Clippy (`cargo clippy --all-targets -- -D warnings`)  
- ✅ Security audit (`cargo audit`)

### 🔨 **Build Process**

- ✅ Build libraries (`cargo build --workspace --lib`)
- ✅ Build binaries (`cargo build --workspace --bins`)
- ✅ Build examples (hello-world, distributed-transactions, etc.)
- ✅ Support for both debug and release builds

### 🧪 **Testing**

- ✅ Run comprehensive test suite (`cargo test --workspace`)
- ✅ Tests for both debug and release builds
- ✅ Memory-optimized for local execution

### 📦 **Artifacts & Documentation**

- ✅ Generate API documentation (`cargo doc`)
- ✅ Create distributable packages with version info
- ✅ Generate SHA256 checksums
- ✅ Include configuration files, Helm charts, K8s manifests

### 🎯 **macOS Optimizations**

- ✅ macOS-specific linker optimizations
- ✅ Reduced memory usage for local builds
- ✅ Proper handling of macOS development environment

## Output & Artifacts

After a successful run, you'll find:

```
target/
├── doc/                          # Generated API documentation
│   └── index.html               # Open this in your browser
├── debug/                       # Debug binaries
├── release/                     # Release binaries  
└── artifacts/                   # Distribution packages
    ├── orbit-rs-0.1.0-aarch64-apple-darwin-debug.tar.gz
    ├── orbit-rs-0.1.0-aarch64-apple-darwin-release.tar.gz
    └── checksums.txt            # SHA256 checksums
```

## Disabled GitHub Actions

The following workflows have been disabled (converted to manual-only):

- **`.github/workflows/ci.yml`** - Basic CI checks
- **`.github/workflows/ci-cd.yml`** - Enhanced CI/CD with multi-platform builds  
- **`.github/workflows/examples.yml`** - Examples building (schedule disabled)

### Re-enabling Automatic Workflows (If Needed)

If you want to re-enable automated workflows in the future:

1. Edit the workflow files in `.github/workflows/`
2. Uncomment the `on:` trigger sections:

   ```yaml
   on:
     push:
       branches: [ main, develop ]
     pull_request:
       branches: [ main, develop ]
   ```

3. Commit and push the changes

## Manual GitHub Actions (If Needed)

You can still trigger the workflows manually via GitHub's web interface:

1. Go to your repository on GitHub
2. Click "Actions" tab
3. Select a workflow (CI, Enhanced CI/CD Pipeline, Examples CI)
4. Click "Run workflow" button
5. Choose the branch and options, then click "Run workflow"

## Troubleshooting

### Common Issues

**"protoc not found":**

```bash
brew install protobuf
```

**"jq not found":**

```bash
brew install jq
```

**Memory issues during build:**

```bash
# Reduce parallel jobs
JOBS=1 ./run-ci.sh

# Skip memory-intensive examples  
SKIP_EXAMPLES=true ./run-ci.sh
```

**Permission denied:**

```bash
chmod +x run-ci.sh scripts/manual-ci-cd.sh
```

### Getting Help

```bash
./run-ci.sh --help
```

This will show all available options and usage examples.

## Integration with Pre-commit Hooks

The manual CI/CD works well with your existing pre-commit preferences:

```bash
# Your existing pre-commit workflow
cargo fmt --all          # Format code  
./run-ci.sh              # Run comprehensive checks
git add .
git commit -m "Your commit message"
git push
```

## Performance Notes

- **Local builds are faster** than GitHub Actions for your development workflow
- **Memory usage is optimized** for macOS development machines
- **Parallel compilation** is tuned for typical developer hardware
- **Examples building** can be skipped to save time during development

---

**Next Steps:**

1. Run `./run-ci.sh` to verify everything works
2. Integrate into your development workflow
3. Customize options as needed for different scenarios
