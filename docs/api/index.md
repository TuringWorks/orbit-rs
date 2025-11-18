---
layout: default
title: "API Reference"
subtitle: "Complete Orbit-RS API Documentation"
category: "api"
permalink: /api/
---

## Complete API documentation for Orbit-RS distributed database system

---

## 🚀 Getting Started with the API

Orbit-RS provides multiple API interfaces for different use cases:

### Client APIs

- **[Orbit Client API](#-orbit-client-api)** - Primary Rust client library
- **[Actor System API](#-actor-system-api)** - Distributed actor interactions
- **[Transaction API](#-transaction-api)** - Distributed transaction management

### Protocol APIs  

- **[PostgreSQL Wire Protocol](#-postgresql-api)** - SQL database compatibility
- **[Redis RESP Protocol](#-redis-api)** - Key-value and caching operations
- **[Model Context Protocol (MCP)](#-mcp-api)** - AI agent integration

---

## 📖 API Documentation Sections

### 🎭 Orbit Client API

The primary Rust client library for interacting with Orbit-RS clusters.

```rust
use orbit_client::{OrbitClient, OrbitClientConfig};

// Create a client
let client = OrbitClient::builder()
    .with_namespace("my-app")
    .with_server_urls(vec!["http://localhost:8080".to_string()])
    .build()
    .await?;

// Get an actor reference
let actor_ref = client.actor_reference::<MyActor>(
    Key::StringKey { key: "actor-id".to_string() }
).await?;
```

**Key Features:**

- Type-safe actor references
- Connection pooling and load balancing
- Automatic retries and failover
- Distributed transaction support

### 🎭 Actor System API

Distributed actor framework for building scalable applications.

```rust
use orbit_shared::{Addressable, ActorImplementation};

#[async_trait]
trait MyActor: Addressable {
    async fn process_message(&self, data: String) -> Result<String>;
}

// Actors are automatically distributed across cluster nodes
```

**Key Features:**

- Location-transparent actor addressing
- Automatic actor lifecycle management
- Cross-cluster message routing
- Actor proxy generation

### 🔄 Transaction API

ACID distributed transactions with 2-phase commit and Saga patterns.

```rust
use orbit_shared::transactions::*;

// Begin distributed transaction
let tx_id = coordinator.begin_transaction(Some(timeout)).await?;

// Add operations
coordinator.add_operation(&tx_id, operation).await?;

// Commit transaction
coordinator.commit_transaction(&tx_id).await?;
```

**Key Features:**

- Two-phase commit (2PC) protocol
- Saga pattern for long-running transactions
- Automatic compensation and rollback
- Distributed locking

---

## 🔌 Protocol APIs

### 📊 PostgreSQL API

Full PostgreSQL wire protocol compatibility for SQL operations.

```sql
-- Standard SQL operations
CREATE TABLE users (id SERIAL, name TEXT, email TEXT);
INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
SELECT * FROM users WHERE name = 'Alice';

-- Vector operations (pgvector compatible)  
CREATE TABLE documents (id SERIAL, content TEXT, embedding VECTOR(384));
SELECT content FROM documents ORDER BY embedding <-> '[0.1, 0.2, 0.3]' LIMIT 5;
```

**Supported Features:**

- Full ANSI SQL compatibility
- PostgreSQL-specific extensions
- Vector similarity search
- Complex queries with JOINs, CTEs, window functions

### 🔑 Redis API

Redis RESP protocol compatibility for key-value operations.

```bash

# String operations
SET key "value"
GET key
MGET key1 key2 key3

# Hash operations
HSET user:123 name "Alice" email "alice@example.com"
HGET user:123 name

# List operations
LPUSH queue item1 item2
RPOP queue

# Vector operations (Redis Stack compatible)
FT.CREATE idx ON hash PREFIX 1 "doc:" SCHEMA embedding VECTOR FLAT 6 DIM 384
FT.SEARCH idx "*=>[KNN 5 @embedding $query]" PARAMS 2 query "[0.1, 0.2, 0.3]"
```

**Supported Commands:**

- 50+ Redis commands
- Redis Stack vector operations  
- Clustering support
- Pub/sub messaging

### 🤖 MCP API  

Model Context Protocol for AI agent integration.

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "query_database",
    "arguments": {
      "sql": "SELECT * FROM users WHERE age > 21"
    }
  }
}
```

**Key Features:**

- Standardized AI agent communication
- Tool calling for database operations
- Resource management and discovery
- Secure sandboxed execution

---

## 📚 Reference Documentation

### 🔧 Configuration

```rust

#[derive(Debug, Clone)]
pub struct OrbitClientConfig {
    pub namespace: String,
    pub server_urls: Vec<String>, 
    pub connection_timeout: Duration,
    pub retry_attempts: u32,
    pub actor_timeout: Duration,
}
```

### 🎯 Error Handling

```rust
pub enum OrbitError {
    NetworkError(String),
    ActorError(String),
    TransactionError(TransactionError),
    SerializationError(String),
    TimeoutError(String),
}
```

### 📊 Performance Monitoring

```rust
pub struct ClientStats {
    pub namespace: String,
    pub server_connections: usize,
    pub node_id: Option<NodeId>,
    pub active_actors: usize,
    pub pending_transactions: usize,
}
```

---

## 🛠️ Development Tools

### 📋 Code Generation

Generate actor proxies and protocol bindings:

```bash

# Generate actor proxies
orbit-codegen actors --input src/actors.rs --output src/generated/

# Generate protocol bindings
orbit-codegen protocols --proto schemas/ --output src/protocols/
```

### 🧪 Testing

```rust
use orbit_testing::MockOrbitClient;

#[tokio::test]
async fn test_actor_interaction() {
    let client = MockOrbitClient::new();
    let actor_ref = client.mock_actor::<MyActor>("test-actor");
    
    let result = actor_ref.process_message("test".to_string()).await?;
    assert_eq!(result, "processed: test");
}
```

### 🔍 Debugging

```bash

# Enable debug logging
export RUST_LOG=orbit_client=debug,orbit_shared=debug

# Monitor actor lifecycle
export ORBIT_DEBUG_ACTORS=true

# Trace transaction execution  
export ORBIT_TRACE_TRANSACTIONS=true
```

---

## 🌐 Language Bindings

### 🦀 Rust (Native)

- **Status**: ✅ Complete
- **Crate**: [`orbit-client`](https://crates.io/crates/orbit-client)
- **Documentation**: [docs.rs](https://docs.rs/orbit-client)

### ☕ Java/Spring Boot

- **Status**: ✅ Complete  
- **Package**: [`orbit-client-spring`](../JAVA_SPRING_INTEGRATION_SUMMARY.md)
- **Integration**: Native JNI bindings with Spring Boot auto-configuration

### 🐍 Python (Planned)

- **Status**: 🔄 In Development
- **Package**: `orbit-client-py`
- **Timeline**: Q2 2025

### 🟦 TypeScript/Node.js (Planned)

- **Status**: 📋 Planned
- **Package**: `@orbit-rs/client`
- **Timeline**: Q3 2025

---

## 📈 Performance Characteristics

### Throughput

- **Actor Messages**: 500K+ messages/second per core
- **SQL Queries**: 100K+ queries/second with connection pooling
- **Transactions**: 50K+ distributed transactions/second

### Latency  

- **Local Actor Calls**: <1ms average
- **Remote Actor Calls**: <10ms average (same datacenter)
- **Transaction Commit**: <50ms average (2-phase commit)

### Scalability

- **Cluster Size**: Up to 1000 nodes tested
- **Concurrent Connections**: 100K+ simultaneous connections
- **Data Scale**: Petabyte-scale storage with LSM-tree backend

---

## 🤝 Support & Community

### 📖 Documentation

- **[Quick Start Guide](../quick_start.md)** - Get started in 5 minutes
- **[Architecture Overview](../overview.md)** - System design principles
- **[Deployment Guide](../kubernetes_deployment.md)** - Production deployment

### 💬 Community  

- **[Discord](https://discord.gg/orbit-rs)** - Real-time chat and support
- **[GitHub Discussions](https://github.com/TuringWorks/orbit-rs/discussions)** - Q&A and feature requests
- **[Stack Overflow](https://stackoverflow.com/questions/tagged/orbit-rs)** - Technical questions

### 🐛 Issue Reporting

- **[Bug Reports](https://github.com/TuringWorks/orbit-rs/issues/new?template=bug_report.md)**
- **[Feature Requests](https://github.com/TuringWorks/orbit-rs/issues/new?template=feature_request.md)**  
- **[Security Issues](mailto:security@turingworks.com)** - Responsible disclosure

---

## 📄 License

This API documentation is part of the Orbit-RS project and is licensed under the [Apache License 2.0](https://github.com/TuringWorks/orbit-rs/blob/main/LICENSE).

---

**🔄 Last Updated**: October 2024  
**📋 Version**: 0.1.0  
**🌟 Status**: Production Ready
