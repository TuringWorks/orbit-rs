# Orbit Server Configuration Reference

Complete reference for all configuration settings in `config/orbit-server.toml`.

## Table of Contents

- [Quick Start](#quick-start)
- [Server Configuration](#server-configuration)
- [Actor System Configuration](#actor-system-configuration)
- [Protocol Servers](#protocol-servers)
  - [gRPC](#grpc-protocol)
  - [PostgreSQL](#postgresql-protocol)
  - [MySQL](#mysql-protocol)
  - [CQL (Cassandra)](#cql-cassandra-protocol)
  - [Redis](#redis-protocol)
  - [REST API](#rest-api-protocol)
  - [Cypher (Neo4j)](#cypher-neo4j-protocol)
  - [OrbitQL](#orbitql-protocol)
  - [AQL (ArangoDB)](#aql-arangodb-protocol)
- [Security Configuration](#security-configuration)
- [Performance Tuning](#performance-tuning)
- [Logging Configuration](#logging-configuration)
- [Monitoring and Metrics](#monitoring-and-metrics)
- [Connection Pooling](#connection-pooling)
- [Environment-Specific Settings](#environment-specific-settings)

---

## Quick Start

The default configuration enables all protocols with zero-configuration deployment:

```bash
# Simply run the server - no flags needed
cargo run --bin orbit-server

# Or specify a custom config file
cargo run --bin orbit-server -- --config /path/to/config.toml
```

All protocols are enabled by default and listen on their standard ports.

---

## Server Configuration

Core server settings that affect overall system behavior.

```toml
[server]
node_id = "orbit-node-1"
bind_address = "0.0.0.0"
environment = "Production"
data_dir = "./data"
config_dir = "./config"
```

### Settings

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `node_id` | String | `"orbit-node-1"` | Unique identifier for this server node in a cluster | **Cluster**: Must be unique across all nodes. Used for distributed coordination and logging. |
| `bind_address` | IP Address | `"0.0.0.0"` | IP address to bind protocol servers to | **Network**: `0.0.0.0` = all interfaces, `127.0.0.1` = localhost only. Affects accessibility. |
| `environment` | String | `"Production"` | Environment name for logging/metrics | **Observability**: Used in logs, metrics tags, and monitoring dashboards. Common values: `Production`, `Staging`, `Development`. |
| `data_dir` | Path | `"./data"` | Directory for persistent data storage | **Storage**: Where database files, WAL logs, and snapshots are stored. Must have sufficient disk space. |
| `config_dir` | Path | `"./config"` | Directory for configuration files | **Configuration**: Location of additional config files like TLS certificates, key files. |

**Performance Impact:**
- `bind_address`: Binding to specific interface can reduce network overhead
- `data_dir`: SSD vs HDD dramatically affects I/O performance
- `environment`: No direct performance impact, affects observability only

---

## Actor System Configuration

Controls the actor runtime that manages concurrent operations.

```toml
[actor_system]
max_actors = 10000
mailbox_size = 1000
supervision_strategy = "OneForOne"
```

### Settings

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `max_actors` | Integer | `10000` | Maximum number of concurrent actors | **Concurrency**: Higher = more concurrent requests but more memory. Each actor ~1-10KB. For 10K actors, expect ~10-100MB memory. |
| `mailbox_size` | Integer | `1000` | Message queue size per actor | **Throughput**: Higher = more buffering during spikes but more memory. 1000 msgs × 10K actors = ~10M message capacity. |
| `supervision_strategy` | String | `"OneForOne"` | Actor failure recovery strategy | **Reliability**: `OneForOne` = restart failed actor only. `OneForAll` = restart all actors. `RestForOne` = restart failed + later actors. |

**Performance Impact:**
- **max_actors**: Too low = request queuing/rejection. Too high = memory pressure.
- **mailbox_size**: Too low = backpressure on producers. Too high = memory bloat, slow shutdown.
- **Supervision**: `OneForOne` = minimal blast radius. `OneForAll` = service disruption on failure.

**Tuning Guidelines:**
- **Low-traffic** (< 100 req/s): `max_actors = 1000`, `mailbox_size = 100`
- **Medium-traffic** (100-1000 req/s): `max_actors = 5000`, `mailbox_size = 500` (default ~)
- **High-traffic** (> 1000 req/s): `max_actors = 20000`, `mailbox_size = 2000`

---

## Protocol Servers

Each protocol server can be independently configured and enabled/disabled.

### gRPC Protocol

High-performance RPC protocol for actor system management and internal APIs.

```toml
[protocols.grpc]
enabled = true
port = 50051
max_concurrent_streams = 1000
max_message_size = 4194304  # 4MB
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable gRPC server | **Availability**: Disable if not using gRPC clients. Saves ~50-100MB memory. |
| `port` | Integer | `50051` | TCP port for gRPC | **Network**: Standard gRPC port. Must not conflict with other services. |
| `max_concurrent_streams` | Integer | `1000` | HTTP/2 concurrent streams per connection | **Concurrency**: Higher = more parallel requests per client but more memory. Each stream ~10KB. |
| `max_message_size` | Integer | `4194304` | Maximum message size in bytes (4MB) | **Payload**: Limits request/response size. Too low = large requests fail. Too high = DoS risk. |

**Performance Impact:**
- **max_concurrent_streams**: 1000 streams × 10KB = ~10MB per connection
- **max_message_size**: Affects memory allocation per request

**Use Cases:**
- Actor system management (create/delete/query actors)
- Internal service-to-service communication
- High-performance bulk operations

---

### PostgreSQL Protocol

Full PostgreSQL wire protocol compatibility with pgvector support.

```toml
[protocols.postgresql]
enabled = true
port = 5432
max_connections = 1000
connection_timeout_secs = 30
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable PostgreSQL server | **Availability**: Most widely used protocol. Disable only if PostgreSQL access not needed. |
| `port` | Integer | `5432` | TCP port for PostgreSQL | **Network**: Standard PostgreSQL port. Use different port if running alongside PostgreSQL. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~1-5MB. 1000 conns = ~1-5GB memory. |
| `connection_timeout_secs` | Integer | `30` | Connection idle timeout | **Resources**: Lower = faster cleanup. Higher = fewer reconnects. |

#### SQL Engine Configuration

```toml
[protocols.postgresql.sql_engine]
max_query_complexity = 1000
query_timeout_secs = 30
enable_optimization = true
enable_caching = true
cache_size_mb = 256
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `max_query_complexity` | Integer | `1000` | Maximum query plan nodes | **Security**: Prevents extremely complex queries. Too low = legitimate queries fail. |
| `query_timeout_secs` | Integer | `30` | Query execution timeout | **Performance**: Prevents runaway queries. Too low = slow queries fail. |
| `enable_optimization` | Boolean | `true` | Enable query optimization | **Performance**: 2-10x speedup for complex queries. Minimal overhead. Always enable. |
| `enable_caching` | Boolean | `true` | Enable query plan caching | **Performance**: Avoids re-planning identical queries. ~10-50ms saved per cached query. |
| `cache_size_mb` | Integer | `256` | Query plan cache size | **Memory**: 256MB = ~10K-50K cached plans. Higher = more cache hits. |

**Performance Impact:**
- **Query optimization**: Always enable unless debugging
- **Query caching**: 80-95% cache hit rate typical = 10-50ms saved per query
- **cache_size_mb**: Hit rate plateaus around 256MB for most workloads

#### Vector Operations Configuration

```toml
[protocols.postgresql.vector_ops]
default_metric = "cosine"
max_dimensions = 4096
batch_size = 1000
enable_simd = true
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `default_metric` | String | `"cosine"` | Default distance metric | **Accuracy**: `cosine`, `euclidean`, `dot_product`. Affects similarity search results. |
| `max_dimensions` | Integer | `4096` | Maximum vector dimensions | **Capability**: Modern embeddings (OpenAI ada-002: 1536, text-embedding-3-large: 3072). Must support your model. |
| `batch_size` | Integer | `1000` | Vectors processed per batch | **Performance**: Higher = better throughput but more memory. 1000 × 1536 dims × 4 bytes = ~6MB per batch. |
| `enable_simd` | Boolean | `true` | Enable SIMD acceleration | **Performance**: 4-8x speedup on vector operations. Always enable on modern CPUs. |

**Performance Impact:**
- **SIMD enabled**: 4-8x faster vector distance calculations
- **batch_size**: Sweet spot 500-2000 depending on vector dimensions
  - Small vectors (128-384 dims): batch_size = 2000
  - Medium vectors (512-1536 dims): batch_size = 1000
  - Large vectors (2048-4096 dims): batch_size = 500

#### PostgreSQL Features

```toml
[protocols.postgresql.features]
enable_pgvector = true
enable_extensions = true
enable_full_text_search = true
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enable_pgvector` | Boolean | `true` | Enable pgvector compatibility | **Capability**: Vector similarity search. Disable if not using embeddings. Saves ~100MB. |
| `enable_extensions` | Boolean | `true` | Enable PostgreSQL extensions | **Capability**: Additional SQL functions. Minimal overhead. |
| `enable_full_text_search` | Boolean | `true` | Enable full-text search | **Capability**: Text search capabilities. ~50MB memory for indices. |

---

### MySQL Protocol

MySQL wire protocol compatibility for ecosystem integration.

```toml
[protocols.mysql]
enabled = true
port = 3306
max_connections = 1000
connection_timeout_secs = 30
server_version = "8.0.0-Orbit"
authentication_enabled = false
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable MySQL server | **Availability**: For MySQL client/tool compatibility. |
| `port` | Integer | `3306` | TCP port for MySQL | **Network**: Standard MySQL port. Change if running alongside MySQL. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~1-3MB. |
| `connection_timeout_secs` | Integer | `30` | Connection idle timeout | **Resources**: Balance reconnect cost vs resource usage. |
| `server_version` | String | `"8.0.0-Orbit"` | Version string reported to clients | **Compatibility**: Some tools check version. Use MySQL 8.0 format. |
| `authentication_enabled` | Boolean | `false` | Require authentication | **Security**: Enable for production. Disable for development. |

**Use Cases:**
- Integration with MySQL-based applications
- MySQL GUI tools (MySQL Workbench, DBeaver)
- ORMs with MySQL support

---

### CQL (Cassandra) Protocol

Cassandra Query Language for wide-column access patterns.

```toml
[protocols.cql]
enabled = true
port = 9042
max_connections = 1000
connection_timeout_secs = 30
protocol_version = 4
authentication_enabled = false
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable CQL server | **Availability**: For Cassandra ecosystem integration. |
| `port` | Integer | `9042` | TCP port for CQL | **Network**: Standard Cassandra port. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~2-5MB. |
| `connection_timeout_secs` | Integer | `30` | Connection idle timeout | **Resources**: CQL sessions are stateful. |
| `protocol_version` | Integer | `4` | CQL protocol version | **Compatibility**: Version 4 = Cassandra 2.1+. Most widely supported. |
| `authentication_enabled` | Boolean | `false` | Require authentication | **Security**: Enable for production. |

**Use Cases:**
- Time-series data ingestion
- Wide-column data models
- Cassandra driver compatibility
- High-throughput write workloads

---

### Redis Protocol

RESP (Redis Serialization Protocol) for key-value operations.

```toml
[protocols.redis]
enabled = true
port = 6379
max_connections = 1000
connection_timeout_secs = 30
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable Redis server | **Availability**: For Redis client/tool compatibility. |
| `port` | Integer | `6379` | TCP port for Redis | **Network**: Standard Redis port. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~100KB-1MB. |
| `connection_timeout_secs` | Integer | `30` | Connection idle timeout | **Resources**: Redis clients often use connection pools. |

**Performance Notes:**
- Redis protocol is extremely lightweight (~100KB per connection vs 1-5MB for SQL protocols)
- Supports pipelining for batch operations
- Ideal for caching, session storage, rate limiting

**Use Cases:**
- Caching layer
- Session storage
- Rate limiting
- Real-time analytics
- Pub/sub messaging

---

### REST API Protocol

HTTP/JSON API for web applications and general-purpose access.

```toml
[protocols.rest]
enabled = true
port = 8080
max_connections = 1000
request_timeout_secs = 30
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable REST API server | **Availability**: HTTP/JSON interface. |
| `port` | Integer | `8080` | TCP port for REST API | **Network**: Standard HTTP alternate port. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~500KB-2MB. |
| `request_timeout_secs` | Integer | `30` | HTTP request timeout | **Performance**: Prevents slow clients from holding connections. |

**Endpoints:**
- `GET /tables` - List tables
- `GET /tables/{table}/rows` - Query rows
- `POST /tables/{table}/rows` - Insert rows
- `PUT /tables/{table}/rows/{id}` - Update row
- `DELETE /tables/{table}/rows/{id}` - Delete row
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics

**Use Cases:**
- Web application integration
- Microservices architecture
- Serverless functions
- Mobile applications
- Third-party integrations

---

### Cypher (Neo4j) Protocol

Neo4j Bolt protocol for graph queries.

```toml
[protocols.cypher]
enabled = true
port = 7687
max_connections = 1000
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable Cypher/Bolt server | **Availability**: For Neo4j client/tool compatibility. |
| `port` | Integer | `7687` | TCP port for Bolt | **Network**: Standard Neo4j Bolt port. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~2-5MB. |

**Use Cases:**
- Graph traversal queries
- Relationship analysis
- Social network graphs
- Knowledge graphs
- Neo4j driver compatibility

---

### OrbitQL Protocol

Unified multi-model query language combining document, graph, time-series, and key-value operations.

```toml
[protocols.orbitql]
enabled = true
port = 8081
max_connections = 1000
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable OrbitQL server | **Availability**: Native Orbit query language. |
| `port` | Integer | `8081` | TCP port for OrbitQL | **Network**: Custom protocol port. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~1-3MB. |

**Features:**
- SQL-like syntax with multi-model extensions
- Document queries: `SELECT * FROM users WHERE metadata.tags CONTAINS 'vip'`
- Graph traversal: `MATCH (a)-[r]->(b) WHERE a.type = 'User'`
- Time-series: `SELECT * FROM metrics WHERE timestamp > NOW() - INTERVAL '1 hour'`
- Vector search: `SELECT * FROM embeddings ORDER BY embedding <-> $query_vector LIMIT 10`

**Use Cases:**
- Multi-model applications
- Complex analytical queries
- GraphRAG applications
- Unified data access layer

---

### AQL (ArangoDB) Protocol

ArangoDB Query Language for multi-model databases.

```toml
[protocols.aql]
enabled = true
port = 8529
max_connections = 1000
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable AQL server | **Availability**: For ArangoDB ecosystem integration. |
| `port` | Integer | `8529` | TCP port for AQL | **Network**: Standard ArangoDB port. |
| `max_connections` | Integer | `1000` | Maximum concurrent connections | **Concurrency**: Each connection ~2-5MB. |

**Use Cases:**
- ArangoDB driver compatibility
- Multi-model queries (document + graph)
- Migration from ArangoDB
- Graph analytics with document storage

---

## Security Configuration

Authentication, authorization, and encryption settings.

```toml
[security.authentication]
enabled = false
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `false` | Require authentication | **Security**: Enable for production. Adds ~1-5ms latency per request for auth check. |

**Production Recommendation:**
```toml
[security.authentication]
enabled = true

[security.jwt]
secret = "${JWT_SECRET}"  # From environment variable
algorithm = "HS256"
expiration_hours = 24

[security.tls]
enabled = true
cert_file = "/path/to/cert.pem"
key_file = "/path/to/key.pem"
```

**Security Impact:**
- Authentication disabled = Anyone can access data (development only!)
- TLS disabled = Credentials/data transmitted in plaintext
- JWT tokens = Stateless authentication, ~1ms overhead

---

## Performance Tuning

System-wide performance configuration.

```toml
[performance]
worker_threads = 0  # 0 = auto-detect (num_cpus)
max_blocking_threads = 512
thread_stack_size_mb = 2
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `worker_threads` | Integer | `0` | Number of async worker threads | **Concurrency**: 0 = CPU count. Higher = more parallelism but context switching overhead. |
| `max_blocking_threads` | Integer | `512` | Maximum blocking task threads | **I/O**: For file I/O, DNS lookups. Too low = blocking tasks queued. Too high = thread thrashing. |
| `thread_stack_size_mb` | Integer | `2` | Stack size per thread | **Memory**: 2MB × (worker + blocking threads). Lower = stack overflow risk. Higher = wasted memory. |

**Tuning Guidelines:**

**CPU-bound workloads** (heavy computation, vector operations):
```toml
worker_threads = 0  # Use all CPU cores
max_blocking_threads = 64  # Minimal I/O
```

**I/O-bound workloads** (high disk I/O, network calls):
```toml
worker_threads = 0  # CPU cores
max_blocking_threads = 1024  # More I/O threads
```

**Mixed workloads** (default):
```toml
worker_threads = 0
max_blocking_threads = 512
```

**Performance Impact:**
- **worker_threads**: Sweet spot = CPU count. More doesn't help CPU-bound tasks.
- **max_blocking_threads**: Too low = I/O stalls. Too high = memory waste (512 × 2MB = 1GB).
- **thread_stack_size_mb**: 2MB is safe. 1MB risky for deep recursion. 4MB+ wasteful.

---

## Logging Configuration

Logging format, level, and output settings.

```toml
[logging]
level = "info"
format = "json"
output = "stdout"
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `level` | String | `"info"` | Minimum log level | **Verbosity**: `trace` > `debug` > `info` > `warn` > `error`. Lower = more logs. |
| `format` | String | `"json"` | Log format | **Parsing**: `json` = structured (for log aggregators). `text` = human-readable. |
| `output` | String | `"stdout"` | Log destination | **Integration**: `stdout` = container logs. `file` = disk. `syslog` = system logger. |

**Log Levels:**
- `trace`: Every function call (~10MB/min on busy server)
- `debug`: Detailed diagnostics (~1MB/min)
- `info`: Important events (default, ~100KB/min)
- `warn`: Warnings and recoverable errors
- `error`: Errors requiring attention

**Performance Impact:**
- `trace`/`debug`: 5-15% overhead due to volume
- `info`: <1% overhead
- `warn`/`error`: Negligible overhead

**Production Recommendation:**
```toml
[logging]
level = "info"  # or "warn" for high-traffic
format = "json"  # For log aggregation (ELK, Datadog, etc.)
output = "stdout"  # Container-friendly
```

---

## Monitoring and Metrics

Observability and health check configuration.

```toml
[monitoring.metrics]
enabled = true
port = 9090
path = "/metrics"

[monitoring.health_checks]
enabled = true
port = 9091
path = "/health"
```

### Metrics Configuration

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable Prometheus metrics | **Observability**: Essential for production monitoring. ~5MB memory + <1% CPU. |
| `port` | Integer | `9090` | Metrics HTTP port | **Network**: Prometheus scrape endpoint. |
| `path` | String | `"/metrics"` | Metrics URL path | **Convention**: Standard Prometheus path. |

**Exposed Metrics:**
- Request latency histograms (p50, p95, p99)
- Request rate by protocol
- Connection pool utilization
- Query execution time
- Memory usage
- CPU utilization
- Error rates
- Cache hit rates

**Integration Example:**
```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'orbit'
    static_configs:
      - targets: ['localhost:9090']
```

### Health Checks Configuration

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable health checks | **Reliability**: For load balancers, orchestrators (K8s). Negligible overhead. |
| `port` | Integer | `9091` | Health check HTTP port | **Network**: Separate from metrics to isolate concerns. |
| `path` | String | `"/health"` | Health check URL path | **Convention**: Standard health check path. |

**Health Check Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "protocols": {
    "postgresql": "up",
    "mysql": "up",
    "redis": "up"
  }
}
```

**Kubernetes Integration:**
```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 9091
  initialDelaySeconds: 10
  periodSeconds: 30

readinessProbe:
  httpGet:
    path: /health
    port: 9091
  initialDelaySeconds: 5
  periodSeconds: 10
```

---

## Connection Pooling

Advanced connection pool management with circuit breaker.

```toml
[pooling]
enabled = true
min_connections = 10
max_connections = 1000
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 3600
health_check_interval_secs = 60
load_balancing_strategy = "RoundRobin"
tier = "Standard"
enable_dynamic_sizing = true
target_utilization = 0.75
```

### Pool Settings

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable connection pooling | **Performance**: Essential for production. Reuses connections = 10-100ms saved per request. |
| `min_connections` | Integer | `10` | Minimum pool size | **Latency**: Pre-warmed connections. Too high = wasted resources. Too low = cold start delay. |
| `max_connections` | Integer | `1000` | Maximum pool size | **Capacity**: Hard limit. Too low = request queuing. Too high = database overload. |
| `connection_timeout_secs` | Integer | `30` | Time to acquire connection | **Reliability**: How long to wait for available connection. |
| `idle_timeout_secs` | Integer | `300` | Idle connection timeout | **Resources**: Close unused connections. 5 min = good balance. |
| `max_lifetime_secs` | Integer | `3600` | Max connection lifetime | **Reliability**: Prevent stale connections. 1 hour = safe default. |
| `health_check_interval_secs` | Integer | `60` | Connection health check frequency | **Reliability**: Detect dead connections. 1 min = good balance. |
| `load_balancing_strategy` | String | `"RoundRobin"` | Connection selection strategy | **Performance**: `RoundRobin`, `LeastConnections`, `Random`. |
| `tier` | String | `"Standard"` | Pool tier | **Capacity**: `Basic`, `Standard`, `Premium`. Affects sizing. |
| `enable_dynamic_sizing` | Boolean | `true` | Auto-scale pool size | **Efficiency**: Adapts to load. Disable for predictable workloads. |
| `target_utilization` | Float | `0.75` | Target pool utilization | **Efficiency**: 75% = good headroom. Higher = less overhead. Lower = more buffer. |

### Circuit Breaker

```toml
[pooling.circuit_breaker]
enabled = true
failure_threshold = 5
failure_window_secs = 60
recovery_timeout_secs = 30
success_threshold = 2
half_open_max_calls = 3
```

| Setting | Type | Default | Description | Impact |
|---------|------|---------|-------------|--------|
| `enabled` | Boolean | `true` | Enable circuit breaker | **Reliability**: Prevents cascading failures. Essential for production. |
| `failure_threshold` | Integer | `5` | Failures before opening circuit | **Sensitivity**: Lower = trip faster. Higher = more tolerant. |
| `failure_window_secs` | Integer | `60` | Time window for failure counting | **Sensitivity**: 1 minute window. Shorter = more aggressive. |
| `recovery_timeout_secs` | Integer | `30` | Time before retry in half-open state | **Recovery**: How long to wait before testing recovery. |
| `success_threshold` | Integer | `2` | Successes to close circuit | **Recovery**: Consecutive successes needed. Higher = more conservative. |
| `half_open_max_calls` | Integer | `3` | Test requests in half-open state | **Recovery**: Limited probes during recovery testing. |

**Circuit Breaker States:**
1. **Closed**: Normal operation, requests flow through
2. **Open**: Circuit tripped, requests fail fast (no database calls)
3. **Half-Open**: Testing recovery, limited requests allowed

**Performance Impact:**
- **Enabled**: <1% overhead, prevents cascading failures worth 100x cost
- **Failure detection**: Fails fast in ~1ms vs waiting for 30s timeout
- **Recovery**: Gradual recovery prevents thundering herd

**Tuning Guidelines:**

**High-traffic, stable database** (low failure rate):
```toml
failure_threshold = 10  # More tolerant
failure_window_secs = 120  # Longer window
recovery_timeout_secs = 60  # Conservative recovery
```

**Unstable database** (frequent failures):
```toml
failure_threshold = 3  # Trip quickly
failure_window_secs = 30  # Short window
recovery_timeout_secs = 10  # Fast recovery attempts
```

**Default** (balanced):
```toml
failure_threshold = 5
failure_window_secs = 60
recovery_timeout_secs = 30
```

---

## Environment-Specific Settings

### Development

```toml
[server]
environment = "Development"

[security.authentication]
enabled = false  # For ease of development

[logging]
level = "debug"  # Detailed logs
format = "text"  # Human-readable

[performance]
worker_threads = 2  # Limit resource usage
max_blocking_threads = 64
```

### Staging

```toml
[server]
environment = "Staging"

[security.authentication]
enabled = true  # Test production auth

[logging]
level = "info"
format = "json"  # Match production

[performance]
worker_threads = 0  # Use all cores
max_blocking_threads = 256
```

### Production

```toml
[server]
environment = "Production"
bind_address = "0.0.0.0"

[security.authentication]
enabled = true

[security.tls]
enabled = true
cert_file = "/etc/orbit/certs/server.crt"
key_file = "/etc/orbit/certs/server.key"

[logging]
level = "info"  # or "warn" for high-traffic
format = "json"
output = "stdout"

[performance]
worker_threads = 0  # Auto-detect CPUs
max_blocking_threads = 512

[pooling]
enabled = true
min_connections = 50  # Pre-warm for traffic
max_connections = 2000
enable_dynamic_sizing = true

[pooling.circuit_breaker]
enabled = true
```

---

## Performance Benchmarks

Expected performance characteristics with default configuration on modern hardware (8-core CPU, 32GB RAM, NVMe SSD):

| Metric | Value | Notes |
|--------|-------|-------|
| **Throughput** | 50,000-100,000 req/s | Simple queries, connection pooling |
| **Latency (p50)** | 1-5ms | Cached queries |
| **Latency (p95)** | 5-15ms | Uncached queries |
| **Latency (p99)** | 15-50ms | Complex queries |
| **Connections** | 10,000+ | With connection pooling |
| **Vector Search** | 1,000-10,000 vec/s | SIMD enabled, 1536 dims |
| **Memory** | 2-8GB | Depends on active connections |
| **CPU Usage** | 20-60% | Under load |

**Scaling:**
- Vertical: Linear to ~32 cores, sub-linear beyond
- Horizontal: Linear with proper load balancing
- Storage: Limited by disk I/O (NVMe recommended)

---

## Troubleshooting

### High Memory Usage

1. Check connection counts: `max_connections` across all protocols
2. Reduce `actor_system.mailbox_size` if message queuing is high
3. Lower `cache_size_mb` for SQL engine
4. Reduce `max_blocking_threads` if many idle threads

### High Latency

1. Enable `enable_optimization = true` and `enable_caching = true`
2. Increase `cache_size_mb` for better query plan cache hit rate
3. Enable `enable_simd = true` for vector operations
4. Check `worker_threads` = CPU count (not higher)
5. Review slow query logs (`logging.level = "debug"`)

### Connection Exhaustion

1. Increase `max_connections` per protocol
2. Lower `connection_timeout_secs` to free idle connections faster
3. Enable `pooling.enable_dynamic_sizing`
4. Check for connection leaks in client applications

### Circuit Breaker Tripping

1. Check database health
2. Increase `failure_threshold` if database has transient errors
3. Increase `failure_window_secs` for more tolerance
4. Review error logs for root cause

---

## Best Practices

1. **Always enable connection pooling** in production
2. **Always enable circuit breaker** to prevent cascading failures
3. **Use JSON logging** for production (structured logs for aggregation)
4. **Enable metrics** for observability (Prometheus/Grafana)
5. **Enable TLS** in production (never transmit credentials in plaintext)
6. **Use environment variables** for secrets (never commit to git)
7. **Set query timeouts** to prevent runaway queries
8. **Monitor resource usage** (CPU, memory, connections) continuously
9. **Test configuration changes** in staging before production
10. **Keep min_connections low** in development to save resources

---

## See Also

- [Deployment Guide](./DEPLOYMENT.md)
- [Security Best Practices](../security/SECURITY.md)
- [Performance Tuning](../performance/TUNING.md)
- [Monitoring Setup](../monitoring/SETUP.md)
