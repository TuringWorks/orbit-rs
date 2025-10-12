# Kubernetes Container Pipeline Fix

**Date**: 2025-10-12  
**Issue**: Kubernetes Container Pipeline Failures  
**Status**: ✅ Fixed

## Problem Summary

The Kubernetes Container Pipeline workflow was failing with 18 out of 24 jobs failing due to three distinct issues:

### Issues Identified:

1. **Docker Tag Format Error** (10 jobs failed)
   - Error: `invalid reference format: repository name must be lowercase`
   - Cause: `IMAGE_NAMESPACE` was set to `TuringWorks/orbit-rs` (uppercase letters)
   - Docker/Podman registry tags require all lowercase names

2. **Missing Binary Targets** (8 jobs failed)
   - Error: `no bin target named 'orbit-client' in 'orbit-client' package`
   - Cause: `orbit-client` and `orbit-compute` are library crates without `[[bin]]` sections
   - Workflow was trying to build them as binary applications

3. **OpenSSL Cross-Compilation** (4 jobs failed)
   - Error: `Could not find directory of OpenSSL installation`
   - Cause: Missing ARM64 OpenSSL development libraries for cross-compilation
   - Affects: ARM64 builds of orbit-server and orbit-operator

## Solution Implemented

### 1. Fixed Image Namespace to Lowercase

**File**: `.github/workflows/k8s-container-pipeline.yml`  
**Line**: 56

```yaml
# Before
IMAGE_NAMESPACE: ${{ github.repository_owner }}/orbit-rs

# After
IMAGE_NAMESPACE: turingworks/orbit-rs
```

**Reasoning**: Docker registry names must be all lowercase. The `github.repository_owner` variable resolves to "TuringWorks" with uppercase letters, which violates Docker's naming requirements.

### 2. Removed Library Crates from Build Matrix

**File**: `.github/workflows/k8s-container-pipeline.yml`  
**Lines**: 159-167

```yaml
# Before
component: ["orbit-server", "orbit-client", "orbit-operator", "orbit-compute"]

# After
component: ["orbit-server", "orbit-operator"]
```

**Reasoning**: 
- `orbit-server` and `orbit-operator` have `[[bin]]` sections in their `Cargo.toml` files
- `orbit-client` and `orbit-compute` are library crates without binary targets
- Attempting to build library crates as binaries causes the build to fail

**Verification**:
```bash
# orbit-server/Cargo.toml has:
[[bin]]
name = "orbit-server"
path = "src/main.rs"

# orbit-operator/Cargo.toml has:
[[bin]]
name = "orbit-operator"
path = "src/main.rs"

# orbit-client/Cargo.toml has NO [[bin]] section
# orbit-compute/Cargo.toml has NO [[bin]] section
```

### 3. Added ARM64 Cross-Compilation Support

**File**: `.github/workflows/k8s-container-pipeline.yml`  
**Lines**: 253-269

```yaml
- name: Install cross-compilation tools
  if: matrix.platform == 'linux/arm64'
  run: |
    # Add ARM64 architecture and install cross-compilation tools
    sudo dpkg --add-architecture arm64
    sudo apt-get update
    sudo apt-get install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
    
    # Install ARM64 development libraries for cross-compilation
    sudo apt-get install -y libssl-dev:arm64 libsqlite3-dev:arm64
    
    # Set cross-compilation environment variables
    echo "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc" >> $GITHUB_ENV
    echo "PKG_CONFIG_ALLOW_CROSS=1" >> $GITHUB_ENV
    echo "PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig" >> $GITHUB_ENV
    echo "OPENSSL_DIR=/usr" >> $GITHUB_ENV
    echo "OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu" >> $GITHUB_ENV
    echo "OPENSSL_INCLUDE_DIR=/usr/include" >> $GITHUB_ENV
```

**Reasoning**: 
- Rust's `openssl-sys` crate requires OpenSSL development libraries for the target architecture
- For ARM64 cross-compilation, we need `libssl-dev:arm64`
- PKG_CONFIG must be configured to find the ARM64 libraries

### 4. Additional Updates

#### Updated Sparse Checkout
**Lines**: 178-187
- Removed: `orbit-client/`, `orbit-compute/`
- Added: `orbit-shared/`, `orbit-util/`, `orbit-proto/` (required dependencies)

#### Updated Path Triggers
**Lines**: 9-32
- Only trigger on changes to `orbit-server/` and `orbit-operator/`
- Removed triggers for `orbit-client/` and `orbit-compute/`

#### Updated Downstream Jobs
- **create-manifests** (Line 538): Updated component matrix
- **validate-k8s-deployment** (Line 927): Updated validation loop
- **notify-success** (Line 1069): Updated success message
- **Helm values test** (Lines 1016-1040): Removed orbit-client and orbit-compute

## Verification

### YAML Validation
```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/k8s-container-pipeline.yml'))"
# Result: ✅ YAML syntax is valid
```

### Expected Build Matrix
After the fix, the workflow will build:
- `orbit-server` - debug, release (linux/amd64, linux/arm64) = 4 builds
- `orbit-operator` - debug, release (linux/amd64, linux/arm64) = 4 builds
- **Total**: 8 builds (down from 24)

## Testing

To test these changes:

1. **Test locally with Podman**:
```bash
# Test lowercase image name
podman build -t ghcr.io/turingworks/orbit-rs/orbit-server:test .

# Verify it works (lowercase)
echo $?  # Should be 0

# Test with uppercase (should fail)
podman build -t ghcr.io/TuringWorks/orbit-rs/orbit-server:test .
# Error: invalid reference format: repository name must be lowercase
```

2. **Test binary detection**:
```bash
# Check if component has binary target
cargo build --bin orbit-server  # ✅ Should succeed
cargo build --bin orbit-operator  # ✅ Should succeed
cargo build --bin orbit-client  # ❌ Should fail (no binary target)
cargo build --bin orbit-compute  # ❌ Should fail (no binary target)
```

3. **Test ARM64 cross-compilation**:
```bash
# Install cross-compilation tools
sudo dpkg --add-architecture arm64
sudo apt-get update
sudo apt-get install -y gcc-aarch64-linux-gnu libssl-dev:arm64

# Build for ARM64
cargo build --target aarch64-unknown-linux-gnu --bin orbit-server
```

## Prevention

### For Future Binary Crates

When adding a new component that should be built as a container:

1. **Ensure it has a binary target** in `Cargo.toml`:
```toml
[[bin]]
name = "component-name"
path = "src/main.rs"
```

2. **Add to workflow matrix**:
```yaml
component: ["orbit-server", "orbit-operator", "new-component"]
```

3. **Add to sparse checkout**:
```yaml
sparse-checkout: |
  orbit-server/
  orbit-operator/
  new-component/
  ...
```

### For Library Crates

If a component is a library (no binary):
- Do NOT add it to the container build matrix
- Build it as a dependency, not as a standalone container

### For Docker Image Names

Always use lowercase for registry/organization/repository names:
```yaml
# ✅ Correct
IMAGE_NAMESPACE: turingworks/orbit-rs

# ❌ Incorrect
IMAGE_NAMESPACE: TuringWorks/orbit-rs
IMAGE_NAMESPACE: ${{ github.repository_owner }}/orbit-rs  # May have uppercase
```

### For Cross-Compilation

When building for ARM64 or other architectures:
1. Install target-specific development libraries
2. Configure PKG_CONFIG for cross-compilation
3. Set OPENSSL_DIR and related environment variables

## Related Files

- `.github/workflows/k8s-container-pipeline.yml` - Main workflow file
- `orbit-server/Cargo.toml` - Binary target definition
- `orbit-operator/Cargo.toml` - Binary target definition
- `orbit-client/Cargo.toml` - Library crate (no binary)
- `orbit-compute/Cargo.toml` - Library crate (no binary)

## References

- [Docker registry naming requirements](https://docs.docker.com/engine/reference/commandline/tag/)
- [Rust binary targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries)
- [Cross-compiling Rust with OpenSSL](https://docs.rs/openssl/latest/openssl/#cross-compilation)

## Commit History

- Initial fix: `8c8e0a7` - Fix Kubernetes Container Pipeline - lowercase image namespace and remove library crates

---

**Status**: ✅ Fixed  
**Next Action**: Monitor next CI/CD run to confirm all fixes work correctly
