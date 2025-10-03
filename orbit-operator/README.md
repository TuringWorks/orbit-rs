# Orbit-RS Kubernetes Operator

The Orbit-RS Kubernetes Operator provides native Kubernetes support for deploying and managing Orbit-RS distributed actor systems.

## Quick Start

### Install the Operator

```bash
# Install CRDs
kubectl apply -f deploy/crds.yaml

# Install RBAC and operator
kubectl apply -f deploy/rbac.yaml
kubectl apply -f deploy/operator.yaml
```

### Deploy an Orbit Cluster

```bash
# Deploy example cluster
kubectl apply -f deploy/examples.yaml

# Check cluster status
kubectl get orbitclusters
kubectl describe orbitcluster example-cluster
```

## Custom Resources

### OrbitCluster

Manages a complete Orbit-RS cluster deployment with StatefulSet, Services, and ConfigMaps.

Key features:
- Automatic StatefulSet and Service creation
- Leader election configuration
- Transaction persistence setup
- Monitoring and metrics integration
- Rolling updates and scaling

### OrbitActor

Manages actor-specific deployments and configurations within an Orbit cluster.

Key features:
- Actor type registration
- Scaling policies
- Activation/deactivation timeouts
- Performance monitoring

### OrbitTransaction

Configures distributed transaction coordination and persistence.

Key features:
- Transaction coordinator setup
- Persistence backend configuration
- Recovery and failover settings
- Performance monitoring and alerting

## Configuration Examples

### Basic Cluster

```yaml
apiVersion: orbit.turingworks.com/v1
kind: OrbitCluster
metadata:
  name: my-cluster
spec:
  replicas: 3
  image:
    repository: orbit-rs/orbit-server
    tag: "latest"
```

### Production Cluster with External Storage

```yaml
apiVersion: orbit.turingworks.com/v1
kind: OrbitCluster
metadata:
  name: prod-cluster
spec:
  replicas: 5
  image:
    repository: orbit-rs/orbit-server
    tag: "v0.1.0"
  storage:
    storageClass: "fast-ssd"
    size: "50Gi"
  service:
    serviceType: LoadBalancer
    annotations:
      service.beta.kubernetes.io/aws-load-balancer-type: "nlb"
  resources:
    cpuRequest: "500m"
    memoryRequest: "1Gi"
    cpuLimit: "2000m"
    memoryLimit: "4Gi"
```

## Monitoring

The operator provides Prometheus metrics for:
- Cluster health and status
- Actor performance metrics
- Transaction success rates
- Resource utilization

Enable ServiceMonitor for Prometheus Operator:

```yaml
spec:
  monitoring:
    enabled: true
    serviceMonitor: true
```

## Development

### Building the Operator

```bash
# Build the operator binary
cargo build --release --package orbit-operator

# Build Docker image
docker build -t orbit-rs/orbit-operator:latest -f orbit-operator/Dockerfile .
```

### Testing

```bash
# Run unit tests
cargo test --package orbit-operator

# Integration tests with kind cluster
make test-integration
```

## Architecture

The operator consists of three main controllers:

1. **ClusterController**: Manages OrbitCluster resources
2. **ActorController**: Manages OrbitActor resources  
3. **TransactionController**: Manages OrbitTransaction resources

Each controller runs independently and watches for changes to their respective Custom Resources.