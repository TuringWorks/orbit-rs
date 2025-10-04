# Deployment Guide

This guide covers deploying Orbit-RS in production environments, including Kubernetes deployment, CI/CD pipelines, and operational best practices.

## Overview

Orbit-RS is designed for production deployment with enterprise-grade features including high availability, scalability, monitoring, and security. The system supports multiple deployment models from single-node development to multi-region distributed clusters.

## Deployment Options

### 1. Docker Deployment
- **Single Node**: Quick setup for development and testing
- **Docker Compose**: Multi-service orchestration for local clusters
- **Container Registry**: Production-ready container images

### 2. Kubernetes Deployment
- **Native Operator**: Custom Kubernetes operator with CRDs
- **Helm Charts**: Production-ready Helm charts
- **Multi-platform Support**: linux/amd64 and linux/arm64

### 3. Bare Metal Deployment
- **Binary Installation**: Static binaries for direct deployment
- **Service Management**: systemd integration for Linux systems
- **Process Management**: PM2 or similar for Node.js-style deployment

## Kubernetes Deployment

### Prerequisites

- **Kubernetes Cluster**: Version 1.20+ recommended
- **kubectl**: Configured to access your cluster
- **Helm**: Version 3.0+ (optional, for Helm deployment)
- **Storage**: Persistent storage for transaction logs and metrics

### Quick Start with Operator

Deploy Orbit-RS using the native Kubernetes operator:

```bash
# Deploy the operator components
kubectl apply -f orbit-operator/deploy/crds.yaml
kubectl apply -f orbit-operator/deploy/rbac.yaml  
kubectl apply -f orbit-operator/deploy/operator.yaml

# Deploy an Orbit cluster
kubectl apply -f orbit-operator/deploy/examples.yaml
```

### Helm Chart Deployment

For production deployments, use the Helm chart:

```bash
# Add the Orbit-RS Helm repository
helm repo add orbit-rs https://charts.turingworks.com/orbit-rs
helm repo update

# Install with custom configuration
helm install orbit-cluster orbit-rs/orbit-rs \
  --set replicaCount=3 \
  --set image.tag=latest \
  --set persistence.enabled=true \
  --set persistence.size=10Gi \
  --set monitoring.enabled=true
```

### Custom Resource Definitions (CRDs)

Orbit-RS provides several CRDs for managing clusters and components:

#### OrbitCluster CRD
```yaml
apiVersion: orbit.turingworks.com/v1alpha1
kind: OrbitCluster
metadata:
  name: production-cluster
  namespace: orbit-system
spec:
  replicas: 5
  version: "1.0.0"
  configuration:
    cluster:
      namespace: "production"
      discovery_type: "etcd"
    storage:
      backend: "postgresql"
      connection_string: "postgresql://orbit:password@postgres:5432/orbit"
    metrics:
      enabled: true
      prometheus_port: 9090
  resources:
    requests:
      memory: "512Mi"
      cpu: "250m"
    limits:
      memory: "2Gi"
      cpu: "1000m"
```

#### OrbitActor CRD
```yaml
apiVersion: orbit.turingworks.com/v1alpha1
kind: OrbitActor
metadata:
  name: bank-account-actor
  namespace: orbit-system
spec:
  actorType: "BankAccountActor"
  replicas: 10
  configuration:
    persistence:
      enabled: true
      backend: "postgresql"
    metrics:
      enabled: true
  resources:
    requests:
      memory: "256Mi"
      cpu: "100m"
    limits:
      memory: "1Gi"
      cpu: "500m"
```

### Production Configuration

#### High Availability Setup

```yaml
apiVersion: orbit.turingworks.com/v1alpha1
kind: OrbitCluster
metadata:
  name: ha-cluster
spec:
  replicas: 5
  affinity:
    podAntiAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchLabels:
            app: orbit-rs
        topologyKey: kubernetes.io/hostname
  tolerations:
  - key: "orbit-dedicated"
    operator: "Equal"
    value: "true"
    effect: "NoSchedule"
  configuration:
    cluster:
      discovery_type: "etcd"
      health_check_interval: "10s"
      leader_election_timeout: "30s"
    storage:
      backend: "postgresql"
      connection_pool_size: 20
      max_connections: 100
```

#### Multi-Region Deployment

```bash
# Deploy to multiple regions with cross-region replication
helm install orbit-us-west orbit-rs/orbit-rs \
  --set cluster.region=us-west \
  --set replication.enabled=true \
  --set replication.peers="orbit-us-east,orbit-eu-west"

helm install orbit-us-east orbit-rs/orbit-rs \
  --set cluster.region=us-east \
  --set replication.enabled=true \
  --set replication.peers="orbit-us-west,orbit-eu-west"
```

### Storage Configuration

#### PostgreSQL Backend

```yaml
configuration:
  storage:
    backend: "postgresql"
    connection_string: "postgresql://orbit:${DB_PASSWORD}@postgres-cluster:5432/orbit"
    connection_pool:
      max_connections: 50
      min_connections: 5
      max_lifetime: "1h"
      idle_timeout: "10m"
    migrations:
      auto_migrate: true
      migration_timeout: "5m"
```

#### Persistent Volumes

```yaml
persistence:
  enabled: true
  storageClass: "fast-ssd"
  size: "50Gi"
  accessModes:
    - ReadWriteOnce
  annotations:
    volume.beta.kubernetes.io/storage-class: "fast-ssd"
```

## CI/CD Pipeline

Orbit-RS includes a comprehensive GitHub Actions CI/CD pipeline for automated testing, building, and deployment.

### Pipeline Overview

The CI/CD pipeline includes:

- **Continuous Integration**:
  - Automated formatting checks (`cargo fmt`)
  - Linting with Clippy (`cargo clippy -D warnings`)
  - Unit and integration tests across all crates
  - Build verification for all examples
  - Security scanning with `cargo-deny`
  - Vulnerability scanning with Trivy

- **Continuous Deployment**:
  - Multi-platform Docker builds (linux/amd64, linux/arm64)
  - SBOM generation for security compliance
  - Container image publishing to registry
  - Kubernetes deployment manifests

### Setting up CI/CD

#### 1. Prepare Secrets

Use the provided script to prepare secrets for GitHub Actions:

```bash
# Prepare secrets for staging environment
./scripts/prepare-secrets.sh staging

# Prepare secrets for production environment  
./scripts/prepare-secrets.sh production
```

This generates the following files:
- `staging-kubeconfig-base64.txt`
- `production-kubeconfig-base64.txt`
- Environment-specific configuration files

#### 2. Configure GitHub Secrets

Add the following secrets in GitHub repository settings (Settings → Secrets and variables → Actions):

- **KUBE_CONFIG_STAGING**: Contents of `staging-kubeconfig-base64.txt`
- **KUBE_CONFIG_PRODUCTION**: Contents of `production-kubeconfig-base64.txt`
- **DOCKER_REGISTRY_USERNAME**: Container registry username
- **DOCKER_REGISTRY_PASSWORD**: Container registry password
- **SLACK_WEBHOOK_URL**: (Optional) Slack notifications

#### 3. Environment Configuration

```yaml
# .github/workflows/deploy.yml
env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}
  KUBE_NAMESPACE_STAGING: orbit-staging
  KUBE_NAMESPACE_PRODUCTION: orbit-production
```

### Deployment Workflow

```yaml
name: Deploy to Kubernetes

on:
  push:
    branches: [main]
    tags: ['v*']
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    - run: cargo test --workspace
    
  build-and-push:
    needs: test
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - uses: docker/setup-buildx-action@v2
    - uses: docker/login-action@v2
      with:
        registry: ${{ env.REGISTRY }}
        username: ${{ github.actor }}
        password: ${{ secrets.GITHUB_TOKEN }}
    - uses: docker/build-push-action@v4
      with:
        platforms: linux/amd64,linux/arm64
        push: true
        tags: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }}
        
  deploy-staging:
    needs: build-and-push
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    environment: staging
    steps:
    - uses: actions/checkout@v4
    - uses: azure/k8s-set-context@v1
      with:
        method: kubeconfig
        kubeconfig: ${{ secrets.KUBE_CONFIG_STAGING }}
    - uses: azure/k8s-deploy@v1
      with:
        namespace: ${{ env.KUBE_NAMESPACE_STAGING }}
        manifests: |
          k8s/staging/
        images: |
          ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }}
```

### Automated Testing in CI

The pipeline runs comprehensive tests:

```bash
# Unit tests across all workspace crates
cargo test --workspace --lib

# Integration tests
cargo test --workspace --test integration

# BDD scenarios  
cargo test --workspace --test bdd

# Example verification
cargo run --package hello-world --example validate
cargo run --package distributed-counter --example validate

# Security scanning
cargo audit
cargo deny check
```

## Container Configuration

### Multi-platform Docker Images

Orbit-RS provides multi-platform container images:

```dockerfile
FROM rust:1.70-slim as builder

WORKDIR /usr/src/app
COPY . .
RUN cargo build --release --bin orbit-server

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/orbit-server /usr/local/bin/orbit-server

EXPOSE 8080 9090
CMD ["orbit-server"]
```

### Container Security

Security best practices for container deployment:

```dockerfile
# Use non-root user
RUN groupadd -r orbit && useradd -r -g orbit orbit
USER orbit

# Minimal attack surface
FROM scratch
COPY --from=builder /usr/src/app/target/release/orbit-server /orbit-server
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Security labels
LABEL security.non-root=true
LABEL security.minimal-surface=true
```

### Image Scanning

Automated security scanning with Trivy:

```yaml
- name: Run Trivy vulnerability scanner
  uses: aquasecurity/trivy-action@master
  with:
    image-ref: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.sha }}
    format: 'sarif'
    output: 'trivy-results.sarif'

- name: Upload Trivy scan results
  uses: github/codeql-action/upload-sarif@v2
  with:
    sarif_file: 'trivy-results.sarif'
```

## Monitoring and Observability

### Prometheus Integration

Orbit-RS includes built-in Prometheus metrics:

```yaml
monitoring:
  enabled: true
  prometheus:
    port: 9090
    path: /metrics
    scrape_interval: 30s
  grafana:
    enabled: true
    dashboards:
      - orbit-overview
      - orbit-transactions  
      - orbit-performance
```

### Key Metrics

- **Actor Metrics**: Active actors, message rates, lifecycle events
- **Transaction Metrics**: Transaction rates, durations, success/failure rates
- **Cluster Metrics**: Node health, network connectivity, load balancing
- **Performance Metrics**: CPU, memory, network I/O, storage I/O

### Grafana Dashboards

Pre-built Grafana dashboards are included:

```bash
# Import dashboards
kubectl apply -f monitoring/grafana/dashboards/
```

### Alerting Rules

Production alerting rules:

```yaml
groups:
- name: orbit.rules
  rules:
  - alert: OrbitHighErrorRate
    expr: rate(orbit_errors_total[5m]) > 0.1
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High error rate in Orbit cluster"
      
  - alert: OrbitTransactionFailures
    expr: rate(orbit_transactions_failed_total[5m]) > 0.05
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "High transaction failure rate"
```

## Security Configuration

### TLS Configuration

Enable TLS for all communications:

```yaml
security:
  tls:
    enabled: true
    cert_file: "/etc/certs/tls.crt"
    key_file: "/etc/certs/tls.key"
    ca_file: "/etc/certs/ca.crt"
  authentication:
    enabled: true
    provider: "jwt"
    jwt:
      secret_key_file: "/etc/secrets/jwt-secret"
      expiration: "24h"
```

### Network Policies

Kubernetes network policies for security:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: orbit-network-policy
spec:
  podSelector:
    matchLabels:
      app: orbit-rs
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app: orbit-client
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: postgresql
    ports:
    - protocol: TCP
      port: 5432
```

## Operational Best Practices

### Resource Allocation

Recommended resource allocation:

```yaml
# Production workload
resources:
  requests:
    memory: "2Gi"
    cpu: "1000m"
  limits:
    memory: "4Gi"
    cpu: "2000m"

# High-throughput workload  
resources:
  requests:
    memory: "4Gi"
    cpu: "2000m" 
  limits:
    memory: "8Gi"
    cpu: "4000m"
```

### Scaling Configuration

Horizontal Pod Autoscaler configuration:

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: orbit-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: orbit-cluster
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
```

### Backup Strategy

Automated backup configuration:

```yaml
backup:
  enabled: true
  schedule: "0 2 * * *"  # Daily at 2 AM
  retention: "30d"
  storage:
    backend: "s3"
    s3:
      bucket: "orbit-backups"
      region: "us-west-2"
      encryption: true
```

### Disaster Recovery

Cross-region disaster recovery:

```bash
# Automated failover configuration
kubectl apply -f disaster-recovery/failover-policy.yaml

# Manual failover process
kubectl patch orbitcluster production-cluster \
  --type='merge' \
  --patch='{"spec":{"failover":{"target":"us-east","mode":"immediate"}}}'
```

## Troubleshooting

### Common Issues

1. **Pod Startup Issues**
   ```bash
   kubectl describe pod <pod-name>
   kubectl logs <pod-name> --previous
   ```

2. **Network Connectivity**
   ```bash
   kubectl exec -it <pod-name> -- netstat -tlnp
   kubectl get svc,ep
   ```

3. **Resource Constraints**
   ```bash
   kubectl top pods
   kubectl describe node <node-name>
   ```

### Performance Tuning

Resource optimization guidelines:

```yaml
# CPU-bound workloads
resources:
  requests:
    cpu: "2000m"
    memory: "1Gi"
  limits:
    cpu: "4000m"
    memory: "2Gi"

# Memory-intensive workloads
resources:
  requests:
    cpu: "500m"
    memory: "4Gi"
  limits:
    cpu: "1000m"
    memory: "8Gi"
```

## Related Documentation

- [Quick Start Guide](../QUICK_START.md) - Basic setup and installation
- [Transaction Features](../features/TRANSACTION_FEATURES.md) - Production transaction configuration
- [Protocol Adapters](../protocols/PROTOCOL_ADAPTERS.md) - Protocol-specific deployment
- [Development Guide](../development/DEVELOPMENT.md) - Development environment setup