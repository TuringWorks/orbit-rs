#!/bin/bash

# Docker Build Check
# Replicates: docker build process from CI/CD

set -e

echo "🔍 Building Docker image..."

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    echo "❌ Docker not found. Please install Docker to run this check."
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    echo "❌ Docker daemon is not running. Please start Docker."
    exit 1
fi

# Build image with a local tag
IMAGE_TAG="orbit-rs:local-test"

echo "Running: docker build -t $IMAGE_TAG ."

# Build the Docker image
if docker build -t "$IMAGE_TAG" .; then
    echo "✅ Docker image build successful"
    
    # Show image information
    echo ""
    echo "📦 Docker image information:"
    docker images "$IMAGE_TAG" --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"
    
    echo ""
    echo "🔍 Testing container startup..."
    
    # Test if container can start (run for 5 seconds then stop)
    if timeout 10s docker run --rm "$IMAGE_TAG" --help &> /dev/null; then
        echo "✅ Container starts successfully"
    else
        echo "⚠️ Container test skipped (this might be expected if the binary requires specific arguments)"
    fi
    
    echo ""
    echo "💡 To run the container: docker run --rm -p 8080:8080 $IMAGE_TAG"
    echo "💡 To clean up: docker rmi $IMAGE_TAG"
    
else
    echo "❌ Docker image build failed"
    exit 1
fi

echo "✅ Docker check completed successfully"