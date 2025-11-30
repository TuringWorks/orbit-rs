# Orbit-RS Product Requirements & Architecture Document

> **Last Updated**: November 29, 2025
> **Status**: Production-Ready Multi-Protocol Database Platform
> **Purpose**: Single source of truth for architecture, modules, and implementation

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Workspace Architecture](#workspace-architecture)
3. [Module Reference](#module-reference)
4. [Client SDKs & Developer Tools](#client-sdks--developer-tools)
5. [Protocol Implementations](#protocol-implementations)
6. [Storage Architecture](#storage-architecture)
7. [AI-Native Subsystems](#ai-native-subsystems)
8. [Feature Status Matrix](#feature-status-matrix)
9. [Development Guidelines](#development-guidelines)
10. [Document Maintenance](#document-maintenance)
11. [Roadmap](#roadmap)

---

## Executive Summary

**Orbit-RS** is a high-performance, distributed multi-protocol database server written in Rust. It natively implements PostgreSQL, MySQL, CQL (Cassandra), Redis, HTTP REST, gRPC, and OrbitQL protocols from a single process, sharing a unified storage layer built on a virtual actor system.

### Value Proposition

- **One Server, All Protocols**: Replace PostgreSQL + MySQL + Cassandra + Redis with a single process
- **Cross-Protocol Consistency**: Write via SQL, read via Redis/CQL - instant consistency with ACID guarantees
- **Zero Data Duplication**: Shared storage across all protocols eliminates synchronization overhead
- **High Performance**: 500k+ ops/sec with memory safety and zero-cost Rust abstractions
- **AI-Native Database**: 8 intelligent subsystems for autonomous optimization and predictive scaling

### Build Metrics

| Metric | Value |
|--------|-------|
| Lines of Code | 148,780+ |
| Source Files | 517+ |
| Test Coverage | 1,078+ tests |
| Compiler Warnings | 0 (zero warnings policy) |
| Workspace Crates | 15 |

---

## Workspace Architecture

```
orbit-rs/
├── orbit/                           # Main source code (Rust workspace)
│   ├── server/                      # Main server binary (orbit-server)
│   ├── client/                      # Client library (OrbitClient)
│   ├── shared/                      # Shared types, traits, clustering
│   ├── engine/                      # Storage engine (OrbitQL, adapters)
│   ├── compute/                     # Hardware acceleration (SIMD, GPU)
│   ├── ml/                          # Machine learning inference
│   ├── proto/                       # Protocol Buffer definitions
│   ├── cli/                         # Interactive CLI client
│   ├── operator/                    # Kubernetes operator
│   ├── application/                 # Application configuration
│   ├── util/                        # Core utilities
│   ├── client-spring/               # Spring framework integration
│   ├── server-etcd/                 # etcd integration
│   └── server-prometheus/           # Prometheus metrics
├── orbit-python-client/             # Python SDK (non-Rust)
├── orbit-vscode-extension/          # VS Code extension (TypeScript)
├── config/                          # Configuration files
├── scripts/                         # Development scripts
├── docs/                            # Documentation (258 files)
├── tests/                           # Integration tests
├── helm/                            # Kubernetes Helm charts
└── k8s/                             # Kubernetes manifests
```

---

## Module Reference

### orbit-server (Main Binary)

**Path**: `orbit/server/`
**Binary**: `orbit-server`
**Purpose**: Multi-protocol database server

#### Directory Structure

```
orbit/server/src/
├── main.rs                          # Entry point, CLI parsing
├── server.rs                        # OrbitServer struct, protocol orchestration
├── lib.rs                           # Library exports
├── features.rs                      # Feature flag definitions
│
├── protocols/                       # Protocol implementations
│   ├── mod.rs                       # Protocol registry
│   ├── error.rs                     # Protocol error types
│   │
│   ├── resp/                        # Redis RESP protocol
│   │   ├── mod.rs                   # RESP server, connection handling
│   │   ├── codec.rs                 # RESP3 wire protocol codec
│   │   ├── types.rs                 # RespValue enum
│   │   ├── actors.rs                # Redis actor implementations
│   │   ├── commands/                # Command handlers
│   │   │   ├── mod.rs               # Command dispatcher
│   │   │   ├── traits.rs            # CommandHandler trait
│   │   │   ├── string_persistent.rs # String commands with RocksDB
│   │   │   ├── hash_commands.rs     # Hash commands
│   │   │   ├── list_commands.rs     # List commands
│   │   │   ├── set_commands.rs      # Set commands
│   │   │   ├── sorted_set.rs        # Sorted set commands
│   │   │   ├── time_series.rs       # TS.* commands (21 tests)
│   │   │   ├── vector.rs            # VECTOR.* commands
│   │   │   ├── graph.rs             # GRAPH.* commands
│   │   │   └── graphrag.rs          # GraphRAG commands
│   │   └── simple_local/            # Local registry implementation
│   │
│   ├── postgres_wire/               # PostgreSQL wire protocol
│   │   ├── mod.rs                   # PostgreSQL server
│   │   ├── messages.rs              # Wire protocol messages
│   │   ├── sql/                     # SQL processing
│   │   │   ├── mod.rs               # SQL engine
│   │   │   ├── parser/              # SQL parser (DDL, DML, DCL, TCL)
│   │   │   ├── lexer.rs             # SQL tokenizer
│   │   │   ├── ast.rs               # Abstract syntax tree
│   │   │   ├── executor.rs          # Query execution
│   │   │   ├── analyzer/            # Semantic analysis
│   │   │   ├── optimizer/           # Query optimization
│   │   │   └── types.rs             # SQL type system
│   │   ├── jsonb/                   # JSONB support
│   │   │   ├── mod.rs               # JSONB types
│   │   │   ├── storage.rs           # Binary storage
│   │   │   ├── path.rs              # JSON path expressions
│   │   │   ├── aggregation.rs       # JSON aggregation
│   │   │   ├── indexing.rs          # GIN/B-Tree indexes
│   │   │   └── schema.rs            # JSON Schema validation
│   │   ├── spatial_functions.rs     # PostGIS-compatible spatial
│   │   └── graphrag_engine.rs       # GraphRAG SQL integration
│   │
│   ├── mysql/                       # MySQL wire protocol
│   │   └── mod.rs                   # MySQL server implementation
│   │
│   ├── cql/                         # CQL (Cassandra) protocol
│   │   └── mod.rs                   # CQL server implementation
│   │
│   ├── rest/                        # HTTP REST API
│   │   ├── server.rs                # Axum HTTP server
│   │   ├── handlers*.rs             # Route handlers
│   │   ├── models.rs                # Request/response models
│   │   └── sse.rs                   # Server-sent events
│   │
│   ├── cypher/                      # Neo4j Cypher
│   │   ├── cypher_parser.rs         # Cypher query parser
│   │   ├── bolt.rs                  # Bolt protocol
│   │   └── graph_algorithms_procedures.rs
│   │
│   ├── aql/                         # ArangoDB AQL
│   │   ├── mod.rs                   # AQL module
│   │   ├── aql_parser.rs            # AQL parser
│   │   └── data_model.rs            # Multi-model data
│   │
│   ├── orbitql/                     # OrbitQL multi-model
│   │   ├── mod.rs                   # OrbitQL module
│   │   └── executor.rs              # Query executor
│   │
│   ├── ml/                          # ML SQL integration
│   │   ├── mod.rs                   # ML module
│   │   ├── models/                  # Model management
│   │   ├── functions/               # ML SQL functions
│   │   ├── engines/                 # Inference engines
│   │   └── sql_integration/         # SQL function registry
│   │
│   ├── mcp/                         # Model Context Protocol
│   │   └── types.rs                 # MCP types
│   │
│   ├── graphrag/                    # GraphRAG
│   │   ├── knowledge_graph.rs       # Knowledge graph
│   │   └── rag_pipeline.rs          # RAG pipeline
│   │
│   └── persistence/                 # Protocol persistence
│       ├── redis_data.rs            # Redis data structures
│       └── tikv_redis_provider.rs   # TiKV integration
│
├── persistence/                     # Storage backends
│   ├── mod.rs                       # Persistence trait
│   ├── factory.rs                   # Backend factory
│   ├── rocksdb.rs                   # RocksDB backend (production)
│   ├── memory.rs                    # In-memory backend (testing)
│   ├── cow_btree.rs                 # Copy-on-write B+Tree
│   ├── lsm_tree.rs                  # LSM-tree implementation
│   └── dynamic.rs                   # Dynamic backend switching
│
├── memory/                          # Memory management
│   ├── mod.rs                       # Memory module
│   ├── actor_memory_manager.rs      # Actor memory allocation
│   ├── extent_index.rs              # Memory extent tracking
│   ├── lifetime_manager.rs          # Lifetime management
│   └── pin_manager.rs               # Memory pinning
│
├── ai/                              # AI-native subsystems
│   ├── mod.rs                       # AI module exports
│   ├── controller.rs                # AI Master Controller
│   ├── decision.rs                  # Decision Engine
│   ├── knowledge.rs                 # Knowledge Base
│   ├── learning.rs                  # Learning Engine
│   ├── integration.rs               # System integration
│   ├── optimizer/                   # Intelligent Query Optimizer
│   │   ├── mod.rs
│   │   ├── cost_model.rs            # Query cost estimation
│   │   ├── index_advisor.rs         # Index recommendations
│   │   └── pattern_classifier.rs    # Query pattern ML
│   ├── resource/                    # Predictive Resource Manager
│   │   ├── mod.rs
│   │   └── workload_predictor.rs    # Workload forecasting
│   └── storage/                     # Smart Storage Manager
│       ├── mod.rs
│       └── tiering_engine.rs        # Hot/warm/cold tiering
│
├── directory.rs                     # Actor directory service
├── load_balancer.rs                 # Load balancing
└── mesh.rs                          # Service mesh
```

### orbit-client

**Path**: `orbit/client/`
**Purpose**: Client library for connecting to Orbit servers

```
orbit/client/src/
├── lib.rs                           # OrbitClient, actor references
├── invocation.rs                    # Remote invocation system
├── mesh.rs                          # Client mesh networking
└── service_discovery.rs             # Service discovery
```

**Key Types**:
- `OrbitClient` - Main client interface
- `ActorReference<T>` - Typed actor proxy
- `InvocationSystem` - Async invocation handling

### orbit-shared

**Path**: `orbit/shared/`
**Purpose**: Shared types, traits, and distributed systems primitives

```
orbit/shared/src/
├── lib.rs                           # Core exports (Actor, Key, etc.)
├── actor_communication.rs           # Actor messaging
├── addressable.rs                   # Addressable trait
├── benchmarks.rs                    # Performance benchmarking
├── cdc.rs                           # Change data capture
├── cluster_manager.rs               # Cluster coordination
├── consensus.rs                     # Raft consensus
├── election_state.rs                # Leader election
├── event_sourcing.rs                # Event sourcing patterns
├── graph.rs                         # Graph data structures
├── graphrag.rs                      # GraphRAG types
├── mesh.rs                          # Service mesh types
├── net.rs                           # Network utilities
├── pooling/                         # Connection pooling
│   ├── mod.rs                       # Pool management
│   ├── circuit_breaker.rs           # Circuit breaker pattern
│   └── health_monitor.rs            # Health checking
├── recovery.rs                      # Failure recovery
├── replication.rs                   # Data replication
├── router.rs                        # Request routing
├── saga.rs                          # Saga pattern
├── security_patterns.rs             # Security utilities
├── serialization.rs                 # Serde utilities
├── stream_processing.rs             # Stream processing
├── transaction_log.rs               # Transaction logging
├── transactions/                    # Transaction management
│   ├── mod.rs                       # Transaction coordinator
│   ├── two_phase.rs                 # 2PC implementation
│   └── distributed_lock.rs          # Distributed locking
└── triggers.rs                      # Database triggers
```

**Key Traits**:
- `Actor` - Base actor trait
- `ActorWithStringKey` - Actor with string identity
- `Addressable` - Location-transparent addressing
- `PersistenceProvider` - Storage abstraction

### orbit-engine

**Path**: `orbit/engine/`
**Purpose**: Storage engine with OrbitQL support

```
orbit/engine/src/
├── lib.rs                           # Engine exports
├── adapters/                        # Storage adapters
│   ├── mod.rs                       # Adapter trait
│   ├── orbitql_adapter.rs           # OrbitQL execution
│   └── memory_adapter.rs            # Memory storage
├── storage/                         # Storage implementations
│   ├── mod.rs                       # Storage traits
│   ├── hybrid_storage.rs            # Hot/warm/cold tiering
│   └── table_storage.rs             # Table abstraction
└── query/                           # Query processing
    ├── mod.rs                       # Query types
    ├── planner.rs                   # Query planning
    └── optimizer.rs                 # Query optimization
```

### orbit-compute

**Path**: `orbit/compute/`
**Purpose**: Hardware acceleration (SIMD, GPU, Neural)

```
orbit/compute/src/
├── lib.rs                           # Compute exports
├── engine.rs                        # Compute engine abstraction
├── scheduler.rs                     # Task scheduling
├── errors.rs                        # Error types
│
├── x86_64.rs                        # x86-64 SIMD (AVX-512)
├── aarch64.rs                       # ARM64 SIMD (NEON, SVE)
│
├── gpu/                             # GPU backends
│   ├── mod.rs                       # GPU trait
│   ├── gpu_metal.rs                 # Apple Metal
│   ├── gpu_cuda.rs                  # NVIDIA CUDA
│   ├── gpu_vulkan.rs                # Vulkan (cross-platform)
│   └── gpu_rocm.rs                  # AMD ROCm
│
├── neural.rs                        # Neural engine abstraction
├── apple.rs                         # Apple Neural Engine
├── linux.rs                         # Linux-specific
├── windows.rs                       # Windows-specific
│
├── filter_operations.rs             # Vectorized filtering
├── bitmap_operations.rs             # Bitmap operations
├── aggregation_operations.rs        # SIMD aggregations
├── vector_similarity.rs             # Vector similarity
├── spatial_distance.rs              # Spatial operations
├── graph_traversal.rs               # Graph algorithms
├── matrix_operations.rs             # Matrix math
└── timeseries_operations.rs         # Time series ops
```

### orbit-ml

**Path**: `orbit/ml/`
**Purpose**: Machine learning inference

```
orbit/ml/src/
├── lib.rs                           # ML exports
├── config.rs                        # Model configuration
├── data.rs                          # Data types (tensors)
├── error.rs                         # Error types
├── inference.rs                     # Inference engine
├── training.rs                      # Training utilities
├── metrics.rs                       # ML metrics
├── utils.rs                         # Utilities
├── models.rs                        # Model definitions
├── models/                          # Model implementations
│   ├── mod.rs
│   ├── neural_network.rs            # Neural networks
│   └── transformer.rs               # Transformer models
└── streaming/                       # Streaming inference
    └── mod.rs
```

### orbit-operator

**Path**: `orbit/operator/`
**Purpose**: Kubernetes operator for Orbit clusters

```
orbit/operator/src/
├── main.rs                          # Operator entry point
├── crd.rs                           # Custom Resource Definitions
├── actor_crd.rs                     # Actor CRD
├── actor_controller.rs              # Actor reconciliation
├── cluster_controller.rs            # Cluster reconciliation
├── transaction_crd.rs               # Transaction CRD
└── transaction_controller.rs        # Transaction reconciliation
```

### orbit-proto

**Path**: `orbit/proto/`
**Purpose**: Protocol Buffer definitions

```
orbit/proto/
├── src/
│   ├── lib.rs                       # Generated code exports
│   └── services.rs                  # Service implementations
└── proto/                           # .proto files
    ├── orbit.proto                  # Core messages
    └── services.proto               # gRPC services
```

---

## Client SDKs & Developer Tools

### orbit-python-client (Python SDK)

**Path**: `orbit-python-client/`
**Language**: Python
**Purpose**: Python client library for Orbit-RS

#### Directory Structure

```
orbit-python-client/
├── orbit_client/
│   ├── __init__.py              # Package exports
│   ├── client.py                # Main OrbitClient class
│   └── protocols.py             # Protocol adapters
├── examples/
│   ├── postgres_example.py      # PostgreSQL usage
│   ├── redis_example.py         # Redis usage
│   ├── cypher_example.py        # Graph query usage
│   ├── multi_protocol_example.py # Multi-protocol demo
│   └── timeseries_example.py    # Time series usage
├── pyproject.toml               # Package configuration
└── README.md                    # Usage documentation
```

#### Features

- PostgreSQL, MySQL, Redis, CQL protocol support
- Async and sync APIs
- Connection pooling
- Multi-protocol transactions
- **Time Series Methods** (Redis TimeSeries compatible):
  - `ts_create()` - Create time series with retention and labels
  - `ts_add()` - Add samples with auto-timestamp support
  - `ts_get()` - Get latest sample
  - `ts_range()` - Query range with aggregation
  - `ts_mrange()` - Multi-key range query with filters
  - `ts_info()` - Get time series metadata
  - `ts_del()` - Delete samples in range
  - `ts_createrule()` / `ts_deleterule()` - Compaction rules

#### Installation

```bash
cd orbit-python-client
pip install -e .
```

### orbit-vscode-extension (VS Code Extension)

**Path**: `orbit-vscode-extension/`
**Language**: TypeScript
**Purpose**: VS Code extension for Orbit-RS development

#### Directory Structure

```
orbit-vscode-extension/
├── src/
│   ├── extension.ts             # Extension entry point
│   ├── connectionManager.ts     # Connection management
│   ├── queryExecutor.ts         # Query execution
│   ├── connectionsView.ts       # Connections panel
│   ├── resultsView.ts           # Query results view
│   ├── schemaBrowser.ts         # Schema browser
│   └── connections/             # Protocol-specific connections
│       ├── postgres.ts          # PostgreSQL connection
│       ├── mysql.ts             # MySQL connection
│       ├── redis.ts             # Redis connection
│       ├── cql.ts               # CQL connection
│       ├── cypher.ts            # Cypher connection
│       ├── aql.ts               # AQL connection
│       └── mcp.ts               # MCP connection
├── syntaxes/
│   ├── orbitql.tmLanguage.json  # OrbitQL syntax highlighting
│   ├── cypher.tmLanguage.json   # Cypher syntax highlighting
│   └── aql.tmLanguage.json      # AQL syntax highlighting
├── snippets/
│   ├── orbitql.json             # OrbitQL code snippets
│   ├── cypher.json              # Cypher code snippets
│   ├── aql.json                 # AQL code snippets
│   └── sql.json                 # SQL code snippets
├── package.json                 # Extension manifest
└── tsconfig.json                # TypeScript configuration
```

#### Features

- Syntax highlighting for OrbitQL, Cypher, AQL
- Code snippets for all query languages
- Multi-protocol connection management
- Query execution and result viewing
- Schema browser
- Language configuration for all supported protocols

#### Development

```bash
cd orbit-vscode-extension
npm install
npm run compile
# Press F5 in VS Code to launch extension
```

---

## Protocol Implementations

### Port Assignments

| Protocol | Port | Module | Status |
|----------|------|--------|--------|
| PostgreSQL | 5432 | `protocols/postgres_wire/` | Complete |
| MySQL | 3306 | `protocols/mysql/` | Complete |
| CQL (Cassandra) | 9042 | `protocols/cql/` | Complete |
| Redis RESP | 6379 | `protocols/resp/` | Complete |
| HTTP REST | 8080 | `protocols/rest/` | Complete |
| gRPC | 50051 | `orbit-proto` | Complete |
| Neo4j Bolt | 7687 | `protocols/cypher/` | Active |
| ArangoDB | 8529 | `protocols/aql/` | Active |

### Redis RESP Commands (124+)

| Category | Commands | Implementation |
|----------|----------|----------------|
| Strings | GET, SET, MGET, MSET, INCR, etc. | `string_persistent.rs` |
| Hashes | HGET, HSET, HGETALL, etc. | `hash_commands.rs` |
| Lists | LPUSH, RPUSH, LPOP, LRANGE | `list_commands.rs` |
| Sets | SADD, SMEMBERS, SINTER, etc. | `set_commands.rs` |
| Sorted Sets | ZADD, ZRANGE, ZSCORE, etc. | `sorted_set.rs` |
| Time Series | TS.CREATE, TS.ADD, TS.RANGE, TS.CREATERULE | `time_series.rs` |
| Vectors | VECTOR.ADD, VECTOR.SEARCH | `vector.rs` |
| Graph | GRAPH.QUERY | `graph.rs` |

### Time Series Commands

```
TS.CREATE key [RETENTION ms] [LABELS label value ...]
TS.ADD key timestamp value
TS.GET key
TS.RANGE key from to [AGGREGATION type bucket]
TS.MRANGE from to FILTER label=value
TS.INFO key
TS.DEL key from to
TS.MADD key timestamp value [key timestamp value ...]
TS.CREATERULE sourceKey destKey AGGREGATION type bucket
TS.DELETERULE sourceKey destKey
```

**Aggregation Types**: AVG, SUM, MIN, MAX, RANGE, COUNT, FIRST, LAST, STD.P, VAR.P, TWA

---

## Storage Architecture

### Persistence Backends

| Backend | File | Use Case | Status |
|---------|------|----------|--------|
| RocksDB | `rocksdb.rs` | Production (default) | Complete |
| Memory | `memory.rs` | Testing | Complete |
| COW B+Tree | `cow_btree.rs` | High-read workloads | Complete |
| LSM Tree | `lsm_tree.rs` | Write-optimized | Complete |
| TiKV | `tikv_redis_provider.rs` | Distributed KV | Active |

### Storage Tiering

```
┌─────────────────────────────────────────────┐
│                Hot Tier                      │
│           (In-Memory / Redis)               │
│         < 100ms access latency              │
├─────────────────────────────────────────────┤
│               Warm Tier                      │
│          (RocksDB / LSM Tree)               │
│         < 10ms access latency               │
├─────────────────────────────────────────────┤
│               Cold Tier                      │
│        (Apache Iceberg / Parquet)           │
│         < 1s access latency                 │
└─────────────────────────────────────────────┘
```

---

## AI-Native Subsystems

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   AI Master Controller                       │
│              (10-second control loop)                       │
├──────────────┬──────────────┬──────────────┬───────────────┤
│   Query      │   Resource   │   Storage    │  Transaction  │
│  Optimizer   │   Manager    │   Manager    │   Manager     │
├──────────────┼──────────────┼──────────────┼───────────────┤
│   Learning Engine    │    Decision Engine    │  Knowledge   │
│                      │                       │    Base      │
└──────────────────────┴───────────────────────┴──────────────┘
```

### Subsystem Details

| Subsystem | Path | Purpose |
|-----------|------|---------|
| AI Master Controller | `ai/controller.rs` | Central orchestration |
| Intelligent Query Optimizer | `ai/optimizer/` | Cost-based optimization |
| Predictive Resource Manager | `ai/resource/` | Workload forecasting |
| Smart Storage Manager | `ai/storage/` | Hot/warm/cold tiering |
| Learning Engine | `ai/learning.rs` | Model improvement |
| Decision Engine | `ai/decision.rs` | Policy-based decisions |
| Knowledge Base | `ai/knowledge.rs` | Pattern storage |

---

## Feature Status Matrix

| Feature | Status | Tests | Key Files |
|---------|--------|-------|-----------|
| Core Actor System | Complete | 731 | `orbit-shared/src/lib.rs` |
| RESP Protocol | Complete | 292 | `protocols/resp/` |
| PostgreSQL Protocol | Complete | 104 | `protocols/postgres_wire/` |
| MySQL Protocol | Complete | 15 | `protocols/mysql/` |
| CQL Protocol | Complete | 12 | `protocols/cql/` |
| REST API | Complete | 25 | `protocols/rest/` |
| Distributed Transactions | Complete | 270 | `shared/src/transactions/` |
| AI-Native Features | Complete | 14 | `server/src/ai/` |
| Vector Database | Complete | 25 | `postgres_wire/sql/pgvector*` |
| Time Series | Active | 36 | `resp/commands/time_series.rs` |
| Graph Database | Active | 38 | `protocols/cypher/` |
| Kubernetes Operator | Active | 16 | `orbit-operator/` |
| Heterogeneous Compute | Active | 81 | `orbit-compute/` |
| Machine Learning | Active | 52 | `orbit-ml/` |

---

## Development Guidelines

### Code Organization

1. **Protocol implementations** go in `orbit/server/src/protocols/`
2. **Shared types and traits** go in `orbit/shared/src/`
3. **Storage backends** go in `orbit/server/src/persistence/`
4. **Hardware acceleration** goes in `orbit/compute/src/`
5. **AI features** go in `orbit/server/src/ai/`

### Adding a New Protocol

1. Create directory under `protocols/`
2. Implement `mod.rs` with server struct
3. Add to protocol registry in `protocols/mod.rs`
4. Add CLI flags in `main.rs`
5. Add configuration in `config/orbit-server.toml`

### Adding Storage Backend

1. Implement `PersistenceProvider` trait
2. Add file in `persistence/`
3. Register in `persistence/factory.rs`
4. Add feature flag if optional

### Testing

```bash
# All tests
cargo test --workspace

# Specific package
cargo test -p orbit-server

# Time series tests
cargo test -p orbit-server time_series::

# Slow integration tests
cargo test --workspace -- --ignored
```

---

## Roadmap

### Completed (Phases 1-8)

1. Foundation & workspace setup
2. Core actor system
3. Network layer (gRPC, Protocol Buffers)
4. Cluster management (Raft, leader election)
5. Transaction system (2PC, Saga)
6. Protocol adapters (Redis, PostgreSQL, MySQL, CQL)
7. Kubernetes integration
8. SQL query engine & vector database

### Current Focus (Phase 9-10)

- Query optimization & vectorized execution
- Production readiness & high availability
- Advanced backup & recovery

### Future (Phases 11+)

| Phase | Focus |
|-------|-------|
| 11 | Stored procedures, triggers, full-text search |
| 12 | TimescaleDB compatibility |
| 13 | Complete Neo4j Bolt protocol |
| 14 | Distributed query optimization |
| 15 | Full ArangoDB compatibility |
| 16 | GraphML/GraphRAG enhancement |
| 17 | GraphQL, MongoDB protocols |
| 18 | Multi-cloud, edge computing |
| 19 | Enterprise compliance |

---

## Quick Reference

### Starting the Server

```bash
cargo run --bin orbit-server                    # Default
cargo run --bin orbit-server -- --dev-mode      # Development
cargo run --bin orbit-server -- --config path   # Custom config
./scripts/start-multiprotocol-server.sh         # Script
```

### Client Connections

```bash
psql -h localhost -p 5432 -U orbit -d actors    # PostgreSQL
redis-cli -h localhost -p 6379                   # Redis
curl http://localhost:8080/health                # REST API
```

### Key Configuration

```toml
# config/orbit-server.toml
[server]
bind = "0.0.0.0"
grpc_port = 50051
http_port = 8080
postgres_port = 5432
redis_port = 6379

[persistence]
backend = "rocksdb"
data_dir = "./data"

[ai]
enabled = true
control_loop_interval_ms = 10000
```

---

## Document Maintenance

> **This document is the single source of truth for Orbit-RS architecture.**
> All AI agents and developers MUST keep it synchronized with the codebase.

### AI Agent Instructions

This document must be read and maintained by all AI coding assistants:
- **Claude Code / Anthropic Claude** - See `CLAUDE.md`
- **Cursor AI** - See `.cursorrules`
- **Gemini, Copilot, Warp, Antigravity, others** - See `AGENTS.md`

### When to Update This Document

Update this PRD.md when you:

| Change Type | What to Update |
|-------------|----------------|
| Add new module/crate | Module Reference section, directory trees |
| Add source files | Module Reference, file descriptions |
| Change directory structure | Module Reference, affected trees |
| Add protocol commands | Protocol Implementations section |
| Update feature flags | Feature Status Matrix |
| Change API interfaces | Protocol Implementations |
| Modify storage/compute | Storage Architecture section |
| Add AI subsystems | AI-Native Subsystems section |
| Change test coverage | Feature Status Matrix, test counts |

### Client SDK & Extension Updates (REQUIRED for Breaking Changes)

**When making breaking changes to protocols or APIs, you MUST also update:**

| Change Type | Update Required |
|-------------|-----------------|
| New Redis/RESP commands | `orbit-python-client/orbit_client/client.py` |
| New PostgreSQL features | `orbit-python-client/orbit_client/protocols.py` |
| Protocol wire format changes | Both Python client and VS Code extension |
| New query languages | `orbit-vscode-extension/syntaxes/` (syntax highlighting) |
| New connection types | `orbit-vscode-extension/src/connections/` |
| API response format changes | `orbit-python-client/` response handling |

#### Breaking Change Checklist

```
[ ] Identify if change affects external clients
[ ] Update orbit-python-client if protocol/API changed
[ ] Update orbit-vscode-extension if syntax/connections affected
[ ] Update examples in orbit-python-client/examples/
[ ] Test Python client against new server
[ ] Update VS Code extension README if features added
```

#### Files to Check for Breaking Changes

**Python Client (`orbit-python-client/`):**
- `orbit_client/client.py` - Main client class, command methods
- `orbit_client/protocols.py` - Protocol adapters
- `examples/*.py` - Usage examples

**VS Code Extension (`orbit-vscode-extension/`):**
- `src/connections/*.ts` - Protocol connections
- `src/queryExecutor.ts` - Query execution logic
- `syntaxes/*.tmLanguage.json` - Syntax highlighting
- `snippets/*.json` - Code snippets

### Update Checklist

```
[ ] Read current PRD.md before making changes
[ ] Make code changes
[ ] Update relevant PRD.md sections
[ ] Update "Last Updated" date at top
[ ] Update test counts if changed
[ ] Run: cargo fmt --all
[ ] Run: cargo clippy --workspace -- -D warnings
[ ] Run: cargo test --workspace
[ ] Commit code AND PRD.md together
```

### Commit Message Format

When updating this document along with code changes:
```
type(scope): description

- code changes summary
- docs: update PRD.md with [what changed]

🤖 Generated with [AI Assistant Name]
```

### Section Ownership

| Section | Updated When |
|---------|--------------|
| Executive Summary | Major releases, metric changes |
| Workspace Architecture | Crate additions/removals |
| Module Reference | Any structural changes |
| Protocol Implementations | Command additions, port changes |
| Storage Architecture | Backend changes |
| AI-Native Subsystems | AI feature changes |
| Feature Status Matrix | Implementation progress |
| Development Guidelines | Process changes |

---

**Orbit-RS: One Server, All Protocols**
