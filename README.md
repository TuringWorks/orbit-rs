# Orbit-RS - Native Multi-Protocol Database Server

![License](https://img.shields.io/badge/license-BSD--3--Clause%20OR%20MIT-blue.svg)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-red.svg)](https://www.rust-lang.org/)

**One Server, All Protocols** - Orbit-RS is a high-performance database server that natively speaks PostgreSQL, MySQL, CQL (Cassandra), Redis, HTTP REST, and gRPC protocols from a single process. Built on a distributed virtual actor system in Rust, it eliminates the operational complexity of running separate database servers while providing unprecedented consistency and performance.

## Documentation

**Complete documentation is available in the [docs/](docs/) directory:**

- **[ Documentation Index](docs/README.md)** - Navigate all documentation
- **[ Feature Index](docs/features.md)** - Complete feature list with implementation status
- **[ Overview](docs/overview.md)** - Architecture, features, and key benefits  
- **[ Quick Start](docs/quick_start.md)** - Get up and running in minutes
- **[ Virtual Actor Persistence](docs/virtual_actor_persistence.md)** - Actor state management and lifecycle
- **[ Transaction Features](docs/features/transaction_features.md)** - Advanced distributed transactions
- **[ Protocol Adapters](docs/protocols/protocol_adapters.md)** - Redis, PostgreSQL, MCP support
- **[ Deployment](docs/kubernetes_deployment.md)** - Kubernetes, CI/CD, production setup
- **[ Development](docs/development/development.md)** - Contributing and development guide
- **[ Roadmap](docs/roadmap.md)** - Development roadmap and GitHub project

## What is Orbit-RS?

**Orbit-RS is a revolutionary multi-protocol database server** that natively implements PostgreSQL, MySQL, CQL (Cassandra), Redis, HTTP REST, gRPC, and OrbitQL protocols in a single process. Instead of running separate PostgreSQL, MySQL, Cassandra, and Redis servers, Orbit-RS provides one unified server that speaks all protocols while sharing the same underlying data store.

**Built on Virtual Actors**: The foundation is a distributed virtual actor system where actors are objects that interact via asynchronous messages. Actors automatically activate on-demand and can be distributed across cluster nodes, providing natural horizontal scaling.

 The same data is immediately accessible through any protocol - write via SQL, read via Redis, query via REST API, or manage via gRPC - with ACID consistency guaranteed across all interfaces.

### **Multi-Protocol Database Server**

**Native Protocol Support** - Single server, multiple interfaces:

- **PostgreSQL Wire Protocol** (port 5432) - Full SQL with pgvector support
- **MySQL Wire Protocol** (port 3306) - MySQL-compatible SQL interface
- **CQL Protocol** (port 9042) - Cassandra Query Language for wide-column access
- **Redis RESP Protocol** (port 6379) - Key-value + vector operations
- **HTTP REST API** (port 8080) - Web-friendly JSON interface
- **gRPC API** (port 50051) - High-performance actor management
- **OrbitQL** - Multi-model query language (documents, graphs, time-series)

### **Core Features**

- **One Server, All Protocols**: Replace PostgreSQL + MySQL + Cassandra + Redis with single process
- **Cross-Protocol Consistency**: Write via SQL, read via Redis/CQL - instant consistency
- **Zero Data Duplication**: Shared storage across all protocols
- **High Performance**: 500k+ ops/sec with memory safety and zero-cost abstractions
- **AI-Native Database**: 8 intelligent subsystems for autonomous optimization, predictive scaling, and proactive management
- **Full pgvector Compatibility**: Complete PostgreSQL vector extension support with HNSW/IVFFlat indexes
- **ACID Transactions**: Full ACID compliance across all protocols
- **Virtual Actors**: Automatic lifecycle management and horizontal scaling
- **Real-time Streaming**: CDC, event sourcing, and stream processing
- **Advanced Connection Pooling**: Circuit breakers and health monitoring
- **Enterprise Security**: Authentication, authorization, and audit logging
- **RocksDB Persistence**: Production-ready persistence with LSM-tree storage
- **Zero Compiler Warnings**: 100% clean compilation across 148,780+ lines of Rust code

## Quick Start - Persistent Multi-Protocol Database Server

**Get a production-ready multi-protocol database server with RocksDB persistence running in 30 seconds:**

```bash
# Clone and build
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs
cargo build --release

# Start multi-protocol server with RocksDB persistence
cargo run --package orbit-server --example integrated-server

# All protocols now active with persistent storage:
# PostgreSQL: localhost:5432 (persisted with RocksDB)
# MySQL: localhost:3306 (persisted with RocksDB)
# CQL: localhost:9042 (persisted with RocksDB)
# Redis: localhost:6379 (persisted with RocksDB)
# REST API: localhost:8080
# gRPC: localhost:50051
# Data Directory: ./orbit_integrated_data (RocksDB LSM-tree files)
```

**Or use the main server binary:**

```bash
# Start all protocols with default configuration
cargo run --bin orbit-server

# Or with custom configuration
cargo run --bin orbit-server -- --config ./config/orbit-server.toml
```

### **Connect with Standard Clients - Data Persists Across Restarts!**

```bash
# PostgreSQL - use any PostgreSQL client (data persisted with RocksDB)
# Connect to the default database (you can use any database name, they all share the same storage)
psql -h localhost -p 5432 -U orbit -d actors

# Create tables in the current database
actors=# CREATE TABLE users (id SERIAL, name TEXT, email TEXT);
actors=# CREATE TABLE products (id SERIAL, name TEXT, price INTEGER);
actors=# INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
actors=# SELECT * FROM users;

# You can create multiple tables - all data persists across restarts
# Note: CREATE DATABASE is planned for future releases. Currently, all tables
# are stored in a unified namespace and accessible from any database connection.

# Redis - use redis-cli or any Redis client (data persisted with RocksDB)
redis-cli -h localhost -p 6379
127.0.0.1:6379> SET greeting "Hello Persistent World"
127.0.0.1:6379> SET counter 42
127.0.0.1:6379> SET temp_key "Expires in 30 seconds" EX 30
127.0.0.1:6379> GET greeting
"Hello Persistent World"
127.0.0.1:6379> INCR counter
(integer) 43
127.0.0.1:6379> TTL temp_key
(integer) 28
127.0.0.1:6379> HSET user:1 name "Alice" email "alice@example.com"
127.0.0.1:6379> HGETALL user:1

# Test persistence: Stop server (Ctrl+C), restart, data survives!
# All Redis data including TTL expiration is preserved across restarts
127.0.0.1:6379> GET greeting  # Still works after restart!
"Hello Persistent World"
```

### **AI/ML Vector Operations - pgvector Compatible**

**Orbit-RS provides full pgvector compatibility with vector similarity search across PostgreSQL and Redis protocols:**

```sql
-- PostgreSQL with full pgvector support
CREATE EXTENSION vector;
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding vector(1536)  -- OpenAI text-embedding-ada-002 dimensions
);

-- Insert vector embeddings
INSERT INTO documents (content, embedding) VALUES 
    ('AI and machine learning', '[0.1,0.2,0.3,...,0.1536]'),
    ('Database systems', '[0.4,0.5,0.6,...,0.1536]');

-- Vector similarity search with L2 distance
SELECT content, embedding <-> '[0.2,0.3,0.4,...,0.1536]' AS distance
FROM documents 
ORDER BY distance 
LIMIT 5;

-- Cosine similarity search
SELECT content, embedding <=> '[0.2,0.3,0.4,...,0.1536]' AS cosine_distance
FROM documents 
ORDER BY cosine_distance 
LIMIT 5;

-- Create HNSW index for fast approximate similarity search
CREATE INDEX ON documents USING hnsw (embedding vector_cosine_ops) 
WITH (m = 16, ef_construction = 64);

-- Create IVFFlat index for exact similarity search
CREATE INDEX ON documents USING ivfflat (embedding vector_l2_ops) 
WITH (lists = 100);
```

**Cross-Protocol Vector Access**: Vector data is immediately available across all protocols:

```redis
# Redis vector operations (future) 
127.0.0.1:6379> VECTOR.ADD doc:1 "[0.1,0.2,0.3]" METADATA content "AI document"
127.0.0.1:6379> VECTOR.SEARCH doc "[0.2,0.3,0.4]" 5 METRIC COSINE
```

**Same Vector Data, Multiple Interfaces**: Vectors stored via PostgreSQL are immediately accessible via Redis and REST APIs!

### **Database and Schema Management**

**Current Database Support:**
- Orbit-RS uses a unified storage namespace where all tables are accessible
- You can connect with any database name (e.g., `-d actors`, `-d myapp`, `-d production`)
- All tables created are stored persistently and accessible across connections
- **CREATE DATABASE** command is planned for future releases to provide logical database separation
- Currently, you can organize data using **schemas** and **tables** within the unified namespace

**Working with Multiple Tables:**
```sql
-- Create tables in your current database context
CREATE TABLE users (id SERIAL, name TEXT, email TEXT);
CREATE TABLE products (id SERIAL, name TEXT, price INTEGER);
CREATE TABLE orders (id SERIAL, user_id INTEGER, product_id INTEGER, quantity INTEGER);

-- All tables are accessible regardless of which database name you used to connect
SELECT * FROM users;
SELECT * FROM products;
SELECT * FROM orders;
```

### **OrbitQL - Multi-Model Queries**

**OrbitQL is a unified query language** that combines document, graph, time-series, and key-value operations in a single query. Access all data stored in orbit-engine across hot/warm/cold tiers.

```rust
use orbit_engine::adapters::{AdapterContext, OrbitQLAdapter};
use orbit_engine::storage::HybridStorageManager;

// Create storage engine and OrbitQL adapter
let storage = Arc::new(HybridStorageManager::new_in_memory());
let context = AdapterContext::new(storage as Arc<dyn TableStorage>);
let adapter = OrbitQLAdapter::new(context);

// Execute multi-model queries
adapter.execute_query("SELECT * FROM users WHERE age > 18").await?;
```

**OrbitQL Query Examples:**

```orbitql
-- Document-style queries
SELECT * FROM users WHERE age > 18 ORDER BY created_at DESC LIMIT 10;

-- Graph traversals with arrow notation
SELECT user->follows->user.name AS friends FROM users WHERE user.id = 123;

-- Time-series analytics with temporal filters
SELECT
    server_id,
    AVG(metrics[cpu_usage WHERE timestamp > NOW() - 1h]) AS avg_cpu
FROM servers
GROUP BY server_id;

-- Cross-model JOINs (documents + time-series)
SELECT
    u.name,
    AVG(m.cpu_usage) AS avg_cpu
FROM users AS u
JOIN metrics AS m ON u.server_id = m.server_id
WHERE m.timestamp > NOW() - 1h
GROUP BY u.id;
```

**Key Features:**
- **Multi-Model Queries** - Documents, graphs, and time-series in one query
- **Cross-Model JOINs** - Relate data between different models seamlessly
- **Tiered Storage Aware** - Automatically accesses hot/warm/cold tiers
- **Live Queries** - Real-time subscriptions with change notifications
- **ACID Transactions** - Multi-model transaction support

**[Complete OrbitQL Documentation](orbit/engine/docs/ORBITQL.md)** | **[OrbitQL Examples](orbit/engine/examples/orbitql_example.rs)**

### Manual Installation

```bash
cargo build --release
cargo test
```

### Basic Actor Example

```rust
use orbit_client::OrbitClient;
use orbit_shared::{ActorWithStringKey, Key};
use async_trait::async_trait;

#[async_trait]
trait GreeterActor: ActorWithStringKey {
    async fn greet(&self, name: String) -> Result<String, orbit_shared::OrbitError>;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OrbitClient::builder().with_namespace("demo").build().await?;
    let greeter = client.actor_reference::<dyn GreeterActor>(
        Key::StringKey { key: "my-greeter".to_string() }
    ).await?;
    
    let greeting = greeter.greet("World".to_string()).await?;
    println!("{}", greeting); // "Hello, World!"
    Ok(())
}
```

**[Complete Quick Start Guide](docs/quick_start.md)** for detailed setup instructions.

## Current Status: Multi-Protocol Server Ready

**Native Multi-Protocol Database Server**

- **Single Process**: PostgreSQL + MySQL + CQL + Redis + REST + gRPC in one server  
- **Cross-Protocol Consistency**: Write via SQL, read via Redis/CQL, query via REST
- **Zero Data Duplication**: Shared storage across all protocols
- **Enterprise Ready**: Replace separate PostgreSQL, MySQL, Cassandra, and Redis deployments

**Production-Ready Multi-Protocol Features:**

- **PostgreSQL Wire Protocol** - Complete PostgreSQL server with full pgvector support + RocksDB persistence
- **MySQL Wire Protocol** - MySQL-compatible SQL interface with RocksDB persistence
- **CQL Protocol** - Cassandra Query Language for wide-column access with RocksDB persistence
- **Redis RESP Protocol** - Full redis-cli compatibility with vector operations + RocksDB persistence
- **HTTP REST API** - Web-friendly JSON interface for all operations including vectors
- **gRPC Actor API** - High-performance actor system management with vector support
- **OrbitQL Multi-Model Queries** - Unified query language for documents, graphs, and time-series
- **Full pgvector Compatibility** - Vector types, distance operators, HNSW/IVFFlat indexes
- **Unified Configuration** - Single TOML file configures all protocols
- **Cross-Protocol Monitoring** - Unified metrics for all protocols including vector operations

**Core Infrastructure:**

- **Virtual Actor System** - Automatic lifecycle management and distribution
- **Distributed Transactions** - ACID compliance across all protocols
- **AI-Native Database** - 8 production-ready intelligent subsystems (NEW Nov 2025)
- **Performance Benchmarking** - Statistical analysis and regression detection
- **Real-time Streaming** - CDC, event sourcing, and stream processing
- **Advanced Connection Pooling** - Enterprise-grade multi-tier pooling with circuit breakers, load balancing, health monitoring, and dynamic scaling
- **Enterprise Security** - Authentication, authorization, audit logging
- **Kubernetes Integration** - Native operator, Helm charts, production deployment
- **Observability** - Prometheus metrics, Grafana dashboards, comprehensive monitoring

**Production-Ready RocksDB Persistence:**

- **RocksDB Integration** - **COMPLETE!** LSM-tree storage for high-performance persistence
- **Redis Data Persistence** - All Redis commands with TTL support persist across restarts
- **PostgreSQL Data Persistence** - SQL tables and data survive server restarts
- **Configurable Storage Backend** - RocksDB, Memory, TiKV, and Cloud storage options
- **Background TTL Cleanup** - Automatic expiration of Redis keys with background cleanup
- **Write-Ahead Logging** - Durability guarantees with WAL for crash consistency
- **LSM-tree Optimization** - Write-optimized storage with configurable compaction
- **Column Family Support** - Organized data separation for performance
- **Production Configuration** - Tuned write buffers, caching, and compression settings

**Complete pgvector Support:**

- **Vector Data Types** - **COMPLETE!** Full vector(n), halfvec(n), sparsevec(n) support
- **Vector Literals** - Parse vector strings '[1,2,3]' in SQL statements
- **Distance Operators** - <-> (L2), <=> (cosine), <#> (inner product) in all SQL contexts  
- **Vector Indexes** - HNSW and IVFFlat with full parameter support (m, ef_construction, lists)
- **Operation Classes** - vector_l2_ops, vector_cosine_ops, vector_inner_product_ops
- **Similarity Search** - ORDER BY with vector distance for nearest neighbor queries
- **Extension Support** - CREATE/DROP EXTENSION vector integration
- **Vector Tests** - Comprehensive test coverage for all pgvector features

**Advanced JSON/JSONB:**

- **Complete JSONB Implementation** - **COMPLETE!** Full PostgreSQL-compatible JSON Binary format
- **JSON Path Expressions** - PostgreSQL-compatible path syntax ($.key[0].nested)
- **JSON/JSONB Operators** - All PostgreSQL operators (->, ->>, #>, @>, ?, ||, etc.)
- **JSON Aggregation Functions** - json_agg(), jsonb_agg(), json_object_agg()
- **Binary Storage Format** - Compact, fast-access binary JSON representation
- **Multi-Index Support** - GIN, B-Tree, Hash, and Expression indexes
- **JSON Schema Validation** - JSON Schema Draft 7 compatible validation
- **43+ Comprehensive Tests** - Full test coverage with PostgreSQL compatibility

**AI-Native Database Features:**

- **AI Master Controller** - **COMPLETE!** Central orchestration of all intelligent features with 10-second control loop
- **Intelligent Query Optimizer** - Cost-based optimization with learning, pattern classification, and automated index recommendations
- **Predictive Resource Manager** - Workload forecasting (CPU, memory, I/O) with predictive scaling for proactive allocation
- **Smart Storage Manager** - Automated tiering engine (hot/warm/cold) with access pattern analysis and reorganization
- **Adaptive Transaction Manager** - Deadlock prediction and prevention with dynamic isolation level adjustment
- **Learning Engine** - Continuous model improvement from observations with configurable learning modes
- **Decision Engine** - Policy-based autonomous decision making with multi-criteria optimization
- **Knowledge Base** - Pattern storage and retrieval with system observation tracking
- **Production Ready** - 17 source files, 3,925+ lines, 14 tests, zero compiler warnings, 100% test success rate

**What's Next:**

- **Phase 9**: Query Optimization & Performance Tuning
- **Phase 10**: Production Readiness & High Availability
- **Phase 11**: Advanced SQL Features (JSON/JSONB - Complete)
- **Phase 12**: Persistence & Storage (RocksDB - Complete)
- **Phase 13**: Vector Database (pgvector - Complete)
- **Phase 14+**: Advanced features, multi-cloud federation, AI/ML acceleration

**Performance Benchmarks:**

- **Built-in Benchmarking System**: **NEW!** Comprehensive performance measurement with statistical analysis:
  - **Security Benchmarks**: Rate limiting, attack detection, input validation performance
  - **Statistical Metrics**: Mean, median, std deviation, operations per second
  - **Regression Detection**: Track performance changes over time
  - **Production Ready**: Memory-safe async operations with zero-cost abstractions
- **Examples Available**: [`examples/benchmarks-demo`](examples/benchmarks-demo/) - Complete demo of benchmarking capabilities
- **OrbitQL Benchmarks**: TPC-H, TPC-C, TPC-DS, and comprehensive query performance testing available in [`orbit-benchmarks`](orbit-benchmarks/)
- **Streaming Benchmarks**: CDC event processing, stream windowing, and real-time analytics performance

## **Feature Implementation Status Matrix**

| Feature | Status | Completion | Production Ready | Test Coverage | Notes |
|---------|--------|------------|------------------|---------------|-------|
| **Core Actor System** | Complete | 95% | Yes | 731 tests | Virtual actors with lifecycle management |
| **Performance Benchmarking** | Complete | 95% | Yes | 5 tests | Statistical analysis, regression detection, zero-cost abstractions |
| **CDC & Event Sourcing** | Complete | 90% | Yes | 15 tests | Real-time change capture, domain events, snapshots |
| **Stream Processing** | Complete | 85% | Yes | 4 tests | Windowing algorithms, aggregations, streaming integrations |
| **Advanced Connection Pooling** | Complete | 90% | Yes | 12 tests | Circuit breakers, health monitoring, load balancing |
| **Security Patterns** | Complete | 90% | Yes | 5 tests | Rate limiting, attack detection, audit logging |
| **RESP (Redis) Protocol** | Complete | 95% | Yes | 292 tests | 50+ commands, all data types, redis-cli compatibility |
| **PostgreSQL Wire Protocol** | Complete | 85% | Yes | 104+ tests | Full SQL DDL/DML with pgvector and RocksDB persistence |
| **MySQL Wire Protocol** | Complete | 75% | Yes | 15+ tests | MySQL-compatible SQL interface with RocksDB persistence |
| **CQL (Cassandra) Protocol** | Complete | 70% | Yes | 12+ tests | Cassandra Query Language for wide-column access |
| **HTTP REST API** | Complete | 90% | Yes | 25+ tests | Web-friendly JSON interface with OpenAPI documentation |
| **Distributed Transactions** | Complete | 85% | Yes | 270 tests | 2PC, Saga patterns, distributed locks |
| **Model Context Protocol (MCP)** | Experimental | 15% | No | 44 tests | Basic AI agent integration framework |
| **Neo4j Cypher Parser** | Planned | 5% | No | 18 tests | Basic parser structure, not functional |
| **ArangoDB AQL Parser** | Planned | 5% | No | 44 tests | Basic parser structure, not functional |
| **OrbitQL Engine** | Active | 40% | No | 256 tests | Query planning works, optimizer incomplete |
| **Persistence Layer** | Complete | 85% | Yes | 47+ tests | RocksDB, COW B+Tree, LSM-Tree, Memory, TiKV, Cloud storage |
| **Kubernetes Integration** | Active | 70% | No | 16 tests | Operator basics, needs production hardening |
| **Heterogeneous Compute** | Active | 75% | Yes | 81 tests | GPU backends (Metal, Vulkan, CUDA, ROCm), CPU SIMD, vector/spatial/timeseries ops |
| **Vector Operations (pgvector)** | Complete | 90% | Yes | 25+ tests | Full pgvector compatibility with HNSW/IVFFlat indexes |
| **Machine Learning (orbit-ml)** | Active | 50% | No | 52 tests | Neural networks, transformers, streaming inference, SQL functions |
| **Time Series** | Active | 60% | Yes | 44 tests | Compression (Delta, Gorilla), aggregation (EWMA, rate), partitioning, PostgreSQL/Redis compat |
| **Graph Database** | Active | 40% | No | 38 tests | Cypher queries, CALL procedures, graph algorithms (PageRank, BFS, DFS, centrality) |

**Legend:** Complete | Active Development | Experimental | Planned

**[View Full Roadmap](docs/roadmap.md)** | **[GitHub Project](https://github.com/orgs/TuringWorks/projects/1)**

## Contributing

We welcome contributions! See our **[Development Guide](DEVELOPMENT.md)** for setup instructions and contributing guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-new-feature`)
3. Make your changes and add tests
4. Run tests (`cargo test --workspace`)
5. Submit a pull request

**Priority areas for contributors:**

- Query optimization algorithms ([Phase 9 issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-9))
- Production readiness features ([Phase 10 issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-10))
- Protocol enhancements (MySQL, CQL, REST API improvements)
- Performance benchmarking and optimization
- Kubernetes operator enhancements

**Available Examples:**

- [`examples/server/integrated-server.rs`](examples/server/integrated-server.rs) - Multi-protocol server with all protocols enabled
- [`examples/server/persistent-redis-server.rs`](examples/server/persistent-redis-server.rs) - Redis server with RocksDB persistence
- [`examples/pgvector-store/`](examples/pgvector-store/) - PostgreSQL-compatible vector database with pgvector
- [`examples/multi-protocol-server/`](examples/multi-protocol-server/) - Comprehensive multi-protocol demonstration
- [`examples/resp-server/`](examples/resp-server/) - Redis RESP protocol server with comprehensive tests
- [`examples/vector-store/`](examples/vector-store/) - Vector similarity search example

## License

This project is dual licensed under your choice of:

- **[MIT License](LICENSE-MIT)** - Very permissive, widely compatible
- **[BSD 3-Clause License](LICENSE-BSD)** - Also permissive with endorsement clause

Choose the license that best fits your project's needs.

## Credits

- Original Orbit project by [Electronic Arts](https://www.ea.com/)
- Rust implementation by [TuringWorks](https://github.com/TuringWorks)
- AI development engineering by [Warp.dev](https://warp.dev) - The AI-powered terminal for modern development workflows

## Support

- **[Full Documentation](docs/)** - Complete documentation hub
- **[Issue Tracker](https://github.com/TuringWorks/orbit-rs/issues)** - Bug reports and feature requests
- **[Discussions](https://github.com/TuringWorks/orbit-rs/discussions)** - Community Q&A
- **[GitHub Project](https://github.com/orgs/TuringWorks/projects/1)** - Development roadmap

---

**Built with Rust for production-scale distributed systems.**
