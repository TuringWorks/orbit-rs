# Orbit-RS Kubernetes Deployment Guide

This guide provides comprehensive instructions for deploying Orbit-RS on Kubernetes in production environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Architecture Overview](#architecture-overview)
- [Manual Deployment](#manual-deployment)
- [Helm Deployment](#helm-deployment)
- [Configuration](#configuration)
- [Monitoring](#monitoring)
- [High Availability](#high-availability)
- [Security](#security)
- [Troubleshooting](#troubleshooting)
- [Maintenance](#maintenance)

## Prerequisites

### Kubernetes Cluster Requirements

- **Kubernetes version**: 1.24+
- **Minimum nodes**: 3 (for high availability)
- **Node resources**: 2 CPU cores, 4GB RAM per node minimum
- **Storage**: Persistent storage with ReadWriteOnce support
- **Network**: Pod-to-pod communication, LoadBalancer support (optional)

### Required Tools

```bash
# Install kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
chmod +x kubectl && sudo mv kubectl /usr/local/bin/

# Install Helm (recommended)
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# Install Docker (for building images)
curl -fsSL https://get.docker.com -o get-docker.sh && sh get-docker.sh
```

### Optional Tools

```bash
# Prometheus Operator (for monitoring)
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update

# Grafana (for visualization)
helm repo add grafana https://grafana.github.io/helm-charts

# cert-manager (for TLS certificates)
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml
```

## Quick Start

### Using Helm (Recommended)

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

### Using Local Helm Chart

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

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                         Internet                              │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                 Load Balancer                                │
│            (Kubernetes Service)                              │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                   Ingress Controller                         │
│              (nginx/traefik/istio)                          │
└─────────────────────┬───────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        │             │             │
┌───────▼──────┬──────▼──────┬──────▼──────┐
│ orbit-server │ orbit-server │ orbit-server │
│    Pod 1     │    Pod 2     │    Pod 3     │
│              │              │              │
│ ┌──────────┐ │ ┌──────────┐ │ ┌──────────┐ │
│ │   App    │ │ │   App    │ │ │   App    │ │
│ └──────────┘ │ └──────────┘ │ └──────────┘ │
│ ┌──────────┐ │ ┌──────────┐ │ ┌──────────┐ │
│ │ SQLite   │ │ │ SQLite   │ │ │ SQLite   │ │
│ │    DB    │ │ │    DB    │ │ │    DB    │ │
│ └──────────┘ │ └──────────┘ │ └──────────┘ │
└──────┬───────┴──────┬───────┴──────┬───────┘
       │              │              │
┌──────▼──────┬───────▼──────┬───────▼──────┐
│Persistent   │Persistent    │Persistent    │
│Volume 1     │Volume 2      │Volume 3      │
└─────────────┴──────────────┴──────────────┘

┌─────────────────────────────────────────────────────────────┐
│                    Monitoring Stack                          │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐           │
│  │ Prometheus  │ │   Grafana   │ │  AlertMgr   │           │
│  └─────────────┘ └─────────────┘ └─────────────┘           │
└─────────────────────────────────────────────────────────────┘
```

### Component Description

- **StatefulSet**: Manages orbit-server pods with stable network identities
- **Headless Service**: Enables pod-to-pod communication for cluster formation
- **Load Balancer Service**: External access point for clients
- **Persistent Volumes**: Durable storage for transaction logs
- **ConfigMaps**: Configuration management
- **ServiceMonitor**: Prometheus metrics collection
- **PrometheusRule**: Alerting rules

## Manual Deployment

### Step 1: Create Namespace and RBAC

```bash
kubectl apply -f k8s/00-namespace.yaml
```

### Step 2: Configure Storage

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

### Step 3: Deploy Configuration

```bash
kubectl apply -f k8s/01-configmap.yaml
```

### Step 4: Deploy StatefulSet and Services

```bash
kubectl apply -f k8s/03-statefulset.yaml
kubectl apply -f k8s/04-services.yaml
```

### Step 5: Verify Deployment

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

## Helm Deployment

### Configuration Values

Create a custom values file `my-values.yaml`:

```yaml
orbitServer:
  replicaCount: 3
  image:
    repository: ghcr.io/turingworks/orbit-rs/orbit-server
    tag: "v0.1.0"
  resources:
    requests:
      memory: "1Gi"
      cpu: "500m"
    limits:
      memory: "4Gi"
      cpu: "2000m"

persistence:
  enabled: true
  size: 20Gi
  storageClass: "fast-ssd"

monitoring:
  serviceMonitor:
    enabled: true
  prometheusRule:
    enabled: true
  grafanaDashboard:
    enabled: true

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70

ingress:
  enabled: true
  className: "nginx"
  annotations:
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
  hosts:
    - host: orbit-rs.yourdomain.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: orbit-rs-tls
      hosts:
        - orbit-rs.yourdomain.com

config:
  logging:
    level: "info"
  transactions:
    maxConnections: 20
  saga:
    maxExecutionTimeSeconds: 7200
```

### Deploy with Helm

```bash
# Deploy
helm install orbit-rs helm/orbit-rs \
  --namespace orbit-rs \
  --create-namespace \
  --values my-values.yaml

# Upgrade
helm upgrade orbit-rs helm/orbit-rs \
  --namespace orbit-rs \
  --values my-values.yaml

# Rollback
helm rollback orbit-rs 1 --namespace orbit-rs

# Uninstall
helm uninstall orbit-rs --namespace orbit-rs
```

## Configuration

### Environment-Specific Configurations

#### Development

```yaml
# dev-values.yaml
orbitServer:
  replicaCount: 1
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

### Configuration Management

```bash
# Create environment-specific configurations
kubectl create configmap orbit-rs-prod-config \
  --from-file=config/production.toml \
  --namespace orbit-rs

# Use external secrets (recommended for production)
kubectl apply -f - <<EOF
apiVersion: external-secrets.io/v1beta1
kind: SecretStore
metadata:
  name: vault-backend
  namespace: orbit-rs
spec:
  provider:
    vault:
      server: "https://vault.example.com"
      path: "secret"
      auth:
        kubernetes:
          mountPath: "kubernetes"
          role: "orbit-rs"
EOF
```

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

### Grafana Dashboards

```bash
# Deploy Grafana dashboard
kubectl apply -f k8s/05-monitoring.yaml

# Access Grafana
kubectl port-forward svc/prometheus-grafana 3000:80 -n monitoring
# Default login: admin/prom-operator
```

### Key Metrics to Monitor

- **Transaction Rate**: `rate(orbit_transactions_total[5m])`
- **Success Rate**: `rate(orbit_transactions_completed_total[5m]) / rate(orbit_transactions_total[5m])`
- **Active Transactions**: `orbit_transactions_active`
- **Saga Statistics**: `rate(orbit_sagas_*_total[5m])`
- **Recovery Operations**: `rate(orbit_recovery_*_total[5m])`
- **Memory Usage**: `container_memory_usage_bytes`
- **CPU Usage**: `rate(container_cpu_usage_seconds_total[5m])`

### Alerting Rules

Key alerts configured in PrometheusRule:

- **OrbitServerDown**: Server instances unavailable
- **OrbitServerHighMemoryUsage**: Memory usage > 90%
- **OrbitServerHighCPUUsage**: CPU usage > 80%
- **OrbitTransactionFailureRate**: Failure rate > 10%
- **OrbitSagaCompensationRate**: Compensation rate > 20%
- **OrbitRecoveryFailure**: Multiple recovery failures

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

### RBAC Configuration

The deployment includes minimal RBAC permissions:

- Read access to nodes, pods, services, endpoints
- Create/update access to events
- Full access to coordination.k8s.io/leases (for leader election)

## Troubleshooting

### Common Issues

#### 1. Pods Not Starting

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

#### 2. Persistent Volume Issues

```bash
# Check PVCs
kubectl get pvc -n orbit-rs

# Check storage class
kubectl get storageclass

# Check events
kubectl get events -n orbit-rs --sort-by=.metadata.creationTimestamp
```

#### 3. Network Connectivity

```bash
# Test internal connectivity
kubectl exec -it orbit-server-0 -n orbit-rs -- curl http://orbit-server-1.orbit-server-headless:8080/health

# Test external connectivity
kubectl port-forward svc/orbit-server 8080:8080 -n orbit-rs
curl http://localhost:8080/health
```

#### 4. Configuration Issues

```bash
# Check configuration
kubectl exec -it orbit-server-0 -n orbit-rs -- cat /tmp/orbit-server.toml

# Validate configuration
kubectl exec -it orbit-server-0 -n orbit-rs -- /app/orbit-server --config /tmp/orbit-server.toml --validate
```

### Debugging Commands

```bash
# Get all resources
kubectl get all -n orbit-rs

# Describe problematic pod
kubectl describe pod <pod-name> -n orbit-rs

# Get logs with timestamps
kubectl logs <pod-name> -n orbit-rs --timestamps

# Execute commands in pod
kubectl exec -it <pod-name> -n orbit-rs -- bash

# Check resource usage
kubectl top pods -n orbit-rs

# Get events
kubectl get events -n orbit-rs --sort-by='.lastTimestamp'
```

## Maintenance

### Backup Procedures

```bash
# Backup persistent volumes
kubectl get pvc -n orbit-rs -o yaml > orbit-rs-pvc-backup.yaml

# Backup configuration
kubectl get configmap -n orbit-rs -o yaml > orbit-rs-config-backup.yaml
kubectl get secret -n orbit-rs -o yaml > orbit-rs-secrets-backup.yaml

# Application-level backup (if supported)
kubectl exec -it orbit-server-0 -n orbit-rs -- /app/orbit-server --backup /app/data/backup.sql
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

```yaml
# Resource optimization
resources:
  requests:
    memory: "2Gi"
    cpu: "1000m"
  limits:
    memory: "8Gi" 
    cpu: "4000m"

# Configuration tuning
config:
  performance:
    workerThreads: 8
    maxBlockingThreads: 1024
  transactions:
    maxConnections: 50
```

### Log Management

```bash
# Centralized logging with Fluentd/Filebeat
# Configure log rotation
kubectl patch statefulset orbit-server -n orbit-rs --type='merge' -p='{
  "spec": {
    "template": {
      "spec": {
        "containers": [{
          "name": "orbit-server",
          "env": [{
            "name": "RUST_LOG_MAX_SIZE",
            "value": "100MB"
          }]
        }]
      }
    }
  }
}'
```

## Support and Documentation

- **GitHub Issues**: https://github.com/TuringWorks/orbit-rs/issues
- **Documentation**: https://github.com/TuringWorks/orbit-rs/docs
- **Discussions**: https://github.com/TuringWorks/orbit-rs/discussions
- **Wiki**: https://github.com/TuringWorks/orbit-rs/wiki

---

This deployment guide provides comprehensive coverage of Orbit-RS Kubernetes deployment scenarios. For additional help or specific use cases, please refer to the project documentation or create an issue on GitHub.