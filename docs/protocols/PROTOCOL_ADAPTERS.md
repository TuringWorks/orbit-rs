# Protocol Adapters

Orbit-RS provides multiple protocol adapters enabling clients to interact with the actor system using familiar protocols. This allows existing applications to integrate with Orbit-RS without requiring significant code changes.

## Overview

Protocol adapters act as translation layers between external protocols and the Orbit actor system. They provide familiar interfaces while leveraging the distributed actor model underneath for scalability, fault tolerance, and consistency.

### Supported Protocols

- ✅ **Redis Protocol (RESP)** - Complete Redis compatibility with 50+ commands
- ✅ **PostgreSQL Wire Protocol** - Full DDL/DML support with vector operations
- ✅ **Model Context Protocol (MCP)** - AI agent integration
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

#### Key-Value Operations
- **GET** - Retrieve value by key
- **SET** - Set key to value (with optional expiration: `SET key value EX 3600`)
- **DEL** - Delete one or more keys
- **EXISTS** - Check if key exists
- **TTL** - Get time-to-live for key
- **EXPIRE** - Set expiration time for key
- **PERSIST** - Remove expiration from key

#### Hash Operations
- **HGET** - Get field value from hash
- **HSET** - Set field value in hash
- **HGETALL** - Get all field-value pairs from hash
- **HDEL** - Delete field from hash
- **HEXISTS** - Check if field exists in hash
- **HKEYS** - Get all field names from hash
- **HVALS** - Get all values from hash
- **HLEN** - Get number of fields in hash

#### List Operations
- **LPUSH** - Push elements to left of list
- **RPUSH** - Push elements to right of list
- **LPOP** - Pop element from left of list
- **RPOP** - Pop element from right of list
- **LRANGE** - Get range of elements from list
- **LLEN** - Get length of list
- **LINDEX** - Get element at index

#### Pub/Sub Operations
- **PUBLISH** - Publish message to channel
- **SUBSCRIBE** - Subscribe to channels
- **UNSUBSCRIBE** - Unsubscribe from channels
- **PSUBSCRIBE** - Subscribe to channel patterns
- **PUNSUBSCRIBE** - Unsubscribe from channel patterns

#### Connection Commands
- **PING** - Test connection
- **ECHO** - Echo message
- **SELECT** - Select database (logical separation)
- **QUIT** - Close connection

#### Server Commands
- **INFO** - Get server information
- **DBSIZE** - Get number of keys
- **FLUSHDB** - Clear current database
- **COMMAND** - Get list of available commands

### Actor Mapping

Each Redis command maps to corresponding Orbit actor operations:

- **String commands** → `KeyValueActor` - Manages key-value pairs with TTL
- **Hash commands** → `HashActor` - Manages hash data structures  
- **List commands** → `ListActor` - Manages ordered lists with push/pop operations
- **Pub/Sub commands** → `PubSubActor` - Manages message publishing and subscriptions

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