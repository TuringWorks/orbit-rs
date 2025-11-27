# Deployment Guide

This guide covers deploying Orbit Engine in production environments.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Deployment Topologies](#deployment-topologies)
- [Configuration](#configuration)
- [Storage Backends](#storage-backends)
- [High Availability](#high-availability)
- [Monitoring](#monitoring)
- [Performance Tuning](#performance-tuning)
- [Security](#security)
- [Backup and Recovery](#backup-and-recovery)

## Architecture Overview

Orbit Engine consists of several layers that can be deployed and scaled independently:

```text
┌─────────────────────────────────────────────────────────────┐
│         Protocol Layer (PostgreSQL, Redis, REST)            │
│                   (Load Balanced)                           │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Orbit Engine Cluster Nodes                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │  Node 1  │  │  Node 2  │  │  Node 3  │  (Raft)           │
│  │ (Leader) │  │(Follower)│  │(Follower)│                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│         Storage Backends (S3, Azure Blob, MinIO)            │
└─────────────────────────────────────────────────────────────┘
```

## Deployment Topologies

### 1. Single Node (Development)

**Use Case**: Development, testing, small workloads

```rust
use orbit_engine::storage::HybridStorageManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create in-memory storage for development
    let storage = Arc::new(HybridStorageManager::new_in_memory());

    // Create adapter context
    let context = orbit_engine::adapters::AdapterContext::new(
        storage as Arc<dyn orbit_engine::storage::TableStorage>
    );

    // Start your protocol adapters
    // ...

    Ok(())
}
```

**Pros**:

- Simple setup
- Low resource requirements
- Fast for development

**Cons**:

- No high availability
- Limited by single node resources
- Data loss if node fails

### 2. Multi-Node with Raft (Production)

**Use Case**: Production workloads requiring high availability

```rust
use orbit_engine::storage::HybridStorageManager;
use orbit_engine::cluster::{ClusterManager, ClusterConfig};
use orbit_engine::storage::{StorageBackend, S3Config};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure storage backend (S3)
    let backend = StorageBackend::S3(S3Config {
        endpoint: std::env::var("S3_ENDPOINT")?,
        region: std::env::var("AWS_REGION")?,
        bucket: std::env::var("S3_BUCKET")?,
        access_key: std::env::var("AWS_ACCESS_KEY_ID")?,
        secret_key: std::env::var("AWS_SECRET_ACCESS_KEY")?,
    });

    // Create storage with cold tier
    let storage = Arc::new(
        HybridStorageManager::with_cold_tier(backend).await?
    );

    // Configure cluster
    let cluster_config = ClusterConfig {
        node_id: std::env::var("NODE_ID")?,
        peers: std::env::var("CLUSTER_PEERS")?
            .split(',')
            .map(String::from)
            .collect(),
        replication_factor: 3,
        election_timeout_ms: 5000,
        heartbeat_interval_ms: 1000,
    };

    // Create cluster manager
    let cluster = Arc::new(
        ClusterManager::new(cluster_config, storage.clone()).await?
    );

    // Start cluster
    cluster.start().await?;

    // Create adapter context with clustering
    let context = orbit_engine::adapters::AdapterContext::with_transactions(
        storage as Arc<dyn orbit_engine::storage::TableStorage>,
        cluster as Arc<dyn orbit_engine::transaction::TransactionManager>,
    );

    // Start your protocol adapters
    // ...

    Ok(())
}
```

**Recommended Setup**:

- 3-5 nodes for optimal Raft consensus
- Odd number of nodes to avoid split-brain
- Nodes distributed across availability zones

**Pros**:

- High availability
- Automatic failover
- Horizontal scalability

**Cons**:

- More complex setup
- Higher resource requirements

### 3. Geo-Distributed (Multi-Region)

**Use Case**: Global applications, disaster recovery

```text
Region 1 (us-east-1)          Region 2 (eu-west-1)
┌─────────────────┐          ┌─────────────────┐
│   Node 1, 2     │ ◄────────┼ Node 3, 4       │
│   (Primary)     │   CDC    │ (Replica)       │
└─────────────────┘          └─────────────────┘
        │                             │
        ▼                             ▼
    S3 Bucket                    S3 Bucket
   (us-east-1)                  (eu-west-1)
```

**Configuration**:

```rust
use orbit_engine::cdc::{CdcPublisher, CdcConfig};

// Primary region (us-east-1)
let cdc_config = CdcConfig {
    enabled: true,
    replication_targets: vec![
        "https://eu-west-1.orbit.example.com".to_string(),
    ],
    batch_size: 1000,
    flush_interval_ms: 100,
};

let cdc_publisher = CdcPublisher::new(cdc_config, storage.clone())?;
cdc_publisher.start().await?;
```

**Pros**:

- Low latency globally
- Disaster recovery
- Data sovereignty compliance

**Cons**:

- Complex setup
- Network latency between regions
- Higher costs

## Configuration

### Environment Variables

Create a `.env` file or set environment variables:

```bash
# Node Configuration
NODE_ID=node-1
CLUSTER_PEERS=node-1:7000,node-2:7000,node-3:7000

# Storage Backend (S3)
S3_ENDPOINT=s3.amazonaws.com
AWS_REGION=us-west-2
S3_BUCKET=orbit-cold-tier
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key

# Storage Backend (Azure Blob)
# AZURE_ACCOUNT=your-account
# AZURE_CONTAINER=orbit-cold-tier
# AZURE_ACCESS_KEY=your-access-key

# Tiered Storage Thresholds
HOT_TIER_RETENTION_HOURS=48      # Default: 48 hours
WARM_TIER_RETENTION_DAYS=30      # Default: 30 days

# Transaction Configuration
DEFAULT_ISOLATION_LEVEL=ReadCommitted
TRANSACTION_TIMEOUT_MS=30000

# Cluster Configuration
RAFT_ELECTION_TIMEOUT_MS=5000
RAFT_HEARTBEAT_INTERVAL_MS=1000
REPLICATION_FACTOR=3

# Performance Tuning
MAX_CONCURRENT_TRANSACTIONS=10000
QUERY_EXECUTOR_THREADS=8
SIMD_ENABLED=true

# Monitoring
METRICS_ENABLED=true
METRICS_PORT=9090
TRACING_ENABLED=true
TRACING_ENDPOINT=http://jaeger:14268/api/traces
```

### Configuration File (config.toml)

```toml
[node]
id = "node-1"
bind_address = "0.0.0.0:7000"
advertise_address = "node-1.orbit.example.com:7000"

[cluster]
peers = ["node-1:7000", "node-2:7000", "node-3:7000"]
replication_factor = 3
election_timeout_ms = 5000
heartbeat_interval_ms = 1000

[storage]
backend = "s3"  # or "azure", "minio"

[storage.s3]
endpoint = "s3.amazonaws.com"
region = "us-west-2"
bucket = "orbit-cold-tier"
# Credentials via environment variables or IAM roles

[storage.tiers]
hot_retention_hours = 48
warm_retention_days = 30

[storage.rocksdb]
path = "/var/lib/orbit/rocksdb"
max_open_files = 10000
write_buffer_size = 67108864  # 64MB

[transaction]
default_isolation_level = "ReadCommitted"
timeout_ms = 30000
max_concurrent = 10000

[query]
executor_threads = 8
simd_enabled = true
max_batch_size = 10000

[monitoring]
metrics_enabled = true
metrics_port = 9090
tracing_enabled = true
tracing_endpoint = "http://jaeger:14268/api/traces"
```

## Storage Backends

### AWS S3

**Prerequisites**:

- AWS account with S3 access
- IAM role or access keys with S3 permissions

**IAM Policy** (minimum required):

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:ListBucket"
      ],
      "Resource": [
        "arn:aws:s3:::orbit-cold-tier",
        "arn:aws:s3:::orbit-cold-tier/*"
      ]
    }
  ]
}
```

**Best Practices**:

- Use IAM roles for EC2 instances instead of access keys
- Enable versioning for disaster recovery
- Configure lifecycle policies for archival
- Use S3 Standard-IA for cold tier to reduce costs

### Azure Blob Storage

**Prerequisites**:

- Azure Storage Account
- Container created for Iceberg tables

**Configuration**:

```rust
use orbit_engine::storage::{StorageBackend, AzureConfig};

let backend = StorageBackend::Azure(AzureConfig {
    account: "orbitstorageaccount".to_string(),
    container: "orbit-cold-tier".to_string(),
    access_key: std::env::var("AZURE_ACCESS_KEY")?,
});
```

**Best Practices**:

- Use Cool or Archive tier for historical data
- Enable soft delete for disaster recovery
- Use Azure Private Link for security
- Configure CORS if accessing from web clients

### MinIO (Self-Hosted)

**Prerequisites**:

- MinIO server running
- Bucket created

**Docker Deployment**:

```bash
docker run -d \
  --name minio \
  -p 9000:9000 \
  -p 9001:9001 \
  -e "MINIO_ROOT_USER=admin" \
  -e "MINIO_ROOT_PASSWORD=your-password" \
  -v /data/minio:/data \
  minio/minio server /data --console-address ":9001"
```

**Configuration**:

```rust
use orbit_engine::storage::{StorageBackend, S3Config};

let backend = StorageBackend::S3(S3Config {
    endpoint: "http://minio:9000".to_string(),
    region: "us-east-1".to_string(),  // MinIO ignores region
    bucket: "orbit-cold-tier".to_string(),
    access_key: "admin".to_string(),
    secret_key: "your-password".to_string(),
});
```

## High Availability

### Raft Cluster Setup

**3-Node Cluster** (Recommended):

```bash
# Node 1 (Leader candidate)
NODE_ID=node-1 \
CLUSTER_PEERS=node-1:7000,node-2:7000,node-3:7000 \
./orbit-engine

# Node 2 (Follower)
NODE_ID=node-2 \
CLUSTER_PEERS=node-1:7000,node-2:7000,node-3:7000 \
./orbit-engine

# Node 3 (Follower)
NODE_ID=node-3 \
CLUSTER_PEERS=node-1:7000,node-2:7000,node-3:7000 \
./orbit-engine
```

**Health Checks**:

```bash
# Check cluster status
curl http://node-1:9090/health
curl http://node-1:9090/cluster/status

# Expected response
{
  "healthy": true,
  "role": "leader",
  "term": 5,
  "peers": ["node-2:7000", "node-3:7000"],
  "replication_lag_ms": 10
}
```

### Load Balancing

**HAProxy Configuration** (haproxy.cfg):

```text
frontend postgres_frontend
    bind *:5432
    mode tcp
    default_backend postgres_backend

backend postgres_backend
    mode tcp
    balance leastconn
    option tcp-check
    server node1 node-1:5432 check
    server node2 node-2:5432 check
    server node3 node-3:5432 check
```

**Nginx Configuration**:

```nginx
upstream orbit_cluster {
    least_conn;
    server node-1:5432;
    server node-2:5432;
    server node-3:5432;
}

server {
    listen 5432;
    proxy_pass orbit_cluster;
}
```

## Monitoring

### Metrics Endpoints

Orbit Engine exposes Prometheus-compatible metrics:

```bash
curl http://localhost:9090/metrics
```

**Key Metrics**:

```text
# Storage metrics
orbit_storage_hot_tier_size_bytes
orbit_storage_warm_tier_size_bytes
orbit_storage_cold_tier_size_bytes
orbit_storage_tier_migration_total
orbit_storage_operations_total{tier="hot|warm|cold",operation="read|write"}
orbit_storage_operation_duration_seconds{tier,operation}

# Transaction metrics
orbit_transactions_active
orbit_transactions_total{status="committed|aborted"}
orbit_transaction_duration_seconds
orbit_transaction_conflicts_total
orbit_deadlocks_detected_total

# Cluster metrics
orbit_cluster_nodes_total
orbit_cluster_leader_changes_total
orbit_cluster_replication_lag_ms
orbit_raft_heartbeat_failures_total

# Query metrics
orbit_query_execution_duration_seconds
orbit_query_rows_scanned_total
orbit_query_cache_hits_total
orbit_simd_operations_total
```

### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'orbit-engine'
    static_configs:
      - targets:
        - 'node-1:9090'
        - 'node-2:9090'
        - 'node-3:9090'
```

### Grafana Dashboard

Import the Orbit Engine dashboard (dashboard.json):

**Key Panels**:

- Storage tier distribution
- Transaction throughput
- Query latency (p50, p95, p99)
- Cluster health
- Replication lag

### Distributed Tracing

**Jaeger Configuration**:

```rust
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

let tracer = opentelemetry_jaeger::new_pipeline()
    .with_service_name("orbit-engine")
    .with_endpoint("http://jaeger:14268/api/traces")
    .install_simple()?;

let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
let subscriber = Registry::default().with(telemetry);
tracing::subscriber::set_global_default(subscriber)?;
```

## Performance Tuning

### Memory Configuration

```bash
# Hot tier (in-memory)
HOT_TIER_MAX_SIZE_GB=16

# Warm tier (RocksDB)
ROCKSDB_BLOCK_CACHE_SIZE_GB=8
ROCKSDB_WRITE_BUFFER_SIZE_MB=64
ROCKSDB_MAX_OPEN_FILES=10000
```

### Query Execution

```bash
# Enable SIMD optimizations
SIMD_ENABLED=true
SIMD_INSTRUCTION_SET=AVX2  # or AVX512, NEON

# Executor threads (usually = CPU cores)
QUERY_EXECUTOR_THREADS=16

# Batch size for vectorized execution
QUERY_BATCH_SIZE=10000
```

### Tiered Storage

```bash
# Adjust retention periods based on workload
HOT_TIER_RETENTION_HOURS=24   # More aggressive migration
WARM_TIER_RETENTION_DAYS=14   # Faster cold tier archival

# Migration batch size
TIER_MIGRATION_BATCH_SIZE=1000
TIER_MIGRATION_INTERVAL_MS=60000  # Check every minute
```

### Replication

```bash
# Replication factor (3 or 5 for production)
REPLICATION_FACTOR=3

# Batch CDC updates for better throughput
CDC_BATCH_SIZE=1000
CDC_FLUSH_INTERVAL_MS=100
```

## Security

### TLS/SSL

**Enable TLS for cluster communication**:

```rust
use orbit_engine::cluster::TlsConfig;

let tls_config = TlsConfig {
    cert_path: "/etc/orbit/certs/server.crt".to_string(),
    key_path: "/etc/orbit/certs/server.key".to_string(),
    ca_cert_path: Some("/etc/orbit/certs/ca.crt".to_string()),
};

let cluster_config = ClusterConfig {
    // ... other config
    tls: Some(tls_config),
};
```

### Authentication

**Protocol-level authentication**:

```rust
use orbit_engine::adapters::postgres::{PostgresAuth, PostgresAdapter};

let auth = PostgresAuth::ScramSha256 {
    users: vec![
        ("admin".to_string(), "hashed-password".to_string()),
    ],
};

let adapter = PostgresAdapter::with_auth(context, auth);
```

### Network Security

**Firewall Rules**:

```bash
# Allow only cluster nodes to communicate
iptables -A INPUT -p tcp --dport 7000 -s node-2 -j ACCEPT
iptables -A INPUT -p tcp --dport 7000 -s node-3 -j ACCEPT
iptables -A INPUT -p tcp --dport 7000 -j DROP

# Allow client connections
iptables -A INPUT -p tcp --dport 5432 -j ACCEPT  # PostgreSQL
iptables -A INPUT -p tcp --dport 6379 -j ACCEPT  # Redis
iptables -A INPUT -p tcp --dport 8080 -j ACCEPT  # REST API
```

### Encryption at Rest

**S3 Server-Side Encryption**:

```rust
use orbit_engine::storage::S3Config;

let config = S3Config {
    // ... other config
    server_side_encryption: Some("AES256".to_string()),
};
```

## Backup and Recovery

### Automated Backups

**Iceberg Snapshots**:

```rust
use orbit_engine::storage::iceberg::IcebergColdStore;

// Create snapshot
let snapshot_id = cold_store.create_snapshot("daily-backup-2025-01-18").await?;

// List snapshots
let snapshots = cold_store.list_snapshots().await?;

// Restore from snapshot
cold_store.restore_snapshot(snapshot_id).await?;
```

### Point-in-Time Recovery

**Transaction Log Archival**:

```bash
# Archive transaction logs to S3
aws s3 sync /var/lib/orbit/transaction-logs s3://orbit-backups/transaction-logs/

# Restore transaction logs
aws s3 sync s3://orbit-backups/transaction-logs/ /var/lib/orbit/transaction-logs/
```

### Disaster Recovery

**Full Cluster Restore**:

1. Restore transaction logs
2. Restore RocksDB data
3. Restore Iceberg snapshots
4. Restart cluster nodes

```bash
# Node 1
systemctl stop orbit-engine
aws s3 sync s3://orbit-backups/node-1/ /var/lib/orbit/
systemctl start orbit-engine

# Repeat for Node 2, 3
```

## Troubleshooting

### Common Issues

#### Issue: High replication lag

```bash
# Check replication status
curl http://node-1:9090/cluster/replication

# Increase replication batch size
CDC_BATCH_SIZE=5000
```

#### Issue: Slow queries

```bash
# Enable query logging
QUERY_LOGGING_ENABLED=true
QUERY_SLOW_LOG_THRESHOLD_MS=1000

# Check SIMD is enabled
SIMD_ENABLED=true

# Increase executor threads
QUERY_EXECUTOR_THREADS=16
```

#### Issue: Leader election failures

```bash
# Increase election timeout
RAFT_ELECTION_TIMEOUT_MS=10000

# Check network connectivity between nodes
ping node-2
telnet node-2 7000
```

## Docker Deployment

**Dockerfile**:

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --package orbit-engine

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates
COPY --from=builder /app/target/release/orbit-engine /usr/local/bin/
EXPOSE 5432 6379 8080 7000 9090
CMD ["orbit-engine"]
```

**docker-compose.yml**:

```yaml
version: '3.8'

services:
  orbit-node-1:
    build: .
    environment:
      NODE_ID: node-1
      CLUSTER_PEERS: orbit-node-1:7000,orbit-node-2:7000,orbit-node-3:7000
      S3_ENDPOINT: ${S3_ENDPOINT}
      AWS_REGION: ${AWS_REGION}
      S3_BUCKET: ${S3_BUCKET}
    ports:
      - "5432:5432"
      - "9090:9090"
    volumes:
      - orbit-node-1-data:/var/lib/orbit

  orbit-node-2:
    build: .
    environment:
      NODE_ID: node-2
      CLUSTER_PEERS: orbit-node-1:7000,orbit-node-2:7000,orbit-node-3:7000
      S3_ENDPOINT: ${S3_ENDPOINT}
      AWS_REGION: ${AWS_REGION}
      S3_BUCKET: ${S3_BUCKET}
    volumes:
      - orbit-node-2-data:/var/lib/orbit

  orbit-node-3:
    build: .
    environment:
      NODE_ID: node-3
      CLUSTER_PEERS: orbit-node-1:7000,orbit-node-2:7000,orbit-node-3:7000
      S3_ENDPOINT: ${S3_ENDPOINT}
      AWS_REGION: ${AWS_REGION}
      S3_BUCKET: ${S3_BUCKET}
    volumes:
      - orbit-node-3-data:/var/lib/orbit

  prometheus:
    image: prom/prometheus
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml

  grafana:
    image: grafana/grafana
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin

volumes:
  orbit-node-1-data:
  orbit-node-2-data:
  orbit-node-3-data:
```

## Kubernetes Deployment

**StatefulSet** (orbit-statefulset.yaml):

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit-engine
spec:
  serviceName: orbit-cluster
  replicas: 3
  selector:
    matchLabels:
      app: orbit-engine
  template:
    metadata:
      labels:
        app: orbit-engine
    spec:
      containers:
      - name: orbit-engine
        image: orbit-engine:latest
        env:
        - name: NODE_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: CLUSTER_PEERS
          value: "orbit-engine-0:7000,orbit-engine-1:7000,orbit-engine-2:7000"
        - name: S3_ENDPOINT
          valueFrom:
            secretKeyRef:
              name: orbit-secrets
              key: s3-endpoint
        ports:
        - containerPort: 5432
          name: postgres
        - containerPort: 7000
          name: cluster
        - containerPort: 9090
          name: metrics
        volumeMounts:
        - name: data
          mountPath: /var/lib/orbit
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 100Gi
```

---

**Next Steps**: See [ADAPTERS.md](ADAPTERS.md) for developing custom protocol adapters.
