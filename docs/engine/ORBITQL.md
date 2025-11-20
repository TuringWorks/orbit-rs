# OrbitQL Integration Guide

**OrbitQL** is a unified multi-model query language that enables querying across documents, graphs, time-series, and key-value data in a single query.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Query Syntax](#query-syntax)
- [Data Access Patterns](#data-access-patterns)
- [Integration with orbit-engine](#integration-with-orbit-engine)
- [Examples](#examples)
- [Performance](#performance)

## Overview

OrbitQL provides a **unified query interface** for all data stored in Orbit, regardless of the underlying storage tier (hot/warm/cold) or data model (document/graph/time-series).

### Key Features

✅ **Multi-Model Queries** - Query documents, graphs, and time-series in one query
✅ **Cross-Model JOINs** - Relate data between different models seamlessly
✅ **Distributed Execution** - Optimized for actor-based systems
✅ **Real-Time Subscriptions** - Live query support with change notifications
✅ **ACID Transactions** - Multi-model transaction support
✅ **Tiered Storage Aware** - Automatically accesses hot/warm/cold tiers

### Comparison with Other Query Languages

| Feature | OrbitQL | SQL | MongoDB | SurrealQL | Cypher |
|---------|---------|-----|---------|-----------|--------|
| Document Queries | ✅ | ✅ | ✅ | ✅ | ❌ |
| Graph Traversals | ✅ | ❌ | ❌ | ✅ | ✅ |
| Time-Series | ✅ | ⚠️ | ⚠️ | ✅ | ❌ |
| Cross-Model JOINs | ✅ | ✅ | ❌ | ✅ | ❌ |
| Live Queries | ✅ | ❌ | ✅ | ✅ | ❌ |
| Distributed | ✅ | ⚠️ | ✅ | ✅ | ⚠️ |

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Applications                           │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    OrbitQL Layer                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│  │  Parser  │-→│ Optimizer│-→│  Planner │-→│ Executor │     │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘     │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   Protocol Adapters                         │
│  OrbitQL Adapter │ PostgreSQL │ Redis │ REST                │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Orbit Engine Core                        │
│  Storage (Hot/Warm/Cold) | Transactions | Clustering        │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Using OrbitQL Adapter with orbit-engine

```rust
use orbit_engine::adapters::{AdapterContext, OrbitQLAdapter};
use orbit_engine::storage::HybridStorageManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage engine
    let storage = Arc::new(HybridStorageManager::new_in_memory());

    // Create OrbitQL adapter
    let context = AdapterContext::new(
        storage as Arc<dyn orbit_engine::storage::TableStorage>
    );
    let adapter = OrbitQLAdapter::new(context);

    // Execute OrbitQL query
    let result = adapter.execute_query(
        "SELECT * FROM users WHERE age > 18"
    ).await?;

    println!("Results: {:?}", result);
    Ok(())
}
```

### 2. Using Existing OrbitQL Implementation

```rust
use orbit_shared::orbitql::{Parser, Lexer, QueryExecutor};

// Parse and execute OrbitQL directly
let query = "SELECT * FROM users";
let mut lexer = Lexer::new(query);
let tokens = lexer.tokenize()?;
let mut parser = Parser::new(tokens);
let statement = parser.parse()?;

// Execute through OrbitQL engine
// (existing implementation in orbit/shared)
```

## Query Syntax

### Basic SELECT

```orbitql
-- Simple SELECT
SELECT * FROM users;

-- With projection
SELECT name, email, age FROM users;

-- With WHERE clause
SELECT * FROM users WHERE age > 18 AND active = true;

-- With ORDER BY
SELECT * FROM users ORDER BY created_at DESC LIMIT 10;

-- With OFFSET and LIMIT
SELECT * FROM users OFFSET 10 LIMIT 20;
```

### INSERT

```orbitql
-- Insert single row (object syntax)
INSERT INTO users {
    id: 1,
    name: "Alice",
    email: "alice@example.com",
    age: 30
};

-- Insert multiple rows (values syntax)
INSERT INTO users (id, name, email, age) VALUES
    (1, "Alice", "alice@example.com", 30),
    (2, "Bob", "bob@example.com", 25);
```

### UPDATE

```orbitql
-- Update with WHERE clause
UPDATE users
SET age = 31, updated_at = NOW()
WHERE id = 1;

-- Update without WHERE (updates all)
UPDATE users SET active = true;
```

### DELETE

```orbitql
-- Delete with WHERE clause
DELETE FROM users WHERE age < 18;

-- Delete all
DELETE FROM users;
```

### CREATE TABLE

```orbitql
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name STRING NOT NULL,
    email STRING UNIQUE NOT NULL,
    age INTEGER,
    created_at TIMESTAMP DEFAULT NOW()
);
```

### Graph Traversals

```orbitql
-- Follow relationships
SELECT user->follows->user.name AS friends
FROM users;

-- Multi-hop traversal
SELECT user->follows->follows->user.name AS friends_of_friends
FROM users;

-- Recursive traversal with depth limits
SELECT user-<1,5>->follows->user.name AS network
FROM users
WHERE user.id = 123;
```

### Time-Series Queries

```orbitql
-- Query recent metrics
SELECT metrics[cpu_usage WHERE timestamp > NOW() - 1h]
FROM servers;

-- Aggregate time-series data
SELECT
    server_id,
    AVG(metrics[cpu_usage WHERE timestamp > NOW() - 1h]) AS avg_cpu
FROM servers
GROUP BY server_id;

-- Time windows
SELECT
    time_bucket('5 minutes', timestamp) AS bucket,
    AVG(cpu_usage) AS avg_cpu
FROM metrics
WHERE timestamp > NOW() - 24h
GROUP BY bucket;
```

### Cross-Model JOINs

```orbitql
-- Join documents with time-series
SELECT
    u.name,
    u.email,
    AVG(m.cpu_usage) AS avg_cpu
FROM users AS u
JOIN metrics AS m ON u.server_id = m.server_id
WHERE m.timestamp > NOW() - 1h
GROUP BY u.id;

-- Join with graph traversal
SELECT
    u.name,
    u->follows->user.name AS friends,
    metrics[cpu_usage] AS cpu
FROM users AS u
WHERE u.active = true;
```

### Live Queries (Subscriptions)

```orbitql
-- Subscribe to changes
LIVE SELECT * FROM users WHERE age > 18;

-- With update frequency
LIVE SELECT * FROM metrics
WHERE timestamp > NOW() - 5m
EVERY 10s;
```

## Data Access Patterns

### 1. Accessing Hot Tier Data (Recent)

```orbitql
-- Recent user activities (hot tier - in-memory)
SELECT * FROM user_activities
WHERE timestamp > NOW() - 24h
ORDER BY timestamp DESC;
```

**Performance**: < 1ms (in-memory)

### 2. Accessing Warm Tier Data (Medium-Age)

```orbitql
-- Last week's orders (warm tier - RocksDB)
SELECT * FROM orders
WHERE created_at > NOW() - 7d
  AND created_at < NOW() - 1d;
```

**Performance**: < 10ms (SSD)

### 3. Accessing Cold Tier Data (Historical)

```orbitql
-- Historical analytics (cold tier - Iceberg/Parquet)
SELECT
    DATE_TRUNC('day', created_at) AS day,
    COUNT(*) AS order_count,
    SUM(total) AS revenue
FROM orders
WHERE created_at > NOW() - 6month
GROUP BY day;
```

**Performance**: 10-100ms (columnar, compressed)

### 4. Cross-Tier Queries

OrbitQL automatically queries across all tiers:

```orbitql
-- Queries hot + warm + cold tiers automatically
SELECT
    customer_id,
    COUNT(*) AS total_orders,
    SUM(total) AS lifetime_value
FROM orders
WHERE created_at > NOW() - 1year
GROUP BY customer_id
ORDER BY lifetime_value DESC
LIMIT 100;
```

## Integration with orbit-engine

### Architecture Integration

```rust
// 1. OrbitQL Adapter (New)
//    orbit/engine/src/adapters/orbitql.rs
//    - Bridges OrbitQL to unified storage
//    - Type mapping OrbitQL ↔ SqlValue
//    - Filter conversion
//    - Query execution

// 2. Existing OrbitQL (orbit/shared)
//    - Parser and Lexer
//    - AST definitions
//    - Query optimizer
//    - Distributed planner

// 3. Protocol Layer (orbit/protocols)
//    - Wire protocol for OrbitQL
//    - Network serialization
//    - Client libraries
```

### Data Flow

```text
OrbitQL Query String
        ↓
    [Lexer] → Tokens
        ↓
    [Parser] → AST
        ↓
    [Optimizer] → Optimized Plan
        ↓
    [OrbitQL Adapter] → Storage API Calls
        ↓
    [Unified Storage] → Hot/Warm/Cold Tiers
        ↓
    QueryResult
```

### Type Mapping

| OrbitQL Type | Engine Type | Storage Format |
|--------------|-------------|----------------|
| STRING       | String      | UTF-8 |
| INTEGER      | Int64       | 64-bit signed |
| FLOAT        | Float64     | IEEE 754 |
| BOOLEAN      | Boolean     | 1 byte |
| TIMESTAMP    | Timestamp   | Unix epoch |
| BINARY       | Binary      | Byte array |
| ARRAY        | String      | JSON-encoded |
| OBJECT       | String      | JSON-encoded |

## Examples

### Example 1: E-Commerce Analytics

```orbitql
-- Find top customers with recent high-value orders
SELECT
    c.id,
    c.name,
    c.email,
    COUNT(o.id) AS order_count,
    SUM(o.total) AS total_spent,
    AVG(o.total) AS avg_order_value,
    MAX(o.created_at) AS last_order
FROM customers AS c
JOIN orders AS o ON c.id = o.customer_id
WHERE o.created_at > NOW() - 90d
  AND o.status = 'completed'
GROUP BY c.id
HAVING total_spent > 1000
ORDER BY total_spent DESC
LIMIT 100;
```

### Example 2: Social Network Analysis

```orbitql
-- Find influential users (high follower count, active)
SELECT
    u.id,
    u.name,
    COUNT(u->follows->user) AS follower_count,
    COUNT(u->posted->post WHERE post.created_at > NOW() - 30d) AS recent_posts,
    AVG(u->posted->post->likes->user) AS avg_likes
FROM users AS u
WHERE u.active = true
GROUP BY u.id
HAVING follower_count > 1000
ORDER BY follower_count DESC
LIMIT 50;
```

### Example 3: IoT Sensor Monitoring

```orbitql
-- Monitor sensors with anomalous readings
SELECT
    s.id,
    s.location,
    s.type,
    metrics[temperature WHERE timestamp > NOW() - 1h] AS temp_readings,
    AVG(metrics[temperature]) AS avg_temp,
    MAX(metrics[temperature]) AS max_temp
FROM sensors AS s
WHERE s.active = true
  AND MAX(metrics[temperature]) > 100  -- Anomaly threshold
GROUP BY s.id
ORDER BY max_temp DESC;
```

### Example 4: Real-Time Dashboard

```orbitql
-- Live query for dashboard metrics
LIVE SELECT
    COUNT(*) AS active_users,
    COUNT(WHERE status = 'online') AS online_users,
    AVG(metrics[response_time WHERE timestamp > NOW() - 5m]) AS avg_response_time,
    SUM(metrics[requests WHERE timestamp > NOW() - 5m]) AS total_requests
FROM users
EVERY 10s;
```

## Performance

### Query Performance by Data Tier

| Query Type | Hot Tier | Warm Tier | Cold Tier |
|------------|----------|-----------|-----------|
| Point Lookup | < 1ms | < 5ms | N/A |
| Filtered Scan | < 10ms | < 50ms | 50-200ms |
| Aggregation | < 20ms | 100-500ms | 200ms-2s |
| JOIN | < 50ms | 200ms-1s | 1-5s |
| Graph Traversal | < 100ms | 500ms-2s | 2-10s |

### Optimization Tips

#### 1. Use Appropriate Filters

```orbitql
-- Good: Filter pushdown to storage tier
SELECT * FROM orders WHERE created_at > NOW() - 1d;

-- Bad: Post-filtering in application
SELECT * FROM orders;  -- Then filter in app
```

#### 2. Leverage Indexes

```orbitql
-- Create index for frequent queries
CREATE INDEX idx_orders_customer ON orders(customer_id, created_at);

-- Query benefits from index
SELECT * FROM orders WHERE customer_id = 123;
```

#### 3. Use Projections

```orbitql
-- Good: Select only needed columns
SELECT id, name, email FROM users;

-- Bad: Select all columns
SELECT * FROM users;
```

#### 4. Batch Operations

```orbitql
-- Good: Single batch insert
INSERT INTO users VALUES (1, "A"), (2, "B"), (3, "C");

-- Bad: Multiple single inserts
INSERT INTO users VALUES (1, "A");
INSERT INTO users VALUES (2, "B");
INSERT INTO users VALUES (3, "C");
```

## Advanced Features

### Transactions

```orbitql
BEGIN TRANSACTION;

UPDATE accounts SET balance = balance - 100 WHERE id = 1;
UPDATE accounts SET balance = balance + 100 WHERE id = 2;

COMMIT;
```

### Parameterized Queries

```rust
let query = "SELECT * FROM users WHERE age > $age AND country = $country";
let params = QueryParams::new()
    .with("age", 18)
    .with("country", "US");

adapter.execute_query_with_params(query, params).await?;
```

### Query Profiling

```orbitql
-- Enable profiling
PROFILE SELECT
    c.name,
    SUM(o.total) AS revenue
FROM customers AS c
JOIN orders AS o ON c.id = o.customer_id
GROUP BY c.id;

-- Shows execution plan and timings
```

## Next Steps

- **Examples**: See [orbitql_example.rs](../examples/orbitql_example.rs)
- **Architecture**: See [ARCHITECTURE.md](ARCHITECTURE.md)
- **API Reference**: See [orbit/shared/src/orbitql/README.md](../../shared/src/orbitql/README.md)
- **Protocol Spec**: See [orbit/protocols/src/orbitql/](../../protocols/src/orbitql/)

## FAQ

**Q: Can OrbitQL access data from all storage tiers?**
A: Yes, OrbitQL automatically queries across hot/warm/cold tiers transparently.

**Q: Does OrbitQL support transactions?**
A: Yes, full ACID transactions with BEGIN/COMMIT/ROLLBACK.

**Q: Can I mix SQL and OrbitQL?**
A: OrbitQL is SQL-compatible for basic queries. Advanced features like graph traversals use OrbitQL-specific syntax.

**Q: How does OrbitQL compare to SurrealQL?**
A: OrbitQL is inspired by SurrealQL but optimized for distributed actor systems and tiered storage.

**Q: Can I use OrbitQL from other languages?**
A: Yes, through the OrbitQL wire protocol (orbit/protocols) or REST API.

---

**Related Documentation:**

- [Protocol Adapters Guide](ADAPTERS.md)
- [Deployment Guide](DEPLOYMENT.md)
- [Architecture Overview](ARCHITECTURE.md)
