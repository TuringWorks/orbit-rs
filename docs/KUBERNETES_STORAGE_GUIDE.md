---
layout: default
title: Kubernetes Storage Guide for Orbit-RS
category: documentation
---

## Kubernetes Storage Guide for Orbit-RS

## 🎯 **TL;DR - What You Need to Know**

**✅ Current Status**: Basic local storage (PVC) works  
**✅ Enhanced**: Now supports all backends (Memory, LSM-Tree, RocksDB, S3, Azure, GCP)  
**✅ Provided**: Enhanced manifests, Helm chart guidance, and updated Operator CRDs  

## 📋 **Storage Backend Overview**

### **Local Storage Backends** (Require PersistentVolumes)

- **Memory** (with disk backup)
- **Copy-on-Write B+ Tree**
- **LSM-Tree**
- **RocksDB**

**Kubernetes Requirements**: StatefulSet + PVC + SSD StorageClass

### **Cloud Storage Backends** (No PersistentVolumes)

- **S3** (AWS, MinIO)
- **Azure Blob Storage**
- **Google Cloud Storage**

**Kubernetes Requirements**: Deployment + Secrets (no PVC needed)

## 🔧 **Implementation Changes Made**

### 1. **Enhanced Configuration**

**File**: [`k8s/01-configmap-enhanced.yaml`](file:///Users/ravindraboddipalli/sources/orbit-rs/k8s/01-configmap-enhanced.yaml)

**Key Features**:

- ✅ Environment variable-driven backend selection
- ✅ Configuration sections for all persistence backends
- ✅ Intelligent entrypoint script that creates directories based on backend
- ✅ Health checks that vary by backend type

**Example Environment Variables**:

```bash

# Backend Selection
ORBIT_PERSISTENCE_BACKEND=lsm_tree

# LSM-Tree Configuration
ORBIT_LSM_DATA_DIR=/app/data/lsm_tree
ORBIT_LSM_MEMTABLE_SIZE=67108864
ORBIT_LSM_ENABLE_COMPACTION=true

# S3 Configuration (from secrets)
ORBIT_S3_ENDPOINT=https://s3.amazonaws.com
ORBIT_S3_BUCKET=orbit-production-state
ORBIT_S3_ACCESS_KEY_ID=<from-secret>
```

### 2. **Enhanced StatefulSet**

**File**: [`k8s/03-statefulset-enhanced.yaml`](file:///Users/ravindraboddipalli/sources/orbit-rs/k8s/03-statefulset-enhanced.yaml)

**Key Features**:

- ✅ All persistence backend environment variables pre-configured
- ✅ Secrets integration for cloud backends
- ✅ Flexible volume mounts
- ✅ Alternative Deployment for cloud-only backends

### 3. **Enhanced Kubernetes Operator**

**File**: [`orbit-operator/src/crd.rs`](file:///Users/ravindraboddipalli/sources/orbit-rs/orbit-operator/src/crd.rs)

**Key Features**:

- ✅ PersistenceConfig added to OrbitCluster CRD
- ✅ Support for memory, local, and cloud storage configurations
- ✅ Type-safe configuration with defaults

## 🚀 **Deployment Examples**

### **Option 1: Raw Kubernetes Manifests**

#### **LSM-Tree Backend** (High-performance local storage)

```bash

# 1. Apply base manifests
kubectl apply -f k8s/00-namespace.yaml
kubectl apply -f k8s/02-storage.yaml  # Creates SSD StorageClass

# 2. Apply enhanced configuration
kubectl apply -f k8s/01-configmap-enhanced.yaml

# 3. Modify StatefulSet for LSM-Tree
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit-server
  namespace: orbit-rs

# ... (copy from 03-statefulset-enhanced.yaml)
spec:
  template:
    spec:
      containers:
      - name: orbit-server
        env:
        - name: ORBIT_PERSISTENCE_BACKEND
          value: "lsm_tree"
        - name: ORBIT_LSM_MEMTABLE_SIZE
          value: "134217728"  # 128MB for high throughput
        - name: ORBIT_LSM_MAX_MEMTABLES
          value: "16"
        # ... other LSM-Tree configs
EOF
```

#### **S3 Backend** (Cloud storage)

```bash

# 1. Create S3 credentials secret
kubectl create secret generic orbit-server-secrets -n orbit-rs \
  --from-literal=s3-access-key-id="AKIA..." \
  --from-literal=s3-secret-access-key="xyz..."

# 2. Use Deployment instead of StatefulSet (no PVC needed)
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: Deployment  # Note: Deployment, not StatefulSet
metadata:
  name: orbit-server-s3
  namespace: orbit-rs
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: orbit-server
        env:
        - name: ORBIT_PERSISTENCE_BACKEND
          value: "s3"
        - name: ORBIT_S3_ENDPOINT
          value: "https://s3.amazonaws.com"
        - name: ORBIT_S3_REGION
          value: "us-west-2"
        - name: ORBIT_S3_BUCKET
          value: "orbit-production-state"
        # Secrets mounted automatically from enhanced config
EOF
```

### **Option 2: Helm Chart**

#### **RocksDB Backend** (Production database)

```yaml

# values-rocksdb.yaml
orbitServer:
  replicaCount: 5  # Scale up for production
  env:
    - name: ORBIT_PERSISTENCE_BACKEND
      value: "rocksdb"
    - name: ORBIT_ROCKSDB_WRITE_BUFFER_SIZE
      value: "268435456"  # 256MB
    - name: ORBIT_ROCKSDB_BLOCK_CACHE_SIZE
      value: "536870912"  # 512MB
    - name: ORBIT_ROCKSDB_MAX_BACKGROUND_JOBS
      value: "8"
  
  resources:
    requests:
      memory: "1Gi"
      cpu: "500m"
    limits:
      memory: "4Gi"
      cpu: "2000m"

persistence:
  enabled: true
  storageClass: "gp3"
  size: 100Gi

# Deploy
helm install orbit-rs ./helm/orbit-rs -f values-rocksdb.yaml
```

#### **Azure Backend** (Cloud storage)

```yaml

# values-azure.yaml
orbitServer:
  env:
    - name: ORBIT_PERSISTENCE_BACKEND
      value: "azure"
    - name: ORBIT_AZURE_ACCOUNT_NAME
      value: "myorbitaccount"
    - name: ORBIT_AZURE_CONTAINER_NAME
      value: "orbit-data"

persistence:
  enabled: false  # No PVC needed for cloud

# Create Azure secret first
kubectl create secret generic orbit-server-secrets -n orbit-rs \
  --from-literal=azure-account-key="abcd1234..."

helm install orbit-rs ./helm/orbit-rs -f values-azure.yaml
```

### **Option 3: Kubernetes Operator**

#### **LSM-Tree with High Performance**

```yaml
apiVersion: orbit.turingworks.com/v1
kind: OrbitCluster
metadata:
  name: production-cluster
  namespace: orbit-rs
spec:
  replicas: 5
  image:
    repository: orbit-rs/orbit-server
    tag: "v1.0.0"
  
  # Persistence configuration
  persistence:
    backend: "lsm_tree"
    local:
      backend_type: "lsm_tree"
      data_dir: "/app/data/lsm_tree"
      enable_compression: true
      write_buffer_size: 268435456   # 256MB
      cache_size: 536870912          # 512MB
      config:
        memtable_size_limit: "134217728"    # 128MB
        max_memtables: "16"
        compaction_threshold: "8"
        bloom_filter_fp_rate: "0.001"
  
  resources:
    cpuRequest: "1000m"
    memoryRequest: "2Gi"
    cpuLimit: "4000m"
    memoryLimit: "8Gi"
  
  storage:
    storageClass: "premium-ssd"
    size: "200Gi"
    accessMode: "ReadWriteOnce"
```

#### **S3 with Multi-Region**

```yaml
apiVersion: orbit.turingworks.com/v1
kind: OrbitCluster
metadata:
  name: s3-cluster
  namespace: orbit-rs
spec:
  replicas: 3
  
  persistence:
    backend: "s3"
    cloud:
      provider: "s3"
      endpoint: "https://s3.amazonaws.com"
      region: "us-west-2"
      bucket: "orbit-global-state"
      prefix: "production"
      connection_timeout: 30
      retry_count: 5
      enable_ssl: true
      credentials_secret: "orbit-s3-credentials"
  
  # No storage section needed for cloud backends
```

## 📊 **Storage Requirements by Backend**

| Backend | Kubernetes Resource | Volume Type | Size Recommendation | IOPS Requirement |
|---------|-------------------|-------------|-------------------|------------------|
| **Memory** | StatefulSet | Small PVC (backup) | 1-10Gi | Low |
| **COW B+Tree** | StatefulSet | SSD PVC | 10-100Gi | Medium |
| **LSM-Tree** | StatefulSet | High-IOPS SSD | 50-500Gi | High |
| **RocksDB** | StatefulSet | Premium SSD | 100-1000Gi | Very High |
| **S3** | Deployment | None | N/A | N/A |
| **Azure** | Deployment | None | N/A | N/A |
| **GCP** | Deployment | None | N/A | N/A |

## 🛡️ **Security Considerations**

### **Secrets Management**

**For Cloud Backends**, create secrets:

```bash

# S3
kubectl create secret generic orbit-server-secrets \
  --from-literal=s3-access-key-id="AKIA..." \
  --from-literal=s3-secret-access-key="xyz..."

# Azure
kubectl create secret generic orbit-server-secrets \
  --from-literal=azure-account-key="abcd..."

# GCP
kubectl create secret generic orbit-server-secrets \
  --from-file=gcp-service-account-key=./service-account.json
```

### **RBAC Requirements**

The Orbit-RS pods need these permissions:

```yaml

# Already included in orbit-server ServiceAccount
rules:
- apiGroups: [""]
  resources: ["pods", "endpoints"]
  verbs: ["get", "list", "watch"]
- apiGroups: ["coordination.k8s.io"]
  resources: ["leases"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
```

## 🔍 **Troubleshooting**

### **Common Issues**

1. **PVC not binding**

   ```bash
   kubectl get pv,pvc -n orbit-rs
   # Check StorageClass availability
   kubectl get storageclass
   ```

2. **Backend not starting**

   ```bash
   # Check logs
   kubectl logs -n orbit-rs orbit-server-0
   # Look for: "Starting Orbit-RS server with persistence backend: xxx"
   ```

3. **Cloud authentication failures**

   ```bash
   # Verify secrets exist
   kubectl get secrets -n orbit-rs orbit-server-secrets -o yaml
   # Check secret keys match expected names
   ```

### **Health Check Endpoints**

The enhanced health checks provide backend-specific information:

```bash

# Basic health
curl http://pod-ip:8080/health

# Detailed health (includes persistence backend status)
curl http://pod-ip:8080/health/ready
```

## 📈 **Performance Tuning**

### **Local Storage Backends**

**LSM-Tree**: High write throughput

```yaml
env:
- name: ORBIT_LSM_MEMTABLE_SIZE
  value: "134217728"  # Larger memtables = fewer flushes
- name: ORBIT_LSM_MAX_MEMTABLES  
  value: "16"         # More memtables = better write buffering
- name: ORBIT_LSM_COMPACTION_THRESHOLD
  value: "8"          # More aggressive compaction
```

**RocksDB**: Balanced read/write performance

```yaml
env:
- name: ORBIT_ROCKSDB_WRITE_BUFFER_SIZE
  value: "268435456"  # 256MB write buffers
- name: ORBIT_ROCKSDB_BLOCK_CACHE_SIZE  
  value: "1073741824" # 1GB block cache for reads
- name: ORBIT_ROCKSDB_MAX_BACKGROUND_JOBS
  value: "8"          # More compaction threads
```

### **Cloud Storage Backends**

```yaml
env:
- name: ORBIT_S3_CONNECTION_TIMEOUT
  value: "60"         # Longer timeout for large objects
- name: ORBIT_S3_RETRY_COUNT
  value: "5"          # More retries for reliability
```

### **Resource Allocation**

```yaml
resources:
  requests:
    memory: "2Gi"     # Minimum for RocksDB/LSM-Tree
    cpu: "1000m"      # Minimum for compaction
  limits:
    memory: "8Gi"     # Allow burst for large operations
    cpu: "4000m"      # Allow burst for compaction
```

## 📚 **Files Reference**

**Enhanced Kubernetes Manifests**:

- [`k8s/01-configmap-enhanced.yaml`](file:///Users/ravindraboddipalli/sources/orbit-rs/k8s/01-configmap-enhanced.yaml) - Configuration with all backends
- [`k8s/03-statefulset-enhanced.yaml`](file:///Users/ravindraboddipalli/sources/orbit-rs/k8s/03-statefulset-enhanced.yaml) - StatefulSet + Deployment variants

**Operator Updates**:

- [`orbit-operator/src/crd.rs`](file:///Users/ravindraboddipalli/sources/orbit-rs/orbit-operator/src/crd.rs) - Enhanced CRD with persistence config

**Documentation**:

- [`docs/KUBERNETES_PERSISTENCE.md`](KUBERNETES_PERSISTENCE.md) - Quick setup guide
- [`docs/VIRTUAL_ACTOR_PERSISTENCE.md`](VIRTUAL_ACTOR_PERSISTENCE.md) - Complete persistence architecture
- [`docs/STORAGE_BACKEND_INDEPENDENCE.md`](STORAGE_BACKEND_INDEPENDENCE.md) - Backend independence explanation

## ✅ **Next Steps**

1. **Test the enhanced manifests** with your preferred backend
2. **Update Helm chart templates** to incorporate the environment variables
3. **Implement operator logic** to translate CRD persistence config to pod specs
4. **Add monitoring** for persistence metrics via Prometheus

The foundation is now in place to support all Orbit-RS persistence backends on Kubernetes! 🚀
