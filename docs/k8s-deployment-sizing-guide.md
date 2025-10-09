# Kubernetes Deployment Sizing Guide

## Table of Contents

- [Overview](#overview)
- [Workload Classifications](#workload-classifications)
- [Component-Specific Sizing](#component-specific-sizing)
- [Storage and Persistent Volume Configurations](#storage-and-persistent-volume-configurations)
- [GPU and Acceleration Configurations](#gpu-and-acceleration-configurations)
- [Memory-Mapped File Storage](#memory-mapped-file-storage)
- [Monitoring and Autoscaling](#monitoring-and-autoscaling)
- [Example Deployments](#example-deployments)

## Overview

Orbit-RS is designed for various workload intensities, from small development environments to enterprise-scale deployments handling petabyte-scale data. Proper resource sizing is crucial for:

- **Performance**: Optimal CPU/memory allocation for workload characteristics
- **Cost Efficiency**: Avoiding over-provisioning while maintaining performance
- **Scalability**: Ensuring resources can scale with demand
- **Reliability**: Providing adequate resources for stability and fault tolerance

## Workload Classifications

### Development/Testing (Small)
- **Use Case**: Development, testing, small demos
- **Data Scale**: < 10GB
- **Concurrent Users**: 1-10
- **Expected Throughput**: < 1,000 ops/sec

### Production/Staging (Medium)
- **Use Case**: Production workloads, staging environments
- **Data Scale**: 10GB - 1TB
- **Concurrent Users**: 10-100
- **Expected Throughput**: 1,000 - 50,000 ops/sec

### Enterprise (Large)
- **Use Case**: Enterprise production, high-availability
- **Data Scale**: 1TB - 100TB
- **Concurrent Users**: 100-1,000
- **Expected Throughput**: 50,000 - 500,000 ops/sec

### Hyperscale (XLarge)
- **Use Case**: Hyperscale deployments, petabyte-scale
- **Data Scale**: > 100TB
- **Concurrent Users**: > 1,000
- **Expected Throughput**: > 500,000 ops/sec

## Component-Specific Sizing

### Orbit-Server (Core Database Engine)

#### Small Workload
```yaml
resources:
  requests:
    cpu: "500m"
    memory: "1Gi"
    ephemeral-storage: "10Gi"
  limits:
    cpu: "2"
    memory: "4Gi"
    ephemeral-storage: "50Gi"

# JVM-style memory allocation for Rust
env:
  - name: RUST_MIN_STACK
    value: "8388608"  # 8MB stack
  - name: ORBIT_CACHE_SIZE
    value: "512MB"
  - name: ORBIT_WAL_BUFFER_SIZE
    value: "64MB"
```

#### Medium Workload
```yaml
resources:
  requests:
    cpu: "2"
    memory: "8Gi"
    ephemeral-storage: "50Gi"
  limits:
    cpu: "8"
    memory: "32Gi"
    ephemeral-storage: "200Gi"

env:
  - name: ORBIT_CACHE_SIZE
    value: "4GB"
  - name: ORBIT_WAL_BUFFER_SIZE
    value: "256MB"
  - name: ORBIT_QUERY_CACHE_SIZE
    value: "1GB"
  - name: ORBIT_MAX_CONNECTIONS
    value: "1000"
```

#### Large Workload
```yaml
resources:
  requests:
    cpu: "8"
    memory: "32Gi"
    ephemeral-storage: "200Gi"
  limits:
    cpu: "32"
    memory: "128Gi"
    ephemeral-storage: "1Ti"

env:
  - name: ORBIT_CACHE_SIZE
    value: "16GB"
  - name: ORBIT_WAL_BUFFER_SIZE
    value: "1GB"
  - name: ORBIT_QUERY_CACHE_SIZE
    value: "4GB"
  - name: ORBIT_MAX_CONNECTIONS
    value: "5000"
  - name: ORBIT_BACKGROUND_THREADS
    value: "16"
```

#### XLarge Workload (Hyperscale)
```yaml
resources:
  requests:
    cpu: "32"
    memory: "128Gi"
    ephemeral-storage: "1Ti"
  limits:
    cpu: "64"
    memory: "512Gi"
    ephemeral-storage: "5Ti"

env:
  - name: ORBIT_CACHE_SIZE
    value: "64GB"
  - name: ORBIT_WAL_BUFFER_SIZE
    value: "4GB"
  - name: ORBIT_QUERY_CACHE_SIZE
    value: "16GB"
  - name: ORBIT_MAX_CONNECTIONS
    value: "20000"
  - name: ORBIT_BACKGROUND_THREADS
    value: "32"
  - name: ORBIT_NUMA_AWARE
    value: "true"
```

### Orbit-Compute (GPU/Acceleration Engine)

#### Small Workload
```yaml
resources:
  requests:
    cpu: "1"
    memory: "2Gi"
  limits:
    cpu: "4"
    memory: "8Gi"

# Optional GPU support
# Uncomment for GPU-enabled nodes
# nvidia.com/gpu: 1
# amd.com/gpu: 1
```

#### Medium Workload
```yaml
resources:
  requests:
    cpu: "4"
    memory: "8Gi"
  limits:
    cpu: "16"
    memory: "32Gi"
    nvidia.com/gpu: 1  # NVIDIA GPU
    # amd.com/gpu: 1   # Alternative: AMD GPU

env:
  - name: ORBIT_GPU_MEMORY_FRACTION
    value: "0.8"  # Use 80% of GPU memory
  - name: ORBIT_CPU_GPU_RATIO
    value: "4:1"  # 4 CPU cores per GPU
```

#### Large Workload
```yaml
resources:
  requests:
    cpu: "8"
    memory: "32Gi"
  limits:
    cpu: "32"
    memory: "128Gi"
    nvidia.com/gpu: 2  # Multiple GPUs
    
env:
  - name: ORBIT_GPU_MEMORY_FRACTION
    value: "0.9"
  - name: ORBIT_MULTI_GPU_STRATEGY
    value: "data_parallel"
  - name: ORBIT_CUDA_STREAMS
    value: "8"
```

#### XLarge Workload (GPU Cluster)
```yaml
resources:
  requests:
    cpu: "32"
    memory: "128Gi"
  limits:
    cpu: "64"
    memory: "256Gi"
    nvidia.com/gpu: 4  # 4 GPUs per pod
    
# Node affinity for GPU nodes
nodeSelector:
  accelerator: nvidia-tesla-v100
  # accelerator: nvidia-a100-40gb
  # accelerator: amd-mi250x

tolerations:
- key: nvidia.com/gpu
  operator: Exists
  effect: NoSchedule

env:
  - name: ORBIT_GPU_MEMORY_FRACTION
    value: "0.95"
  - name: ORBIT_MULTI_GPU_STRATEGY
    value: "model_parallel"
  - name: ORBIT_GPU_PEER_ACCESS
    value: "true"
```

### Orbit-Client (Connection Pool)

#### Small Workload
```yaml
resources:
  requests:
    cpu: "100m"
    memory: "256Mi"
  limits:
    cpu: "500m"
    memory: "1Gi"

env:
  - name: ORBIT_MAX_POOL_SIZE
    value: "50"
  - name: ORBIT_MIN_POOL_SIZE
    value: "5"
```

#### Medium Workload
```yaml
resources:
  requests:
    cpu: "500m"
    memory: "1Gi"
  limits:
    cpu: "2"
    memory: "4Gi"

env:
  - name: ORBIT_MAX_POOL_SIZE
    value: "500"
  - name: ORBIT_MIN_POOL_SIZE
    value: "50"
  - name: ORBIT_KEEPALIVE_TIMEOUT
    value: "30s"
```

#### Large Workload
```yaml
resources:
  requests:
    cpu: "2"
    memory: "4Gi"
  limits:
    cpu: "8"
    memory: "16Gi"

env:
  - name: ORBIT_MAX_POOL_SIZE
    value: "2000"
  - name: ORBIT_MIN_POOL_SIZE
    value: "200"
  - name: ORBIT_CIRCUIT_BREAKER_ENABLED
    value: "true"
```

### Orbit-Operator (Kubernetes Operator)

#### All Workloads (Control Plane)
```yaml
resources:
  requests:
    cpu: "100m"
    memory: "256Mi"
  limits:
    cpu: "1"
    memory: "2Gi"

env:
  - name: ORBIT_RECONCILE_INTERVAL
    value: "30s"
  - name: ORBIT_LEADER_ELECTION
    value: "true"
```

## Storage and Persistent Volume Configurations

### Development Storage
```yaml

# Basic PVC for development
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: orbit-data-small
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: standard  # Use cluster default
  resources:
    requests:
      storage: 50Gi
```

### Production Storage with Fast NVMe
```yaml

# High-performance storage for production
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: orbit-data-nvme
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: fast-nvme  # NVMe-based storage class
  resources:
    requests:
      storage: 1Ti
      
---

# Memory-mapped file storage (tmpfs/RAM disk)
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: orbit-mmap-cache
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: memory  # RAM-based storage
  resources:
    requests:
      storage: 32Gi  # Large memory-mapped cache
```

### Enterprise Storage with Replication
```yaml

# Replicated storage for enterprise
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: orbit-data-replicated
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: fast-replicated  # Replicated NVMe
  resources:
    requests:
      storage: 10Ti

---

# Backup storage (slower, cheaper)
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: orbit-backup
spec:
  accessModes:
    - ReadWriteMany
  storageClassName: backup-storage
  resources:
    requests:
      storage: 50Ti
```

## GPU and Acceleration Configurations

### NVIDIA GPU Node Pool
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: gpu-node-config
data:
  # GPU node configuration
  gpu-driver-version: "535.129.03"
  cuda-version: "12.2"
  nvidia-docker-runtime: "true"
  
---

# GPU-optimized storage class
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: gpu-optimized-nvme
provisioner: kubernetes.io/aws-ebs  # or your cloud provider
parameters:
  type: io2  # High IOPS for GPU workloads
  iopsPerGB: "50"
  fsType: ext4
allowVolumeExpansion: true
volumeBindingMode: WaitForFirstConsumer
```

### AMD GPU Support
```yaml

# AMD ROCm configuration
apiVersion: v1
kind: ConfigMap
metadata:
  name: rocm-config
data:
  rocm-version: "5.7.0"
  hip-version: "5.7.0"
  rocblas-version: "2.45.0"
  
---

# ROCm device plugin daemonset
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: amd-gpu-device-plugin
spec:
  selector:
    matchLabels:
      name: amd-gpu-device-plugin
  template:
    metadata:
      labels:
        name: amd-gpu-device-plugin
    spec:
      nodeSelector:
        accelerator: amd-gpu
      containers:
      - name: amd-gpu-device-plugin
        image: rocm/k8s-device-plugin:latest
        securityContext:
          privileged: true
```

## Memory-Mapped File Storage

### Fast Storage Configuration
```yaml

# Storage class for memory-mapped files
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: mmap-optimized
provisioner: kubernetes.io/aws-ebs
parameters:
  type: io2
  iopsPerGB: "100"  # Very high IOPS
  encrypted: "true"
  fsType: ext4
mountOptions:
  - noatime  # Disable access time updates
  - data=writeback  # Optimize for throughput
allowVolumeExpansion: true

---

# Pod configuration with memory-mapped storage
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit-server-mmap
spec:
  template:
    spec:
      containers:
      - name: orbit-server
        env:
        - name: ORBIT_MMAP_PATH
          value: "/mmap-cache"
        - name: ORBIT_MMAP_SIZE
          value: "64GB"
        - name: ORBIT_MMAP_HUGEPAGES
          value: "true"
        volumeMounts:
        - name: mmap-cache
          mountPath: /mmap-cache
        - name: hugepages-2mi
          mountPath: /hugepages-2Mi
      volumes:
      - name: hugepages-2mi
        emptyDir:
          medium: HugePages-2Mi
  volumeClaimTemplates:
  - metadata:
      name: mmap-cache
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: mmap-optimized
      resources:
        requests:
          storage: 100Gi
```

### RAM Disk Configuration
```yaml

# RAM disk for ultra-fast memory mapping
apiVersion: v1
kind: Pod
metadata:
  name: orbit-server-ramdisk
spec:
  containers:
  - name: orbit-server
    resources:
      requests:
        memory: "64Gi"  # Ensure enough RAM
      limits:
        memory: "128Gi"
        hugepages-2Mi: "32Gi"  # Huge pages for performance
    volumeMounts:
    - name: ramdisk
      mountPath: /ramdisk
    - name: hugepages
      mountPath: /hugepages
    env:
    - name: ORBIT_RAMDISK_PATH
      value: "/ramdisk"
    - name: ORBIT_USE_HUGEPAGES
      value: "true"
  volumes:
  - name: ramdisk
    emptyDir:
      medium: Memory
      sizeLimit: "32Gi"
  - name: hugepages
    emptyDir:
      medium: HugePages-2Mi
```

## Monitoring and Autoscaling

### Horizontal Pod Autoscaler (HPA)
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: orbit-server-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: StatefulSet
    name: orbit-server
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  # Custom metrics for GPU utilization
  - type: Pods
    pods:
      metric:
        name: gpu_utilization
      target:
        type: AverageValue
        averageValue: "70"

---

# Vertical Pod Autoscaler (VPA)
apiVersion: autoscaling.k8s.io/v1
kind: VerticalPodAutoscaler
metadata:
  name: orbit-server-vpa
spec:
  targetRef:
    apiVersion: apps/v1
    kind: StatefulSet
    name: orbit-server
  updatePolicy:
    updateMode: "Auto"  # Automatically apply recommendations
  resourcePolicy:
    containerPolicies:
    - containerName: orbit-server
      maxAllowed:
        cpu: "64"
        memory: "512Gi"
      minAllowed:
        cpu: "500m"
        memory: "1Gi"
      controlledResources: ["cpu", "memory"]
```

### GPU Node Autoscaling
```yaml

# Cluster Autoscaler configuration for GPU nodes
apiVersion: v1
kind: ConfigMap
metadata:
  name: cluster-autoscaler-status
  namespace: kube-system
data:
  gpu-node-group.min: "0"
  gpu-node-group.max: "10"
  gpu-node-group.scale-down-delay: "10m"
  gpu-node-group.scale-down-utilization: "0.5"

---

# GPU-specific node affinity and taints
apiVersion: v1
kind: Node
metadata:
  name: gpu-node-template
  labels:
    accelerator: nvidia-tesla-v100
    instance-type: p3.2xlarge
    orbit.rs/gpu-enabled: "true"
spec:
  taints:
  - key: nvidia.com/gpu
    value: "true"
    effect: NoSchedule
  - key: orbit.rs/gpu-workload
    value: "true"
    effect: NoSchedule
```

## Example Deployments

### Small Development Environment
```bash

# Deploy development environment
kubectl apply -f - <<EOF
apiVersion: v1
kind: Namespace
metadata:
  name: orbit-dev

---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit-server
  namespace: orbit-dev
spec:
  replicas: 1
  serviceName: orbit-server
  selector:
    matchLabels:
      app: orbit-server
  template:
    metadata:
      labels:
        app: orbit-server
    spec:
      containers:
      - name: orbit-server
        image: ghcr.io/turingworks/orbit-rs/orbit-server:latest-release
        ports:
        - containerPort: 8080
        resources:
          requests:
            cpu: "500m"
            memory: "1Gi"
          limits:
            cpu: "2"
            memory: "4Gi"
        env:
        - name: ORBIT_CACHE_SIZE
          value: "512MB"
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: orbit-data
          mountPath: /data
  volumeClaimTemplates:
  - metadata:
      name: orbit-data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 50Gi
EOF
```

### Production Environment with GPU
```bash

# Deploy production environment with GPU acceleration
kubectl apply -f - <<EOF
apiVersion: v1
kind: Namespace
metadata:
  name: orbit-prod
  labels:
    orbit.rs/environment: production

---

# Orbit Server with high availability
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit-server
  namespace: orbit-prod
spec:
  replicas: 3
  serviceName: orbit-server
  selector:
    matchLabels:
      app: orbit-server
  template:
    metadata:
      labels:
        app: orbit-server
    spec:
      affinity:
        podAntiAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
          - labelSelector:
              matchLabels:
                app: orbit-server
            topologyKey: kubernetes.io/hostname
      containers:
      - name: orbit-server
        image: ghcr.io/turingworks/orbit-rs/orbit-server:latest-release
        ports:
        - containerPort: 8080
        resources:
          requests:
            cpu: "8"
            memory: "32Gi"
          limits:
            cpu: "32"
            memory: "128Gi"
        env:
        - name: ORBIT_CACHE_SIZE
          value: "16GB"
        - name: ORBIT_WAL_BUFFER_SIZE
          value: "1GB"
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: orbit-data
          mountPath: /data
        - name: orbit-mmap
          mountPath: /mmap-cache
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
  volumeClaimTemplates:
  - metadata:
      name: orbit-data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: fast-nvme
      resources:
        requests:
          storage: 1Ti
  - metadata:
      name: orbit-mmap
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: mmap-optimized
      resources:
        requests:
          storage: 100Gi

---

# Orbit Compute with GPU
apiVersion: apps/v1
kind: Deployment
metadata:
  name: orbit-compute
  namespace: orbit-prod
spec:
  replicas: 2
  selector:
    matchLabels:
      app: orbit-compute
  template:
    metadata:
      labels:
        app: orbit-compute
    spec:
      nodeSelector:
        accelerator: nvidia-tesla-v100
      tolerations:
      - key: nvidia.com/gpu
        operator: Exists
        effect: NoSchedule
      containers:
      - name: orbit-compute
        image: ghcr.io/turingworks/orbit-rs/orbit-compute:latest-release-gpu
        resources:
          requests:
            cpu: "8"
            memory: "32Gi"
          limits:
            cpu: "32"
            memory: "128Gi"
            nvidia.com/gpu: 2
        env:
        - name: ORBIT_GPU_MEMORY_FRACTION
          value: "0.9"
        - name: ORBIT_MULTI_GPU_STRATEGY
          value: "data_parallel"
        - name: RUST_LOG
          value: "info"
EOF
```

### Enterprise Hyperscale Deployment
```bash

# Deploy enterprise hyperscale environment
kubectl apply -f - <<EOF
apiVersion: v1
kind: Namespace
metadata:
  name: orbit-enterprise
  labels:
    orbit.rs/environment: enterprise
    orbit.rs/scale: hyperscale

---

# High-availability Orbit Server cluster
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit-server
  namespace: orbit-enterprise
spec:
  replicas: 9  # 3 regions × 3 replicas
  serviceName: orbit-server
  selector:
    matchLabels:
      app: orbit-server
  template:
    metadata:
      labels:
        app: orbit-server
    spec:
      topologySpreadConstraints:
      - maxSkew: 1
        topologyKey: topology.kubernetes.io/zone
        whenUnsatisfiable: DoNotSchedule
        labelSelector:
          matchLabels:
            app: orbit-server
      - maxSkew: 1
        topologyKey: kubernetes.io/hostname
        whenUnsatisfiable: DoNotSchedule
        labelSelector:
          matchLabels:
            app: orbit-server
      nodeSelector:
        orbit.rs/node-class: compute-optimized
      containers:
      - name: orbit-server
        image: ghcr.io/turingworks/orbit-rs/orbit-server:latest-release
        ports:
        - containerPort: 8080
        resources:
          requests:
            cpu: "32"
            memory: "128Gi"
          limits:
            cpu: "64"
            memory: "512Gi"
            hugepages-2Mi: "64Gi"
        env:
        - name: ORBIT_CACHE_SIZE
          value: "64GB"
        - name: ORBIT_WAL_BUFFER_SIZE
          value: "4GB"
        - name: ORBIT_QUERY_CACHE_SIZE
          value: "16GB"
        - name: ORBIT_MAX_CONNECTIONS
          value: "20000"
        - name: ORBIT_NUMA_AWARE
          value: "true"
        - name: ORBIT_USE_HUGEPAGES
          value: "true"
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: orbit-data
          mountPath: /data
        - name: orbit-mmap
          mountPath: /mmap-cache
        - name: hugepages
          mountPath: /hugepages
      volumes:
      - name: hugepages
        emptyDir:
          medium: HugePages-2Mi
  volumeClaimTemplates:
  - metadata:
      name: orbit-data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: enterprise-nvme
      resources:
        requests:
          storage: 10Ti
  - metadata:
      name: orbit-mmap
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: mmap-optimized
      resources:
        requests:
          storage: 1Ti

---

# GPU compute cluster
apiVersion: apps/v1
kind: Deployment
metadata:
  name: orbit-compute-gpu
  namespace: orbit-enterprise
spec:
  replicas: 6
  selector:
    matchLabels:
      app: orbit-compute-gpu
  template:
    metadata:
      labels:
        app: orbit-compute-gpu
    spec:
      nodeSelector:
        accelerator: nvidia-a100-40gb
      tolerations:
      - key: nvidia.com/gpu
        operator: Exists
        effect: NoSchedule
      containers:
      - name: orbit-compute
        image: ghcr.io/turingworks/orbit-rs/orbit-compute:latest-release-gpu
        resources:
          requests:
            cpu: "32"
            memory: "128Gi"
          limits:
            cpu: "64"
            memory: "256Gi"
            nvidia.com/gpu: 4
        env:
        - name: ORBIT_GPU_MEMORY_FRACTION
          value: "0.95"
        - name: ORBIT_MULTI_GPU_STRATEGY
          value: "model_parallel"
        - name: ORBIT_GPU_PEER_ACCESS
          value: "true"
        - name: NCCL_DEBUG
          value: "INFO"
EOF
```

## Performance Tuning Tips

1. **CPU Affinity**: Use CPU pinning for consistent performance
2. **NUMA Awareness**: Configure NUMA topology for multi-socket systems
3. **Huge Pages**: Enable huge pages for large memory workloads
4. **Network Optimization**: Use SR-IOV for high-performance networking
5. **Storage Optimization**: Use NVMe with appropriate filesystems (ext4, xfs)
6. **GPU Memory**: Configure GPU memory fractions based on workload
7. **Monitoring**: Implement comprehensive monitoring for all resources

This guide provides a foundation for deploying Orbit-RS at any scale. Adjust the configurations based on your specific workload characteristics and performance requirements.