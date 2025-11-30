---
layout: default
title: "Quick Start Guide - Multi-Protocol Database Server"
subtitle: "Get PostgreSQL, Redis, REST API, and gRPC running in 30 seconds"
category: "getting-started"
---

# Orbit-RS Quick Start Guide

**One Server, All Protocols, Persistent Storage** - Get a production-ready PostgreSQL + Redis + REST API + gRPC server with RocksDB persistence running in 30 seconds.

##  What You'll Get

**Single `orbit-server` command gives you:**

-  **PostgreSQL server** (port 15432) - Full SQL with pgvector support + **RocksDB persistence**
-  **Redis server** (port 6379) - Key-value + TTL operations + **RocksDB persistence**
-  **HTTP REST API** (port 8080) - Web-friendly JSON interface  
-  **gRPC API** (port 50051) - High-performance actor management
-  **RocksDB Storage** - LSM-tree persistent storage for all data
-  **Data Persistence** - All data survives server restarts with TTL support

** Key Innovation**: Same data accessible through any protocol with instant consistency **and full persistence**!

## Prerequisites

Before you begin, ensure you have the following installed:

- **Rust 1.70+** - [Install Rust](https://rustup.rs/)
- **Protocol Buffers compiler (`protoc`)** - [Install Protocol Buffers](https://grpc.io/docs/protoc-installation/)
- (Optional) **Docker** - For containerized deployment
- (Optional) **Kubernetes cluster** - For operator deployment

### Installing Protocol Buffers

#### macOS

```bash
brew install protobuf
```

#### Ubuntu/Debian

```bash
sudo apt update
sudo apt install protobuf-compiler
```

#### Windows

Download from [Protocol Buffers releases](https://github.com/protocolbuffers/protobuf/releases) or use:

```powershell
choco install protoc
```

##  30-Second Quick Start

### 1. Clone and Build

```bash
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs
cargo build --release
```

### 2. Start Integrated Multi-Protocol Server with RocksDB Persistence

```bash
# Run the integrated server with all protocols enabled and RocksDB persistence
cargo run --package orbit-server --example integrated-server

#  Server starting with all protocols and persistent storage:
# gRPC: localhost:50051 (Orbit clients)
# Redis: localhost:6379 (redis-cli, Redis clients) - PERSISTED with RocksDB
# PostgreSQL: localhost:15432 (psql, PostgreSQL clients) - PERSISTED with RocksDB
# Data Directory: ./orbit_integrated_data (LSM-tree files)
```

### Alternative: Simple Examples

```bash
# Simple server without protocols (basic actor system only)
cargo run --example hello-world

# PostgreSQL server only
cargo run --package orbit-server --example postgres-server

# Redis server only
cargo run --example resp-server
```

### 3. Connect with Standard Clients

**PostgreSQL** - Use any PostgreSQL client:

```bash
psql -h localhost -p 15432 -U orbit -d actors
```

**Redis** - Use redis-cli or any Redis client:

```bash
redis-cli -h localhost -p 6379
```

**gRPC** - Use OrbitClient or grpcurl:

```bash
# List gRPC services
grpcurl -plaintext localhost:50051 list
```

### 4. Verify Multi-Protocol Access

```bash
# Test all protocols are working
psql -h localhost -p 15432 -U orbit -d actors -c "SELECT 'PostgreSQL Connected!';"
redis-cli -h localhost -p 6379 ping
grpcurl -plaintext localhost:50051 orbit.HealthService/Check
```

##  Multi-Protocol Data Demo

**The same data is accessible through all protocols** - here's how:

### Cross-Protocol Data Consistency Demo

**The key innovation: Same data, different protocols!**

```bash
# Terminal 1: Write data via Redis
redis-cli -h localhost -p 6379
127.0.0.1:6379> SET greeting "Hello from Redis!"
OK
127.0.0.1:6379> HSET user:alice name "Alice" email "alice@orbit.com" role "admin"
(integer) 3

# Terminal 2: Create table and insert via PostgreSQL
psql -h localhost -p 15432 -U orbit -d actors
actors=# CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email TEXT, role TEXT);
CREATE TABLE
actors=# INSERT INTO users (name, email, role) VALUES ('Bob', 'bob@orbit.com', 'user');
INSERT 0 1

# Terminal 3: Verify cross-protocol access
# Read Redis data via SQL
actors=# SELECT * FROM orbit_keys WHERE key = 'greeting';
# Read SQL data via Redis  
127.0.0.1:6379> HGETALL user:bob
1) "name"
2) "Bob"
3) "email"
4) "bob@orbit.com"
5) "role"
6) "user"
```

 **Same underlying data store, multiple protocol interfaces!**

### Vector Operations Across Protocols

```bash
# PostgreSQL with pgvector
psql -h localhost -p 5432 -U postgres
postgres=# CREATE EXTENSION vector;
postgres=# CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding VECTOR(3)  -- Using 3D vectors for demo
);
postgres=# INSERT INTO documents (content, embedding) VALUES 
    ('Machine learning', '[0.1, 0.2, 0.3]'),
    ('Deep learning', '[0.15, 0.25, 0.35]'),
    ('Data science', '[0.2, 0.3, 0.4]');

# Vector similarity search via SQL
postgres=# SELECT content, embedding <-> '[0.1, 0.2, 0.3]' AS distance 
           FROM documents 
           ORDER BY embedding <-> '[0.1, 0.2, 0.3]' 
           LIMIT 2;
```

```bash
# Same vector operations via Redis
redis-cli -h localhost -p 6379

# Add vectors with metadata
127.0.0.1:6379> VECTOR.ADD doc-embeddings doc1 "0.1,0.2,0.3" content "Machine learning"
127.0.0.1:6379> VECTOR.ADD doc-embeddings doc2 "0.15,0.25,0.35" content "Deep learning"
127.0.0.1:6379> VECTOR.ADD doc-embeddings doc3 "0.2,0.3,0.4" content "Data science"

# Vector similarity search
127.0.0.1:6379> VECTOR.SEARCH doc-embeddings "0.1,0.2,0.3" 2 METRIC COSINE
1) 1) "doc1"
   2) "1.000000"
2) 1) "doc2"
   2) "0.998"
```

### REST API Access

```bash
# Query data via HTTP REST API
curl "http://localhost:8080/api/users"
curl "http://localhost:8080/api/users/1"

# Vector search via REST
curl -X POST "http://localhost:8080/api/vectors/search" \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "doc-embeddings",
    "vector": [0.1, 0.2, 0.3],
    "limit": 5,
    "metric": "cosine"
  }'

# Health and status endpoints
curl http://localhost:8080/health
curl http://localhost:8080/metrics
```

## Basic Usage

### Simple Actor Example

Here's a minimal example to get you started with Orbit-RS:

```rust
use orbit_client::OrbitClient;
use orbit_shared::{ActorWithStringKey, Key};
use async_trait::async_trait;

// Define an actor trait

#[async_trait]
trait GreeterActor: ActorWithStringKey {
    async fn greet(&self, name: String) -> Result<String, orbit_shared::OrbitError>;
}

// Implement the actor
struct GreeterActorImpl;

#[async_trait]
impl GreeterActor for GreeterActorImpl {
    async fn greet(&self, name: String) -> Result<String, orbit_shared::OrbitError> {
        Ok(format!("Hello, {}!", name))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client
    let client = OrbitClient::builder()
        .with_namespace("demo")
        .build()
        .await?;
    
    // Get an actor reference
    let greeter = client.actor_reference::<dyn GreeterActor>(
        Key::StringKey { key: "my-greeter".to_string() }
    ).await?;
    
    // Invoke the actor
    let greeting = greeter.greet("World".to_string()).await?;
    println!("{}", greeting); // "Hello, World!"
    
    Ok(())
}
```

### Distributed Transactions Example

For more advanced usage with distributed transactions:

```rust
use orbit_shared::{
    transactions::*,
    transport::*,
    AddressableReference, Key, NodeId,
};
use std::{sync::Arc, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create transaction coordinator
    let node_id = NodeId::new("coordinator".to_string(), "cluster".to_string());
    let config = TransactionConfig::default();
    let transport = Arc::new(GrpcTransactionMessageSender::new(/* ... */));
    
    let coordinator = TransactionCoordinator::new(node_id, config, transport);
    
    // Begin distributed transaction
    let tx_id = coordinator.begin_transaction(Some(Duration::from_secs(30))).await?;
    
    // Add banking operations
    let debit_operation = TransactionOperation::new(
        AddressableReference {
            addressable_type: "BankAccount".to_string(),
            key: Key::StringKey { key: "alice".to_string() },
        },
        "debit".to_string(),
        serde_json::json!({"amount": 100}),
    ).with_compensation(serde_json::json!({"amount": 100, "action": "credit"}));
    
    let credit_operation = TransactionOperation::new(
        AddressableReference {
            addressable_type: "BankAccount".to_string(),
            key: Key::StringKey { key: "bob".to_string() },
        },
        "credit".to_string(),
        serde_json::json!({"amount": 100}),
    ).with_compensation(serde_json::json!({"amount": 100, "action": "debit"}));
    
    coordinator.add_operation(&tx_id, debit_operation).await?;
    coordinator.add_operation(&tx_id, credit_operation).await?;
    
    // Execute 2-phase commit
    coordinator.commit_transaction(&tx_id).await?;
    
    println!(" Transaction {} committed successfully!", tx_id);
    Ok(())
}
```

## Running Examples

Orbit-RS includes several working examples to demonstrate different features:

### Integrated Multi-Protocol Server (RECOMMENDED)

```bash
# Run the full integrated server with all protocols
cargo run --package orbit-server --example integrated-server

# Connect with any client:
# PostgreSQL: psql -h localhost -p 15432 -U orbit -d actors
# Redis: redis-cli -h localhost -p 6379
# gRPC: grpcurl -plaintext localhost:50051 list
```

### Individual Protocol Examples

### Hello World (Basic Actor System)

```bash
cargo run --example hello-world
```

#### PostgreSQL Server Only

```bash
cargo run --package orbit-server --example postgres-server
# Connect: psql -h localhost -p 5433 -U orbit -d actors
```

#### Redis Server Only

```bash
cargo run --example resp-server
# Connect: redis-cli -h localhost -p 6379
```

#### Other Demos

```bash
# Storage architecture demo
cargo run --example postgres_storage_demo

# Query language parsing
cargo run --example aql_parser_test

# Distributed counter
cargo run --example distributed_counter
```

##  Configuration

### Development vs Production Modes

**Development Mode** (`--dev-mode`):

- All protocols enabled automatically
- Verbose debug logging
- Development-friendly defaults
- Use for local testing only

```bash
# Development - all protocols active
orbit-server --dev-mode
```

**Production Mode**:

- Uses configuration file
- Controlled protocol activation
- Production logging levels
- Security settings enabled

```bash
# Production with configuration file
orbit-server --config /etc/orbit/production.toml
```

### Production Configuration

Create `/etc/orbit/production.toml`:

```toml
[server]
bind_address = "0.0.0.0"
environment = "Production"
node_id = "orbit-prod-01"

# Enable only needed protocols
[protocols.postgresql]
enabled = true
port = 5432
max_connections = 5000

[protocols.redis]  
enabled = true
port = 6379
max_connections = 5000

[protocols.rest]
enabled = false  # Disable if not needed

[protocols.grpc]
enabled = true
port = 50051

# Vector operations
[protocols.postgresql.vector_ops]
default_metric = "cosine"
max_dimensions = 1536
batch_size = 1000
enable_simd = true

# Production security
[security.authentication]
enabled = true
methods = ["JWT"]

[security.authentication.jwt]
secret_key = "${JWT_SECRET}"  # From environment
expiration_secs = 3600

# Performance tuning
[performance.memory]
max_memory_mb = 16384  # 16GB

[performance.cpu]
worker_threads = 32
enable_simd = true

# Production logging
[logging]
level = "warn"
format = "json"

[[logging.outputs]]
output_type = "file"
file_path = "/var/log/orbit/orbit-server.log"

# Monitoring
[monitoring.metrics]
enabled = true
port = 9090

[monitoring.health_checks]
enabled = true
port = 8081
```

### Generate Configuration Template

```bash
# Generate example configuration
orbit-server --generate-config > orbit-server.toml

# Edit for your needs
vim orbit-server.toml

# Run with configuration
orbit-server --config orbit-server.toml
```

### Command Line Options

```bash
# Enable specific protocols
orbit-server \
  --enable-postgresql \
  --enable-redis \
  --postgres-port 5432 \
  --redis-port 6379

# With clustering
orbit-server \
  --config production.toml \
  --seed-nodes node1:7946,node2:7946
```

## Development Setup

### IDE Setup

#### VS Code

Recommended extensions:

- `rust-analyzer` - Rust language server
- `CodeLLDB` - Debugger for Rust
- `crates` - Cargo dependency management

#### IntelliJ IDEA

- Install the Rust plugin
- Configure the Rust toolchain in settings

### Development Commands

```bash

# Fast compile check
cargo check

# Lint with Clippy
cargo clippy

# Format code
cargo fmt

# Security audit
cargo audit

# Run benchmarks
cargo bench
```

### Testing

```bash

# Run unit tests
cargo test --workspace --lib

# Run integration tests  
cargo test --workspace --test integration

# Run BDD scenarios
cargo test --workspace --test bdd

# Run with coverage
cargo tarpaulin --out Html
```

## Troubleshooting

### Common Issues

#### Protocol Buffers Not Found

```text
error: Could not find `protoc` installation
```

**Solution**: Install Protocol Buffers compiler as described in prerequisites.

#### Rust Version Too Old

```text
error: package requires Rust 1.70 or newer
```

**Solution**: Update Rust using `rustup update`

#### Port Already in Use

```text
error: Address already in use (os error 48)
```

**Solution**: Use a different port or stop the conflicting service.

### Getting Help

-  [Full Documentation](README.md)
-  [Issue Tracker](https://github.com/TuringWorks/orbit-rs/issues)
-  [Discussions](https://github.com/TuringWorks/orbit-rs/discussions)

##  Next Steps

Now that you have Orbit-RS multi-protocol server running, explore these guides:

### **Multi-Protocol Features**

-  **[Native Multi-Protocol Guide](NATIVE_MULTIPROTOCOL.md)** - Complete multi-protocol documentation

-  **[Configuration Reference](deployment/CONFIGURATION.md)** - Complete configuration guide
-  **[Performance Tuning](PETABYTE_SCALE_PERFORMANCE.md)** - Optimize for your workload

### **Protocol-Specific Guides**  

-  **[PostgreSQL Compatibility](protocols/POSTGRES_WIRE_IMPLEMENTATION.md)** - SQL features and pgvector
-  **[Redis Compatibility](protocols/REDIS_COMMANDS_REFERENCE.md)** - Key-value and vector operations
-  **[REST API Reference](protocols/protocol_adapters.md)** - HTTP endpoints and usage
-  **[Vector Operations Guide](vector_commands.md)** - Cross-protocol vector search

### **Advanced Features**

-  **[Transaction Features](features/transaction_features.md)** - Distributed ACID transactions
-  **[Real-time Streaming](PHASE_11_CDC_STREAMING.md)** - CDC and event sourcing
-  **[Kubernetes Complete Documentation](KUBERNETES_COMPLETE_DOCUMENTATION.md)** - Production deployment
-  **[Actor System Guide](virtual_actor_persistence.md)** - Virtual actors and distribution

### **Development & Operations**

-  **[Development Guide](DEVELOPMENT.md)** - Contributing to Orbit-RS
-  **[Security Guide](SECURITY_COMPLETE_DOCUMENTATION.md)** - Authentication and authorization
-  **[Monitoring Guide](advanced_transaction_features.md)** - Metrics and observability
-  **[Troubleshooting](operations/OPERATIONS_RUNBOOK.md)** - Common issues and solutions

### **Migration Guides**

-  **[PostgreSQL Migration](MIGRATION_GUIDE.md)** - Migrate from PostgreSQL
-  **[Redis Migration](MIGRATION_GUIDE.md)** - Migrate from Redis
-  **[Multi-Database Migration](MIGRATION_GUIDE.md)** - Consolidate multiple databases
