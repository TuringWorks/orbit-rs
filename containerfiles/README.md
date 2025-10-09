# Orbit-RS Container Files

This directory contains Containerfiles (equivalent to Dockerfiles) used for building Orbit-RS container images with Podman.

## Overview

The CI/CD pipeline uses Podman instead of Docker for building container images. This provides several benefits:
- Rootless container builds
- Better security isolation
- OCI-compliant images
- Multi-platform build support

## Files

- `Containerfile.base` - Base template for all components
- Individual component Containerfiles are generated dynamically during CI builds

## Building Locally with Podman

### Prerequisites

Install Podman:
- **macOS**: `brew install podman`
- **Ubuntu/Debian**: `sudo apt-get install podman`
- **RHEL/CentOS**: `sudo yum install podman`
- **Fedora**: `sudo dnf install podman`

### Build Commands

```bash
# Build orbit-server (example)
podman build \
  --platform=linux/amd64 \
  --file=containerfiles/Containerfile.orbit-server \
  --tag=orbit-server:local \
  --label="version=local-build" \
  .

# Build for multiple platforms
podman build \
  --platform=linux/amd64,linux/arm64 \
  --file=containerfiles/Containerfile.orbit-server \
  --tag=orbit-server:multiarch \
  .

# Run the built image
podman run -d -p 8080:8080 orbit-server:local
```

## Security Features

All container images include:
- Non-root user execution (`orbit` user)
- Minimal Debian base image
- Security scanning with Trivy
- Health checks
- Read-only root filesystem
- Capability dropping

## Multi-Platform Support

The CI pipeline builds images for:
- `linux/amd64` (Intel/AMD x86_64)
- `linux/arm64` (ARM64/Apple Silicon)

## Registry

Images are published to GitHub Container Registry:
```
ghcr.io/turingworks/orbit-rs/{component}:{version}-{build_type}
```

Build types:
- `release` - Optimized production builds
- `debug` - Development builds with debug symbols

## Components

The following Orbit-RS components have container images:
- `orbit-server` - Main distributed system server
- `orbit-client` - Client library container
- `orbit-operator` - Kubernetes operator
- `orbit-compute` - Compute node component

## Local Development

For local development, the containerfiles are generated dynamically during the CI build process. To build locally, you can use the base containerfile as a template and modify it for your specific component.

## Related Files

- `.github/workflows/k8s-container-pipeline.yml` - CI/CD pipeline
- `.dockerignore` - Files excluded from container builds
- `k8s/` - Kubernetes deployment manifests
- `helm/` - Helm charts for deployment