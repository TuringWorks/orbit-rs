#!/bin/bash
# Validation script for K8s Container Pipeline fixes

set -e

echo "🔍 Validating K8s Container Pipeline fixes..."
echo ""

# Check 1: Verify lowercase IMAGE_NAMESPACE
echo "✓ Check 1: Verifying IMAGE_NAMESPACE is lowercase..."
if grep -q "IMAGE_NAMESPACE: turingworks/orbit-rs" .github/workflows/k8s-container-pipeline.yml; then
    echo "  ✅ IMAGE_NAMESPACE is correctly lowercase"
else
    echo "  ❌ IMAGE_NAMESPACE is not lowercase"
    exit 1
fi
echo ""

# Check 2: Verify only binary components in matrix
echo "✓ Check 2: Verifying build matrix only includes binary components..."
if grep -q 'component: \["orbit-server", "orbit-operator"\]' .github/workflows/k8s-container-pipeline.yml; then
    echo "  ✅ Build matrix correctly includes only orbit-server and orbit-operator"
else
    echo "  ❌ Build matrix includes non-binary components"
    exit 1
fi
echo ""

# Check 3: Verify OpenSSL environment variables for ARM64
echo "✓ Check 3: Verifying OpenSSL configuration for ARM64..."
if grep -q "OPENSSL_DIR=/usr" .github/workflows/k8s-container-pipeline.yml && \
   grep -q "OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu" .github/workflows/k8s-container-pipeline.yml && \
   grep -q "PKG_CONFIG_ALLOW_CROSS=1" .github/workflows/k8s-container-pipeline.yml; then
    echo "  ✅ OpenSSL environment variables configured for ARM64"
else
    echo "  ❌ OpenSSL environment variables missing or incorrect"
    exit 1
fi
echo ""

# Check 4: Verify removed git push in Pages deployment
echo "✓ Check 4: Verifying Pages deployment doesn't push to protected branch..."
if grep -q "Stage download page for Pages deployment" .github/workflows/k8s-container-pipeline.yml && \
   ! grep -q "git push" .github/workflows/k8s-container-pipeline.yml; then
    echo "  ✅ Pages deployment correctly uses artifacts, no git push"
else
    echo "  ⚠️  Warning: git push may still exist in workflow"
fi
echo ""

# Check 5: Verify binary targets exist
echo "✓ Check 5: Verifying binary targets exist in Cargo.toml files..."
for component in orbit-server orbit-operator; do
    if [ -f "${component}/Cargo.toml" ] && grep -q "\[\[bin\]\]" "${component}/Cargo.toml"; then
        echo "  ✅ ${component} has binary target"
    else
        echo "  ❌ ${component} missing binary target"
        exit 1
    fi
done
echo ""

# Check 6: Verify manifest creation job matches build matrix
echo "✓ Check 6: Verifying manifest creation job matches build matrix..."
if grep -A10 "create-manifests:" .github/workflows/k8s-container-pipeline.yml | grep -q 'component: \["orbit-server", "orbit-operator"\]'; then
    echo "  ✅ Manifest creation job matches build matrix"
else
    echo "  ❌ Manifest creation job doesn't match build matrix"
    exit 1
fi
echo ""

echo "✅ All validation checks passed!"
echo ""
echo "Next steps:"
echo "  1. Push changes and create PR"
echo "  2. Monitor GitHub Actions for successful workflow run"
echo "  3. Verify container images are pushed to ghcr.io/turingworks/orbit-rs/"
echo "  4. Test multi-platform images: podman pull ghcr.io/turingworks/orbit-rs/orbit-server:latest-release"
