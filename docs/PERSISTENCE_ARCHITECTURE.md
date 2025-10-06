# Orbit Server Provider-Based Persistence Architecture

## Overview

The Orbit server has been upgraded with a comprehensive provider-based persistence system that supports multiple storage backends. This architecture allows you to choose the most appropriate storage solution for your deployment scenario, from simple in-memory storage to cloud-scale object storage systems.

## Architecture Components

### Core Traits

The persistence system is built around several key traits:

#### `PersistenceProvider`
Base trait for all providers with common functionality:
- **Lifecycle Management**: `initialize()`, `shutdown()`
- **Health Monitoring**: `health_check()`, `metrics()`
- **Transaction Support**: `begin_transaction()`, `commit_transaction()`, `rollback_transaction()`

#### `AddressableDirectoryProvider`
Specialized trait for storing addressable (actor) lease information:
- Store, retrieve, update, and remove leases
- List leases by node or globally
- Cleanup expired leases
- Bulk operations for efficiency

#### `ClusterNodeProvider`
Specialized trait for storing cluster node information:
- Store, retrieve, update, and remove node information
- List active and all nodes
- Cleanup expired nodes
- Node lease renewal

### Provider Registry

The `PersistenceProviderRegistry` manages multiple providers and provides:
- Registration of providers with names
- Default provider management
- Provider lookup by name
- Support for different providers for addressable vs. cluster data

## Available Providers

### 1. Memory Provider (`MemoryConfig`)

**Best for**: Development, testing, high-performance scenarios with limited data

**Features**:
- Ultra-fast in-memory storage using concurrent data structures (`DashMap`)
- Optional disk backup with compression support (Gzip, LZ4, Zstd)
- Automatic periodic backup and restore on startup
- Memory usage limits with health monitoring
- Full metrics and transaction support

**Configuration**:
```toml
[providers.memory]
type = "Memory"
max_entries = 100000
disk_backup = { path = "/data/orbit-backup.json", sync_interval = 300, compression = "Gzip" }
```

### 2. S3-Compatible Providers (`S3Config`)

**Best for**: AWS environments, MinIO deployments, high durability requirements

**Features**:
- Compatible with AWS S3, MinIO, and other S3-compatible services
- Configurable endpoints and regions
- SSL/TLS support
- Retry mechanisms and timeout controls
- Automatic health checks
- Object versioning and metadata

**Configuration**:
```toml
[providers.s3]
type = "S3"
endpoint = "https://s3.amazonaws.com"
region = "us-west-2"
bucket = "orbit-data"
access_key_id = "YOUR_ACCESS_KEY"
secret_access_key = "YOUR_SECRET_KEY"
prefix = "orbit"
enable_ssl = true
connection_timeout = 30
retry_count = 3
```

### 3. Azure Blob Storage (`AzureConfig`)

**Best for**: Microsoft Azure environments

**Features**:
- Native Azure Blob Storage integration
- Container-based organization
- Configurable endpoints and retry policies
- Built-in health monitoring

**Configuration**:
```toml
[providers.azure]
type = "Azure"
account_name = "orbitdata"
account_key = "YOUR_ACCOUNT_KEY"
container_name = "orbit"
prefix = "orbit"
connection_timeout = 30
retry_count = 3
```

### 4. Google Cloud Storage (`GoogleCloudConfig`)

**Best for**: Google Cloud Platform environments

**Features**:
- Native GCS integration
- Service account authentication
- Project-based organization
- Configurable retry policies

**Configuration**:
```toml
[providers.gcp]
type = "GoogleCloud"
project_id = "your-project-id"
bucket_name = "orbit-data"
credentials_path = "/path/to/service-account.json"
prefix = "orbit"
connection_timeout = 30
retry_count = 3
```

### 5. etcd Provider (`EtcdConfig`)

**Best for**: Kubernetes environments, distributed systems requiring consistency

**Features**:
- Distributed key-value storage
- Strong consistency guarantees
- Native lease support with TTL
- Watch capabilities for real-time updates
- TLS/mTLS authentication support

**Configuration**:
```toml
[providers.etcd]
type = "Etcd"
endpoints = ["http://localhost:2379", "http://localhost:22379", "http://localhost:32379"]
prefix = "orbit"
lease_ttl = 300
username = "orbit"
password = "secret"
ca_cert = "/path/to/ca.crt"
client_cert = "/path/to/client.crt"
client_key = "/path/to/client.key"
```

### 6. Redis Provider (`RedisConfig`)

**Best for**: High-performance caching, pub/sub scenarios

**Features**:
- In-memory data structure store
- Cluster mode support
- Connection pooling
- Multiple database support
- TTL-based expiration

**Configuration**:
```toml
[providers.redis]
type = "Redis"
url = "redis://localhost:6379"
cluster_mode = false
database = 0
password = "secret"
prefix = "orbit"
pool_size = 10
retry_count = 3
```

### 7. Kubernetes Provider (`KubernetesConfig`)

**Best for**: Cloud-native Kubernetes deployments

**Features**:
- ConfigMap and Secret storage
- Namespace isolation
- Native Kubernetes integration
- RBAC support

**Configuration**:
```toml
[providers.kubernetes]
type = "Kubernetes"
namespace = "orbit"
config_map_name = "orbit-data"
secret_name = "orbit-secrets"
in_cluster = true
```

### 8. MinIO Provider (`MinIOConfig`)

**Best for**: Self-hosted object storage, hybrid cloud scenarios

**Features**:
- S3-compatible self-hosted storage
- High performance and scalability
- Multi-tenant support
- Distributed deployment support

**Configuration**:
```toml
[providers.minio]
type = "MinIO"
endpoint = "http://minio.example.com:9000"
access_key = "minioadmin"
secret_key = "minioadmin"
bucket_name = "orbit"
region = "us-east-1"
secure = false
prefix = "orbit"
```

### 9. Flash-Optimized Provider (`FlashConfig`)

**Best for**: High-performance scenarios with NVMe storage, multipathing requirements

**Features**:
- Optimized for flash storage devices
- Multipathing support for redundancy
- Configurable I/O depth and block sizes
- Compression support
- Direct I/O operations

**Configuration**:
```toml
[providers.flash]
type = "Flash"
data_dir = "/nvme/orbit"
enable_multipathing = true
io_depth = 32
block_size = 4096
cache_size = 1073741824  # 1GB
compression = "Lz4"
paths = ["/nvme0/orbit", "/nvme1/orbit", "/nvme2/orbit"]
```

### 10. Composite Provider (`CompositeConfig`)

**Best for**: High availability scenarios requiring failover

**Features**:
- Primary/backup provider configuration
- Automatic health monitoring
- Configurable failover thresholds
- Data synchronization between providers
- Transparent failover

**Configuration**:
```toml
[providers.composite]
type = "Composite"
sync_interval = 60
health_check_interval = 30
failover_threshold = 3

[providers.composite.primary]
type = "Memory"
max_entries = 50000

[providers.composite.backup]
type = "S3"
endpoint = "http://localhost:9000"
region = "us-east-1"
bucket = "orbit-backup"
access_key_id = "minioadmin"
secret_access_key = "minioadmin"
enable_ssl = false
```

## Configuration Methods

### 1. Programmatic Configuration

```rust
use orbit_server::persistence::*;

let config = PersistenceProviderConfig::builder()
    .with_memory(\"memory\", MemoryConfig::default(), false)
    .with_s3(\"s3\", s3_config, true)  // Default provider
    .build()?;
```

### 2. Configuration Files

#### TOML Format
```toml
[defaults]
addressable = \"s3\"
cluster = \"etcd\"

[providers.memory]
type = \"Memory\"
max_entries = 100000

[providers.s3]
type = \"S3\"
endpoint = \"https://s3.amazonaws.com\"
region = \"us-west-2\"
bucket = \"orbit-data\"
# ... other S3 config
```

#### JSON Format
```json
{
  \"defaults\": {
    \"addressable\": \"s3\",
    \"cluster\": \"etcd\"
  },
  \"providers\": {
    \"memory\": {
      \"type\": \"Memory\",
      \"max_entries\": 100000
    },
    \"s3\": {
      \"type\": \"S3\",
      \"endpoint\": \"https://s3.amazonaws.com\",
      \"region\": \"us-west-2\",
      \"bucket\": \"orbit-data\"
    }
  }
}
```

### 3. Environment Variables

```rust
// S3 configuration from environment
let s3_config = EnvironmentConfig::s3_from_env(\"ORBIT_S3\")?;
```

Environment variables:
```bash
ORBIT_S3_ENDPOINT=https://s3.amazonaws.com
ORBIT_S3_REGION=us-west-2
ORBIT_S3_BUCKET=orbit-data
ORBIT_S3_ACCESS_KEY_ID=your_access_key
ORBIT_S3_SECRET_ACCESS_KEY=your_secret_key
```

## Integration with Orbit Server

### Server Configuration

```rust
use orbit_server::*;

let persistence_config = PersistenceProviderConfig::from_file(\"config.toml\").await?;

let server = OrbitServer::builder()
    .with_namespace(\"production\")
    .with_port(50051)
    .with_persistence(persistence_config)
    .build()
    .await?;

server.start().await?;
```

### Usage Examples

#### Basic Usage
```rust
// Get the registry from server
let registry = server.persistence_registry();

// Use default providers
let addressable_provider = registry.get_default_addressable_provider().await?;
let cluster_provider = registry.get_default_cluster_provider().await?;

// Store an addressable lease
let lease = AddressableLease { /* ... */ };
addressable_provider.store_lease(&lease).await?;

// Store node information
let node = NodeInfo { /* ... */ };
cluster_provider.store_node(&node).await?;
```

#### Advanced Usage
```rust
// Use specific providers
let s3_provider = registry.get_addressable_provider(\"s3_addressable\").await?;
let etcd_provider = registry.get_cluster_provider(\"etcd_cluster\").await?;

// Bulk operations
let leases = vec![lease1, lease2, lease3];
s3_provider.store_leases_bulk(&leases).await?;

// Health monitoring
let health = s3_provider.health_check().await;
let metrics = s3_provider.metrics().await;
```

## Deployment Scenarios

### Scenario 1: Development Environment
**Configuration**: Memory provider with disk backup
- Fast development cycles
- Data persistence across restarts
- No external dependencies

### Scenario 2: Production on AWS
**Configuration**: S3 for persistence, ElastiCache Redis for caching
- High durability and availability
- Scalable storage
- Cross-region replication

### Scenario 3: Kubernetes Deployment
**Configuration**: etcd for cluster state, ConfigMaps for configuration
- Cloud-native integration
- Automatic service discovery
- RBAC integration

### Scenario 4: Hybrid Cloud
**Configuration**: Composite provider with on-premises primary and cloud backup
- Data sovereignty requirements
- Disaster recovery
- Cost optimization

### Scenario 5: High-Performance Computing
**Configuration**: Flash-optimized storage with multipathing
- Ultra-low latency
- High throughput
- Hardware optimization

## Migration Between Providers

The system includes migration utilities for moving data between providers:

```rust
use orbit_server::persistence::migration::*;

let migration = DataMigration::new()
    .from_provider(old_provider)
    .to_provider(new_provider)
    .with_batch_size(1000)
    .with_validation(true);

let result = migration.execute().await?;
println!(\"Migrated {} leases and {} nodes\", result.leases_migrated, result.nodes_migrated);
```

## Monitoring and Metrics

All providers expose standard metrics:

- **Operation Counts**: read_operations, write_operations, delete_operations
- **Latency**: read_latency_avg, write_latency_avg, delete_latency_avg  
- **Error Tracking**: error_count
- **Cache Statistics**: cache_hits, cache_misses (where applicable)

Health checks provide three states:
- **Healthy**: Provider is operating normally
- **Degraded**: Provider is operational but with reduced performance
- **Unhealthy**: Provider is not operational

## Security Considerations

### Authentication
- **Cloud Providers**: Use native authentication (IAM roles, service accounts)
- **etcd**: Support for username/password and mTLS
- **Redis**: Password authentication and SSL/TLS
- **Kubernetes**: RBAC and service account tokens

### Encryption
- **At Rest**: Leverage provider-native encryption (S3 SSE, Azure encryption, etc.)
- **In Transit**: TLS/SSL support for all network communications
- **Key Management**: Integration with cloud KMS services

### Access Control
- **Least Privilege**: Configure minimal required permissions
- **Network Security**: Use VPCs, security groups, network policies
- **Audit Logging**: All operations are logged with appropriate detail

## Performance Tuning

### General Guidelines
1. **Choose the Right Provider**: Match provider capabilities to your use case
2. **Configure Timeouts**: Set appropriate connection and operation timeouts
3. **Enable Compression**: Use compression for large objects (configurable algorithms)
4. **Batch Operations**: Use bulk operations when possible
5. **Monitor Metrics**: Track latency and error rates

### Provider-Specific Tuning
- **Memory**: Adjust max_entries and backup intervals
- **S3**: Configure retry counts and timeout values
- **etcd**: Tune lease TTL and watch timeout
- **Redis**: Optimize connection pool size and pipeline operations

## Future Enhancements

The provider-based architecture is designed for extensibility. Future providers might include:

- **Apache Cassandra**: For large-scale distributed scenarios
- **CockroachDB**: For global, consistent storage
- **TiKV**: For cloud-native distributed storage
- **Custom Providers**: Plugin system for proprietary storage systems

## Conclusion

The provider-based persistence architecture transforms Orbit server from a simple in-memory system to a production-ready platform that can adapt to any deployment scenario. Whether you're running a single development instance or a global distributed system, there's a provider configuration that meets your needs.

The system's flexibility, combined with comprehensive monitoring, security features, and migration capabilities, ensures that your Orbit deployment can evolve with your requirements without architectural changes.