#!/bin/bash

# Helm Chart Check
# Replicates: helm lint and helm template commands from CI/CD

set -e

echo "🔍 Checking Helm charts..."

# Check if Helm is available
if ! command -v helm &> /dev/null; then
    echo "❌ Helm not found. Please install Helm to run this check."
    echo "💡 Install Helm: https://helm.sh/docs/intro/install/"
    exit 1
fi

# Check if chart-testing (ct) is available
if ! command -v ct &> /dev/null; then
    echo "⚠️ chart-testing (ct) not found. Installing via Helm plugin..."
    if ! helm plugin install https://github.com/helm/chart-testing; then
        echo "❌ Failed to install chart-testing plugin"
        echo "💡 You can install ct manually: https://github.com/helm/chart-testing"
    fi
fi

CHART_DIR="helm/orbit-rs"

if [ ! -d "$CHART_DIR" ]; then
    echo "❌ Helm chart directory not found: $CHART_DIR"
    exit 1
fi

# Run helm lint
echo "Running: helm lint $CHART_DIR"
if helm lint "$CHART_DIR"; then
    echo "✅ Helm lint passed"
else
    echo "❌ Helm lint failed"
    exit 1
fi

echo ""

# Run chart-testing lint if available
if command -v ct &> /dev/null; then
    echo "Running: ct lint --chart-dirs helm --charts $CHART_DIR"
    if ct lint --chart-dirs helm --charts "$CHART_DIR"; then
        echo "✅ Chart-testing lint passed"
    else
        echo "❌ Chart-testing lint failed"
        exit 1
    fi
else
    echo "⚠️ Skipping chart-testing lint (ct not available)"
fi

echo ""

# Test helm template
VALUES_FILE="$CHART_DIR/values.yaml"
echo "Running: helm template orbit-rs $CHART_DIR --values $VALUES_FILE"

if helm template orbit-rs "$CHART_DIR" --values "$VALUES_FILE" > /tmp/helm-template-output.yaml; then
    echo "✅ Helm template generation successful"
    
    # Basic validation of generated YAML
    LINE_COUNT=$(wc -l < /tmp/helm-template-output.yaml)
    echo "📄 Generated $LINE_COUNT lines of Kubernetes manifests"
    
    # Clean up
    rm -f /tmp/helm-template-output.yaml
else
    echo "❌ Helm template generation failed"
    exit 1
fi

echo "✅ Helm check completed successfully"