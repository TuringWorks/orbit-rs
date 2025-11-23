---
layout: default
title: Kubernetes Complete Documentation
category: documentation
---

# Kubernetes Complete Documentation

**Comprehensive Kubernetes Deployment Guide for Orbit-RS**

## Table of Contents

1. [Overview](#overview)
2. [Storage Guide](#storage-guide)
3. [Deployment Guide](#deployment-guide)
4. [Persistence Configuration](#persistence-configuration)
5. [Configuration](#configuration)
6. [Monitoring](#monitoring)
7. [High Availability](#high-availability)
8. [Security](#security)
9. [Troubleshooting](#troubleshooting)
10. [Maintenance](#maintenance)

---

## Overview

This guide provides comprehensive instructions for deploying Orbit-RS on Kubernetes in production environments, including storage backend configuration, persistence setup, and operational best practices.

### Key Features

- **Multiple Storage Backends**: Support for Memory, LSM-Tree, RocksDB, S3, Azure, GCP, and more
- **Flexible Deployment**: StatefulSet for local storage, Deployment for cloud storage
- **Production Ready**: Complete monitoring, security, and high availability configurations
- **Cloud Native**: Full Kubernetes integration with Helm charts and operators

---

## Storage Guide

### Storage Backend Overview

#### Local Storage Backends (Require PersistentVolumes)

- **Memory** (with disk backup)
- **Copy-on-Write B+ Tree**
- **LSM-Tree**
- **RocksDB**

**Kubernetes Requirements**: StatefulSet + PVC + SSD StorageClass

#### Cloud Storage Backends (No PersistentVolumes)

- **S3** (AWS, MinIO)
- **Azure Blob Storage**
- **Google Cloud Storage**

**Kubernetes Requirements**: Deployment + Secrets (no PVC needed)

### Storage Requirements by Backend

| Backend | Kubernetes Resource | Volume Type | Size Recommendation | IOPS Requirement |
|---------|-------------------|-------------|-------------------|------------------|
| **Memory** | StatefulSet | Small PVC (backup) | 1-10Gi | Low |
| **COW B+Tree** | StatefulSet | SSD PVC | 10-100Gi | Medium |
| **LSM-Tree** | StatefulSet | High-IOPS SSD | 50-500Gi | High |
| **RocksDB** | StatefulSet | Premium SSD | 100-1000Gi | Very High |
| **S3** | Deployment | None | N/A | N/A |
| **Azure** | Deployment | None | N/A | N/A |
| **GCP** | Deployment | None | N/A | N/A |

### Backend Selection

Set the persistence backend using one of these methods:

- Environment variable: `ORBIT_PERSISTENCE_BACKEND`
- In TOML config: `[server] persistence_backend = "lsm_tree"`

Supported values:
- `memory`, `cow_btree`, `lsm_tree`, `rocksdb`, `s3`, `azure`, `gcp`

---

## Deployment Guide

### Prerequisites

#### Kubernetes Cluster Requirements

- **Kubernetes version**: 1.24+
- **Minimum nodes**: 3 (for high availability)
- **Node resources**: 2 CPU cores, 4GB RAM per node minimum
- **Storage**: Persistent storage with ReadWriteOnce support
- **Network**: Pod-to-pod communication, LoadBalancer support (optional)

#### Required Tools

```bash
# Install kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
chmod +x kubectl && sudo mv kubectl /usr/local/bin/

# Install Helm (recommended)
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# Install Docker (for building images)
curl -fsSL https://get.docker.com -o get-docker.sh && sh get-docker.sh
```

### Quick Start

#### Using Helm (Recommended)

```bash
# Add Helm repository (once available)
helm repo add orbit-rs https://turingworks.github.io/orbit-rs
helm repo update

# Install Orbit-RS
helm install orbit-rs orbit-rs/orbit-rs \
  --namespace orbit-rs \
  --create-namespace \
  --set monitoring.serviceMonitor.enabled=true \
  --set autoscaling.enabled=true
```

#### Using Local Helm Chart

```bash
# Clone the repository
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs

# Build Docker image
docker build -t orbit-rs/orbit-server:latest .

# Deploy using local Helm chart
helm install orbit-rs helm/orbit-rs \
  --namespace orbit-rs \
  --create-namespace \
  --values helm/orbit-rs/values.yaml
```

### Architecture Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                         Internet                            │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                 Load Balancer                               │
│            (Kubernetes Service)                             │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                   Ingress Controller                        │
│              (nginx/traefik/istio)                          │
└─────────────────────┬───────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        │             │             │
┌───────▼──────┬──────▼──────┬──────▼────--──┐
│ orbit-server │ orbit-server │ orbit-server │
│    Pod 1     │    Pod 2     │    Pod 3     │
│              │              │              │
│ ┌──────────┐ │ ┌──────────┐ │ ┌──────────┐ │
│ │   App    │ │ │   App    │ │ │   App    │ │
│ └──────────┘ │ └──────────┘ │ └──────────┘ │
│ ┌──────────┐ │ ┌──────────┐ │ ┌──────────┐ │
│ │ Storage  │ │ │ Storage  │ │ │ Storage  │ │
│ │ Backend  │ │ │ Backend  │ │ │ Backend  │ │
│ └──────────┘ │ └──────────┘ │ └──────────┘ │
└──────┬───────┴──────┬───────┴──────┬───────┘
       │              │              │
┌──────▼──────┬───────▼──────┬───────▼──────┐
│Persistent   │Persistent    │Persistent    │
│Volume 1     │Volume 2      │Volume 3      │
└─────────────┴──────────────┴──────────────┘

┌─────────────────────────────────────────────────────────────┐
│                    Monitoring Stack                         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│  │ Prometheus  │ │   Grafana   │ │  AlertMgr   │            │
│  └─────────────┘ └─────────────┘ └─────────────┘            │
└─────────────────────────────────────────────────────────────┘
```

### Manual Deployment

#### Step 1: Create Namespace and RBAC

```bash
kubectl apply -f k8s/00-namespace.yaml
```

#### Step 2: Configure Storage

Update the storage class in `k8s/02-storage.yaml` based on your cloud provider:

```yaml
# For AWS EKS
provisioner: ebs.csi.aws.com
parameters:
  type: gp3

# For Google GKE  
provisioner: pd.csi.storage.gke.io
parameters:
  type: pd-ssd

# For Azure AKS
provisioner: disk.csi.azure.com
parameters:
  storageaccounttype: Premium_LRS
```

```bash
kubectl apply -f k8s/02-storage.yaml
```

#### Step 3: Deploy Configuration

```bash
kubectl apply -f k8s/01-configmap-enhanced.yaml
```

#### Step 4: Deploy StatefulSet and Services

```bash
kubectl apply -f k8s/03-statefulset-enhanced.yaml
kubectl apply -f k8s/04-services.yaml
```

#### Step 5: Verify Deployment

```bash
# Check pods
kubectl get pods -n orbit-rs

# Check services
kubectl get services -n orbit-rs

# Check persistent volumes
kubectl get pvc -n orbit-rs

# Check logs
kubectl logs -n orbit-rs orbit-server-0
```

---

## Persistence Configuration

### LSM-Tree Backend (High-performance local storage)

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
EOF
```

### S3 Backend (Cloud storage)

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
EOF
```

### RocksDB Backend (Production database)

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

### Azure Backend (Cloud storage)

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

### Kubernetes Operator Configuration

#### LSM-Tree with High Performance

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

#### S3 with Multi-Region

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

---

## Configuration

### Environment-Specific Configurations

#### Development

```yaml
# dev-values.yaml
orbitServer:
  replicaCount: 1
  env:
    - name: ORBIT_PERSISTENCE_BACKEND
      value: "memory"
  resources:
    requests:
      memory: "256Mi"
      cpu: "100m"
    limits:
      memory: "1Gi"
      cpu: "500m"

persistence:
  size: 5Gi

config:
  logging:
    level: "debug"
```

#### Staging

```yaml
# staging-values.yaml
orbitServer:
  replicaCount: 2
  env:
    - name: ORBIT_PERSISTENCE_BACKEND
      value: "lsm_tree"
  resources:
    requests:
      memory: "512Mi"
      cpu: "250m"
    limits:
      memory: "2Gi"
      cpu: "1000m"

persistence:
  size: 10Gi

config:
  logging:
    level: "info"
```

#### Production

```yaml
# prod-values.yaml
orbitServer:
  replicaCount: 5
  env:
    - name: ORBIT_PERSISTENCE_BACKEND
      value: "rocksdb"
  resources:
    requests:
      memory: "1Gi"
      cpu: "500m"
    limits:
      memory: "4Gi"
      cpu: "2000m"

persistence:
  size: 50Gi
  
autoscaling:
  enabled: true
  minReplicas: 5
  maxReplicas: 20

monitoring:
  serviceMonitor:
    enabled: true
  prometheusRule:
    enabled: true

config:
  logging:
    level: "warn"
  transactions:
    maxConnections: 50
```

### Secrets Management

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

---

## Monitoring

### Prometheus Integration

Deploy Prometheus Operator:

```bash
helm install prometheus prometheus-community/kube-prometheus-stack \
  --namespace monitoring \
  --create-namespace \
  --set prometheus.prometheusSpec.serviceMonitorSelectorNilUsesHelmValues=false
```

Enable monitoring in Orbit-RS:

```yaml
monitoring:
  serviceMonitor:
    enabled: true
    interval: 30s
    additionalLabels:
      release: prometheus
  prometheusRule:
    enabled: true
    additionalLabels:
      release: prometheus
```

### Key Metrics to Monitor

- **Transaction Rate**: `rate(orbit_transactions_total[5m])`
- **Success Rate**: `rate(orbit_transactions_completed_total[5m]) / rate(orbit_transactions_total[5m])`
- **Active Transactions**: `orbit_transactions_active`
- **Memory Usage**: `container_memory_usage_bytes`
- **CPU Usage**: `rate(container_cpu_usage_seconds_total[5m])`
- **Storage Usage**: `kubelet_volume_stats_used_bytes`

### Alerting Rules

Key alerts configured in PrometheusRule:

- **OrbitServerDown**: Server instances unavailable
- **OrbitServerHighMemoryUsage**: Memory usage > 90%
- **OrbitServerHighCPUUsage**: CPU usage > 80%
- **OrbitTransactionFailureRate**: Failure rate > 10%
- **StorageFull**: Persistent volume usage > 85%

---

## High Availability

### Multi-Zone Deployment

```yaml
# Enable pod anti-affinity
affinity:
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
    - labelSelector:
        matchExpressions:
        - key: app.kubernetes.io/name
          operator: In
          values:
          - orbit-rs
      topologyKey: failure-domain.beta.kubernetes.io/zone

# Use zone-aware storage
persistence:
  storageClass: "zone-aware-ssd"
```

### Pod Disruption Budget

```yaml
podDisruptionBudget:
  enabled: true
  minAvailable: 2  # Always keep at least 2 replicas running
```

### Automated Recovery

```yaml
# Configure probe timeouts
livenessProbe:
  enabled: true
  initialDelaySeconds: 60
  periodSeconds: 30
  failureThreshold: 3

readinessProbe:
  enabled: true
  initialDelaySeconds: 30
  periodSeconds: 10
  failureThreshold: 3

# Configure rolling update strategy
strategy:
  type: RollingUpdate
  rollingUpdate:
    maxUnavailable: 1
    maxSurge: 1
```

---

## Security

### Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: orbit-rs-network-policy
  namespace: orbit-rs
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: orbit-rs
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    ports:
    - protocol: TCP
      port: 50051
  - from: []
    ports:
    - protocol: TCP
      port: 8080  # Health checks
    - protocol: TCP
      port: 9090  # Metrics
  egress:
  - {} # Allow all outbound (restrict as needed)
```

### Pod Security Standards

```yaml
# Enable restricted pod security
apiVersion: v1
kind: Namespace
metadata:
  name: orbit-rs
  labels:
    pod-security.kubernetes.io/enforce: restricted
    pod-security.kubernetes.io/audit: restricted
    pod-security.kubernetes.io/warn: restricted
```

### TLS Configuration

```yaml
# Enable TLS
config:
  tls:
    enabled: true
    certFile: "/app/certs/tls.crt"
    keyFile: "/app/certs/tls.key"

# Mount TLS certificates
extraVolumes:
- name: tls-certs
  secret:
    secretName: orbit-rs-tls
    
extraVolumeMounts:
- name: tls-certs
  mountPath: /app/certs
  readOnly: true
```

### RBAC Requirements

The Orbit-RS pods need these permissions:

```yaml
rules:
- apiGroups: [""]
  resources: ["pods", "endpoints"]
  verbs: ["get", "list", "watch"]
- apiGroups: ["coordination.k8s.io"]
  resources: ["leases"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
```

---

## Troubleshooting

### Common Issues

#### 1. PVC not binding

```bash
kubectl get pv,pvc -n orbit-rs
# Check StorageClass availability
kubectl get storageclass
```

#### 2. Backend not starting

```bash
# Check logs
kubectl logs -n orbit-rs orbit-server-0
# Look for: "Starting Orbit-RS server with persistence backend: xxx"
```

#### 3. Cloud authentication failures

```bash
# Verify secrets exist
kubectl get secrets -n orbit-rs orbit-server-secrets -o yaml
# Check secret keys match expected names
```

#### 4. Pods Not Starting

```bash
# Check pod status
kubectl describe pod orbit-server-0 -n orbit-rs

# Check logs
kubectl logs orbit-server-0 -n orbit-rs --previous

# Common causes:
# - Resource constraints
# - Image pull failures
# - Configuration errors
# - Storage issues
```

### Health Check Endpoints

The enhanced health checks provide backend-specific information:

```bash
# Basic health
curl http://pod-ip:8080/health

# Detailed health (includes persistence backend status)
curl http://pod-ip:8080/health/ready
```

---

## Maintenance

### Backup Procedures

```bash
# Backup persistent volumes
kubectl get pvc -n orbit-rs -o yaml > orbit-rs-pvc-backup.yaml

# Backup configuration
kubectl get configmap -n orbit-rs -o yaml > orbit-rs-config-backup.yaml
kubectl get secret -n orbit-rs -o yaml > orbit-rs-secrets-backup.yaml
```

### Updates and Upgrades

```bash
# Update image version
helm upgrade orbit-rs helm/orbit-rs \
  --set orbitServer.image.tag=v0.2.0 \
  --namespace orbit-rs

# Rollback if needed
helm rollback orbit-rs --namespace orbit-rs

# Zero-downtime update strategy
kubectl patch statefulset orbit-server -n orbit-rs -p '{"spec":{"updateStrategy":{"type":"RollingUpdate"}}}'
```

### Scaling Operations

```bash
# Manual scaling
kubectl scale statefulset orbit-server --replicas=5 -n orbit-rs

# Using Helm
helm upgrade orbit-rs helm/orbit-rs \
  --set orbitServer.replicaCount=5 \
  --namespace orbit-rs

# Enable autoscaling
helm upgrade orbit-rs helm/orbit-rs \
  --set autoscaling.enabled=true \
  --set autoscaling.minReplicas=3 \
  --set autoscaling.maxReplicas=10 \
  --namespace orbit-rs
```

### Performance Tuning

#### Local Storage Backends

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

#### Cloud Storage Backends

```yaml
env:
- name: ORBIT_S3_CONNECTION_TIMEOUT
  value: "60"         # Longer timeout for large objects
- name: ORBIT_S3_RETRY_COUNT
  value: "5"          # More retries for reliability
```

#### Resource Allocation

```yaml
resources:
  requests:
    memory: "2Gi"     # Minimum for RocksDB/LSM-Tree
    cpu: "1000m"      # Minimum for compaction
  limits:
    memory: "8Gi"     # Allow burst for large operations
    cpu: "4000m"      # Allow burst for compaction
```

---

## Summary

This comprehensive Kubernetes documentation covers all aspects of deploying Orbit-RS on Kubernetes, including:

- **Storage Backend Configuration**: Support for all persistence backends
- **Deployment Options**: Helm charts, manual deployment, and Kubernetes operators
- **Production Best Practices**: Monitoring, security, high availability
- **Troubleshooting**: Common issues and solutions
- **Maintenance**: Backup, updates, and scaling operations

The foundation is now in place to support all Orbit-RS persistence backends on Kubernetes!

---

## Support and Documentation

- **GitHub Issues**: <https://github.com/TuringWorks/orbit-rs/issues>
- **Documentation**: <https://github.com/TuringWorks/orbit-rs/docs>
- **Discussions**: <https://github.com/TuringWorks/orbit-rs/discussions>

