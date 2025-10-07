# CI/CD Pipeline Documentation

## Overview

Orbit-RS uses a comprehensive CI/CD pipeline that provides:

- ✅ **Multi-platform builds** for ARM64 and x64 on macOS, Windows, and Linux
- 🔒 **Comprehensive security scanning** with cargo-audit
- 📦 **Automated releases** with checksums and detailed changelogs
- 🛡️ **Helm chart validation** for Kubernetes deployments
- 🧪 **Multiple build profiles** (debug and release)
- 📊 **Automated failure notifications** via GitHub issues

## Workflows

### 1. Enhanced CI/CD Pipeline (`ci-cd-enhanced.yml`)

**Primary production pipeline** that handles all aspects of building, testing, and releasing Orbit-RS.

#### Triggers
- **Push** to `main` or `develop` branches
- **Tags** matching `v*` pattern
- **Pull Requests** to `main` or `develop`
- **Manual dispatch** with custom options

#### Jobs

##### Security & Quality Checks
- **Rust checks**: Format, lint, test, security audit
- **Helm checks**: Chart validation and templating
- **Security scans**: Cargo audit and vulnerability scanning

##### Multi-Platform Builds
Builds both debug and release binaries for:

| Platform | Architecture | Package Format |
|----------|-------------|----------------|
| **macOS** | Intel (x64) | `.tar.gz` |
| **macOS** | Apple Silicon (ARM64) | `.tar.gz` |
| **Linux** | x86_64 | `.tar.gz` |
| **Linux** | ARM64 | `.tar.gz` |
| **Windows** | x86_64 | `.zip` |
| **Windows** | ARM64 | `.zip` |

##### Release Creation
Automatically creates GitHub releases with:
- All platform binaries (debug + release)
- SHA256 checksums
- Comprehensive changelog
- Security information

#### Usage Examples

**Trigger a release:**
```bash
git tag v1.0.0
git push origin v1.0.0
```

**Manual dispatch with options:**
1. Go to Actions → Enhanced CI/CD Pipeline
2. Click "Run workflow"
3. Select branch and options

### 2. Test Release Workflow (`test-release.yml`)

**Development/testing pipeline** for validating builds without creating official releases.

#### Features
- Manual platform selection
- Test tag creation and cleanup
- Focused testing on specific targets
- 7-day artifact retention

#### Usage
1. Go to Actions → Test Release
2. Click "Run workflow"
3. Configure:
   - **Test tag**: `test-v0.1.0`
   - **Platforms**: Select which platforms to build

### 3. Legacy CI/CD Pipeline (`ci-cd.yml`)

Original pipeline renamed to avoid conflicts. Can be disabled or removed after migration.

## Supported Features

### Build Configurations

**Debug Builds:**
- Include debug symbols
- Faster compilation
- Larger binary size
- Useful for development and troubleshooting

**Release Builds:**
- Optimized performance
- Smaller binary size
- Production-ready

### Security Features

1. **Cargo Security Audit**
   - Scans dependencies for known vulnerabilities
   - Generates JSON reports
   - Uploaded as artifacts

2. **Dependency Checking**
   - `cargo-deny` for license and advisory checking
   - Automated vulnerability detection

### Release Assets

Each release includes:

```
📦 Release Assets
├── 🍎 macOS
│   ├── orbit-rs-v1.0.0-aarch64-apple-darwin-release.tar.gz (Apple Silicon)
│   ├── orbit-rs-v1.0.0-x86_64-apple-darwin-release.tar.gz (Intel)
│   ├── orbit-rs-v1.0.0-aarch64-apple-darwin-debug.tar.gz (Debug)
│   └── orbit-rs-v1.0.0-x86_64-apple-darwin-debug.tar.gz (Debug)
├── 🐧 Linux
│   ├── orbit-rs-v1.0.0-aarch64-unknown-linux-gnu-release.tar.gz (ARM64)
│   ├── orbit-rs-v1.0.0-x86_64-unknown-linux-gnu-release.tar.gz (x64)
│   ├── orbit-rs-v1.0.0-aarch64-unknown-linux-gnu-debug.tar.gz (Debug)
│   └── orbit-rs-v1.0.0-x86_64-unknown-linux-gnu-debug.tar.gz (Debug)
├── 🪟 Windows
│   ├── orbit-rs-v1.0.0-aarch64-pc-windows-msvc-release.zip (ARM64)
│   ├── orbit-rs-v1.0.0-x86_64-pc-windows-msvc-release.zip (x64)
│   ├── orbit-rs-v1.0.0-aarch64-pc-windows-msvc-debug.zip (Debug)
│   └── orbit-rs-v1.0.0-x86_64-pc-windows-msvc-debug.zip (Debug)
└── 📋 checksums.txt (SHA256 hashes)
```

Each package contains:
- `orbit-server` binary
- `orbit-client` binary (if available)
- `README.md`
- `LICENSE` files
- `config/` directory
- `helm/` charts
- `k8s/` manifests
- `VERSION.txt` with build information

## Configuration

### Environment Variables

Set these in your repository settings under Settings → Secrets and Variables → Actions:

#### Required Secrets
```env
GITHUB_TOKEN=<automatic>  # Provided by GitHub
```

#### Optional Secrets (for Kubernetes deployment)
```env
KUBE_CONFIG_STAGING=<base64-encoded-kubeconfig>
KUBE_CONFIG_PRODUCTION=<base64-encoded-kubeconfig>
```

#### Optional Variables
```env
RUST_BACKTRACE=1  # Enable Rust backtraces
```

### Helm Configuration

The pipeline auto-creates basic Helm charts if they don't exist. For custom deployments:

1. Create `helm/orbit-rs/` directory
2. Add your `Chart.yaml`, `values.yaml`, and templates
3. Pipeline will validate and use your charts

## Local Development

### Testing the Pipeline Locally

**1. Run individual checks:**
```bash
# Format check
cargo fmt --all -- --check

# Clippy lints
cargo clippy --all-targets --features="resp,postgres-wire,cypher,rest" -- -D warnings

# Tests
cargo test --workspace --verbose --all-features

# Security audit
cargo audit

# Helm validation (if available)
helm lint helm/orbit-rs
```

**2. Cross-compilation testing:**
```bash
# Install target
rustup target add aarch64-apple-darwin

# Build for different target
cargo build --target aarch64-apple-darwin --release
```


### Creating Releases

#### Development Releases
1. Use the Test Release workflow for validation
2. Select specific platforms to reduce build time
3. Review artifacts before creating official releases

#### Official Releases
1. Ensure all tests pass on `main` branch
2. Create and push a version tag:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```
3. Monitor the Enhanced CI/CD Pipeline
4. Review the created release on GitHub

### Version Naming

| Pattern | Type | Example | Description |
|---------|------|---------|-------------|
| `v*.*.*` | Release | `v1.0.0` | Official release |
| `v*.*.*-alpha.*` | Pre-release | `v1.0.0-alpha.1` | Alpha version |
| `v*.*.*-beta.*` | Pre-release | `v1.0.0-beta.2` | Beta version |
| `v*.*.*-rc.*` | Pre-release | `v1.0.0-rc.1` | Release candidate |
| `test-*` | Test | `test-v1.0.0` | Test builds (auto-cleanup) |

## Troubleshooting

### Common Issues

**1. Build failures**
- Check Rust toolchain compatibility
- Verify all features are available on target platform
- Review dependency issues in logs

**2. Helm validation failures**
- Ensure `helm/orbit-rs/` directory exists
- Validate Chart.yaml syntax
- Check template rendering

**3. Security scan failures**
- Review cargo-audit output
- Update vulnerable dependencies
- Add exceptions for known false positives


### Debugging Steps

1. **Check workflow logs** in GitHub Actions
2. **Test locally** using the commands above
3. **Use Test Release workflow** for isolated testing
4. **Review artifacts** for specific failure patterns
5. **Check GitHub Issues** for automated failure reports

### Getting Help

- **GitHub Issues**: Automated failure notifications
- **Workflow Logs**: Detailed step-by-step information
- **Artifacts**: Download and inspect build outputs
- **Security Tab**: Review vulnerability scan results

## Best Practices

### For Developers

1. **Test locally** before pushing
2. **Use descriptive commit messages** (affects changelog)
3. **Update documentation** when adding features
4. **Review security scan results** regularly

### For Releases

1. **Use semantic versioning** (`v1.0.0`, `v1.0.1`, etc.)
2. **Test with Test Release workflow** first
3. **Review generated artifacts** before public release
4. **Monitor deployment** health after release

### For Security

1. **Keep dependencies updated**
2. **Review security audit results**
3. **Monitor vulnerability notifications**
4. **Test security patches** promptly

## Migration from Legacy Pipeline

1. **Test new pipeline** with Test Release workflow
2. **Verify all required secrets** are configured
3. **Disable legacy workflow** by renaming or removing
4. **Update documentation** references
5. **Train team** on new workflow features

---

For questions or issues with the CI/CD pipeline, please create a GitHub issue or check the automated failure notifications.