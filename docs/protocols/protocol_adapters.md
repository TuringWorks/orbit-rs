# Protocol Adapters

Orbit-RS provides multiple protocol adapters enabling clients to interact with the actor system using familiar protocols. This allows existing applications to integrate with Orbit-RS without requiring significant code changes.

## Overview

Protocol adapters act as translation layers between external protocols and the Orbit actor system. They provide familiar interfaces while leveraging the distributed actor model underneath for scalability, fault tolerance, and consistency.

### Supported Protocols

- ✅ **Redis Protocol (RESP)** - **Complete Redis compatibility with 50+ commands** including VECTOR.*, TS.*, GRAPH.*, FT.* extensions
- ✅ **PostgreSQL Wire Protocol** - Full DDL/DML support with vector operations and complex SQL parsing
- ✅ **Model Context Protocol (MCP)** - AI agent integration with comprehensive tool support
- 🚧 **Redis Time Series** - Time-series database with RedisTimeSeries compatibility (Phase 12)
- 🚧 **PostgreSQL TimescaleDB** - Advanced time-series analytics and hypertables (Phase 12)
- 🚧 **REST API** - HTTP/JSON interface for web applications (planned)
- 🚧 **Neo4j Bolt Protocol** - Graph database compatibility (planned)

## Redis Protocol (RESP) Support ✅

Connect to Orbit actors using any Redis client through the RESP (REdis Serialization Protocol) adapter.

### Quick Start

Start the RESP server and connect with any Redis client:

```bash
# Start the RESP server example
cargo run --example resp-server

# In another terminal, connect with redis-cli
redis-cli -h 127.0.0.1 -p 6380

# Use Redis commands that operate on Orbit actors
> SET mykey "hello world"
OK
> GET mykey  
"hello world"
> HSET myhash field1 "value1"
(integer) 1
> HGET myhash field1
"value1"
> LPUSH mylist item1 item2
(integer) 2
> PUBLISH mychannel "hello subscribers"
(integer) 0
```

### Supported Redis Commands

**📚 [Complete Command Reference](REDIS_COMMANDS_REFERENCE.md)** - Detailed documentation for all 50+ commands

#### Key-Value Operations (15+ commands)
- **GET** - Retrieve value by key
- **SET** - Set key to value (with optional expiration: `SET key value EX 3600`)
- **DEL** - Delete one or more keys
- **EXISTS** - Check if key exists
- **TTL** - Get time-to-live for key
- **EXPIRE** - Set expiration time for key
- **PERSIST** - Remove expiration from key
- **APPEND** - Append value to existing string
- **GETRANGE** - Get substring by byte range
- **GETSET** - Atomically set new value and return old value
- **MGET** - Get multiple keys at once
- **MSET** - Set multiple key-value pairs
- **SETEX** - Set key with expiration time
- **SETRANGE** - Overwrite string at specific offset
- **STRLEN** - Get string length
- **PEXPIRE** - Set expiration in milliseconds
- **PTTL** - Get TTL in milliseconds
- **RANDOMKEY** - Get random key
- **RENAME** - Rename key
- **TYPE** - Get type of key
- **UNLINK** - Asynchronous delete

#### Hash Operations (10+ commands)
- **HGET** - Get field value from hash
- **HSET** - Set field value in hash
- **HGETALL** - Get all field-value pairs from hash
- **HDEL** - Delete field from hash
- **HEXISTS** - Check if field exists in hash
- **HKEYS** - Get all field names from hash
- **HVALS** - Get all values from hash
- **HLEN** - Get number of fields in hash
- **HMGET** - Get multiple field values
- **HMSET** - Set multiple field values
- **HINCRBY** - Increment field by integer value

#### List Operations (12+ commands)
- **LPUSH** - Push elements to left of list
- **RPUSH** - Push elements to right of list
- **LPOP** - Pop element from left of list
- **RPOP** - Pop element from right of list
- **LRANGE** - Get range of elements from list
- **LLEN** - Get length of list
- **LINDEX** - Get element at index
- **LSET** - Set element at index
- **LREM** - Remove elements equal to value
- **LTRIM** - Trim list to specified range
- **LINSERT** - Insert element before or after pivot
- **BLPOP** - Blocking left pop (non-blocking implementation)
- **BRPOP** - Blocking right pop (non-blocking implementation)

#### Set Operations (7+ commands)
- **SADD** - Add members to set
- **SREM** - Remove members from set
- **SMEMBERS** - Get all members of set
- **SCARD** - Get cardinality (size) of set
- **SISMEMBER** - Check if member exists in set
- **SUNION** - Return union of sets
- **SINTER** - Return intersection of sets
- **SDIFF** - Return difference of sets

#### Sorted Set Operations (8+ commands)
- **ZADD** - Add members with scores to sorted set
- **ZREM** - Remove members from sorted set
- **ZCARD** - Get cardinality of sorted set
- **ZSCORE** - Get score of member
- **ZINCRBY** - Increment score of member
- **ZRANGE** - Get members by rank range
- **ZRANGEBYSCORE** - Get members by score range
- **ZCOUNT** - Count members in score range
- **ZRANK** - Get rank of member

#### Pub/Sub Operations (6+ commands)
- **PUBLISH** - Publish message to channel
- **SUBSCRIBE** - Subscribe to channels
- **UNSUBSCRIBE** - Unsubscribe from channels
- **PSUBSCRIBE** - Subscribe to channel patterns
- **PUNSUBSCRIBE** - Unsubscribe from channel patterns
- **PUBSUB** - Introspect pub/sub system

#### Connection Commands (5+ commands)
- **PING** - Test connection
- **ECHO** - Echo message
- **SELECT** - Select database (logical separation)
- **AUTH** - Authenticate with server
- **QUIT** - Close connection

#### Server Commands (5+ commands)
- **INFO** - Get server information with Orbit-specific details
- **DBSIZE** - Get number of keys
- **FLUSHDB** - Clear current database
- **FLUSHALL** - Clear all databases
- **COMMAND** - Get list of available commands

## Redis Extensions - Advanced Features ✨

Orbit-RS extends Redis with enterprise-grade features for AI/ML, time series, and graph database workloads.

### Vector Operations (VECTOR.* namespace) 🤖
**AI/ML vector search with multiple similarity metrics**

- **VECTOR.ADD** index id vector [metadata...] - Add vector with optional metadata
- **VECTOR.GET** index id - Get vector and metadata by ID
- **VECTOR.DEL** index id - Delete vector from index
- **VECTOR.STATS** index - Get statistics for vector index
- **VECTOR.LIST** index - List all vector IDs in index
- **VECTOR.COUNT** index - Get count of vectors in index
- **VECTOR.SEARCH** index vector limit [options...] - Perform similarity search
- **VECTOR.KNN** index vector k [metric] - K-nearest neighbors search

**Similarity Metrics**: COSINE, EUCLIDEAN, DOT_PRODUCT, MANHATTAN

```redis
VECTOR.ADD embeddings doc1 "0.1,0.2,0.3,0.4" title "AI Document" category "tech"
VECTOR.SEARCH embeddings "0.1,0.2,0.3,0.4" 5 METRIC COSINE THRESHOLD 0.8
```

### RedisSearch Compatible (FT.* namespace) 🔍
**Full-text and vector search engine compatibility**

- **FT.CREATE** index DIM dimension [options] - Create vector search index
- **FT.ADD** index id vector [metadata...] - Add document to search index
- **FT.DEL** index id - Delete document from index
- **FT.SEARCH** index vector limit [options] - Search documents
- **FT.INFO** index - Get index information and statistics

### Time Series (TS.* namespace) 📊
**Complete RedisTimeSeries compatibility for IoT and monitoring**

- **TS.CREATE** key [options] - Create time series with retention policies
- **TS.ALTER** key [options] - Modify time series configuration
- **TS.ADD** key timestamp value - Add sample to time series
- **TS.MADD** key1 ts1 val1 [key2 ts2 val2...] - Add multiple samples
- **TS.INCRBY** / **TS.DECRBY** key value - Increment/decrement by value
- **TS.DEL** key fromTimestamp toTimestamp - Delete samples in range
- **TS.GET** key - Get latest sample
- **TS.MGET** key1 [key2...] - Get latest from multiple series
- **TS.INFO** key - Get time series information and statistics
- **TS.RANGE** / **TS.REVRANGE** key from to [AGGREGATION func duration] - Query ranges
- **TS.MRANGE** / **TS.MREVRANGE** - Query multiple time series
- **TS.CREATERULE** / **TS.DELETERULE** - Manage compaction rules
- **TS.QUERYINDEX** - Query time series by labels

**Aggregation Functions**: AVG, SUM, MIN, MAX, COUNT, FIRST, LAST, STDDEV, VAR

```redis
TS.CREATE temperature:sensor1 RETENTION 3600000 LABELS sensor_id "001" location "office"
TS.ADD temperature:sensor1 * 23.5
TS.RANGE temperature:sensor1 - + AGGREGATION AVG 60000
```

### Graph Database (GRAPH.* namespace) 🕸️
**Cypher-like graph queries with execution planning**

- **GRAPH.QUERY** graph_name query - Execute graph query with write operations
- **GRAPH.RO_QUERY** graph_name query - Execute read-only graph query
- **GRAPH.DELETE** graph_name - Delete entire graph
- **GRAPH.LIST** - List all graphs in system
- **GRAPH.EXPLAIN** graph_name query - Get query execution plan
- **GRAPH.PROFILE** graph_name query - Profile query with metrics
- **GRAPH.SLOWLOG** graph_name - Get slow query log
- **GRAPH.CONFIG** GET|SET parameter [value] - Configure graph settings

```redis
GRAPH.QUERY social "MATCH (p:Person {name: 'Alice'}) RETURN p"
GRAPH.EXPLAIN social "MATCH (p:Person) WHERE p.age > 25 RETURN p.name"
```

### Actor Mapping

Each Redis command family maps to corresponding Orbit actor operations:

#### Core Data Types
- **String commands** → `KeyValueActor` - TTL-aware key-value pairs with expiration
- **Hash commands** → `HashActor` - Hash maps with field-value operations
- **List commands** → `ListActor` - Ordered lists with push/pop/range operations
- **Set commands** → `SetActor` - Unique member sets with union/intersection
- **Sorted Set commands** → `SortedSetActor` - Score-ordered sets with ranking
- **Pub/Sub commands** → `PubSubActor` - Message channels with pattern matching

#### Advanced Extensions
- **Vector commands** → `VectorActor` - AI/ML vector operations with similarity search
- **Time Series commands** → `TimeSeriesActor` - Time-series data with aggregation
- **Graph commands** → `GraphActor` - Graph database with Cypher-like queries

### Redis Configuration Example

```toml
[redis]
enabled = true
host = "127.0.0.1"
port = 6380
max_connections = 1000
default_ttl = 3600

[redis.actors]
key_value_actor = "KeyValueActor"
hash_actor = "HashActor"
list_actor = "ListActor"
pubsub_actor = "PubSubActor"
```

## PostgreSQL Wire Protocol with Vector Support ✅

Connect to Orbit actors using any PostgreSQL client with full SQL support and native vector operations for AI/ML workloads.

### Quick Start

Start the PostgreSQL-compatible server and connect with psql:

```bash
# Start the PostgreSQL-compatible server example
cargo run --example pgvector-store

# In another terminal, connect with psql
psql -h 127.0.0.1 -p 5433 -d orbit
```

### Vector Database Capabilities

Create tables with vector support for similarity search and AI applications:

```sql
-- Enable vector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Create table with vector embeddings
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    content TEXT,
    embedding VECTOR(768),  -- 768-dimensional vector
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Create vector indexes for similarity search
CREATE INDEX embedding_hnsw_idx 
ON documents USING hnsw (embedding) 
WITH (m = 16, ef_construction = 64);

-- Create IVFFLAT index for large datasets
CREATE INDEX embedding_ivfflat_idx
ON documents USING ivfflat (embedding)
WITH (lists = 100);

-- Insert documents with embeddings
INSERT INTO documents (title, content, embedding) VALUES 
('AI in Healthcare', 'Article about AI applications...', '[0.1, 0.2, 0.3, ...]'),
('Machine Learning Basics', 'Introduction to ML concepts...', '[0.2, 0.1, 0.4, ...]');

-- Perform vector similarity searches
SELECT title, content, embedding <-> '[0.1, 0.2, 0.3, ...]' AS distance
FROM documents 
ORDER BY distance 
LIMIT 5;

-- Hybrid search combining vector similarity and text filtering
SELECT title, content, distance
FROM (
    SELECT title, content, embedding <-> '[0.1, 0.2, ...]' AS distance
    FROM documents 
    WHERE content LIKE '%machine learning%'
    ORDER BY distance
    LIMIT 10
) subq;
```

### Supported PostgreSQL Features

#### DDL Operations (Data Definition Language)
- **CREATE/ALTER/DROP TABLE** - Complete table lifecycle management
- **CREATE/DROP INDEX** - B-tree, Hash, IVFFLAT, HNSW index support
- **CREATE/DROP VIEW** - Virtual table support
- **CREATE/DROP SCHEMA** - Database organization
- **CREATE/DROP EXTENSION** - Extension management

#### DCL Operations (Data Control Language)
- **GRANT/REVOKE** - Permission management
- **Role-based Access Control** - User and role management
- **Fine-grained Permissions** - Table, column, and operation-level access control

#### TCL Operations (Transaction Control Language)
- **BEGIN/START TRANSACTION** - Transaction initiation
- **COMMIT** - Transaction confirmation
- **ROLLBACK** - Transaction cancellation
- **SAVEPOINT** - Nested transaction points
- **Isolation Levels** - READ COMMITTED, REPEATABLE READ, SERIALIZABLE

#### Vector Data Types
- **VECTOR(n)** - Dense vectors with n dimensions
- **HALFVEC(n)** - Half-precision vectors for memory efficiency
- **SPARSEVEC(n)** - Sparse vectors for high-dimensional data

#### Vector Indexes
- **IVFFLAT** - Inverted file with flat compression for large datasets
- **HNSW** - Hierarchical Navigable Small World for high accuracy

#### Vector Operations
- **Distance Operators**:
  - `<->` - L2 (Euclidean) distance
  - `<#>` - Inner product (dot product)
  - `<=>` - Cosine distance
- **Similarity Functions**: Built-in similarity scoring and ranking

#### Advanced SQL Features
- **ANSI SQL Types**: All standard SQL data types
- **JSON/JSONB**: Full JSON support with indexing
- **Arrays**: Multi-dimensional array support
- **Complex Expressions**: Full operator precedence parsing
- **Window Functions**: ROW_NUMBER, RANK, DENSE_RANK, etc.
- **Common Table Expressions (CTEs)**: WITH clauses for complex queries
- **Subqueries**: Correlated and non-correlated subqueries

### Complex SQL Examples

```sql
-- Complex expressions with proper precedence
SELECT * FROM documents 
WHERE (score > 0.8 AND category = 'research') 
   OR (embedding <-> '[0.1, 0.2, ...]' < 0.5 AND published_at > '2024-01-01');

-- Function calls and arithmetic operations
SELECT 
    title,
    COALESCE(score * 100, 0) + bonus_points AS final_score,
    GREATEST(created_at, updated_at) AS last_modified
FROM documents
WHERE NOT deleted AND (category IN ('ai', 'ml', 'research'));

-- Window functions with vector similarity
SELECT 
    title,
    content,
    embedding <-> '[0.1, 0.2, ...]' AS distance,
    ROW_NUMBER() OVER (ORDER BY embedding <-> '[0.1, 0.2, ...]') AS rank,
    PERCENT_RANK() OVER (ORDER BY score DESC) AS score_percentile
FROM documents
WHERE category = 'research';

-- Common Table Expression with recursive queries
WITH RECURSIVE category_hierarchy AS (
    SELECT id, name, parent_id, 1 as level
    FROM categories 
    WHERE parent_id IS NULL
    UNION ALL
    SELECT c.id, c.name, c.parent_id, ch.level + 1
    FROM categories c
    JOIN category_hierarchy ch ON c.parent_id = ch.id
)
SELECT * FROM category_hierarchy ORDER BY level, name;
```

### SQL Expression Parser Engine

The PostgreSQL wire protocol includes a comprehensive expression parser:

```rust path=null start=null
use orbit_protocols::postgres_wire::sql::parser::ExpressionParser;
use orbit_protocols::postgres_wire::sql::lexer::Lexer;

// Parse complex SQL expressions
let mut lexer = Lexer::new();
let tokens = lexer.tokenize("score * 100 + bonus > threshold AND NOT deleted")?;
let mut parser = ExpressionParser::new();
let mut pos = 0;
let expression = parser.parse_expression(&tokens, &mut pos)?;
```

#### Operator Precedence (lowest to highest)
1. **OR** - Logical disjunction
2. **AND** - Logical conjunction  
3. **Equality** - `=`, `!=`, `<>`, `IS`, `IS NOT`
4. **Comparison** - `<`, `<=`, `>`, `>=`, `LIKE`, `ILIKE`, `IN`, vector operators (`<->`, `<#>`, `<=>`)
5. **Additive** - `+`, `-`, `||` (concatenation)
6. **Multiplicative** - `*`, `/`, `%`
7. **Unary** - `NOT`, unary `-`, unary `+`
8. **Primary** - Literals, identifiers, function calls, parenthesized expressions

#### Supported Expression Types
- **Literals**: Strings, numbers, booleans, NULL
- **Identifiers**: Column references with optional table qualification
- **Function Calls**: `COALESCE(a, b)`, `GREATEST(x, y)`, `COUNT(*)` with argument parsing
- **Binary Operations**: All SQL operators including vector distance operations
- **Unary Operations**: Logical NOT, arithmetic negation/positive
- **Parenthesized Expressions**: Explicit precedence control with nested parsing
- **Vector Operations**: pgvector compatibility with distance operators

## Model Context Protocol (MCP) ✅

AI agent integration through the standardized Model Context Protocol, enabling AI systems to interact with Orbit-RS.

### MCP Server Setup

```rust path=null start=null
use orbit_protocols::mcp::{McpServer, McpConfig};

// Configure MCP server for AI agents
let mcp_config = McpConfig {
    name: "orbit-mcp-server".to_string(),
    version: "0.1.0".to_string(),
    capabilities: McpCapabilities::default()
        .with_tools()
        .with_resources()
        .with_prompts(),
};

let mcp_server = McpServer::new(mcp_config).await?;
```

### AI Agent Capabilities

Through MCP, AI agents can:

- **Execute SQL Queries**: Run complex SQL queries against the actor system
- **Manage Actor Lifecycles**: Create, configure, and monitor actor instances
- **Access Vector Operations**: Perform similarity searches and AI/ML workloads
- **Query System Resources**: Access metrics, logs, and system information
- **Transaction Management**: Execute distributed transactions with ACID compliance
- **Real-time Monitoring**: Subscribe to system events and performance metrics

### MCP Tool Examples

```json
{
  "tools": [
    {
      "name": "execute_sql",
      "description": "Execute SQL query against Orbit actor system",
      "input_schema": {
        "type": "object",
        "properties": {
          "query": {"type": "string"},
          "parameters": {"type": "array"}
        }
      }
    },
    {
      "name": "vector_search",
      "description": "Perform vector similarity search",
      "input_schema": {
        "type": "object",
        "properties": {
          "table": {"type": "string"},
          "vector": {"type": "array"},
          "limit": {"type": "integer"},
          "threshold": {"type": "number"}
        }
      }
    }
  ]
}
```

## Architecture Highlights

### Modular Design
- **Pluggable Protocols**: Easy to add new protocol adapters
- **Consistent Actor Mapping**: All protocols map to the same underlying actor system
- **Performance Optimized**: Each adapter optimized for its specific protocol

### Protocol Integration
- **Wire Protocol Compatibility**: Full compatibility with existing clients
- **Type Safety**: Strong typing across protocol boundaries
- **Error Handling**: Consistent error handling and reporting

### Distributed Features
- **Actor Distribution**: Protocols can access actors across the cluster
- **Load Balancing**: Automatic load distribution for protocol endpoints
- **High Availability**: Fault tolerance at the protocol level

## Configuration

### Global Protocol Configuration

```toml
[protocols]
# Enable specific protocols
redis.enabled = true
postgres.enabled = true
mcp.enabled = true

# Global settings
max_connections_per_protocol = 1000
connection_timeout = "30s"
idle_timeout = "300s"

[protocols.redis]
host = "0.0.0.0"
port = 6380
default_database = 0

[protocols.postgres]
host = "0.0.0.0"
port = 5433
database = "orbit"
ssl_mode = "prefer"

[protocols.mcp]
host = "127.0.0.1"
port = 8080
capabilities = ["tools", "resources", "prompts"]
```

## Upcoming Time Series Features 🚀

Orbit-RS will add comprehensive time-series database capabilities in **Phase 12 (Q1 2025)**, providing compatibility with two major time-series ecosystems.

### Redis Time Series Compatibility
**Full RedisTimeSeries module compatibility** - [📖 Detailed Documentation](REDIS_TIMESERIES.md)

#### Key Features
- **TimeSeriesActor**: Distributed time-series management with automatic partitioning
- **Core Commands**: TS.CREATE, TS.ADD, TS.GET, TS.RANGE, TS.REVRANGE
- **Aggregation Rules**: TS.CREATERULE, TS.DELETERULE with automated downsampling
- **Multi-Series Operations**: TS.MRANGE, TS.MREVRANGE, TS.MGET with label filtering
- **Built-in Functions**: AVG, SUM, MIN, MAX, COUNT, STDDEV, VAR
- **Retention Policies**: Automatic data expiration and compression
- **Labeling System**: Multi-dimensional time series organization

#### Example Usage
```python
import redis
r = redis.Redis(host='localhost', port=6380)  # Orbit-RS RESP server

# Create time series with retention
r.execute_command('TS.CREATE', 'sensor:temp:001', 
                 'RETENTION', 3600000,
                 'LABELS', 'sensor_id', '001', 'type', 'temperature')

# Add readings
import time
now = int(time.time() * 1000)
r.execute_command('TS.ADD', 'sensor:temp:001', now, 23.5)
r.execute_command('TS.ADD', 'sensor:temp:001', now + 1000, 24.1)

# Query with aggregation
results = r.execute_command('TS.RANGE', 'sensor:temp:001',
                           now - 300000, now,
                           'AGGREGATION', 'AVG', 60000)
print(f"1-minute averages: {results}")
```

### PostgreSQL TimescaleDB Compatibility
**Complete TimescaleDB extension compatibility** - [📖 Detailed Documentation](POSTGRESQL_TIMESCALE.md)

#### Key Features
- **Hypertables**: Distributed time-partitioned tables with automatic chunking
- **Time Functions**: time_bucket(), time_bucket_gapfill(), locf(), interpolate()
- **Continuous Aggregates**: Materialized views with automatic refresh policies
- **Compression**: Column-store compression for historical data
- **Data Retention**: Automatic chunk expiration with configurable policies
- **Multi-dimensional Partitioning**: Time and space partitioning
- **Advanced Analytics**: Hyperfunctions for time-series analysis

#### Example Usage
```sql
-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Create hypertable
CREATE TABLE sensor_data (
    timestamp TIMESTAMPTZ NOT NULL,
    sensor_id TEXT NOT NULL,
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION
);

SELECT create_hypertable('sensor_data', 'timestamp',
    chunk_time_interval => INTERVAL '1 hour');

-- Time bucket aggregation
SELECT 
    time_bucket('15 minutes', timestamp) AS bucket,
    sensor_id,
    AVG(temperature) as avg_temp,
    MAX(temperature) - MIN(temperature) as temp_range
FROM sensor_data
WHERE timestamp >= NOW() - INTERVAL '4 hours'
GROUP BY bucket, sensor_id
ORDER BY bucket DESC;
```

### Integration Benefits
- **Distributed Architecture**: Time series data distributed across cluster nodes
- **ACID Transactions**: Full transaction support for time series operations
- **Vector Integration**: Combine time series with vector similarity search
- **Real-time Analytics**: Stream processing with Apache Kafka integration
- **High Availability**: Automatic failover and replication for time series
- **Migration Tools**: Seamless migration from RedisTimeSeries and TimescaleDB

## Future Protocols 🚧

### REST API
HTTP/JSON interface for web applications with:
- RESTful actor endpoints
- OpenAPI/Swagger documentation
- Authentication integration
- Rate limiting and throttling

### Neo4j Bolt Protocol
Graph database compatibility featuring:
- Cypher query language support
- Graph traversal algorithms
- Relationship mapping to actors
- Property graph model

### Additional Protocols Under Consideration
- **Apache Kafka Protocol** - Event streaming integration
- **MQTT** - IoT device communication
- **WebSocket** - Real-time web applications
- **GraphQL** - Modern API interface
- **InfluxDB Line Protocol** - Time series data ingestion

## Best Practices

### Protocol Selection
1. **Redis**: Ideal for caching, session storage, and pub/sub messaging
2. **PostgreSQL**: Best for complex queries, ACID transactions, and vector operations
3. **MCP**: Perfect for AI agent integration and programmatic access
4. **REST**: Great for web applications and microservices integration

### Performance Optimization
1. **Connection Pooling**: Use connection pools for high-throughput scenarios
2. **Batch Operations**: Group operations when possible to reduce overhead
3. **Index Strategy**: Create appropriate indexes for your query patterns
4. **Monitor Metrics**: Use built-in metrics to identify performance bottlenecks

### Security Considerations
1. **Network Security**: Use TLS for production deployments
2. **Authentication**: Implement proper authentication for each protocol
3. **Authorization**: Configure fine-grained access controls
4. **Audit Logging**: Enable comprehensive audit trails

## Related Documentation

- [Quick Start Guide](../QUICK_START.md) - Getting started with protocols
- [Transaction Features](../features/TRANSACTION_FEATURES.md) - Distributed transaction support
- [Deployment Guide](../deployment/DEPLOYMENT.md) - Production deployment with protocols
- [Development Guide](../development/DEVELOPMENT.md) - Contributing to protocol adapters