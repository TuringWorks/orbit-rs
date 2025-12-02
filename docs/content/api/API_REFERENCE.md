---
layout: default
title: "API Reference"
subtitle: "Getting Started with Orbit-RS APIs"
category: "api"
permalink: /api/
---

# API Reference

Orbit-RS provides multiple API interfaces for different use cases. This guide covers all available APIs and how to get started with each.

## Table of Contents

- [Overview](#overview)
- [Client APIs](#client-apis)
  - [Orbit Client API](#orbit-client-api)
  - [Actor System API](#actor-system-api)
  - [Transaction API](#transaction-api)
- [Protocol APIs](#protocol-apis)
  - [PostgreSQL Wire Protocol](#postgresql-wire-protocol)
  - [Redis RESP Protocol](#redis-resp-protocol)
  - [MySQL Wire Protocol](#mysql-wire-protocol)
  - [CQL (Cassandra) Protocol](#cql-cassandra-protocol)
  - [REST API](#rest-api)
  - [gRPC API](#grpc-api)
- [Integration APIs](#integration-apis)
  - [Model Context Protocol (MCP)](#model-context-protocol-mcp)
- [Project Structure](#project-structure)

---

## Overview

Orbit-RS is designed with a layered API architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
├─────────────────────────────────────────────────────────────┤
│  Orbit Client │ Actor API │ Transaction API │ MCP API       │
├─────────────────────────────────────────────────────────────┤
│                    Protocol Layer                            │
├──────────┬──────────┬──────────┬──────────┬────────────────┤
│ PostgreSQL│  Redis  │  MySQL   │   CQL    │  REST │ gRPC   │
│   :5432   │  :6379  │  :3306   │  :9042   │ :8080 │ :50051 │
├──────────┴──────────┴──────────┴──────────┴────────────────┤
│                    Storage Layer                             │
│              RocksDB + Tiered Storage + MVCC                 │
└─────────────────────────────────────────────────────────────┘
```

---

## Client APIs

### Orbit Client API

The primary Rust client library for interacting with Orbit-RS.

**Installation:**

```toml
# Cargo.toml
[dependencies]
orbit-client = { git = "https://github.com/TuringWorks/orbit-rs" }
orbit-shared = { git = "https://github.com/TuringWorks/orbit-rs" }
```

**Basic Usage:**

```rust
use orbit_client::{OrbitClient, OrbitClientConfig};
use orbit_shared::Key;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the client
    let config = OrbitClientConfig {
        namespace: "my-app".to_string(),
        server_urls: vec!["http://localhost:50051".to_string()],
        connection_timeout: std::time::Duration::from_secs(10),
        request_timeout: std::time::Duration::from_secs(30),
        ..Default::default()
    };

    // Create client instance
    let client = OrbitClient::new(config).await?;

    // Get an actor reference
    let actor = client.actor_reference::<dyn MyActor>(
        Key::StringKey { key: "my-actor-1".to_string() }
    ).await?;

    // Invoke actor method
    let result = actor.my_method("hello").await?;
    println!("Result: {}", result);

    Ok(())
}
```

**Configuration Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `namespace` | `String` | `"default"` | Actor namespace for isolation |
| `server_urls` | `Vec<String>` | `[]` | gRPC server endpoints |
| `connection_timeout` | `Duration` | `10s` | Connection establishment timeout |
| `request_timeout` | `Duration` | `30s` | Individual request timeout |
| `max_retries` | `u32` | `3` | Maximum retry attempts |
| `enable_tls` | `bool` | `false` | Enable TLS encryption |

---

### Actor System API

Distributed actor interactions for building scalable applications.

**Defining an Actor:**

```rust
use orbit_shared::{ActorWithStringKey, Key, OrbitError};
use async_trait::async_trait;

// Define actor trait
#[async_trait]
pub trait CounterActor: ActorWithStringKey {
    async fn increment(&self, amount: i64) -> Result<i64, OrbitError>;
    async fn get_value(&self) -> Result<i64, OrbitError>;
    async fn reset(&self) -> Result<(), OrbitError>;
}

// Implement actor
pub struct CounterActorImpl {
    value: std::sync::atomic::AtomicI64,
}

#[async_trait]
impl CounterActor for CounterActorImpl {
    async fn increment(&self, amount: i64) -> Result<i64, OrbitError> {
        let new_value = self.value.fetch_add(amount, std::sync::atomic::Ordering::SeqCst) + amount;
        Ok(new_value)
    }

    async fn get_value(&self) -> Result<i64, OrbitError> {
        Ok(self.value.load(std::sync::atomic::Ordering::SeqCst))
    }

    async fn reset(&self) -> Result<(), OrbitError> {
        self.value.store(0, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}
```

**Actor Lifecycle:**

```rust
// Actors are activated on-demand
let counter = client.actor_reference::<dyn CounterActor>(
    Key::StringKey { key: "counter-1".to_string() }
).await?;

// First call activates the actor
let value = counter.increment(10).await?;

// Actor remains active for configured timeout
// Then automatically deactivates and persists state
```

**Actor Keys:**

```rust
use orbit_shared::Key;

// String-based keys
let key = Key::StringKey { key: "user-123".to_string() };

// Integer keys
let key = Key::Int64Key { key: 12345 };

// Composite keys
let key = Key::CompositeKey {
    keys: vec![
        Key::StringKey { key: "region-us".to_string() },
        Key::Int64Key { key: 42 },
    ]
};
```

---

### Transaction API

Distributed ACID transaction management with 2PC and Saga patterns.

**Two-Phase Commit:**

```rust
use orbit_shared::transactions::{
    TransactionCoordinator, TransactionConfig, TransactionOperation
};

// Create transaction coordinator
let coordinator = TransactionCoordinator::new(node_id, config, transport);

// Begin transaction
let tx_id = coordinator.begin_transaction(Some(Duration::from_secs(30))).await?;

// Add operations
coordinator.add_operation(&tx_id, TransactionOperation::new(
    account_a_ref,
    "debit".to_string(),
    serde_json::json!({"amount": 100}),
).with_compensation(serde_json::json!({"amount": 100, "action": "credit"}))).await?;

coordinator.add_operation(&tx_id, TransactionOperation::new(
    account_b_ref,
    "credit".to_string(),
    serde_json::json!({"amount": 100}),
).with_compensation(serde_json::json!({"amount": 100, "action": "debit"}))).await?;

// Commit (or rollback on failure)
coordinator.commit_transaction(&tx_id).await?;
```

**Saga Pattern:**

```rust
use orbit_shared::transactions::saga::{SagaCoordinator, SagaStep};

let saga = SagaCoordinator::new();

// Define saga steps with compensations
saga.add_step(SagaStep::new("reserve_inventory", reserve_fn, cancel_reserve_fn));
saga.add_step(SagaStep::new("charge_payment", charge_fn, refund_fn));
saga.add_step(SagaStep::new("send_confirmation", send_fn, void_fn));

// Execute saga
saga.execute(order_data).await?;
```

---

## Protocol APIs

### PostgreSQL Wire Protocol

Full PostgreSQL compatibility on port 5432.

**Connection:**

```bash
# Using psql
psql -h localhost -p 5432 -U orbit -d actors

# Using connection string
psql "postgresql://orbit:orbit@localhost:5432/actors"
```

**Supported Features:**

```sql
-- DDL Operations
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    email VARCHAR(255) UNIQUE,
    metadata JSONB,
    embedding VECTOR(384),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_embedding ON users USING ivfflat (embedding vector_cosine_ops);

-- DML Operations
INSERT INTO users (name, email, metadata, embedding) VALUES
    ('Alice', 'alice@example.com', '{"role": "admin"}', '[0.1, 0.2, ...]');

SELECT * FROM users WHERE metadata->>'role' = 'admin';

-- Vector Operations (pgvector compatible)
SELECT id, name, embedding <-> '[0.1, 0.2, ...]' AS distance
FROM users
ORDER BY embedding <-> '[0.1, 0.2, ...]'
LIMIT 10;

-- Transactions
BEGIN;
UPDATE accounts SET balance = balance - 100 WHERE id = 1;
UPDATE accounts SET balance = balance + 100 WHERE id = 2;
COMMIT;

-- Window Functions
SELECT name,
       SUM(amount) OVER (PARTITION BY category ORDER BY date) as running_total
FROM transactions;
```

**Python Example:**

```python
import psycopg2

conn = psycopg2.connect(
    host="localhost",
    port=5432,
    user="orbit",
    password="orbit",
    database="actors"
)

cursor = conn.cursor()
cursor.execute("SELECT * FROM users WHERE name = %s", ("Alice",))
rows = cursor.fetchall()
```

---

### Redis RESP Protocol

Full Redis compatibility on port 6379 with 124+ commands.

**Connection:**

```bash
redis-cli -h localhost -p 6379
```

**String Operations:**

```bash
SET user:1:name "Alice"
GET user:1:name
MSET key1 "value1" key2 "value2"
MGET key1 key2
INCR counter
INCRBY counter 10
APPEND key "additional text"
```

**Hash Operations:**

```bash
HSET user:1 name "Alice" email "alice@example.com" role "admin"
HGET user:1 name
HGETALL user:1
HMSET user:2 name "Bob" email "bob@example.com"
HINCRBY user:1 login_count 1
```

**List Operations:**

```bash
LPUSH queue task1 task2 task3
RPOP queue
LRANGE queue 0 -1
LLEN queue
```

**Set Operations:**

```bash
SADD tags:post:1 rust database distributed
SMEMBERS tags:post:1
SISMEMBER tags:post:1 rust
SUNION tags:post:1 tags:post:2
```

**Sorted Set Operations:**

```bash
ZADD leaderboard 100 player1 85 player2 92 player3
ZRANGE leaderboard 0 -1 WITHSCORES
ZRANGEBYSCORE leaderboard 90 100
ZRANK leaderboard player1
```

**Time Series (RedisTimeSeries Compatible):**

```bash
TS.CREATE temperature:sensor1 RETENTION 86400000 LABELS sensor_id 1 location warehouse
TS.ADD temperature:sensor1 * 23.5
TS.RANGE temperature:sensor1 - + AGGREGATION avg 3600000
TS.MRANGE - + FILTER location=warehouse
```

**Vector Operations:**

```bash
VECTOR.ADD embeddings doc1 "0.1,0.2,0.3,0.4" content "Machine learning guide"
VECTOR.SEARCH embeddings "0.1,0.2,0.3,0.4" 5 METRIC COSINE
VECTOR.GET embeddings doc1
```

**Python Example:**

```python
import redis

r = redis.Redis(host='localhost', port=6379)

# String operations
r.set('mykey', 'myvalue')
value = r.get('mykey')

# Hash operations
r.hset('user:1', mapping={'name': 'Alice', 'email': 'alice@example.com'})
user = r.hgetall('user:1')

# Time series
r.execute_command('TS.ADD', 'sensor:temp', '*', 23.5)
```

---

### MySQL Wire Protocol

MySQL client compatibility on port 3306.

**Connection:**

```bash
mysql -h localhost -P 3306 -u orbit -p
```

**Usage:**

```sql
-- Create database and tables
CREATE DATABASE myapp;
USE myapp;

CREATE TABLE products (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    price DECIMAL(10,2),
    stock INT DEFAULT 0
);

-- Insert data
INSERT INTO products (name, price, stock) VALUES ('Widget', 19.99, 100);

-- Query data
SELECT * FROM products WHERE stock > 0;
```

---

### CQL (Cassandra) Protocol

Cassandra Query Language on port 9042.

**Connection:**

```bash
cqlsh localhost 9042
```

**Usage:**

```sql
-- Create keyspace
CREATE KEYSPACE myapp WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE myapp;

-- Create table
CREATE TABLE users (
    user_id UUID PRIMARY KEY,
    name TEXT,
    email TEXT,
    created_at TIMESTAMP
);

-- Insert data
INSERT INTO users (user_id, name, email, created_at)
VALUES (uuid(), 'Alice', 'alice@example.com', toTimestamp(now()));

-- Query data
SELECT * FROM users WHERE user_id = ?;
```

---

### REST API

HTTP/JSON API on port 8080.

**Health Check:**

```bash
curl http://localhost:8080/health
# {"status": "healthy", "version": "1.0.0"}
```

**Metrics:**

```bash
curl http://localhost:8080/metrics
# Prometheus-format metrics
```

**Data Operations:**

```bash
# Query data
curl "http://localhost:8080/api/tables/users"

# Insert data
curl -X POST "http://localhost:8080/api/tables/users" \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "email": "alice@example.com"}'

# Vector search
curl -X POST "http://localhost:8080/api/vectors/search" \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "embeddings",
    "vector": [0.1, 0.2, 0.3],
    "limit": 10,
    "metric": "cosine"
  }'
```

---

### gRPC API

High-performance actor communication on port 50051.

**Service Definition:**

```protobuf
// orbit.proto
service ActorService {
    rpc Invoke(InvocationRequest) returns (InvocationResponse);
    rpc Stream(stream Message) returns (stream Message);
}

message InvocationRequest {
    string actor_type = 1;
    string actor_key = 2;
    string method = 3;
    bytes arguments = 4;
}

message InvocationResponse {
    bytes result = 1;
    string error = 2;
}
```

**Using grpcurl:**

```bash
# List services
grpcurl -plaintext localhost:50051 list

# Invoke actor
grpcurl -plaintext -d '{
  "actor_type": "CounterActor",
  "actor_key": "counter-1",
  "method": "increment",
  "arguments": "eyJhbW91bnQiOiAxMH0="
}' localhost:50051 orbit.ActorService/Invoke
```

---

## Integration APIs

### Model Context Protocol (MCP)

AI agent integration for LLM-powered applications.

**Capabilities:**

- **Resource Access**: Query databases through natural language
- **Tool Execution**: Execute SQL, Redis commands, or actor methods
- **Context Management**: Maintain conversation state
- **Streaming**: Real-time response streaming

**Configuration:**

```json
{
  "mcpServers": {
    "orbit-rs": {
      "command": "orbit-mcp-server",
      "args": ["--host", "localhost", "--port", "50051"],
      "env": {
        "ORBIT_NAMESPACE": "ai-assistant"
      }
    }
  }
}
```

**Available Tools:**

| Tool | Description |
|------|-------------|
| `query_sql` | Execute SQL queries |
| `query_redis` | Execute Redis commands |
| `invoke_actor` | Call actor methods |
| `search_vectors` | Semantic similarity search |
| `get_schema` | Retrieve database schema |

---

## Project Structure

```
orbit-rs/
├── orbit/                      # Main source code (15 workspace crates)
│   ├── server/                 # Main server binary
│   │   ├── src/
│   │   │   ├── main.rs        # Entry point
│   │   │   ├── protocols/     # Protocol implementations
│   │   │   │   ├── resp/      # Redis RESP protocol
│   │   │   │   ├── postgres/  # PostgreSQL wire protocol
│   │   │   │   ├── mysql/     # MySQL wire protocol
│   │   │   │   └── cql/       # Cassandra CQL protocol
│   │   │   ├── persistence/   # Storage backends
│   │   │   └── ai/            # AI subsystems
│   │   └── Cargo.toml
│   │
│   ├── client/                 # Client library
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── client.rs      # OrbitClient implementation
│   │   │   └── invocation.rs  # Actor invocation system
│   │   └── Cargo.toml
│   │
│   ├── shared/                 # Shared types and traits
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── actor.rs       # Actor traits
│   │   │   ├── transactions/  # Transaction types
│   │   │   ├── error.rs       # Error types
│   │   │   └── key.rs         # Key types
│   │   └── Cargo.toml
│   │
│   ├── engine/                 # Storage engine
│   │   └── src/
│   │       ├── storage/       # Storage backends
│   │       ├── transaction/   # MVCC transactions
│   │       └── adapters/      # Protocol adapters
│   │
│   ├── compute/                # Hardware acceleration
│   │   └── src/
│   │       ├── simd.rs        # SIMD operations
│   │       ├── gpu_metal.rs   # Apple Metal
│   │       └── gpu_cuda.rs    # NVIDIA CUDA
│   │
│   ├── ml/                     # Machine learning
│   │   └── src/
│   │       ├── inference/     # Model inference
│   │       └── sql_extensions/ # SQL ML functions
│   │
│   ├── proto/                  # Protocol Buffers
│   │   └── src/
│   │       └── orbit.proto    # gRPC definitions
│   │
│   ├── cli/                    # Interactive CLI
│   ├── operator/               # Kubernetes operator
│   └── util/                   # Core utilities
│
├── config/                     # Configuration files
│   ├── orbit-server.toml      # Production config
│   └── orbit-server-example.toml
│
├── docs/                       # Documentation
│   ├── index.md               # Home page
│   ├── quick_start.md         # Quick start guide
│   └── content/               # Detailed documentation
│
├── tests/                      # Integration tests
├── benchmarks/                 # Performance benchmarks
├── helm/                       # Kubernetes Helm charts
└── k8s/                        # Kubernetes manifests
```

**Key Crates:**

| Crate | Description |
|-------|-------------|
| `orbit-server` | Main server binary with all protocols |
| `orbit-client` | Rust client library |
| `orbit-shared` | Shared types, traits, and error handling |
| `orbit-engine` | Unified storage engine |
| `orbit-compute` | Hardware acceleration (SIMD, GPU) |
| `orbit-ml` | Machine learning inference |
| `orbit-proto` | Protocol Buffer definitions |
| `orbit-cli` | Interactive command-line interface |
| `orbit-operator` | Kubernetes operator |

---

## Next Steps

- [Quick Start Guide](../quick_start.md) - Get running in 30 seconds
- [PostgreSQL Compatibility](../protocols/POSTGRES_WIRE_IMPLEMENTATION.md) - SQL features
- [Redis Commands Reference](../protocols/REDIS_COMMANDS_REFERENCE.md) - All 124+ commands
- [Transaction Features](../../planning/features/transaction_features.md) - Distributed transactions
- [Migration Guide](../migration/MIGRATION_GUIDE.md) - Migrate from other databases

---

**Need help?** Open an issue at [GitHub](https://github.com/TuringWorks/orbit-rs/issues)
