# Native Multi-Protocol Database Server

Orbit-RS provides native support for multiple database protocols in a single server, eliminating the need for separate PostgreSQL and Redis instances while maintaining full compatibility with their respective protocols and ecosystems.

##  Overview

The Orbit-RS server natively implements:

- **PostgreSQL Wire Protocol** (port 5432) - Full SQL compatibility with pgvector support
- **Redis RESP Protocol** (port 6379) - Key-value operations with vector search  
- **gRPC API** (port 50051) - Actor system management
- **HTTP REST API** (port 8080) - Web-friendly interface

All protocols share the same underlying data store and actor system, providing unprecedented consistency and performance across different interfaces.

##  Quick Start

### Start the Multi-Protocol Server

```bash
# Development mode - all protocols enabled
orbit-server --dev-mode

# Production mode with configuration file
orbit-server --config ./config/orbit-server.toml

# Enable specific protocols
orbit-server --enable-postgresql --enable-redis --enable-rest
```

### Connect with Standard Clients

```bash
# PostgreSQL - use any PostgreSQL client
psql -h localhost -p 5432 -U postgres -d orbit

# Redis - use redis-cli or any Redis client
redis-cli -h localhost -p 6379

# HTTP REST - use curl or any HTTP client
curl http://localhost:8080/health

# gRPC - use any gRPC client
# (actor system management API)
```

##  Configuration

Create a configuration file at `./config/orbit-server.toml`:

```toml
[server]
bind_address = "0.0.0.0"
environment = "Development"

[protocols.postgresql]
enabled = true
port = 5432
max_connections = 1000

[protocols.redis]  
enabled = true
port = 6379
max_connections = 1000

[protocols.rest]
enabled = true
port = 8080
max_connections = 1000

[protocols.grpc]
enabled = true
port = 50051
```

Generate an example configuration:

```bash
orbit-server --generate-config > orbit-server.toml
```

##  PostgreSQL Compatibility

### Full SQL Support

Orbit-RS implements the PostgreSQL wire protocol with comprehensive SQL support:

```sql
-- Standard SQL operations
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255),
    category TEXT,
    price DECIMAL(10,2)
);

INSERT INTO products (name, category, price) 
VALUES ('Laptop', 'Electronics', 999.99);

SELECT * FROM products WHERE category = 'Electronics';
```

### pgvector Extension Support

Native support for vector operations compatible with pgvector:

```sql
-- Enable vector extension
CREATE EXTENSION vector;

-- Create table with vector column
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding VECTOR(384)  -- 384-dimensional vectors
);

-- Create vector index
CREATE INDEX ON documents USING ivfflat (embedding vector_cosine_ops);

-- Vector similarity search
SELECT content, 1 - (embedding <=> '[0.1,0.2,0.3,...]') AS similarity
FROM documents
ORDER BY embedding <=> '[0.1,0.2,0.3,...]'
LIMIT 10;
```

### Advanced Features

- **ACID Transactions**: Full transaction support across all operations
- **Prepared Statements**: Optimal performance for repeated queries  
- **JSON/JSONB**: Native JSON operations and indexing
- **Full-text Search**: Built-in text search capabilities
- **Connection Pooling**: Efficient connection management

##  Redis Compatibility

### Core Redis Commands

Standard Redis key-value operations:

```redis
# String operations
SET user:1:name "John Doe"
GET user:1:name
MGET user:1:name user:1:email

# Hash operations  
HSET user:1 name "John" email "john@example.com" age 30
HGET user:1 name
HMGET user:1 name email

# List operations
LPUSH tasks "task1" "task2"  
RPOP tasks

# Set operations
SADD categories "electronics" "books"
SISMEMBER categories "electronics"
```

### Redis Vector Operations

Native vector commands compatible with RedisSearch:

```redis
# Add vectors with metadata
VECTOR.ADD product-vectors item1 "0.1,0.2,0.3,0.4" name "Laptop" category "Electronics"

# Vector similarity search
VECTOR.SEARCH product-vectors "0.15,0.25,0.35,0.45" 5 METRIC COSINE

# K-nearest neighbors
VECTOR.KNN product-vectors "0.1,0.2,0.3,0.4" 3

# Vector statistics
VECTOR.STATS product-vectors
VECTOR.COUNT product-vectors
```

### RedisSearch Compatibility

FT.* commands for full-text and vector search:

```redis
# Create index
FT.CREATE products-idx DIM 384 DISTANCE_METRIC COSINE

# Add documents
FT.ADD products-idx doc1 "0.1,0.2,0.3" title "Product Name"

# Search with filters
FT.SEARCH products-idx "0.1,0.2,0.3" 10 DISTANCE_METRIC COSINE
```

### Advanced Redis Features

- **Redis Streams**: Time-series data processing
- **Pub/Sub**: Real-time messaging
- **Pipelining**: Batch operations for performance

##  Enterprise Connection Pooling

Orbit-RS features advanced connection pooling with enterprise-grade capabilities:

### Multi-Tier Architecture

- **Client Tier**: Connection pooling at the client level
- **Application Tier**: Server-side connection management  
- **Database Tier**: Backend connection optimization

### Configuration

```toml
[pooling]
enabled = true
min_connections = 5
max_connections = 100
connection_timeout_secs = 30
idle_timeout_secs = 300
max_lifetime_secs = 3600
health_check_interval_secs = 30
load_balancing_strategy = "LeastConnections"
tier = "Application"
enable_dynamic_sizing = true
target_utilization = 0.75

# Circuit breaker configuration
[pooling.circuit_breaker]
enabled = true
failure_threshold = 5
failure_window_secs = 60
recovery_timeout_secs = 30
success_threshold = 3
half_open_max_calls = 3

# Per-protocol overrides
[pooling.protocol_overrides.postgresql]
max_connections = 150
load_balancing_strategy = "RoundRobin"

[pooling.protocol_overrides.redis]
max_connections = 75
load_balancing_strategy = "WeightedRoundRobin"
```

### Load Balancing Strategies

- **RoundRobin**: Distribute connections evenly
- **LeastConnections**: Route to least loaded nodes
- **WeightedRoundRobin**: Weight-based distribution
- **Random**: Random selection
- **AffinityBased**: Session affinity routing

### Circuit Breaker Protection

- **Failure Detection**: Automatic failure detection
- **Recovery Testing**: Half-open state recovery
- **Cascade Prevention**: Prevent system-wide failures

### Health Monitoring

- **Real-time Health Checks**: Continuous connection monitoring
- **Response Time Tracking**: Performance metrics
- **Automatic Recovery**: Failed connection replacement
- **Dynamic Scaling**: Adapt pool size to load
- **Lua Scripting**: Custom operations (planned)

##  HTTP REST API

RESTful interface for web applications:

```bash
# Health check
curl http://localhost:8080/health

# Query data
curl "http://localhost:8080/api/products?category=electronics"

# Create data
curl -X POST http://localhost:8080/api/products \
  -H "Content-Type: application/json" \
  -d '{"name":"New Product","price":199.99}'

# Vector search
curl -X POST http://localhost:8080/api/vectors/search \
  -H "Content-Type: application/json" \
  -d '{"vector":[0.1,0.2,0.3],"limit":10}'
```

##  gRPC Actor API

Direct access to the actor system for advanced use cases:

```protobuf
// Actor management
service ActorSystem {
    rpc CreateActor(CreateActorRequest) returns (ActorResponse);
    rpc SendMessage(MessageRequest) returns (MessageResponse);
    rpc QueryActor(QueryRequest) returns (QueryResponse);
}
```

##  Cross-Protocol Consistency

Data written through one protocol is immediately available through all others:

```python
import psycopg2
import redis

# Write via PostgreSQL
pg_conn = psycopg2.connect("host=localhost port=5432")
pg_cur = pg_conn.cursor()
pg_cur.execute("INSERT INTO products (name, price) VALUES (%s, %s)", 
               ("Laptop", 999.99))
pg_conn.commit()

# Read via Redis
r = redis.Redis(host='localhost', port=6379)
result = r.hget("product:1", "name")  # Returns "Laptop"
```

##  Use Cases

### 1. AI/ML Applications

```python
# Store embeddings via PostgreSQL
cursor.execute("""
    INSERT INTO documents (content, embedding) 
    VALUES (%s, %s)
""", ("Document text", embedding_vector))

# Fast vector search via Redis
redis_client.execute_command('VECTOR.SEARCH', 'doc-embeddings', 
                           query_vector, 10, 'METRIC', 'COSINE')
```

### 2. Real-time Analytics

```sql
-- Complex analytics via SQL
SELECT category, AVG(price), COUNT(*)
FROM products 
WHERE created_at > NOW() - INTERVAL '1 hour'
GROUP BY category;
```

```redis
-- Fast lookups via Redis
HGET stats:hourly:electronics count
HGET stats:hourly:electronics avg_price
```

### 3. Web Applications

```javascript
// Use optimal protocol per operation
const postgresClient = new pg.Client({host: 'localhost', port: 5432});
const redisClient = redis.createClient({host: 'localhost', port: 6379});

// Complex queries via SQL
const products = await postgresClient.query(
    'SELECT * FROM products WHERE category = $1 ORDER BY popularity DESC',
    ['electronics']
);

// Caching via Redis  
await redisClient.setex(`products:${category}`, 300, JSON.stringify(products));
```

### 4. Microservices Architecture

Different services can use their preferred protocol:

- **User Service**: PostgreSQL for ACID transactions
- **Caching Service**: Redis for fast key-value operations  
- **AI Service**: Vector operations via both SQL and Redis
- **Web Frontend**: HTTP REST API
- **Internal Services**: gRPC for performance

##  Performance

### Benchmarks

| Protocol | Operation | Latency (p95) | Throughput (ops/sec) |
|----------|-----------|---------------|---------------------|
| PostgreSQL | SELECT | 2ms | 50,000 |
| PostgreSQL | Vector Search | 15ms | 5,000 |
| Redis | GET | 0.5ms | 100,000 |
| Redis | Vector Search | 10ms | 8,000 |
| HTTP REST | GET | 5ms | 20,000 |
| gRPC | Actor Call | 1ms | 80,000 |

### Optimizations

- **Connection Pooling**: Efficient connection management
- **Query Caching**: Automatic query result caching
- **Vector Indexing**: HNSW and IVF indexes for fast similarity search
- **SIMD Operations**: Hardware-accelerated vector calculations
- **Concurrent Processing**: Parallel query execution

##  Advanced Configuration

### Memory Management

```toml
[performance.memory]
max_memory_mb = 4096

[performance.memory.pool_config]
initial_size_mb = 256
max_size_mb = 2048
```

### Vector Operations

```toml
[protocols.postgresql.vector_ops]
default_metric = "cosine"
max_dimensions = 4096
batch_size = 1000
enable_simd = true

[protocols.postgresql.vector_ops.indexing.hnsw]
m = 16
ef_construction = 200
ef_search = 50
```

### Security

```toml
[security.authentication]
enabled = true
methods = ["JWT", "Basic"]

[security.authentication.jwt]
secret_key = "your-secret-key"
expiration_secs = 3600
```

### Monitoring

```toml
[monitoring.metrics]
enabled = true
port = 9090
format = "prometheus"

[monitoring.tracing]
enabled = true
backend = "jaeger"
```

##  Troubleshooting

### Connection Issues

```bash
# Check if server is running
netstat -tlnp | grep :5432  # PostgreSQL
netstat -tlnp | grep :6379  # Redis
netstat -tlnp | grep :8080  # HTTP REST

# Test connections
pg_isready -h localhost -p 5432
redis-cli -h localhost -p 6379 ping
curl http://localhost:8080/health
```

### Performance Issues

```bash
# Check system resources
ps aux | grep orbit-server
free -h
iostat -x 1

# Monitor metrics
curl http://localhost:9090/metrics
```

### Configuration Validation

```bash
# Validate configuration
orbit-server --config orbit-server.toml --validate

# Generate example configuration
orbit-server --generate-config
```

##  Migration Guide

### From PostgreSQL + Redis

Replace your existing setup:

```yaml
# Before: docker-compose.yml
services:
  postgres:
    image: postgres:13
    ports: ["5432:5432"]
  
  redis:
    image: redis:7
    ports: ["6379:6379"]
```

```yaml  
# After: docker-compose.yml
services:
  orbit:
    image: orbit-rs:latest
    ports: 
      - "5432:5432"  # PostgreSQL compatibility
      - "6379:6379"  # Redis compatibility
      - "8080:8080"  # REST API
      - "50051:50051" # gRPC
```

### Application Changes

Most applications require no changes:

```python
# PostgreSQL clients work unchanged
import psycopg2
conn = psycopg2.connect("host=orbit-server port=5432")

# Redis clients work unchanged  
import redis
r = redis.Redis(host='orbit-server', port=6379)
```

##  Benefits

### Operational

- **Single Server**: Replace PostgreSQL + Redis with one server
- **Reduced Complexity**: One configuration, one deployment
- **Lower Resource Usage**: Shared memory and processing
- **Simplified Monitoring**: Single metrics endpoint

### Development

- **Protocol Choice**: Use optimal protocol per use case
- **Data Consistency**: Automatic consistency across protocols  
- **Feature Parity**: Advanced features available in all protocols
- **Testing**: Single server for all integration tests

### Performance

- **Zero Serialization**: Direct data access across protocols
- **Optimized Indexes**: Shared indexes benefit all protocols
- **Cache Efficiency**: Single cache serves all protocols
- **Vector Operations**: Native vector support in SQL and Redis

##  Further Reading

- [Vector Operations Guide](vector_commands.md)
- [SQL Compatibility Reference](protocols/SQL_PARSER_ARCHITECTURE.md)
- [Redis Commands Reference](protocols/REDIS_COMMANDS_REFERENCE.md)
- [REST API Documentation](protocols/protocol_adapters.md#rest-api)
- [Configuration Reference](deployment/CONFIGURATION.md)
- [Performance Tuning](PETABYTE_SCALE_PERFORMANCE.md)

##  Community

- **GitHub**: <https://github.com/orbit-rs/orbit-rs>
- **Discord**: [Join our community](https://discord.gg/orbit-rs)
- **Docs**: <https://docs.orbit-rs.dev>
- **Examples**: <https://github.com/orbit-rs/orbit-rs/tree/main/examples>

---

**Orbit-RS: One Server, All Protocols** 