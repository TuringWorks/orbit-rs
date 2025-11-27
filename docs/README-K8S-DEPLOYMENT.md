---
layout: default
title: Orbit-RS Kubernetes Deployment Optimization
category: documentation
---

## Orbit-RS Kubernetes Deployment Optimization

This document provides comprehensive guidance for deploying Orbit-RS in Kubernetes environments with optimized resource allocation, GPU acceleration, and various workload profiles.

##  Quick Start

### Prerequisites

- Kubernetes cluster (v1.24+)
- Helm 3.x
- kubectl
- Podman or Docker (for local testing)

### Basic Deployment

```bash

# Add the Orbit-RS Helm repository (when available)
helm repo add orbit-rs https://turingworks.github.io/orbit-rs

# Deploy with small profile (development)
helm install orbit-rs orbit-rs/orbit-rs -f helm/orbit-rs/values-small.yaml

# Deploy with enterprise profile (production)
helm install orbit-rs orbit-rs/orbit-rs -f helm/orbit-rs/values-enterprise.yaml
```

##  Workload Profiles

### Small Profile (Development/Testing)

- **Use Case**: Development, testing, small demos
- **Data Scale**: < 10GB
- **Concurrent Users**: 1-10
- **Expected Throughput**: < 1,000 ops/sec
- **Resources**: 500m CPU, 1Gi Memory

```bash
helm install orbit-rs-dev orbit-rs/orbit-rs -f values-small.yaml
```

### Medium Profile (Production/Staging)  

- **Use Case**: Production workloads, staging environments
- **Data Scale**: 10GB - 1TB
- **Concurrent Users**: 10-100
- **Expected Throughput**: 1,000 - 50,000 ops/sec
- **Resources**: 2-8 CPU, 8-32Gi Memory

```bash
helm install orbit-rs-prod orbit-rs/orbit-rs -f values-medium.yaml
```

### Large Profile (Enterprise Production)

- **Use Case**: Enterprise production, high-availability
- **Data Scale**: 1TB - 100TB
- **Concurrent Users**: 100-1,000
- **Expected Throughput**: 50,000 - 500,000 ops/sec
- **Resources**: 8-32 CPU, 32-128Gi Memory

```bash
helm install orbit-rs-enterprise orbit-rs/orbit-rs -f values-large.yaml
```

### Enterprise Profile (Hyperscale)

- **Use Case**: Hyperscale deployments, petabyte-scale
- **Data Scale**: > 100TB
- **Concurrent Users**: > 1,000
- **Expected Throughput**: > 500,000 ops/sec
- **Resources**: 32-64 CPU, 128-512Gi Memory
- **Features**: GPU acceleration, multi-region, auto-scaling

```bash
helm install orbit-rs-hyperscale orbit-rs/orbit-rs -f values-enterprise.yaml
```

##  GPU Acceleration Support

Orbit-RS provides comprehensive compute acceleration through the [orbit-compute framework](COMPUTE_ACCELERATION_GUIDE.md) supporting various workload types:

### GPU-Accelerated Database Workloads

- **Vector Operations**: Similarity search, embeddings, vector aggregations (8-15x speedup)
- **Matrix Operations**: Large JOINs, linear algebra, ML workloads (8-15x speedup)
- **Time Series Analytics**: Aggregations, moving averages (5-12x speedup)
- **Graph Operations**: Traversals, community detection (4-10x speedup)
- **Text Processing**: Search, NLP, regex operations (2-8x speedup)

 **See the [Compute Acceleration Guide](COMPUTE_ACCELERATION_GUIDE.md) for detailed workload types, client configuration options, and performance tuning.**

### Supported GPU Types

| Vendor | Architecture | Models | Support Status |
|--------|-------------|---------|---------------|
| NVIDIA | Ampere | A100, A40, RTX 4090 |  Full |
| NVIDIA | Ada Lovelace | RTX 4080, RTX 4070 |  Full |  
| NVIDIA | Hopper | H100, H200 |  Full |
| AMD | RDNA3 | RX 7900 XTX |  Experimental |
| AMD | CDNA2 | MI250X, MI210 |  Experimental |
| Apple | M1/M2/M3 | Mac Studio, MacBook Pro |  Metal Support |

### GPU-Enabled Deployment

```bash

# Deploy with GPU support (enterprise profile)
helm install orbit-rs-gpu orbit-rs/orbit-rs \
  -f values-enterprise.yaml \
  --set orbitCompute.gpu.enabled=true \
  --set orbitCompute.gpu.count=4 \
  --set orbitCompute.gpu.vendor=nvidia
```

### Container Images

The pipeline automatically builds GPU-enabled variants:

```bash

# Pull GPU-enabled orbit-compute image
podman pull ghcr.io/turingworks/orbit-rs/orbit-compute:latest-release-gpu-linux-amd64

# Run with GPU support
podman run -d --gpus all -p 8080:8080 \
  ghcr.io/turingworks/orbit-rs/orbit-compute:latest-release-gpu-linux-amd64
```

##  Storage Configuration

### Storage Classes

Create appropriate storage classes for different workload types:

```yaml

# Fast NVMe storage for production
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: fast-nvme
provisioner: kubernetes.io/aws-ebs  # or your cloud provider
parameters:
  type: io2
  iopsPerGB: "50"
  encrypted: "true"
allowVolumeExpansion: true
```

### Memory-Mapped Files

For high-performance workloads, configure memory-mapped file storage:

```yaml

# RAM disk for ultra-fast access
orbitServer:
  mmapStorage:
    enabled: true
    storageClass: "memory"
    size: "32Gi"
    mountPath: "/mmap-cache"
```

##  Monitoring and Autoscaling

### Horizontal Pod Autoscaling (HPA)

```yaml
autoscaling:
  enabled: true
  hpa:
    enabled: true
    minReplicas: 3
    maxReplicas: 20
    metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

### Custom Metrics

For GPU workloads, configure GPU utilization metrics:

```yaml

# Custom GPU metrics
- type: Pods
  pods:
    metric:
      name: gpu_utilization
    target:
      type: AverageValue
      averageValue: "80"
```

##  Security Configuration

### Pod Security Standards

Enterprise deployments use restricted security policies:

```yaml
securityContext:
  allowPrivilegeEscalation: false
  capabilities:
    drop:
    - ALL
  readOnlyRootFilesystem: true
  runAsNonRoot: true
  runAsUser: 1000
```

### Network Policies

Control network access between components:

```yaml
networkPolicy:
  enabled: true
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: orbit-clients
```

##  Deployment Validation

Use the validation script to check your configuration:

```bash

# Run validation script
./scripts/validate-k8s-deployments.sh

# Dry-run validation
./scripts/validate-k8s-deployments.sh --dry-run
```

##  Container Image Download Page

Visit the automatically generated container image page for easy access to all available images:

- **GitHub Pages**: `https://turingworks.github.io/orbit-rs/container-images/`
- **Features**: Click-to-copy pull commands, GPU variants, multi-platform support

##  Troubleshooting

### Common Issues

#### GPU Not Detected

```bash

# Check GPU device plugin
kubectl get pods -n kube-system | grep nvidia-device-plugin

# Verify GPU nodes
kubectl get nodes -l accelerator=nvidia-tesla-v100

# Check GPU resources
kubectl describe node <gpu-node-name>
```

#### Storage Issues

```bash

# Check storage class
kubectl get storageclass

# Verify PVC status
kubectl get pvc -n <namespace>

# Check volume mounting
kubectl describe pod <pod-name> -n <namespace>
```

#### Performance Issues

```bash

# Check resource usage
kubectl top pods -n <namespace>

# Monitor HPA status
kubectl get hpa -n <namespace>

# Check custom metrics
kubectl get --raw /apis/custom.metrics.k8s.io/v1beta1
```

##  Performance Tuning

### CPU Optimization

```yaml

# CPU pinning and NUMA awareness
orbitServer:
  env:
    ORBIT_NUMA_AWARE: "true"
    ORBIT_CPU_AFFINITY: "0-31"
  resources:
    requests:
      hugepages-2Mi: "32Gi"
```

### Memory Optimization

```yaml

# Huge pages and memory-mapped files
orbitServer:
  env:
    ORBIT_USE_HUGEPAGES: "true"
    ORBIT_MMAP_SIZE: "64GB"
  volumes:
  - name: hugepages
    emptyDir:
      medium: HugePages-2Mi
```

### GPU Optimization

```yaml

# Multi-GPU configuration
orbitCompute:
  gpu:
    count: 4
    strategy: "model_parallel"
    peerAccess: true
    nvlink: true
  env:
    NCCL_DEBUG: "INFO"
    ORBIT_GPU_PEER_ACCESS: "true"
```

##  Additional Resources

- [Kubernetes Deployment Sizing Guide](k8s-deployment-sizing-guide.md)
- [Container Images Page](https://turingworks.github.io/orbit-rs/container-images/)
- [GPU Support Documentation](GPU_COMPLETE_DOCUMENTATION.md)
- [Performance Tuning Guide](PETABYTE_SCALE_PERFORMANCE.md)

##  Contributing

When adding new deployment configurations:

1. Update the appropriate values file
2. Run the validation script
3. Test with different workload profiles
4. Update documentation

```bash

# Run validation before submitting PR
./scripts/validate-k8s-deployments.sh

# Format code
cargo fmt --all
```

##  License

This project is licensed under the BSD-3-Clause OR MIT license - see the [LICENSE](../LICENSE) file for details.

---

For questions or support, please open an issue in the [GitHub repository](https://github.com/TuringWorks/orbit-rs/issues)