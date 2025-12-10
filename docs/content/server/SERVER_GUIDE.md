---
layout: default
title: "Server Guide"
subtitle: "Orbit-RS server configuration and operation"
category: "server"
---

# Orbit-RS Server Guide

Comprehensive guide for deploying and operating Orbit-RS.

---

## Quick Start

### Running the Server

```bash
# Development mode
cargo run --bin orbit-server

# With custom config
cargo run --bin orbit-server -- --config ./config/orbit-server.toml

# Production build
cargo build --release
./target/release/orbit-server --config /etc/orbit/orbit-server.toml
```

### Default Ports

| Protocol | Port | Description |
|----------|------|-------------|
| PostgreSQL | 5432 | SQL interface |
| MySQL | 3306 | MySQL-compatible |
| Redis | 6379 | Key-value + extensions |
| CQL | 9042 | Cassandra-compatible |
| Cypher | 7687 | Graph queries |
| AQL | 8529 | ArangoDB-compatible |
| MongoDB | 27017 | Document queries |
| HTTP REST | 8080 | JSON API |
| gRPC | 50051 | Actor management |
| Metrics | 9090 | Prometheus metrics |

---

## Configuration

### Main Configuration File

```toml
# orbit-server.toml

[server]
name = "orbit-node-1"
bind_address = "0.0.0.0"
data_dir = "/var/lib/orbit"

[protocols]
postgresql_port = 5432
mysql_port = 3306
redis_port = 6379
cql_port = 9042
cypher_port = 7687
http_port = 8080
grpc_port = 50051

[protocols.enabled]
postgresql = true
mysql = true
redis = true
cql = true
cypher = true
http = true
grpc = true

[storage]
hot_tier_size = "8GB"
warm_tier_path = "/var/lib/orbit/rocksdb"
cold_tier_bucket = "orbit-cold-storage"

[cluster]
enabled = true
node_id = "node-1"
seeds = ["node-2:7946", "node-3:7946"]

[security]
mtls_enabled = false
cert_file = "/etc/orbit/server.crt"
key_file = "/etc/orbit/server.key"
ca_file = "/etc/orbit/ca.crt"

[logging]
level = "info"
format = "json"
```

### Environment Variables

```bash
ORBIT_CONFIG=/etc/orbit/orbit-server.toml
ORBIT_NODE_ID=node-1
ORBIT_BIND_ADDRESS=0.0.0.0
ORBIT_DATA_DIR=/var/lib/orbit
RUST_LOG=orbit_server=info
```

---

## Cluster Mode

### Cluster Configuration

```toml
[cluster]
enabled = true
node_id = "node-1"
bind_address = "0.0.0.0:7946"
advertise_address = "192.168.1.10:7946"
seeds = ["192.168.1.11:7946", "192.168.1.12:7946"]
consensus = "raft"

[cluster.raft]
election_timeout_ms = 1000
heartbeat_interval_ms = 100
snapshot_threshold = 10000
```

### Starting a Cluster

```bash
# Node 1 (first node)
orbit-server --config node1.toml

# Node 2
orbit-server --config node2.toml --join 192.168.1.10:7946

# Node 3
orbit-server --config node3.toml --join 192.168.1.10:7946
```

### Cluster Operations

```bash
# Check cluster status
orbit-cli cluster status

# Add node
orbit-cli cluster add-node --address 192.168.1.14:7946

# Remove node
orbit-cli cluster remove-node --id node-4

# Rebalance data
orbit-cli cluster rebalance
```

---

## Security

### mTLS Configuration

```toml
[security]
mtls_enabled = true
cert_file = "/etc/orbit/server.crt"
key_file = "/etc/orbit/server.key"
ca_file = "/etc/orbit/ca.crt"
client_cert_required = true
```

### Authentication

```toml
[security.auth]
enabled = true
method = "password"  # password, ldap, oauth2

[security.auth.password]
users_file = "/etc/orbit/users.toml"

[security.auth.ldap]
server = "ldap://ldap.example.com:389"
base_dn = "dc=example,dc=com"
bind_dn = "cn=admin,dc=example,dc=com"
```

### Role-Based Access Control

```sql
-- Create role
CREATE ROLE analyst;

-- Grant permissions
GRANT SELECT ON ALL TABLES IN SCHEMA public TO analyst;
GRANT INSERT, UPDATE ON users TO analyst;

-- Create user with role
CREATE USER alice WITH PASSWORD 'secret' ROLE analyst;
```

---

## Kubernetes Deployment

### Helm Chart

```bash
# Add Helm repository
helm repo add orbit https://charts.orbit-db.io

# Install
helm install orbit-cluster orbit/orbit-rs \
  --set replicaCount=3 \
  --set storage.size=100Gi \
  --set resources.memory=16Gi

# Upgrade
helm upgrade orbit-cluster orbit/orbit-rs --set replicaCount=5
```

### Custom Resource Definitions

```yaml
# OrbitCluster CRD
apiVersion: orbit.io/v1
kind: OrbitCluster
metadata:
  name: production-cluster
spec:
  replicas: 3
  version: "0.5.0"
  storage:
    size: 100Gi
    storageClass: ssd
  resources:
    memory: 16Gi
    cpu: 4
```

### StatefulSet Configuration

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: orbit
spec:
  serviceName: orbit
  replicas: 3
  template:
    spec:
      containers:
      - name: orbit
        image: turingworks/orbit-rs:latest
        ports:
        - containerPort: 5432
        - containerPort: 6379
        - containerPort: 8080
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

## Monitoring

### Prometheus Metrics

```bash
# Scrape endpoint
curl http://localhost:9090/metrics
```

Key metrics:
- `orbit_queries_total` - Total queries processed
- `orbit_query_latency_seconds` - Query latency histogram
- `orbit_connections_active` - Active connections
- `orbit_storage_bytes` - Storage usage per tier
- `orbit_cluster_nodes` - Number of cluster nodes

### Health Checks

```bash
# Liveness
curl http://localhost:8080/health/live

# Readiness
curl http://localhost:8080/health/ready

# Detailed status
curl http://localhost:8080/health/status
```

### Logging

```toml
[logging]
level = "info"           # trace, debug, info, warn, error
format = "json"          # json, text
output = "stdout"        # stdout, file
file_path = "/var/log/orbit/server.log"
rotation = "daily"
retention_days = 30
```

---

## Performance Tuning

### Memory Configuration

```toml
[memory]
hot_tier_size = "8GB"      # In-memory cache
write_buffer_size = "256MB" # Per writer
block_cache_size = "4GB"    # RocksDB block cache
max_connections = 1000      # Connection limit
```

### Connection Pooling

```toml
[connections]
max_connections = 1000
idle_timeout_seconds = 300
connection_timeout_seconds = 30
```

### Query Optimization

```toml
[query]
max_parallel_workers = 8
statement_timeout_seconds = 60
query_cache_size = "1GB"
plan_cache_size = "256MB"
```

---

## Backup and Recovery

### Backup Commands

```bash
# Full backup
orbit-cli backup create \
  --path /backup/$(date +%Y%m%d) \
  --compress

# Incremental backup
orbit-cli backup incremental \
  --base /backup/20250101 \
  --path /backup/20250102

# Upload to S3
orbit-cli backup upload \
  --path /backup/20250101 \
  --destination s3://backup-bucket/orbit/
```

### Recovery

```bash
# List backups
orbit-cli backup list

# Restore from backup
orbit-cli backup restore \
  --path /backup/20250101

# Point-in-time recovery
orbit-cli recovery \
  --to-timestamp "2025-01-01T12:00:00Z"
```

---

## Client Libraries

### Python

```bash
pip install orbit-client
```

```python
from orbit_client import OrbitClient

client = OrbitClient(
    host="localhost",
    port=5432,
    user="orbit",
    password="secret"
)

# Execute query
result = client.query("SELECT * FROM users LIMIT 10")
```

### Node.js

```javascript
const { OrbitClient } = require('orbit-client');

const client = new OrbitClient({
  host: 'localhost',
  port: 5432,
  user: 'orbit',
  password: 'secret'
});

const result = await client.query('SELECT * FROM users LIMIT 10');
```

### Java (Spring Boot)

```java
@Configuration
public class OrbitConfig {
    @Bean
    public DataSource dataSource() {
        return DataSourceBuilder.create()
            .url("jdbc:postgresql://localhost:5432/orbit")
            .username("orbit")
            .password("secret")
            .build();
    }
}
```

---

## Troubleshooting

### Common Issues

**Port already in use**
```bash
lsof -i :5432  # Find process
kill -9 <PID>  # Kill process
```

**Out of memory**
```bash
# Reduce cache sizes
export ORBIT_HOT_TIER_SIZE=4GB
export ORBIT_BLOCK_CACHE_SIZE=2GB
```

**Cluster node unreachable**
```bash
# Check connectivity
orbit-cli cluster ping --node node-2

# Check firewall
sudo ufw allow 7946/tcp
```

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=orbit_server=debug cargo run --bin orbit-server

# Enable all debug logging
RUST_LOG=debug cargo run --bin orbit-server
```

---

## Resources

- **Source**: `orbit/server/`
- **CLI**: `orbit/cli/`
- **Configuration**: `config/orbit-server.toml`
- **Helm Charts**: `helm/orbit-rs/`
