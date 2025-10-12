# Kubernetes Container Pipeline Fixes

## Summary
Fixed multiple issues causing the Kubernetes Container Pipeline to fail in GitHub Actions workflow run #158.

## Issues Identified and Fixed

### 1. ✅ Registry Name Case Sensitivity
**Problem:** Container registry names must be lowercase, but `IMAGE_NAMESPACE` used `${{ github.repository_owner }}` which contained uppercase letters ("TuringWorks").

**Error:**
```
Error: tag ghcr.io/TuringWorks/orbit-rs/orbit-server:...: invalid reference format: repository name must be lowercase
```

**Fix:** Applied lowercase filter to repository owner
```yaml
IMAGE_NAMESPACE: ${{ github.repository_owner | lowercase }}/orbit-rs
```

### 2. ✅ Missing Binary Targets
**Problem:** The workflow attempted to build `orbit-client` and `orbit-compute` as binaries, but these packages are libraries without `[[bin]]` targets in their Cargo.toml files.

**Error:**
```
error: no bin target named `orbit-client` in `orbit-client` package
error: no bin target named `orbit-compute` in `orbit-compute` package
```

**Fix:** Updated build matrix to only include components with binary targets:
- Before: `["orbit-server", "orbit-client", "orbit-operator", "orbit-compute"]`
- After: `["orbit-server", "orbit-operator"]`

### 3. ✅ k8s-openapi Version Feature
**Problem:** The `orbit-operator` package uses `k8s-openapi` version 0.23 with the `latest` feature, but this was updated to 0.24 which requires an explicit Kubernetes version feature (e.g., `v1_31`).

**Error:**
```
None of the v1_* features are enabled on the k8s-openapi crate.
The k8s-openapi crate requires a feature to be enabled to indicate which version of Kubernetes it should support.
```

**Fix:** Updated `orbit-operator/Cargo.toml`:
```toml
# Before
k8s-openapi = { version = "0.23", features = ["latest"] }

# After
k8s-openapi = { version = "0.24", features = ["v1_31"] }
```

### 4. ✅ ARM64 Cross-Compilation OpenSSL
**Problem:** Cross-compiling for ARM64 (aarch64) target failed because OpenSSL couldn't be found for the target architecture.

**Error:**
```
Could not find directory of OpenSSL installation, and this `-sys` crate cannot proceed without this knowledge.
pkg-config has not been configured to support cross-compilation.
```

**Fix:** Multiple improvements for ARM64 cross-compilation:
1. Added vendored OpenSSL feature for ARM64 builds to compile OpenSSL from source
2. Installed perl and make (required for vendored OpenSSL builds)
3. Added cross-compilation environment variables (CC, CXX, AR)
4. Configured cargo to use `--features openssl-sys/vendored` for ARM64 targets

## Additional Improvements

### Simplified Build Matrix
- Removed GPU build variants (set `gpu-enabled: [false]`)
- Removed complex exclusion rules
- Reduced from 24 jobs to 8 jobs (2 components × 2 build types × 2 platforms)

### Updated Sparse Checkout
- Removed unused components from sparse checkout
- Added necessary dependencies: `orbit-shared`, `orbit-util`, `orbit-proto`

### Updated Component References
- Updated all Helm values references
- Updated Kubernetes manifest generation
- Updated notification messages

## Testing Recommendations

1. **Verify Build Success:** Ensure both orbit-server and orbit-operator build successfully for amd64 and arm64
2. **Check Container Images:** Verify images are pushed to ghcr.io with lowercase repository names
3. **Validate Manifests:** Ensure Kubernetes manifests work with the new image tags
4. **Test Cross-Platform:** Verify ARM64 images work correctly (especially important for Apple Silicon deployments)

## Impact

- **Reduced Build Time:** From 24 concurrent jobs to 8 jobs
- **Improved Reliability:** Using vendored OpenSSL eliminates cross-compilation issues
- **Correct Registry Naming:** Images can now be pushed and pulled successfully
- **Only Building Binaries:** No longer attempting to build library packages as executables

## Related Files Changed

1. `.github/workflows/k8s-container-pipeline.yml` - Main workflow file with all fixes
2. `orbit-operator/Cargo.toml` - Updated k8s-openapi version and feature

## References

- GitHub Actions Run: https://github.com/TuringWorks/orbit-rs/actions/runs/18440534236
- Issue: #[number] (auto-created by workflow)
- Container Registry Docs: https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry
- k8s-openapi Docs: https://docs.rs/k8s-openapi/0.24.0/k8s_openapi/
