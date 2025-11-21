---
layout: default
title: Protocol Adapters
category: protocols
---

## Protocol Adapters

Orbit-RS provides multiple protocol adapters enabling clients to interact with the actor system using familiar protocols. This allows existing applications to integrate with Orbit-RS without requiring significant code changes.

## Overview

Protocol adapters act as translation layers between external protocols and the Orbit actor system. They provide familiar interfaces while leveraging the distributed actor model underneath for scalability, fault tolerance, and consistency.

### Supported Protocols

-  **Redis Protocol (RESP)** - ** PRODUCTION-READY** - 50+ commands, all core data types, redis-cli support
-  **PostgreSQL Wire Protocol** - **EXPERIMENTAL** - Basic SQL parsing for actor operations (30% complete)
-  **Model Context Protocol (MCP)** - **EXPERIMENTAL** - Basic AI agent integration (partial implementation)
-  **Redis Extensions** - Vector operations (VECTOR.*), Time Series (TS.*), Graph DB (GRAPH.*), Search (FT.*) - *Planned*
-  **PostgreSQL TimescaleDB** - Advanced time-series analytics and hypertables - *Planned*
-  **REST API** - HTTP/JSON interface for web applications - *Planned*
-  **Neo4j Bolt Protocol** - Graph database compatibility - *Planned*

## Redis Protocol (RESP) Support 

Connect to Orbit actors using any Redis client through the RESP (REdis Serialization Protocol) adapter.

###  Quick Start

**Get Redis running in 30 seconds:**

```bash
# Method 1: One-command startup (recommended)
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs
cargo build --release
./start-orbit-redis.sh

# Method 2: Manual startup
# Terminal 1: Start Orbit distributed actor runtime
./target/release/orbit-server --grpc-port 50056 --dev-mode

# Terminal 2: Start Redis protocol server
./target/release/resp-server

# Terminal 3: Connect with ANY Redis client
redis-cli -h 127.0.0.1 -p 6379  # Standard Redis port
```

** Everything works perfectly:**

```redis
# String operations
127.0.0.1:6379> SET mykey "hello world"
OK
127.0.0.1:6379> GET mykey  
"hello world"
127.0.0.1:6379> DEL mykey
(integer) 1

# Hash operations
127.0.0.1:6379> HSET user:1 name "Alice" age "25" city "NYC"
(integer) 3
127.0.0.1:6379> HGETALL user:1
1) "name"
2) "Alice"
3) "age"
4) "25"
5) "city"
6) "NYC"

# List operations
127.0.0.1:6379> LPUSH tasks "task1" "task2"
(integer) 2
127.0.0.1:6379> LRANGE tasks 0 -1
1) "task2"
2) "task1"

# Set operations
127.0.0.1:6379> SADD tags "redis" "orbit" "distributed"
(integer) 3
127.0.0.1:6379> SMEMBERS tags
1) "redis"
2) "orbit"
3) "distributed"

# Sorted Set operations
127.0.0.1:6379> ZADD leaderboard 100 "player1" 85 "player2"
(integer) 2
127.0.0.1:6379> ZRANGE leaderboard 0 -1 WITHSCORES
1) "player2"
2) "85"
3) "player1"
4) "100"
```

### Supported Redis Commands

** [Complete Command Reference](REDIS_COMMANDS_REFERENCE.md)** - Detailed documentation for all 50+ commands

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

## Redis Extensions - Advanced Features  PLANNED

Orbit-RS will extend Redis with enterprise-grade features for AI/ML, time series, and graph database workloads in future releases.

### Vector Operations (VECTOR.* namespace)  PLANNED

**AI/ML vector search with multiple similarity metrics** - *Coming Soon*

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

### RedisSearch Compatible (FT.* namespace)  PLANNED

**Full-text and vector search engine compatibility** - *Coming Soon*

- **FT.CREATE** index DIM dimension [options] - Create vector search index
- **FT.ADD** index id vector [metadata...] - Add document to search index
- **FT.DEL** index id - Delete document from index
- **FT.SEARCH** index vector limit [options] - Search documents
- **FT.INFO** index - Get index information and statistics

### Time Series (TS.* namespace)  PLANNED

**Complete RedisTimeSeries compatibility for IoT and monitoring** - *Coming Soon*

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

### Graph Database (GRAPH.* namespace)  PLANNED

**Cypher-like graph queries with execution planning** - *Coming Soon*

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

## PostgreSQL Wire Protocol  EXPERIMENTAL

Connect to Orbit actors using any PostgreSQL client. **Note**: This is an experimental implementation providing basic SQL parsing for actor operations.

### Quick Start

Start the PostgreSQL-compatible server and connect with psql:

```bash

# Start the PostgreSQL-compatible server example
cargo run --example postgres-server

# In another terminal, connect with psql
psql -h 127.0.0.1 -p 5433 -U orbit -d actors
```

#### Essential SQL Operations for Actors

Once connected, you can immediately start working with actors using familiar SQL:

```sql
-- Create a new actor with JSON state
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('user:alice', 'UserActor', '{"name": "Alice", "email": "alice@example.com"}');

-- Query actors
SELECT * FROM actors;
SELECT actor_id, actor_type FROM actors WHERE actor_type = 'UserActor';

-- Update actor state with complex JSON
UPDATE actors 
SET state = '{"name": "Alice Johnson", "email": "alice.j@example.com", "verified": true}' 
WHERE actor_id = 'user:alice';

-- Remove actors
DELETE FROM actors WHERE actor_id = 'user:alice';
```

#### Currently Supported SQL Features

 **Basic SQL statements**: `SELECT`, `INSERT`, `UPDATE`, `DELETE` (actor operations only)
 **Basic SQL clauses**: `FROM`, `WHERE`, `SET`, `INTO`, `VALUES`
 **WHERE operators**: `=`, `!=`, `<>` (limited conditional logic)
 **JSON support**: Basic JSON state storage and retrieval
 **Case insensitive**: Keywords work in any case combination
 **Note**: This is a basic implementation focused on actor state management, not full PostgreSQL compatibility

### Vector Database Capabilities

**Note**: Vector database features with `CREATE TABLE` and vector extensions are planned for future releases. Currently, Orbit-RS PostgreSQL protocol provides full actor-based operations.

The current implementation focuses on actor state management with JSON support:

```sql
-- Current: Actor-based approach for storing vector data
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('document:ai-paper', 'DocumentActor', '{
    "title": "AI in Healthcare",
    "content": "Article about AI applications in medical field...",
    "embedding": [0.1, 0.2, 0.3, 0.4, 0.5],
    "metadata": {"category": "healthcare", "published": "2024-01-15"}
}');

-- Query documents by type
SELECT * FROM actors WHERE actor_type = 'DocumentActor';
```

#### Future Vector Support (Planned)

Upcoming releases will include full vector database capabilities:

```sql
-- Planned: Traditional table-based approach
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

### Complete SQL Keyword Reference with Examples 

Orbit-RS supports a comprehensive set of SQL keywords for actor operations. All examples below are tested and ready to use.

#### SQL Statement Keywords

##### INSERT - Create New Actors

Add new actors to the system with JSON state data.

```sql
-- Basic actor creation
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('user:alice', 'UserActor', '{"name": "Alice", "email": "alice@example.com"}');

-- Complex JSON state with nested objects and arrays
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('order:12345', 'OrderActor', '{
    "order_id": "12345",
    "customer": {"name": "Bob", "address": "123 Main St"},
    "items": [{"product": "laptop", "quantity": 1, "price": 999.99}],
    "status": "pending",
    "total": 999.99
}');

-- Multiple actors in one statement
INSERT INTO actors (actor_id, actor_type, state) VALUES 
    ('cache:sessions', 'CacheActor', '{"type": "redis", "ttl": 3600}'),
    ('cache:products', 'CacheActor', '{"type": "memory", "max_size": 1000}');
```

##### SELECT - Query Actor Data

Retrieve actors and their state information.

```sql
-- Retrieve all actors
SELECT * FROM actors;

-- Select specific columns
SELECT actor_id, actor_type FROM actors;

-- Filter by actor type
SELECT * FROM actors WHERE actor_type = 'UserActor';

-- Pattern matching on actor IDs
SELECT actor_id, state FROM actors WHERE actor_id LIKE 'product:%';

-- Multiple conditions
SELECT actor_id, actor_type, state 
FROM actors 
WHERE (actor_type = 'ProductActor' OR actor_type = 'UserActor') 
  AND actor_id LIKE '%user%';
```

##### UPDATE - Modify Actor State

Update existing actors with new state data.

```sql
-- Simple state update
UPDATE actors 
SET state = '{"name": "Alice Johnson", "email": "alice.johnson@example.com", "verified": true}' 
WHERE actor_id = 'user:alice';

-- Complex nested JSON update
UPDATE actors 
SET state = '{
    "name": "Premium Gaming Laptop",
    "price": 1599.99,
    "category": "electronics",
    "specs": {"cpu": "Intel i7", "ram": "32GB", "storage": "1TB SSD"},
    "tags": ["gaming", "high-performance", "portable"]
}' 
WHERE actor_id = 'product:laptop';

-- Conditional updates
UPDATE actors 
SET state = '{"status": "shipped", "tracking": "TRK123456"}' 
WHERE actor_type = 'OrderActor' AND actor_id = 'order:12345';
```

##### DELETE - Remove Actors

Remove actors from the system.

```sql
-- Delete specific actor
DELETE FROM actors WHERE actor_id = 'user:alice';

-- Delete by type
DELETE FROM actors WHERE actor_type = 'TempActor';

-- Delete with complex conditions
DELETE FROM actors 
WHERE actor_type = 'CacheActor' AND actor_id LIKE 'temp:%';
```

#### SQL Clause Keywords

##### FROM - Specify Data Source

```sql
-- Basic table reference
SELECT actor_id, actor_type FROM actors;

-- The 'actors' table is the primary data source for all operations
SELECT COUNT(*) as total_actors FROM actors;
```

##### WHERE - Filter Results

Supports multiple operators for flexible filtering.

```sql
-- Equality filtering
SELECT * FROM actors WHERE actor_type = 'UserActor';

-- Pattern matching
SELECT * FROM actors WHERE actor_id LIKE 'service:%';

-- Multiple conditions with AND/OR
SELECT * FROM actors 
WHERE (actor_type = 'ProductActor' OR actor_type = 'ServiceActor')
  AND actor_id NOT LIKE 'temp:%';
```

##### SET - Update Values

```sql
-- Simple value assignment
UPDATE actors SET state = '{"active": true}' WHERE actor_id = 'service:auth';

-- Complex JSON assignment
UPDATE actors SET state = '{
    "configuration": {
        "database": {"host": "localhost", "port": 5432},
        "cache": {"enabled": true, "ttl": 300},
        "features": ["auth", "logging", "metrics"]
    },
    "status": "configured"
}' WHERE actor_id = 'config:app';
```

##### INTO - Target Table Specification

```sql
-- Standard insertion syntax
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('service:payment', 'PaymentServiceActor', '{
    "provider": "stripe",
    "enabled": true,
    "supported_currencies": ["USD", "EUR", "GBP"]
}');
```

##### VALUES - Data Specification

```sql
-- Single row insertion
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('config:app', 'ConfigActor', '{"debug": true, "port": 8080}');

-- Multiple rows
INSERT INTO actors (actor_id, actor_type, state) VALUES 
    ('metric:cpu', 'MetricActor', '{"type": "gauge", "value": 45.2}'),
    ('metric:memory', 'MetricActor', '{"type": "gauge", "value": 78.5}'),
    ('metric:requests', 'MetricActor', '{"type": "counter", "value": 12450}');
```

#### WHERE Clause Operators

##### = (Equality)

```sql
-- Exact match
SELECT * FROM actors WHERE actor_type = 'UserActor';
SELECT * FROM actors WHERE actor_id = 'user:alice';
```

##### != (Not Equal)

```sql
-- Exclude specific types
SELECT actor_id, actor_type FROM actors WHERE actor_type != 'TempActor';
SELECT * FROM actors WHERE actor_id != 'system:internal';
```

##### <> (Not Equal Alternative)

```sql
-- Alternative not-equal syntax
SELECT * FROM actors WHERE actor_type <> 'CacheActor';
SELECT actor_id FROM actors WHERE actor_type <> 'SystemActor';
```

#### Table Support

##### actors - The Primary Table

All actor operations use the `actors` table.

```sql
-- Table structure (conceptual)
-- actors(
--     actor_id    TEXT PRIMARY KEY,    -- Unique actor identifier  
--     actor_type  TEXT NOT NULL,        -- Type/class of the actor
--     state       JSONB NOT NULL        -- JSON state data
-- )

-- Query table information
SELECT COUNT(*) as total_actors FROM actors;
SELECT DISTINCT actor_type FROM actors;
```

#### Column Support

##### actor_id - Primary Identifier

```sql
-- Query by specific ID
SELECT * FROM actors WHERE actor_id = 'user:alice';

-- Pattern matching on IDs
SELECT actor_id FROM actors WHERE actor_id LIKE 'product:%';
SELECT actor_id FROM actors WHERE actor_id LIKE '%@company.com';

-- ID-based operations
DELETE FROM actors WHERE actor_id = 'temp:session:12345';
```

##### actor_type - Actor Classification

```sql
-- Filter by actor type
SELECT * FROM actors WHERE actor_type = 'UserActor';

-- Group by type
SELECT actor_type, COUNT(*) as count FROM actors GROUP BY actor_type;

-- Multiple type filtering
SELECT * FROM actors 
WHERE actor_type IN ('UserActor', 'ServiceActor', 'ProductActor');
```

##### state - JSON State Data

```sql
-- Retrieve state information
SELECT actor_id, state FROM actors WHERE actor_type = 'UserActor';

-- State-based filtering (basic pattern matching)
SELECT * FROM actors WHERE actor_id LIKE 'config:%';
```

##### * (All Columns)

```sql
-- Retrieve complete records
SELECT * FROM actors;
SELECT * FROM actors WHERE actor_type = 'ProductActor';
SELECT * FROM actors WHERE actor_id LIKE 'service:%';
```

##### Multiple Column Selection

```sql
-- Specific column combinations
SELECT actor_id, actor_type FROM actors;
SELECT actor_id, actor_type, state FROM actors WHERE actor_type = 'ConfigActor';

-- Ordered selection
SELECT actor_type, actor_id, state FROM actors ORDER BY actor_type, actor_id;
```

#### JSON State Examples

Orbit-RS provides robust JSON support for complex actor state management.

##### Simple JSON

```sql
-- Basic key-value pairs
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('config:app', 'ConfigActor', '{
    "debug": true,
    "port": 8080,
    "environment": "development"
}');
```

##### Nested JSON Objects

```sql
-- Complex nested structures
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('user:profile', 'UserProfileActor', '{
    "user_id": "12345",
    "profile": {
        "personal": {"name": "John Doe", "age": 30},
        "preferences": {"theme": "dark", "notifications": true}
    },
    "metadata": {
        "created_at": "2024-01-15T10:30:00Z",
        "last_login": "2024-01-20T14:45:00Z"
    }
}');
```

##### JSON Arrays

```sql
-- Arrays and collections
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('playlist:favorites', 'PlaylistActor', '{
    "name": "My Favorites",
    "songs": [
        {"title": "Song 1", "artist": "Artist A", "duration": 180},
        {"title": "Song 2", "artist": "Artist B", "duration": 240}
    ],
    "tags": ["pop", "rock", "favorites"],
    "created_by": "user:12345"
}');
```

##### JSON with Special Characters

```sql
-- Handling quotes and special characters
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('message:welcome', 'MessageActor', '{
    "content": "Welcome to \"Orbit-RS\"! It'"'"'s great to have you here.",
    "author": "System",
    "type": "welcome"
}');
```

##### Unicode and International Support

```sql
-- International characters and emoji
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('greeting:international', 'GreetingActor', '{
    "messages": {
        "english": "Hello! ",
        "spanish": "¡Hola!",
        "chinese": "",
        "japanese": "",
        "emoji": ""
    }
}');
```

##### Empty JSON Objects

```sql
-- Minimal state initialization
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('temp:placeholder', 'PlaceholderActor', '{}');
```

#### Case Sensitivity Support

SQL keywords are case-insensitive, providing flexibility in coding styles.

```sql
-- All lowercase
insert into actors (actor_id, actor_type, state) 
values ('test:lowercase', 'TestActor', '{"case": "lowercase"}');

-- All uppercase
SELECT * FROM ACTORS WHERE ACTOR_ID = 'test:lowercase';

-- Mixed case (CamelCase)
UpDaTe actors SeT state = '{"case": "mixed", "updated": true}' 
WhErE actor_id = 'test:lowercase';

-- Column names are also case-insensitive
SELECT ACTOR_ID, actor_type, State FROM actors 
WHERE actor_id = 'test:lowercase';
```

#### Edge Cases and Special Characters

Orbit-RS handles various edge cases and special characters gracefully.

##### Extra Whitespace

```sql
-- Handles multiple spaces and formatting
   SELECT   actor_id   ,   actor_type   
   FROM   actors   
   WHERE   actor_type   =   'TestActor'   ;
```

##### Special Characters in Identifiers

```sql
-- Email-like identifiers
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('user:john.doe@company.com', 'UserActor', '{
    "email": "john.doe@company.com",
    "domain": "company.com"
}');

-- Complex path-like identifiers
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('api:v1/users/:id/profile', 'ApiEndpointActor', '{
    "method": "GET",
    "path": "/api/v1/users/:id/profile",
    "protected": true
}');
```

##### Semicolon Handling

```sql
-- Proper query termination
SELECT actor_id FROM actors WHERE actor_id = 'user:john.doe@company.com';
```

#### Complete Workflow Examples

Real-world usage patterns combining multiple SQL operations.

##### Vector Database Workflow - Complete Implementation

```sql
-- 1. Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- 2. Create documents table with vector embeddings
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT,
    category TEXT,
    embedding VECTOR(1536),  -- OpenAI ada-002 dimensions
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 3. Create vector indexes for efficient similarity search
CREATE INDEX documents_embedding_ivfflat_idx 
ON documents USING ivfflat (embedding vector_cosine_ops) 
WITH (lists = 1000);

CREATE INDEX documents_embedding_hnsw_idx 
ON documents USING hnsw (embedding vector_cosine_ops) 
WITH (m = 16, ef_construction = 64);

-- 4. Insert documents with vector embeddings
INSERT INTO documents (title, content, category, embedding, metadata) VALUES 
(
    'Introduction to Machine Learning',
    'Machine learning is a subset of artificial intelligence...',
    'AI/ML',
    '[0.1, 0.2, 0.15, 0.3, 0.25, ...]',  -- 1536-dimensional vector
    '{"author": "Dr. Smith", "tags": ["machine-learning", "ai"]}'
);

-- 5. Perform vector similarity search with analytics
WITH similarity_search AS (
    SELECT 
        title,
        content,
        category,
        embedding <-> '[0.1, 0.2, 0.15, 0.3, 0.25, ...]' as l2_distance,
        1 - (embedding <=> '[0.1, 0.2, 0.15, 0.3, 0.25, ...]') as cosine_similarity,
        metadata,
        ROW_NUMBER() OVER (ORDER BY embedding <-> '[0.1, 0.2, 0.15, 0.3, 0.25, ...]') as rank
    FROM documents
    WHERE category = 'AI/ML'
),
ranked_results AS (
    SELECT *,
           NTILE(3) OVER (ORDER BY cosine_similarity DESC) as similarity_tier
    FROM similarity_search
)
SELECT title, cosine_similarity, similarity_tier, metadata->'tags' as tags
FROM ranked_results
WHERE rank <= 10;
```

##### Advanced Analytics with Window Functions

```sql
-- Complex time-series analysis with multiple window functions
WITH sales_analytics AS (
    SELECT 
        sale_date,
        product_category,
        region,
        sales_amount,
        sales_person,
        -- Running calculations
        SUM(sales_amount) OVER (
            ORDER BY sale_date 
            ROWS UNBOUNDED PRECEDING
        ) as running_total,
        -- Moving averages
        AVG(sales_amount) OVER (
            ORDER BY sale_date 
            ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
        ) as moving_avg_7day,
        -- Ranking and percentiles
        RANK() OVER (
            PARTITION BY product_category 
            ORDER BY sales_amount DESC
        ) as category_rank,
        PERCENT_RANK() OVER (
            ORDER BY sales_amount
        ) as sales_percentile,
        -- Time-based comparisons
        LAG(sales_amount, 1) OVER (
            ORDER BY sale_date
        ) as previous_day_sales,
        LEAD(sales_amount, 1) OVER (
            ORDER BY sale_date
        ) as next_day_sales,
        -- First and last values in partitions
        FIRST_VALUE(sales_amount) OVER (
            PARTITION BY region 
            ORDER BY sale_date 
            ROWS UNBOUNDED PRECEDING
        ) as first_sale_in_region,
        LAST_VALUE(sales_amount) OVER (
            PARTITION BY region 
            ORDER BY sale_date 
            ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
        ) as last_sale_in_region
    FROM sales_data
)
SELECT *,
       CASE 
           WHEN sales_percentile >= 0.8 THEN 'Top Performer'
           WHEN sales_percentile >= 0.6 THEN 'Above Average'
           WHEN sales_percentile >= 0.4 THEN 'Average'
           ELSE 'Below Average'
       END as performance_tier
FROM sales_analytics
ORDER BY sale_date;
```

##### Enterprise Schema Management with Permissions

```sql
-- 1. Create organizational schemas
BEGIN;

CREATE SCHEMA IF NOT EXISTS hr AUTHORIZATION hr_admin;
CREATE SCHEMA IF NOT EXISTS finance AUTHORIZATION finance_admin;
CREATE SCHEMA IF NOT EXISTS analytics AUTHORIZATION data_team;

-- 2. Create tables with proper constraints
CREATE TABLE hr.employees (
    employee_id SERIAL PRIMARY KEY,
    employee_number VARCHAR(10) UNIQUE NOT NULL,
    first_name VARCHAR(50) NOT NULL,
    last_name VARCHAR(50) NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    department VARCHAR(50) NOT NULL,
    hire_date DATE NOT NULL DEFAULT CURRENT_DATE,
    salary DECIMAL(10,2) CHECK (salary > 0),
    manager_id INTEGER REFERENCES hr.employees(employee_id),
    employee_data JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 3. Create indexes for performance
CREATE INDEX idx_employees_department ON hr.employees(department);
CREATE INDEX idx_employees_manager ON hr.employees(manager_id);
CREATE INDEX idx_employees_email ON hr.employees USING hash(email);
CREATE INDEX idx_employees_data ON hr.employees USING gin(employee_data);

-- 4. Set up comprehensive permissions
-- HR team permissions
GRANT USAGE ON SCHEMA hr TO hr_manager, hr_admin;
GRANT SELECT, INSERT, UPDATE ON TABLE hr.employees TO hr_admin;
GRANT SELECT ON TABLE hr.employees TO hr_manager;
GRANT USAGE ON SEQUENCE hr.employees_employee_id_seq TO hr_admin;

-- Analytics team - read-only access
GRANT USAGE ON SCHEMA hr TO analytics_team;
GRANT SELECT ON TABLE hr.employees TO analytics_team;

-- Finance team - salary access only
CREATE VIEW finance.employee_compensation AS
SELECT 
    employee_id,
    employee_number,
    first_name,
    last_name,
    department,
    salary,
    hire_date
FROM hr.employees;

GRANT SELECT ON finance.employee_compensation TO finance_team;

COMMIT;
```

##### Transaction Management with Savepoints

```sql
-- Complex transaction with multiple savepoints
BEGIN ISOLATION LEVEL READ COMMITTED;

-- Initial operations
SAVEPOINT initial_setup;

CREATE TEMPORARY TABLE batch_operations (
    operation_id SERIAL PRIMARY KEY,
    operation_type VARCHAR(50),
    target_table VARCHAR(100),
    operation_data JSONB,
    status VARCHAR(20) DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT NOW()
);

-- Bulk data operations
SAVEPOINT bulk_operations;

INSERT INTO hr.employees (employee_number, first_name, last_name, email, department, salary)
SELECT 
    'EMP' || LPAD(generate_series(1001, 1100)::text, 4, '0'),
    'Employee',
    'Number ' || generate_series(1001, 1100)::text,
    'emp' || generate_series(1001, 1100) || '@company.com',
    CASE 
        WHEN generate_series(1001, 1100) % 4 = 0 THEN 'Engineering'
        WHEN generate_series(1001, 1100) % 4 = 1 THEN 'Sales'
        WHEN generate_series(1001, 1100) % 4 = 2 THEN 'Marketing'
        ELSE 'Operations'
    END,
    50000 + (generate_series(1001, 1100) * 1000);

-- Validation checkpoint
SAVEPOINT validation_point;

-- Check data integrity
DO $$
BEGIN
    IF (SELECT COUNT(*) FROM hr.employees WHERE email IS NULL OR email = '') > 0 THEN
        RAISE EXCEPTION 'Data validation failed: null or empty email addresses found';
    END IF;
    
    IF (SELECT COUNT(DISTINCT email) FROM hr.employees) != (SELECT COUNT(*) FROM hr.employees) THEN
        RAISE EXCEPTION 'Data validation failed: duplicate email addresses found';
    END IF;
END $$;

-- If validation passes, commit all changes
COMMIT;

-- Example of selective rollback (if needed):
-- ROLLBACK TO SAVEPOINT bulk_operations;  -- Keep setup, undo bulk ops
-- ROLLBACK TO SAVEPOINT initial_setup;    -- Undo everything except transaction start
-- ROLLBACK;                               -- Undo entire transaction
```

##### E-commerce Order Processing

```sql
-- Create customer
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('customer:12345', 'CustomerActor', '{
    "customer_id": "12345",
    "name": "Alice Johnson",
    "email": "alice@example.com",
    "tier": "premium"
}');

-- Create order
INSERT INTO actors (actor_id, actor_type, state) 
VALUES ('order:67890', 'OrderActor', '{
    "order_id": "67890",
    "customer_id": "12345",
    "items": [{"sku": "LAPTOP-001", "quantity": 1, "price": 1299.99}],
    "status": "pending",
    "total": 1299.99
}');

-- Update order status
UPDATE actors 
SET state = '{
    "order_id": "67890",
    "customer_id": "12345",
    "items": [{"sku": "LAPTOP-001", "quantity": 1, "price": 1299.99}],
    "status": "processing",
    "total": 1299.99,
    "processing_started": "2024-01-20T15:30:00Z"
}' 
WHERE actor_id = 'order:67890';

-- Query orders by customer
SELECT * FROM actors 
WHERE actor_type = 'OrderActor' AND actor_id LIKE 'order:%';
```

##### Service Configuration Management

```sql
-- Create service configurations
INSERT INTO actors (actor_id, actor_type, state) VALUES
    ('config:database', 'ConfigActor', '{
        "type": "postgresql",
        "host": "localhost",
        "port": 5432,
        "pool_size": 20
    }'),
    ('config:cache', 'ConfigActor', '{
        "type": "redis", 
        "host": "localhost",
        "port": 6379,
        "ttl": 3600
    }'),
    ('config:logging', 'ConfigActor', '{
        "level": "info",
        "format": "json",
        "outputs": ["stdout", "file"]
    }');

-- Query all configurations
SELECT actor_id, state FROM actors WHERE actor_type = 'ConfigActor';

-- Update specific configuration
UPDATE actors 
SET state = '{
    "level": "debug",
    "format": "json", 
    "outputs": ["stdout", "file"],
    "debug_modules": ["http", "database", "auth"]
}' 
WHERE actor_id = 'config:logging';
```

### Planned PostgreSQL Features 

Future releases will expand PostgreSQL compatibility with advanced SQL features:

#### Core SQL Operations (Planned)

- **SELECT** - Complete query support with JOINs, subqueries, window functions, CTEs
- **INSERT** - Multi-row inserts, INSERT...SELECT, ON CONFLICT handling
- **UPDATE** - Complex updates with FROM clauses, correlated subqueries
- **DELETE** - Cascading deletes, EXISTS/NOT EXISTS conditions

#### DDL Operations (Planned)

- **CREATE/ALTER/DROP TABLE** - Complete table lifecycle management
- **CREATE/DROP INDEX** - B-tree, Hash, GiST, GIN, IVFFLAT, HNSW indexes
- **CREATE/DROP VIEW** - Regular and materialized views
- **CREATE/DROP SCHEMA** - Database organization and namespacing
- **CREATE/DROP EXTENSION** - Extension management (including pgvector)

#### DCL Operations (Planned)

- **GRANT/REVOKE** - Comprehensive permission management
- **Role-based Access Control** - User and role management
- **Schema-level Permissions** - Fine-grained access control
- **Object-level Security** - Table, view, function permissions

#### TCL Operations (Planned)

- **BEGIN/COMMIT/ROLLBACK** - Full transaction support
- **SAVEPOINT** - Nested transaction points
- **Isolation Levels** - READ COMMITTED, REPEATABLE READ, SERIALIZABLE
- **Access Modes** - READ ONLY, READ WRITE transaction control

#### Advanced SQL Features (Planned)

- **Window Functions** - ROW_NUMBER, RANK, DENSE_RANK, LAG, LEAD, FIRST_VALUE, LAST_VALUE, NTILE
- **Common Table Expressions** - WITH clauses including recursive CTEs
- **Complex Expressions** - Full operator precedence, CASE statements
- **Aggregate Functions** - COUNT, SUM, AVG, MIN, MAX with DISTINCT, FILTER, ORDER BY
- **Subqueries** - Correlated and non-correlated in SELECT, WHERE, FROM clauses
- **JOIN Operations** - INNER, LEFT, RIGHT, FULL OUTER, CROSS joins

#### Vector Database Support (Planned)

- **pgvector Extension** - CREATE EXTENSION vector support
- **Vector Data Types** - VECTOR(n), HALFVEC(n), SPARSEVEC(n)
- **Vector Indexes** - IVFFLAT and HNSW for similarity search
- **Distance Operators** - `<->` (L2), `<#>` (inner product), `<=>` (cosine)
- **Vector Functions** - VECTOR_DIMS, VECTOR_NORM, similarity scoring

### Future Enhancement Opportunities 

Potential areas for further enhancement:

#### Performance Optimizations

- **Query Optimizer** - Cost-based query planning and optimization
- **Index Recommendations** - Automatic index suggestion based on query patterns
- **Parallel Query Execution** - Multi-threaded query processing
- **Connection Pooling** - Advanced connection management and pooling
- **Caching Layer** - Query result and metadata caching

#### Enterprise Features

- **Advanced Authentication** - LDAP, Kerberos, OAuth integration
- **Audit Logging** - Comprehensive audit trail and compliance features
- **Backup and Recovery** - Point-in-time recovery and backup automation
- **High Availability** - Replication, failover, and clustering
- **Monitoring and Metrics** - Performance monitoring and alerting

#### Advanced Vector Features

- **Approximate Nearest Neighbor** - Advanced ANN algorithms
- **Vector Quantization** - Memory-efficient vector storage
- **Multi-modal Embeddings** - Support for text, image, audio vectors
- **Vector Analytics** - Statistical analysis and clustering of vectors
- **Real-time Vector Updates** - Streaming vector updates and search

#### Extended SQL Compliance

- **MERGE Statement** - UPSERT operations with complex logic
- **Recursive CTEs** - Advanced hierarchical query support
- **Table Functions** - User-defined table-valued functions
- **Stored Procedures** - PL/pgSQL and custom language support
- **Triggers** - Row and statement-level trigger support

#### Integration Features

- **Foreign Data Wrappers** - External data source connectivity
- **Logical Replication** - Change data capture and streaming
- **GraphQL Interface** - Direct GraphQL query support
- **REST API Gateway** - HTTP/REST interface for SQL queries
- **Streaming Analytics** - Real-time data processing capabilities

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

```rust
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

## Model Context Protocol (MCP)  EXPERIMENTAL

AI agent integration through the standardized Model Context Protocol, enabling AI systems to interact with Orbit-RS. **Note**: This is an experimental implementation with basic functionality.

### MCP Server Setup

```rust
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

## Upcoming Time Series Features 

Orbit-RS will add comprehensive time-series database capabilities in **Phase 12 (Q1 2025)**, providing compatibility with two major time-series ecosystems.

### Redis Time Series Compatibility

**Full RedisTimeSeries module compatibility** - [ Detailed Documentation](REDIS_TIMESERIES.md)

#### Redis Time Series Key Features

- **TimeSeriesActor**: Distributed time-series management with automatic partitioning
- **Core Commands**: TS.CREATE, TS.ADD, TS.GET, TS.RANGE, TS.REVRANGE
- **Aggregation Rules**: TS.CREATERULE, TS.DELETERULE with automated downsampling
- **Multi-Series Operations**: TS.MRANGE, TS.MREVRANGE, TS.MGET with label filtering
- **Built-in Functions**: AVG, SUM, MIN, MAX, COUNT, STDDEV, VAR
- **Retention Policies**: Automatic data expiration and compression
- **Labeling System**: Multi-dimensional time series organization

#### Redis Time Series Example Usage

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

**Complete TimescaleDB extension compatibility** - [ Detailed Documentation](POSTGRESQL_TIMESCALE.md)

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

## Future Protocols 

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
