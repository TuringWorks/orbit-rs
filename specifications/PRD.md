---
layout: default
title: "Product Requirements & Architecture Document"
subtitle: "Single source of truth for architecture, modules, and implementation"
category: "architecture"
permalink: /PRD.html
---

> **Last Updated**: December 12, 2024
> **Status**: Production-Ready Multi-Protocol Database Platform
> **Architecture Reference**: See [`docs/content/architecture/ORBIT_ARCHITECTURE.md`](content/architecture/ORBIT_ARCHITECTURE.md) for detailed architecture patterns, transaction layer (MVCC, 2PC, Saga), query execution (vectorized, SIMD), network layer (gRPC, Protocol Buffers), and hybrid storage architecture.
> **Protocol Analysis**: See [`protocols/PROTOCOL_COMPLETION_ANALYSIS.md`](protocols/PROTOCOL_COMPLETION_ANALYSIS.md) for detailed protocol implementation status and gaps.

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
| Lines of Code | 365,000+ |
| Source Files | 530+ |
| Test Coverage | 2,400+ tests |
| Compiler Warnings | 0 (zero warnings policy) |
| Workspace Crates | 15 |

---

## Workspace Architecture

```text
orbit-rs/
├── orbit/                           # Main source code (Rust workspace)
│   ├── server/                      # Main server binary (orbit-server)
│   ├── client/                      # Client library (OrbitClient)
│   ├── shared/                      # Shared types, traits, clustering
│   ├── engine/                      # Storage engine (OrbitQL, adapters)
│   ├── compute/                     # Hardware acceleration (SIMD, GPU)
│   ├── ml/                          # Machine learning inference
│   ├── llm/                         # LLM provider abstraction, registry, router
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

#### Server Module Structure

```text
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
│   │   │   ├── llm.rs               # LLM.* model management and inference
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
│   ├── orbitql/                     # OrbitQL multi-model query language
│   │   ├── mod.rs                   # OrbitQL module exports
│   │   ├── lexer.rs                 # Token lexer (SurrealDB-style keywords)
│   │   ├── ast.rs                   # Abstract Syntax Tree definitions
│   │   ├── parser.rs                # Recursive descent parser
│   │   ├── executor.rs              # Query executor
│   │   ├── optimizer.rs             # Query optimization
│   │   ├── planner.rs               # Query planning
│   │   ├── streaming.rs             # LIVE query support
│   │   └── ml_functions.rs          # ML function integration
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
├── lua/                             # Lua UDF Support (mlua-based)
│   ├── mod.rs                       # Lua module exports
│   ├── mlua_runtime.rs              # mlua runtime integration (LuaJIT/Lua 5.4)
│   ├── lua_value.rs                 # LuaValue ↔ SqlValue conversion
│   ├── security.rs                  # Execution limits, sandbox
│   ├── udf_registry.rs              # Lua function registry
│   ├── redis_api.rs                 # redis.call(), redis.pcall()
│   ├── database_api.rs              # sql.execute(), db.query()
│   └── tests.rs                     # Comprehensive test suite
│
├── python/                          # Python UDF Support (subprocess-based)
│   ├── mod.rs                       # Python module exports (~2,300 lines total)
│   ├── worker.py                    # Python worker process (327 lines)
│   ├── runtime.rs                   # Connection pooling runtime (479 lines)
│   ├── types.rs                     # PythonValue ↔ SqlValue conversion (204 lines)
│   ├── config.rs                    # Configuration (PythonConfig) (128 lines)
│   ├── udf_registry.rs              # Function metadata management (376 lines)
│   ├── udf_handler.rs               # SQL statement handler (389 lines)
│   └── tests.rs                     # Comprehensive test suite (530 lines)
│
├── directory.rs                     # Actor directory service
├── load_balancer.rs                 # Load balancing
└── mesh.rs                          # Service mesh
```

### orbit-client

**Path**: `orbit/client/`
**Purpose**: Client library for connecting to Orbit servers

```text
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

```text
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
├── patterns/                        # Reusable Rust idiom implementations
│   ├── mod.rs                       # Pattern exports
│   ├── conversions.rs               # From/TryFrom, Cow, AsRef boundaries
│   ├── interior_mutability.rs       # Cell/RefCell/RwLock ownership
│   ├── iterators.rs                 # Custom iterator adapters
│   ├── phantom_types.rs             # Units, branded IDs, capability markers
│   ├── raii_guards.rs               # Drop-based transaction/metric guards
│   ├── sealed_traits.rs             # Sealed traits for evolvable APIs
│   ├── strategy.rs                  # Retry/serialization/compression strategies
│   ├── typestate.rs                 # Compile-time state machines
│   └── visitors.rs                  # AST visitors (SQL gen, optimize, validate)
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

```text
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

```text
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

```text
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

### orbit-llm

**Path**: `orbit/llm/`
**Purpose**: Provider-agnostic LLM and embedding layer — a model gateway inside the database process

```text
orbit/llm/src/
├── lib.rs                           # Crate exports
├── types.rs                         # ChatRequest/Response, Message, TokenUsage, Cost
├── provider.rs                      # LlmProvider / EmbeddingProvider traits, ProviderKind
├── config.rs                        # ModelProfile, ProviderConfig, LlmConfig, env layering
├── registry.rs                      # LlmRegistry — named, hot-swappable model profiles
├── router.rs                        # timeout → retry → breaker → fallback → accounting
├── retry.rs                         # Exponential backoff with full jitter
├── breaker.rs                       # Per-profile circuit breaker
├── usage.rs                         # Token/cost/failure counters
├── secret.rs                        # SecretString (redacting Debug/Display/Serialize)
├── http.rs                          # Shared pooled reqwest client
├── compat.rs                        # orbit_shared::graphrag::LLMProvider → ModelProfile
├── testing.rs                       # In-process stub provider (test-only)
└── providers/
    ├── openai_shape.rs              # Shared /chat/completions + /embeddings wire shape
    ├── openai.rs                    # OpenAI (max_completion_tokens)
    ├── anthropic.rs                 # Anthropic Messages API
    ├── ollama.rs                    # Ollama /api/chat + /api/embed
    └── compatible.rs                # Azure/vLLM/Groq/Together/OpenRouter/LM Studio/DeepSeek
```

**Capabilities**: unified API over 4 wire shapes (~15 named services), named model profiles
switchable at runtime with no restart, fallback chains, retries with jittered backoff, per-profile
circuit breakers, per-attempt timeouts, connection pooling, and token/cost accounting.

**Consumers**: GraphRAG (`server/src/protocols/graphrag/`), the `LLM.*` RESP commands
(`server/src/protocols/resp/commands/llm.rs`), via the shared runtime in `server/src/llm/`.

**Design notes**: token counts and cost are `Option` — a provider that reports nothing yields
absent, never zero. No model price table is bundled; cost is computed only from configured prices.
See [`AI_LLM_ROADMAP.md`](AI_LLM_ROADMAP.md) and [`COMPETITIVE_ANALYSIS.md`](COMPETITIVE_ANALYSIS.md).

### orbit-operator

**Path**: `orbit/operator/`
**Purpose**: Kubernetes operator for Orbit clusters

```text
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

```text
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

#### Python SDK Structure

```text
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

#### Python SDK Features

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

#### VS Code Extension Structure

```text
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

### orbit-desktop (Desktop GUI)

**Path**: `orbit/desktop/`
**Language**: Rust (Tauri 1.x backend) + TypeScript/React (frontend)
**Purpose**: Desktop client for connecting to Orbit-RS, running statements,
reading results, and controlling a local development cluster.

> Note: this crate carries its own `[workspace]` in `src-tauri/Cargo.toml`, so
> it is **not** built by the root workspace. `make check` and `cargo test` at the
> repository root do not cover it — it must be built and tested separately.

#### Desktop Structure

```text
orbit/desktop/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Tauri commands + application state
│   │   ├── connections.rs       # Connection descriptions and live sessions
│   │   ├── queries.rs           # Statement execution, timeouts, history
│   │   ├── cluster.rs           # Local cluster lifecycle and observation
│   │   ├── models.rs            # ML function catalogue
│   │   ├── storage.rs           # Persisted connections and settings
│   │   └── encryption.rs        # AES-GCM password storage
│   └── Cargo.toml               # Separate workspace
├── src/
│   ├── App.tsx                  # Shell, editor tabs, results, side panels
│   ├── components/
│   │   ├── ConnectionDialog.tsx # Create/edit a connection
│   │   ├── ConnectionManager.tsx# Connect, disconnect, delete
│   │   ├── ClusterPanel.tsx     # Cluster start/stop/status/logs
│   │   ├── QueryEditor.tsx      # CodeMirror editor
│   │   ├── QueryResultsTable.tsx# Result grid and CSV/JSON export
│   │   ├── QueryHistoryPanel.tsx# Past statements with real timings
│   │   ├── DataVisualization.tsx# Chart.js views
│   │   └── MLModelManager.tsx   # ML function reference
│   ├── services/tauri.ts        # Typed wrapper over the Tauri commands
│   ├── utils/queryFormatter.ts  # ReDoS-hardened SQL formatter
│   └── types/index.ts           # Mirrors the Rust command payloads
└── package.json
```

#### Connection Model

A **connection** is a saved description (host, port, credentials); a **session**
is a live handle opened from it. Descriptions persist across restarts, sessions
do not. Sessions open lazily on first use and are held open across statements,
so `SET`, temporary tables, open transactions and Redis `SELECT` behave as
expected. A dead session is detected by ping and transparently reopened.

| Protocol | Client | Statement support |
|----------|--------|-------------------|
| PostgreSQL | `tokio-postgres` | Full: typed columns, real affected-row counts |
| MySQL | `mysql_async` | Full: real column names and types |
| Redis | `redis` (multiplexed) | Full: quoted-argument parsing, recursive RESP decoding |
| CQL | TCP probe only | Reachability only — no binary-protocol client; statements are refused |
| OrbitQL, Cypher, AQL, FlightSQL, OrbitWire | HTTP | Reaches `/api/v1/sql`, which executes against the shared SQL engine |

#### Cluster Lifecycle

The cluster panel drives `scripts/start-cluster.sh` and reports only observed
state: PID files on disk, process liveness and uptime from `ps`, port numbers
read from each live process's own command line, and TCP reachability probed per
port. It deliberately does **not** read `/api/v1/cluster/*`, whose handlers
return fixed values rather than measurements. "Running" and "serving" are
reported separately so a node that is up with dead listeners is visible.

#### Transport Security

`ssl_mode` accepts `disable`, `prefer`, `require`, `verify-ca` and
`verify-full`. `prefer` is treated as `require`: PostgreSQL's own `prefer`
falls back to plaintext silently, which downgrades a connection without anyone
noticing. `require` encrypts without authenticating the peer; `verify-ca` and
`verify-full` verify against the system root store. An unrecognised value is
rejected rather than defaulted to plaintext.

PostgreSQL negotiates TLS in-band (`SSLRequest`); Redis selects it by scheme
(`rediss://`). Both are wired to the same `ssl_mode`.

#### Known Limitations

- **Model management is not implemented.** Listing and deleting models report
  that plainly instead of returning fabricated models.
- **CQL is reachability-only.** There is no CQL binary-protocol client here;
  statements are refused rather than appearing to run.
- **REST SQL takes no parameters.** Inline the values, or use the PostgreSQL
  protocol, which binds parameters server-side.

#### Development

```bash
cd orbit/desktop
npm install
npm run dev                  # Vite + Tauri
npm run typecheck            # tsc --noEmit
npm test                     # vitest

# Backend (separate workspace)
cargo test --manifest-path src-tauri/Cargo.toml
# Tests marked #[ignore] need a running server / spawn real processes:
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --test-threads=1
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
| Arrow Flight SQL | 50052 | `protocols/flight/` | Specified |
| OrbitWire | 50053 | `protocols/orbitwire/` | Specified |

### OrbitQL Wire Protocols

OrbitQL queries can be executed via two purpose-built wire protocols:

| Protocol | Best For | Key Features |
|----------|----------|--------------|
| **Arrow Flight SQL** | Analytics, bulk data | Zero-copy columnar transfer, gRPC/HTTP2, client ecosystem |
| **OrbitWire** | Interactive, CLI | Low-latency, stream multiplexing, LIVE queries |

**Detailed Specifications**:
- [`protocols/Protocol-specs/arrow-flight-sql-specification.md`](protocols/Protocol-specs/arrow-flight-sql-specification.md) - Arrow Flight SQL integration
- [`protocols/Protocol-specs/orbitwire-protocol-specification.md`](protocols/Protocol-specs/orbitwire-protocol-specification.md) - Custom OrbitWire binary protocol

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
| GraphRAG | GRAPHRAG.BUILD, GRAPHRAG.QUERY, GRAPHRAG.STATS | `graphrag.rs` |
| LLM | LLM.PROVIDERS, LLM.MODELS, LLM.INFO, LLM.REGISTER, LLM.UNREGISTER, LLM.USE, LLM.GENERATE, LLM.EMBED, LLM.STATS | `llm.rs` |

### Time Series Commands

```text
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

### LLM Commands

Model management and inference. Registering or switching a model takes effect on the next request
with no server restart, and applies to every AI surface (GraphRAG included) because all of them
resolve models through one registry.

```text
LLM.PROVIDERS                       # wire shapes this build supports
LLM.MODELS                          # registered profiles, marking the default
LLM.INFO <profile>                  # full profile detail (credentials redacted)
LLM.REGISTER <profile> <provider> <model> [option value ...]
LLM.UNREGISTER <profile>
LLM.USE <profile>                   # switch the default model, no restart
LLM.GENERATE <prompt> [MODEL p] [SYSTEM s] [MAXTOKENS n] [TEMPERATURE t]
LLM.EMBED <text> [text ...] [MODEL p]
LLM.STATS [profile]                 # requests, failures, fallbacks, tokens, cost, breaker state
```

**Providers**: `openai`, `anthropic`, `ollama`, and the OpenAI-compatible shape, reachable by the
service aliases `azure`, `vllm`, `groq`, `together`, `openrouter`, `lmstudio`, `deepseek`,
`fireworks`, `local`.

**`LLM.REGISTER` options**: `APIKEY`, `BASEURL`, `APIVERSION`, `ORGANIZATION`, `PROJECT`,
`EMBEDDINGMODEL`, `TEMPERATURE`, `MAXTOKENS`, `TIMEOUTMS`, `FALLBACKS` (comma-separated),
`PRICEPROMPT`, `PRICECOMPLETION` (USD per million tokens).

An unrecognized option is rejected rather than ignored, so a typo cannot silently do nothing.

**Example** — add a second model and switch to it at runtime:

```text
LLM.REGISTER groq groq llama-3.3-70b-versatile BASEURL https://api.groq.com/openai/v1 APIKEY $KEY
LLM.REGISTER claude anthropic claude-sonnet-4-5 MAXTOKENS 4096 FALLBACKS groq
LLM.USE claude
LLM.GENERATE "summarise the last incident" MAXTOKENS 200
LLM.STATS
```

Configuration lives in the `[llm]` section of `config/orbit-server.toml`, layered under `LLM_*`
environment variables. Credentials belong in the environment, never the file.

---

### REST SQL and Catalogue Endpoints

`/api/v1/sql` executes through the same `QueryEngine` the PostgreSQL protocol
uses, over the same RocksDB storage, so a table created via `psql` is visible
over HTTP and vice versa. `/tables`, `/tables/{schema}/{table}` and `/schemas`
read the real catalogue; `/stats` reports measured uptime and table count.

Endpoints report what they can observe and omit what they cannot. Fields that
were previously filled with fixed values — node CPU/memory/disk, actor counts,
replication factor, on-disk size, query history — are now absent, zero where
zero is the truth, or reported as unavailable. A number in a response is a
measurement.


### PostgreSQL Protocol Conformance

Measured, not asserted. `tests/integration/pg_conformance.rs` drives the server
with `tokio-postgres` — a conforming client — and prints a pass/fail matrix:

```bash
./target/debug/orbit-server --dev-mode --data-dir /tmp/pgconf --config config/orbit-server.toml &
cargo test -p orbit-integration-tests --test pg_conformance -- --ignored --nocapture
```

Current score: **212/212**. By area:

| Area | Score | Notes |
|------|-------|-------|
| Connection (SCRAM) | 1/1 | |
| Simple query | 9/9 | Includes `SET`/`SHOW` and `SHOW ALL` |
| Extended query | 7/7 | Parse/Bind/Describe/Execute, typed and bound parameters |
| Portals / cursors | 1/1 | `Execute` row limits, `PortalSuspended` |
| Data types | 15/15 | Text, binary, `SMALLINT`, `BIGINT`, `NUMERIC`, `REAL`, `DOUBLE PRECISION`, `DATE`, `TIMESTAMP`, `JSON`; empty string distinct from NULL |
| Transactions | 26/26 | Per-row undo for every write path; `SET CONSTRAINTS` switches when deferred keys are checked |
| Catalog | 5/5 | `version()`, `current_database()`, `pg_class`, `pg_type`, `information_schema` |
| COPY | 3/3 | `TO STDOUT` and `FROM STDIN`, text and binary formats |
| LISTEN / NOTIFY | 1/1 | Cross-session, delivered while idle |
| Error reporting | 7/7 | SQLSTATE, session survives errors, aborted blocks reject work, statements after a failure do not run |
| SQL surface | 104/104 | See below |

The SQL surface covers `WHERE` (`AND`/`OR`/`NOT`/`LIKE`/`ILIKE`/`BETWEEN`/`IS NULL`/`IN`/`NOT IN`),
`ORDER BY` (expressions, ordinals, aliases, `NULLS FIRST`/`LAST`), `LIMIT`/`OFFSET`,
`GROUP BY`, `HAVING`, `DISTINCT`, `DISTINCT ON`, `COUNT(DISTINCT)`, aggregates,
`STRING_AGG`/`ARRAY_AGG`, `JOIN`/`LEFT JOIN`, scalar, `IN`, `EXISTS` and
**correlated** subqueries, derived tables, CTEs, set operations, window functions,
`CASE`, `COALESCE`, `NULLIF`, `GREATEST`/`LEAST`, casts to and from text,
string, math and date functions (`EXTRACT`), `RETURNING`, `INSERT ... SELECT`,
multi-row `INSERT`, `ON CONFLICT`, `UPDATE` with expressions, `NOT NULL`,
`PRIMARY KEY`/`UNIQUE`, `DEFAULT`, `CHECK` and `REFERENCES` enforcement,
`WITH RECURSIVE`, `LATERAL` joins, comma joins, `NATURAL JOIN`, window frames
(`ROWS`, `RANGE` and `GROUPS`), `PERCENT_RANK`/`CUME_DIST`, table-level
`PRIMARY KEY`/`UNIQUE`/`FOREIGN KEY`/`CHECK` clauses, composite foreign keys
with `ON DELETE`/`ON UPDATE` actions, `MATCH FULL`/`PARTIAL`, `DEFERRABLE` checking with
`SET CONSTRAINTS`, domains with `ALTER DOMAIN` and `CHECK (VALUE ...)`,
triggers with `INSTEAD OF` and `NEW`/`OLD`, `ALTER TABLE RENAME`,
views, materialized views with `REFRESH`,
`CREATE TABLE AS SELECT`, `ALTER TABLE ADD`/`DROP COLUMN`, `TRUNCATE`,
schema-qualified and quoted identifiers, FROM-less selects, `EXPLAIN` and
`CREATE INDEX`.

#### One source of truth for SELECT

`sql/select_pipeline.rs` applies a statement's clauses — `WHERE` → window
functions → `GROUP BY`/aggregates → `HAVING` → `DISTINCT` → `ORDER BY` →
`OFFSET`/`LIMIT` → projection — to rows read from **persistent storage**, the
same rows a plain `SELECT` reads. A select with no `FROM` runs over one empty
row, so it uses the same path.

Earlier arrangements that were wrong, and are worth recording because each one
reported success while answering incorrectly:

1. The executor read every row and projected by column name, dropping those
   clauses silently. `SELECT ... LIMIT 2` returned the whole table, `ORDER BY`
   returned rows unordered, `COUNT(*)` returned one empty column per row.
2. Routing clause-bearing statements to the in-memory engine instead made them
   read a *different copy* of the table, so `SELECT id FROM t` and
   `SELECT id FROM t ORDER BY id` disagreed about how many rows existed.
3. **Two routing tables.** The simple-query wire path dispatched over the parsed
   AST while everything else dispatched over the statement text, so features
   worked from one and not the other. `execute_multiple_queries` now splits the
   message into statements and sends each through `execute_query`.
4. **`WHERE` was dropped by writes.** The condition was built as a closure and
   handed to a `TableStorage` method that discards it, so
   `DELETE FROM t WHERE id = 2` emptied the table. Likewise `drop_table`
   removed only the schema, so the next `CREATE TABLE` resurrected the rows.
5. **`SET n = n + 1` stored the text `"amount + 1"`** in an integer column.
6. **`INSERT ... SELECT` inserted nothing** and reported success.
7. **Constraints were parsed and then dropped** by the schema round trip, so
   `PRIMARY KEY` accepted duplicates and `DEFAULT` left NULL.
8. **An unknown column returned NULLs** rather than an error.
9. **`ORDER BY 1` and `ORDER BY <alias>` were evaluated as expressions**, so the
   statement returned rows in storage order while appearing to sort.
10. **A correlated subquery was resolved once** and applied to every row.
11. **Dead parser branches.** `NULLS FIRST`/`LAST` matched
    `Token::Identifier("NULLS")` while the lexer emits `Token::Nulls`;
    `NOT IN` was looked for as `IN NOT`. Neither could ever fire.
12. **`CREATE TABLE t AS SELECT` panicked the connection's task** —
    `parse_create_table` unwrapped `find('(')`. Every later statement on that
    connection failed with "connection closed". The same unwraps were reachable
    in the `INSERT` and `DROP TABLE` parsers.
13. **`STRING_AGG(x, sep)` ignored its separator** and always joined on a comma.
14. **A quoted identifier lost its case on read.** The stored name is already
    final; folding it again lowercased it, so `SELECT "Id"` could not find a
    column that `SELECT *` reported as `id`.
15. **An aborted transaction accepted more statements.** PostgreSQL rejects
    everything but a rollback once a statement in the block has failed.
16. **`UPDATE`/`DELETE ... WHERE a = 1 AND b = 2` changed nothing.** The
    simple parser read the whole predicate as one condition, comparing `a`
    against the text `1 AND b = 2`, so the statement matched no rows and
    reported success. Conjuncts are now separate conditions, and an `OR` —
    which a condition list cannot express — is refused rather than mis-read.
17. **Most keyword-named columns could not be parsed.** The lexer turns 393
    words into keyword tokens, and only a handful were accepted as
    identifiers, so `INSERT INTO t (id, label)` failed to parse — PostgreSQL
    reserves fewer than 70 of them. The parser now consults the lexer's own
    keyword table in reverse and accepts any word not on the reserved list,
    which is the rule PostgreSQL itself applies. This was found through a
    trigger that silently never fired, because the AST parse it depended on
    was failing while the statement itself succeeded through another path.
18. **One session's `ROLLBACK` destroyed another session's committed rows.**
    Undo restored a copy of the whole table taken before the block's first
    write, so a row a second session committed while that block was open was
    wiped by the first session's rollback. That is data loss caused by an
    unrelated connection, not a visibility anomaly. Undo is now row-scoped:
    rows this session inserted are matched by value and removed, rows it
    changed or deleted are put back only if they are no longer there, and
    nothing else is touched. Savepoints record how far each undo log had
    grown, so `ROLLBACK TO SAVEPOINT` undoes exactly the writes made after it.
19. **Every scan was silently truncated at 10,000 rows.** `UnifiedStorage::scan`
    capped a caller that asked for no limit at `max_scan_limit`, *after*
    reading every matching record into memory — so the cap saved nothing and
    corrupted the answer. `SELECT COUNT(*)` on a 12,000-row table returned
    10,000 and reported success. The cap now refuses the scan, naming the row
    count and the config key, rather than returning a short answer; an
    explicit `LIMIT` is the caller's decision and is honoured as given. The
    default moved from 10,000 to 1,000,000, which is a runaway threshold
    rather than an ordinary table size.
20. **Nothing was persisted at all.** `UnifiedStorageIntegration` built a
    `MemoryBackend` on *both* arms of its `use_memory_backend` branch — the
    flag documented an intention and selected nothing — while the log said
    "persistent backend". Every table and row served over the SQL protocols
    lived only in that process. `UnifiedTableStorage` compounded it by holding
    schemas in a map that was the record rather than a cache. See
    **Durability** below.

#### Beyond the wire protocol

Properties the harness cannot reach, verified by hand against a running server:

| Property | How it was checked | Result |
|----------|--------------------|--------|
| Durability | Write, stop, restart, read | Rows and schemas survive |
| Crash safety | Write, `SIGKILL`, restart, read | Survives via the RocksDB WAL — now automated as `pg_crash_durability` |
| Power-loss safety | `sync_wal` issues an fsync before acknowledging | Writes reach the disk; **not** verified by a real power cut |
| Corruption | Overwrite bytes inside an SST, reopen, scan | Reported as a checksum error, never served as data |
| Redis durability | `SET`, restart, `GET` | Survives |
| MySQL protocol | Raw handshake on 3306 | Server greeting, protocol 10 |
| CQL protocol | `OPTIONS` frame on 9042 | `SUPPORTED` reply |
| Write concurrency | 8 workers x 100 inserts | All 800 rows, no errors |
| Logical replication | Raw walsender session on 5432 | `IDENTIFY_SYSTEM`, `CREATE_REPLICATION_SLOT`, `START_REPLICATION` → `CopyBothResponse`, then an `XLogData` frame carrying a live `INSERT` |
| Replication replay | Write with nobody streaming, then connect and `START_REPLICATION ... 0/1` | Both missed writes replayed from the requested LSN |
| Replication feedback | Standby status update with the reply flag set | Keepalive returned; the confirmed position is written to the slot |
| Slot durability | Create a slot, restart, `START_REPLICATION` on it | Slot survives |
| Durable change log | Write two rows, restart, replay from `0/1` | Both replayed from the log on disk |
| `pgoutput` format | Slot created with the `pgoutput` plugin | `Begin`, `Relation`, `Insert`, `Commit` in one frame |
| Physical replication | `START_REPLICATION 0/0 PHYSICAL` | Refused with `0A000` (`feature_not_supported`), not answered with logical frames; the session stays usable and `IDENTIFY_SYSTEM` still replies |
| Transaction grouping | A two-statement block on a `pgoutput` stream | One `Begin`, both rows, one `Commit` |
| Change-log trimming | `VACUUM` with and without an unconfirmed slot | Trimmed when nothing is subscribed; retained while a slot has not confirmed |
| Binary `pgoutput` | Slot streamed with `(binary 'true')` | Values arrive as `b` frames — an `int8` in network byte order, not its decimal spelling |
| Stale slot invalidation | 110,000 writes past an unconfirmed slot, then `VACUUM` | Slot invalidated, log trimmed to zero, a healthy slot still streams |
| Trimming at scale | `VACUUM` over a 110,000-row change log | 2.5s; deleting per row instead took longer than the request timeout |
| No write amplification without a subscriber | Five writes with no slot, then five with one | Log absent in the first case, five rows in the second |
| Batched log writes | A 1000-row `INSERT` with a slot open | One log row, not 1000; all 1000 changes still replay, from the window and from disk after a restart |
| Selective log read | 20,000 changes in 100 batches, resume near the end | 33 changes returned, not 20,000 |
| Query cancellation | `CancelRequest` on a second connection with the session's `BackendKeyData` | Next message refused with "canceling statement due to user request"; session stays usable; a wrong secret is ignored |
| Fast-path function call | `FunctionCall` on a raw connection, with the OID read from `pg_proc` | `fp(6)` → 42 and `greetfp('world')` → `hi world` over the `F`/`V` messages; an OID nobody published is refused with `42883` and the session stays usable |
| Mid-statement cancellation | `CancelRequest` 0.3s into a 200,000-row scan | The running statement stops: 4.04s → 1.56s, no rows, "canceling statement due to user request"; an uncancelled run and a wrong-secret run both return all 200,000 rows |
| Concurrency under load | `SELECT 1` on a second connection, and an HTTP health check, during a 200,000-row scan | Health check 0.56s (was 2.66s — see below) |

The multi-protocol checks matter because all four protocols share the storage
backend that was replaced; verifying only PostgreSQL would have left the others
unmeasured after the change.

#### Defects found by measuring, not by building

A green build and a passing harness said nothing about any of these. Each was
found by running the server and timing it, and each is recorded with the number
that exposed it.

| Defect | Symptom | Cause | After |
|--------|---------|-------|-------|
| Autovacuum scanned everything, always | 200,000 rows left the server at 101% CPU and 1.3 GB RSS, unresponsive | Each 60s tick materialised every row of every table just to discover whether anything was reclaimable — the tick ran far more often than the thing it looked for changed | A counter of rows marked deleted answers the same question without a scan: 0.0% idle CPU, 131 MB RSS |
| `LIMIT` applied after filtering everything | `... WHERE note LIKE '%x%' LIMIT 1` over 200,000 rows took 190.8s | The limit was applied at the end of the pipeline, so every row was filtered before all but one was discarded | The filter stops once the limit is reached — 2.70s. Guarded: `ORDER BY`, `GROUP BY`, `DISTINCT`, aggregates and window functions all still scan in full, since they need the rows the limit would skip |
| `LIKE` compiled its pattern per row | 20,000 rows: 18.97s for `LIKE`, 0.26s for `=` on the same column — ~0.94ms per row, all in `Regex::new` | The pattern is identical for every row of a scan, but was recompiled for each one | A bounded per-thread cache of compiled patterns: 0.35s. The 200,000-row scan went 190.4s → 3.90s |
| Every session shared one backend id | — | `process_id` was `std::process::id()`, the same value for every connection | A per-session counter. Two things read that id and both were wrong: the cancel registry is keyed by it, so only the newest connection could ever be cancelled, and `NOTIFY` reports it so a listener can tell its own notifications apart — with one shared id every notification looked self-sent |
| `INSERT ... VALUES` stored expressions as text | `INSERT INTO t (id) VALUES (500 + 1)` put the string `500 + 1` into an `INTEGER` column, while `SELECT 500 + 1` correctly gave 501 | Anything that was not a quoted string, `NULL`, a boolean or a number fell through to being stored verbatim | The value is evaluated when it is not a plain literal. This was found while testing PL/pgSQL and had nothing to do with it — it stayed invisible because the row *was* written, it just never matched a comparison afterwards |
| A function's catalog key drifted between writer and reader | Every stored function stopped resolving: `SELECT addone(41)` answered `Function 'ADDONE' not implemented` | Adding the argument count to the key changed the *lookup* but the edit to the *store* silently did not apply, so one wrote `function:f` and the other read `function:f/1` | Both sides changed together. The two are eight lines apart in one file and still drifted — the edit that missed reported nothing, and only a live call caught it |
| A function's return type was read as its body | A fast-path call asking for a binary result got text: the return type resolved to nothing recognisable, so `text` was written | An edit swapping two fields of a tuple was written without an assertion, and `make format` had reflowed the code so it silently matched nothing | Both fields swapped, with the assertion that would have caught it. This happened five times this session; every replacement that carried an assertion failed loudly and was fixed at once, and every one that did not cost a debugging cycle |
| Every overload of a name collided inside a query | With `f(int4)`, `f(text)` and `f(int8)` all defined, `SELECT f(a_text_column) FROM t` answered `int8` — whichever was created last | The registry that makes a function callable from an expression was keyed by name and argument *count*, so each definition overwrote the previous one | Candidates are kept per signature and resolved by the arguments' own types. This was a defect in the previous round's work, found by asking what happened when the feature met the overloading built two rounds before it |
| `= ANY(...)` was parsed as a call to a function named `ANY` | `WHERE id = ANY(ARRAY[1,3])` matched no rows; `SELECT 1 = ANY(ARRAY[1,2])` failed with `Function 'ANY' not implemented` | The parser handles a quantified comparison after `<`, `<=`, `>`, `>=` but the equality level had no such branch — so the two forms people actually write, `= ANY` and `<> ANY`, were the ones that did not work | The same branch at the equality level. The evaluator already understood `Expression::Any`; only the parser never produced one |
| A comparison's right-hand side was stored as text | `WHERE id = ANY(ARRAY[1,3])` was stored as `id = 'ANY(ARRAY[1,3])'` and matched nothing; `WHERE id = 1 + 1` matched nothing | The earlier fix checked the column and the operator but not the value, so an expression on the right fell through the same crack from the other side | The value must be a literal — or a parenthesised list for `IN`, or `NULL` for `IS` — and anything else routes to the pipeline |
| `SHOW` disagreed with the startup handshake | `SHOW server_version` returned an empty string while `ParameterStatus` carried a version. A driver reads that value to decide what the server supports | The two came from different places: the handshake sent a literal, and `SHOW` read a session map nothing had populated | One list feeds both. `SHOW ALL` now reports the advertised settings alongside the session's own |
| An expression in `WHERE` was dropped entirely | `SELECT name FROM t WHERE id * 2 = 4` returned **every row**; `WHERE UPPER(name) = 'ADA'` returned none. Adding ` AND id > 0` made both correct, because the conjunction routed the statement to the full pipeline | The simple parser read the first whitespace-separated word as the column and the second as the operator, so `id * 2 = 4` became column `id`, operator `*` — and the storage matcher treats an operator it does not know as matching every row | The parser refuses a conjunct it cannot represent as `column op value`, which sends the statement to the path that evaluates expressions. Found while checking why a stored function did not work in a `WHERE`; the arithmetic case is the worse one, and nothing was looking for it |
| A catalogue query dropped its `WHERE` | `SELECT ... FROM pg_class WHERE relname = $1` returned the whole catalogue, so a driver read the first entry as its answer | The clause was parsed and then not passed to the catalogue path, which only projected columns | A catalogue query carrying a clause goes through the path that can evaluate one. Found by writing `WHERE proname = ...` against the new `pg_proc` and getting every row back |
| One query stalled the whole server | During a 200,000-row scan, `SELECT 1` on a second connection took 3.3s and an HTTP health check 2.7s; the server never even read the second connection's message until the scan finished | The scan ran as one long CPU-bound stretch that never yielded, so the runtime could not service anything else — 100% of one core with nine idle | The row-conversion loop yields every 512 rows: health check 0.56s. This is also what made cancellation work at all — a `CancelRequest` could not be *received* in time before |

The cancellation case is worth stating plainly, because the first three attempts
to verify it all failed for reasons that were not the server's: a Python timer
thread starved by the GIL, then interpreter startup that took 3.5s under load,
then a `wait()` on a child that was sleeping 99s. Only after the harness was
made deterministic — the child announces readiness before the query is sent, and
the parent signals the exact moment the query goes out — did the measurement
mean anything. Two of those runs would have been reported as "cancellation does
not work" and one as a hang.

**Left alone deliberately:** the remaining 0.56s stall is in the synchronous
part of the select pipeline (`run_select_values`), which cannot `await`. Making
it async ripples through `execution_strategy.rs`; `block_in_place` is not an
option because it panics on a current-thread runtime, which is what
`#[tokio::test]` gives. The dominant term is fixed and the rest is recorded
here rather than rushed.

**Also noted, not fixed:** `server.worker_threads` in `config.rs` is read by
nothing — `#[tokio::main]` takes no arguments, so the runtime always uses the
default worker count. It is a knob that cannot change any output.

#### Scale

The conformance harness works on three-row tables, so nothing in it could reach
a page boundary. `SELECT COUNT(*)` over a 12,000-row table is now a check
(`a table larger than one scan page is counted in full`), and the guard-rail
behaviour has unit tests in `orbit/engine/src/unified/storage.rs`: a scan under
the limit returns every row, a scan over it errors, and an explicit limit is
never overridden.

#### Durability

Rows and schemas are held by `RocksDbBackend`
(`orbit/engine/src/unified/rocksdb_backend.rs`), which implements
`UnifiedStorageBackend` over a RocksDB database under
`<unified_storage.data_dir>/unified`. `UnifiedTableStorage` writes each table's
definition to a reserved `__orbit_table_schemas` relation keyed by
`dialect:name` and keeps its in-memory map strictly as a read-through cache.

This was found by restarting the server and querying a table written before the
stop — not by the conformance harness, which connects to an already-running
server and so cannot see the difference between a durable store and a map. The
regression test for it is
`protocols::common::storage::unified::tests::a_table_survives_a_restart`, which
opens the store twice over one directory; it fails if the backend is switched
back to memory.

Note that `unified_storage.data_dir` in `config/orbit-server.toml` is a
separate setting from the `--data-dir` flag, which the unified store does not
read.

##### An acknowledged write reaches the disk

`set_sync` appeared nowhere in the repository, so every RocksDB write used the
default `WriteOptions`, where `sync` is false. A `put` returned once the log
record was in the operating system's page cache. That distinction is invisible
to a `SIGKILL` test — the kernel still holds the buffer, so the process-crash
check passed and proved only process-crash safety. A power cut or kernel panic
lost every acknowledged write since the last flush.

`RocksDbBackend` now builds one `WriteOptions` at open and uses it on every
`put`, `delete`, and batch, so durability is a property of the store rather
than of which call site made the write. Shutdown flushes the log before the
memtable, so a stop interrupted between the two still has every write
recoverable.

The trade is real and belongs to the operator: `sync_wal = false` is roughly an
order of magnitude faster and loses recent writes on power loss.
`RocksDbBackendConfig::unsafe_fast()` names that choice for tests.

##### Corruption is detected, not served

Verified by writing 5,000 rows, flushing them to SST files, overwriting 512
bytes in the middle of each, and reopening: the scan fails with a checksum
error rather than returning damaged rows
(`orbit/engine/tests/durability.rs::corrupted_data_on_disk_is_detected_rather_than_served`).
RocksDB's per-block CRC32c already did this; the test pins it so a future
options change cannot silently turn it off. `paranoid_checks` and
`wal_recovery_mode` are now set explicitly rather than inherited — point-in-time
recovery keeps every completed write and discards only a torn tail, which is
the record that was being written when the power went out and that no client
was told had succeeded.

##### The warm-tier configuration was decorative

Every knob under `[unified_storage.warm_tier]` — `sync_wal`, `block_cache_mb`,
`write_buffer_mb`, `max_write_buffers`, `enable_bloom_filters`,
`bloom_bits_per_key`, `max_disk_gb` — was parsed into `WarmTierConfig` and read
by nothing; `grep` found zero read sites. `RocksDbBackend::open` took a path and
no options. An operator who set `sync_wal = true` to get durable writes got no
fsync and no warning, which is the failure mode a configuration file is
supposed to prevent.

`compression_algorithm = "lz4"` could not have worked either: the `rocksdb`
dependency was built with `default-features = false`, so no codec was linked
in. `lz4` and `zstd` are now enabled in both `orbit-engine` and `orbit-server`
— they must match, because cargo unifies them into one build of
`librocksdb-sys` — and a test opens a database with each codec in turn, so an
unlinked codec fails the build rather than the server's start-up.
`max_disk_gb` remains unread and is called out here rather than left to imply a
quota that nothing enforces.

Two contradictions are now refused at start-up instead of per operation:
`sync_wal` with `enable_wal = false` (RocksDB rejects each such write
individually, so the server would start clean and then fail everything), and an
unknown `compression_algorithm`.

##### Verified against a running server

`tests/integration/pg_crash_durability.rs` owns the server process rather than
connecting to one: it writes 25 rows over the PostgreSQL wire, sends `SIGKILL`
so no shutdown hook or destructor runs, restarts over the same directory, and
checks the rows are all present, in order, undamaged — and that the `PRIMARY
KEY` still rejects a duplicate, which proves the constraints persisted and not
merely the column names. It derives its configuration from the shipped
`config/orbit-server.toml` so it cannot drift from what operators run, and it
asserts `sync_wal` is on, so the test stops claiming durability if that default
is ever turned back off.

```bash
cargo test -p orbit-integration-tests --test pg_crash_durability -- --ignored --nocapture
```

##### Ports in the configuration file are ignored

Not fixed, recorded because it misleads: `apply_cli_overrides` in
`orbit/server/src/main.rs` assigns `args.postgres_port` (and the redis, mysql,
cql, grpc and metrics ports, `bind_address`, and `data_dir`) over the parsed
configuration unconditionally. Clap supplies its default whether or not the
flag was passed, so a port set in `config/orbit-server.toml` can never take
effect. The crash-durability test passes ports on the command line for this
reason.

#### SQLSTATE

Every error left as `XX000` — `internal_error`, the code PostgreSQL uses for
"something went wrong that we cannot name". Drivers branch on this: an
application could not tell a duplicate key from a crashed backend, so no
retry-on-conflict loop and no ORM's "is this a unique violation?" test could
work. The message also carried `PostgreSQL protocol error: ` — our plumbing
showing through into text meant for the user.

`orbit/server/src/protocols/postgres_wire/sqlstate.rs` now classifies errors,
and each of these is triggered end-to-end by a conformance check that asserts
the code a real client receives:

| Condition | Code |
|-----------|------|
| `undefined_table` | `42P01` |
| `undefined_column` | `42703` |
| `undefined_function` | `42883` |
| `ambiguous_function` | `42725` |
| `duplicate_table` | `42P07` |
| `unique_violation` | `23505` |
| `not_null_violation` | `23502` |
| `foreign_key_violation` | `23503` |
| `check_violation` | `23514` |
| `division_by_zero` | `22012` |
| `serialization_failure` | `40001` |
| `query_canceled` | `57014` |
| `raise_exception` | `P0001` |

The right shape is a code at every raise site. There are several hundred, and a
half-converted error type would be worse than none — some codes honest, others
silently still `XX000`, with no way to tell which from outside. So the mapping
is in one place, keyed on the message text the engine produces, with one
exception: `ProtocolError::SqlState` carries a code explicitly, for the case
where the message cannot say. A `RAISE EXCEPTION` is `P0001` whatever text it
carries, and no amount of reading that text would reveal it.

Classifying text is a contract between the raise sites and that table, and such
contracts drift. The guard is the conformance checks above: a reworded message
shows up as a failing check rather than as a silent return to `XX000`. An error
nobody has categorised still reports `XX000`, which is what it is — returning a
plausible-looking code for an unclassified error would be worse than admitting
it.

#### Found by probing

Widening the harness into areas it had never covered found six defects. Each is
written down with the statement that showed it, so they are gaps with evidence
rather than a feeling that something is missing.

A `NUMERIC` column renders at its **declared scale** on every read path —
clause-free, simple `WHERE`, and `ORDER BY` go through different code, and a
check walks all three. A column without a declared scale does not gain one.
That fix had been written once before and **removed as dead code**, correctly
at the time: it was inert because the column's type was still `Text`, the
mapping bug not yet found. Re-applied afterwards, it works. The removal was
still right — code that changes no output should not sit in the tree looking
like a feature — but it is worth recording that "this patch does nothing" can
mean "something upstream is broken" rather than "this patch is wrong".

Two of the three were the same defect: `NUMERIC`, `DECIMAL`, `JSON`, `JSONB`,
`INTERVAL`, `BYTEA`, `UUID` and `CHAR` were reachable as **column** types but
missing from the **cast-target** list, so a type you could declare was not a
type you could cast to. The parser now accepts them with an optional
`(precision, scale)`, and the conversions exist: a declared scale is rendered
in full (`1.5::NUMERIC(10,2)` is `1.50`, not `1.5`) because rounding alone
leaves the value at its original scale. A malformed value is still refused —
adding the cast must not make everything castable, and there is a check for
that.

Date arithmetic followed: only the *timestamp* forms existed, so
`DATE '2024-01-01' + INTERVAL '1 day'` — the way anyone writes it — failed.
`date + interval` (a timestamp, as in PostgreSQL), `date ± integer` (a date),
`date - date` (a count of days) and `interval ± interval` all work now.

Chasing the `NUMERIC(10,2)` scale found something larger: **`ColumnType` had no
numeric variant at all**, so `NUMERIC(10,2)` matched none of the declared type
names and fell through to the unknown case, which is `TEXT`. The declared scale
existed nowhere, and a type that exists specifically to avoid binary floating
point was not being stored as one. `ColumnType::Numeric { precision, scale }`
now exists, the DDL parser produces it, `pg_type` reports `NUMERIC`, and a
stored value is read back as an exact decimal at its declared scale.

And chasing *that* found the worst of the round, which had nothing to do with
numerics: the storage matcher compared only `BigInt` against `BigInt`, so
`WHERE amount > 5` on a numeric, float or text column **matched no rows at
all**. Not an error — an empty result. `WHERE id > 1` on an integer column
worked, which is why it had never been noticed. Comparison is now numeric
across the integer, float and decimal types, ordered for text, dates,
timestamps and booleans, and `None` — no match — only for values that genuinely
cannot be compared.

Following the `NUMERIC(10,2)` rendering to its cause found something larger
than rendering. `SqlType` → `ColumnType` mapping in
`orbit/server/src/protocols/common/storage/unified.rs` had no arm for
`Numeric`, `Decimal`, `Real` or `DoublePrecision`, so all of them fell to
`_ => ColumnType::Text`: **every numeric and floating-point column was stored
as text**, and the declared scale existed nowhere. With the mapping added,
`SELECT amt` renders `10.50`, `SUM` is exact, comparisons work, and arithmetic
on the column keeps the type. Two things had to follow it — arithmetic and
`SUM` over an exact decimal, neither of which had an arm — because values that
had been floats were now decimals.

A first attempt to fix the rendering by patching `execute_persistent_select`
was **removed**: no query reached it, so it changed no output, and code that
changes no output is the decorative kind this document argues against
elsewhere.

#### An UPDATE with an expression — fixed, after two wrong attempts

`UPDATE t SET n = n + 1` changed nothing while `RETURNING` reported the new
value: a client was told a write had happened that had not. Both halves are
fixed, and both wrong attempts are recorded because each failed for a reason
worth knowing.

**Where the fix belongs.** A `SET` value was stripped of its quotes by
`parse_single_set_clause`, which threw away the only thing distinguishing a
text literal from an expression — `SET t = 'n + 1'` and `SET n = n + 1`
arrived identical. The first attempt tried to tell them apart downstream by
guessing (a bare word is a literal unless it names a column or carries an
operator) and that is wrong: `SET note = 'a + b'` carries an operator. The
quotes are now kept and `literal_to_json` unquotes them, exactly as the
`INSERT ... VALUES` path already did. That also fixed a corruption nobody had
noticed: `trim_matches` turned `'it''s'` into `it''s`, storing the doubled
quote.

**Ordering.** The second attempt computed each expression *after* the old rows
were marked deleted, so an expression that failed to evaluate left the row
marked and no new version written — the update did not merely fail, it
destroyed the row. Every replacement row is now built before the first mark, so
a failure returns an error having changed nothing. There is a check for that.

**And a regression this document has to own.** Adding `ColumnType::Numeric`
made stored values `Decimal`, and two `SqlValue`→JSON converters had no arm for
it, so a decimal was written as the *string* `"10.00"`. Because an update
identifies its row by **every** column's value, one column converting wrongly
matched no row at all: a table that merely *contained* a `NUMERIC` column
silently dropped updates to its other columns. That shipped in
`804e0a16` and was found by testing the update path against a table shaped like
a real one rather than the two-integer table the first test used.

The lesson is about the number rather than the three: the check count had been
presented as covering the remaining work, and one afternoon of probing
untested constructs found six things wrong. Two of them —
`WHERE id = ANY(...)` and `WHERE id = 1 + 1` — returned wrong rows rather than
errors, which is the class this document keeps recording and the class no
passing suite reveals until someone writes the check.

#### Parameters of unspecified type

A driver may leave a parameter's type to the server — that is what OID `0`
means, and it is what most drivers send. Every such parameter was filled in as
**text**, which broke the extended query protocol in six ways at once. The
conformance harness had not caught any of them because it used simple queries
almost throughout; these arrive through `Parse`/`Bind`/`Execute`.

| Statement | What happened |
|-----------|---------------|
| `WHERE id = $1` | matched no rows — an integer column compared against `'2'` |
| `WHERE id = $1 AND name = $2` | failed outright: `Cannot compare Integer(1) and Text("1")` |
| `WHERE amt = $1` on `NUMERIC` | matched no rows |
| `WHERE flag = $1` on `BOOLEAN` | matched no rows |
| `LIMIT $1` | the clause was ignored and every row came back |
| `UPDATE ... WHERE id = $2` | reported success and changed nothing |

The engine could already work out a parameter's type from the column it is
compared against — `describe_parameters` does exactly that — but it was only
consulted to *answer* a `Describe`, never to decide how to bind. Parse now asks
it for anything the client left unspecified, falling back to text only when
inference finds nothing. Two gaps in the inference itself went with it:
`NUMERIC` was missing from the types written unquoted, and a placeholder in
`LIMIT`/`OFFSET` is compared against no column at all, so nothing typed it.

The shape of this one is worth keeping: the server *knew* the right answer and
told clients so when asked, while using a different answer internally. Nothing
about the code looked wrong, and the check that would have caught it is the one
nobody had written.

Two more from the same probe, both the same shape — a capability implemented
but reachable only by a route the client is not obliged to take:

- **Binary result format was ignored.** A client asks for it in `Bind`. The
  encoder existed and worked, but the column types it needs were recorded only
  by `Describe`, which the protocol does not require. Without one, every value
  fell back to text and a client that asked for binary silently got characters.
  The types are now described on demand, and only when binary was actually
  asked for, so a text query pays nothing.
- **An empty statement was rejected.** The extended path answered a parse
  error where PostgreSQL answers `EmptyQueryResponse` — which is how a client
  tells "nothing to run" from "your statement was refused". The simple-query
  path had always answered it correctly; only the extended one had not.

What the same probe found already correct, now with checks: portal suspension
(a row-limited `Execute` replies `PortalSuspended` and the next `Execute`
continues rather than restarting), `Describe` of a statement returning both a
`ParameterDescription` and a `RowDescription` with the right type OIDs,
re-binding one statement with different parameters, and using a closed
statement failing rather than silently succeeding.

#### Transaction state

Probing the transaction state machine — the part a driver relies on to know
what it may send next — found two defects, and confirmed the rest correct.

- **A statement in a failed block reported `XX000`.** PostgreSQL reports
  `25P02` (`in_failed_sql_transaction`), which is how a driver knows it must
  roll back rather than retry; as `XX000` it was indistinguishable from the
  backend falling over. The refusal itself was already right, and
  `ReadyForQuery` already reported `E` — only the code was wrong.
- **A block sent as one message left a transaction open.** The session's state
  was read from the first word of the whole message, so
  `BEGIN; INSERT ...; COMMIT` was seen as a `BEGIN` alone and the trailing
  `COMMIT` went unnoticed. The connection was left holding a transaction the
  client had already ended — every later statement silently joined it, and a
  disconnect would have discarded them. Each statement in a message is now
  noted in turn.

Correct already, and now checked: `ReadyForQuery` reporting `I`/`T`/`E` as the
session moves; `COMMIT` of a failed block rolling back rather than committing;
`SAVEPOINT` and `ROLLBACK TO SAVEPOINT`; and two statements in one message
returning two results with their own command tags.

#### COPY, notification and type formatting

Probing the three surfaces the harness had barely touched found one defect and
confirmed a good deal already right.

**`WITH CSV` was parsed by nothing.** `COPY ... TO STDOUT WITH CSV` wrote
tab-separated text and `COPY ... FROM STDIN WITH CSV` read a CSV line as one
field, failing with a column-count mismatch. Both directions now handle CSV
properly: a field is quoted only when it contains a comma, a quote or a line
break; a quote inside a quoted field is doubled; and an empty unquoted field is
NULL, which is how CSV spells it — the text format's `\N` means nothing here.
There is a round-trip check covering exactly those three cases, because they
are what separate CSV from splitting on commas.

Correct already: `COPY FROM STDIN` and `COPY TO STDOUT` in the text format,
including backslash escapes and `\N`; a client-initiated `CopyFail` aborting
the load and leaving the table unchanged; `LISTEN` and `NOTIFY` accepted with a
payload.

**Type formatting was correct throughout** — booleans as `t`/`f`, timestamps in
ISO form, NULL sorting last by default and first under `NULLS FIRST`, NULL
rendered as a real NULL rather than the text "NULL", an empty result set
carrying its row description, and `COUNT(n)` counting non-nulls where
`COUNT(*)` counts rows. Nothing to fix; worth recording that it was checked
rather than assumed.

#### Sequences, conflicts, identifiers, text and subqueries

A probe across five more surfaces found one defect and confirmed the rest.

**A scalar subquery worked in `WHERE` but not in the select list.** Subqueries
were resolved for the predicate and for `HAVING`, so the very same subquery
that filtered correctly failed as unimplemented one clause to the left:
`SELECT (SELECT COUNT(*) FROM t)` reached the evaluator with the subquery still
in it. The select list is now resolved too, and an empty subquery yields NULL
rather than an error, as SQL requires.

Correct already, and now partly checked: `SERIAL` producing distinct non-null
keys; `ON CONFLICT ... DO NOTHING` leaving the existing row and
`ON CONFLICT ... DO UPDATE` replacing it; a quoted mixed-case identifier
keeping its case, with the unquoted spelling correctly *not* finding it;
UTF-8 round-tripping including accents, CJK and emoji, with `LENGTH` counting
characters rather than bytes and `LIKE` matching across multibyte text; and
`IN (subquery)`, `NOT IN (subquery)` and correlated `EXISTS`.

#### DDL evolution, views, indexes and joins

**`ALTER TABLE ... ADD COLUMN` reported success and did nothing.** No branch
handled it, so it fell through to a generic "Command completed successfully"
and the column was simply not there. Every later reference then failed with
`column does not exist`, pointing at the query rather than at the DDL that
never happened — the same silent acceptance this document records for `DO`
blocks, `CREATE FUNCTION` and `DROP TYPE`. It now adds the column, fills the
rows that already exist when a `DEFAULT` is given (without which the same table
answers two ways depending on when a row arrived), makes `COLUMN` optional as
PostgreSQL does, and refuses a duplicate with `42701`.

Extracting that meant the declared-type table now has **one** copy, shared by
`CREATE TABLE` and `ADD COLUMN`. Two copies would have drifted, which is the
failure mode recorded here more than any other.

Correct already, and checked: views (`CREATE VIEW`, selecting from one with and
without a clause, aggregating over one, `DROP VIEW`); `CREATE INDEX` and
`DROP INDEX`; `ALTER TABLE ... RENAME TO`; `INNER`, `LEFT` and `CROSS JOIN`,
and `JOIN ... USING`; multi-row `RETURNING` on `INSERT`, `UPDATE` and `DELETE`.

**`CREATE UNIQUE INDEX` did not enforce uniqueness.** Nothing handled the
statement, so it reported success and duplicates went in silently — an
integrity constraint the caller asked for by name. Uniqueness is recorded on
the column, which is where it is already checked, so the index now works;
`DROP INDEX` takes the constraint away again; creating one over rows that
already violate it is refused with `23505` rather than claiming something about
the table that is not true; and a multi-column unique index is refused with
`0A000`, because a schema records uniqueness per column and there is nowhere
for one to live. A plain, non-unique index is still accepted without being
built — it changes no answer, only speed.

**The outer joins kept only unmatched *left* rows.** `RIGHT JOIN` therefore
behaved as an inner join and `FULL OUTER JOIN` lost both unmatched sides —
counting 2 where 4 were right. Worse, an unmatched row was pushed *without* the
other side's columns rather than with them set to NULL, so
`SELECT val FROM a LEFT JOIN b ...` failed with `column "val" does not exist`
instead of returning NULL. Unmatched rows on both sides are now kept and padded.

**A finding this document got wrong.** `ALTER TABLE ... DROP COLUMN` and
`RENAME COLUMN` were recorded here as failing on a column that exists. They do
not. The probe that "found" them dropped and renamed a column it had added a
moment earlier with `ADD COLUMN` — which was silently doing nothing, so the
column was never there and both statements were right to refuse. Two working
features were written down as broken because the failure upstream was silent.
Both are now checked directly, on columns declared in `CREATE TABLE`.

#### Pipeline error recovery

**A failed statement did not stop the rest of its pipeline.** The protocol
requires everything between an error and the client's next `Sync` to be
discarded; instead the queued statements ran, so a client pipelining writes had
later ones applied when it expected them skipped. Verified against the raw
protocol: three statements sent before one `Sync` with the middle one failing
now yield the first statement's rows, the error, and then only
`ReadyForQuery` — where before, the third statement's `ParseComplete`,
`BindComplete`, `DataRow` and `CommandComplete` all followed the error.

**Two mistakes on the way, both of which hung the harness**, and both worth
recording because a hang is the least informative failure there is:

- The first attempt discarded `CopyData` and `CopyDone` too. Those are what end
  a copy stream, so both sides waited for each other for ever.
- The second scoped the skipping to *all* messages rather than the extended
  protocol's. A simple query synchronises with its own `ReadyForQuery` and
  never sends `Sync`, so after any failing simple statement the connection
  discarded everything the client sent next — including the queries that would
  have cleared the state. Nothing recovered it.

Both were found by logging the last statement the server saw before the silence
rather than by reading the code again: the hang pointed at
`INSERT INTO conf_notnull (id) VALUES (NULL)`, a statement whose *failure* was
the trigger, which named the mechanism immediately.

The state is now entered only by `Parse`/`Bind`/`Execute`/`Describe`/`Close`,
cleared by `Sync`, and never applied while a copy is open.

Also checked and already correct: a prepared statement re-plans after the table
under it changes — after `ADD COLUMN` it returns the new column, after
`DROP COLUMN` it does not, with no stale result and no error.

#### Set operations, aggregates, windows and functions

A probe across this surface found two defects and confirmed a great deal.

- **`strpos` had its arguments reversed.** `position(sub IN str)` and
  `strpos(str, sub)` are the same function with opposite argument orders, and
  both were routed to one implementation — so `strpos('abc', 'b')` searched
  "abc" inside "b" and answered `0`. A wrong answer, not an error.
- **`AVG` over exact inputs went through a float.** The mean of 2, 3 and 5 came
  back as `3.3333333333333335`, whose last digit is a rounding artifact of
  binary floating point. PostgreSQL averages integers as `numeric`; so does
  this now, when every input is exact.

Correct already, and checked: `UNION`, `UNION ALL`, `INTERSECT`, `EXCEPT`;
`GROUP BY` with `COUNT`/`SUM`/`MIN`/`MAX`, `HAVING` both including and
excluding, `COUNT(DISTINCT ...)`; `ROW_NUMBER`, `RANK` with ties, `SUM OVER ()`,
`PARTITION BY`, `LAG` with its leading NULL; `UPPER`/`LOWER`, `TRIM`,
`REPLACE`, `||`, `ABS`, `ROUND`, `MOD`, `CEIL`, `FLOOR`, `GREATEST`, `LEAST`,
`NOW`, `CURRENT_DATE` and `STRING_AGG`.

**`POSITION(sub IN str)`** — the standard spelling — now parses. The first
attempt added `IN` to the argument-separator list and changed nothing, because
by the time that list is consulted the comparison rules have already taken
`sub IN str` and built an `IN` expression; that attempt was removed rather than
left in looking like a feature. The needle is now parsed one level below the
comparison rules, where `IN` is not an operator, and only for `POSITION` —
`IN` keeps its meaning everywhere else, which is checked both as a list
operator and as `NOT IN`. Character positions, not byte offsets:
`POSITION('語' IN '日本語')` is 3.

#### Observed once, unexplained

A single run of the conformance harness failed one check with
`connect: authentication error: invalid nonce`; eight consecutive runs before
and after were clean, and it has not reproduced. The SCRAM nonce alphabet was
checked and is correct — `0x21..=0x7E` with the comma removed, 93 code points,
and the shift that skips the comma cannot reach `DEL`. The handshake state is
per-session, so there is no shared nonce to race on. That leaves it unexplained
rather than fixed, and it is written down here because an intermittent
authentication failure is not something to leave in a passing run's shadow.

#### Not yet implemented

- **Isolation is by write stamping with per-transaction row versions.** Every
  row carries the id of the transaction that wrote it, and a delete marks the
  row with the id that removed it. `READ COMMITTED` judges those ids against
  what is open now; `REPEATABLE READ` and `SERIALIZABLE` judge them against the
  set of transactions open when the block began, which is what makes a repeated
  read return the same rows. An `UPDATE` inside a transaction writes a **new
  row version** rather than overwriting: the previous row is marked deleted by
  that block and the new values are stored under a key carrying the writing
  transaction's id, so a reader holding an older snapshot still reads the row
  as it stood. The storage key had to change for this — keying by the primary
  key alone made the newer version replace the older, leaving nowhere to keep
  it. An update outside a transaction still writes in place, since no reader
  can observe the difference. `SERIALIZABLE` adds a check at commit: a block
  that read a table another transaction has since written cannot be placed in
  any serial order after it, so it fails with SQLSTATE `40001` rather than
  committing. The grain is the **row** for a table with a key: the
  block records the rows it read and only a write to one of those conflicts.
  A table with no unique column falls back to whole-table grain, because a
  keyless row cannot be named across a change — its identity would be its
  contents, and an update changes those. The predicate a block read is recorded
  alongside the rows, so a row that *starts* satisfying it — a phantom — is a
  conflict too, while a row outside it is not. `=`, `!=`, `<`, `<=`, `>`, `>=`, `LIKE`,
  `ILIKE` and `IN` are understood; an operator this does not know is treated
  as matching, which widens the watch rather than narrowing it. Every
  approximation refuses more than a real serializable scheduler would, never
  fewer.
- Undo is row-scoped, so a rollback no longer damages another session's rows,
  and concurrent writers were measured landing every row (8 workers x 100
  inserts, all 800 present).
- **Old versions and deleted rows are reclaimed only once no other block is
  open**, so a snapshot reader cannot lose a row mid-transaction. Until then
  the mark hides them, which means a long-running block delays reclamation —
  versions accumulate while one is open. `VACUUM [table]` reclaims them
  explicitly, and a background worker runs the same reclaim every 60 seconds.
  A version is reclaimed once the block that removed it has finished *and*
  finished before the oldest block now running began — so a long-lived
  transaction holds back only the versions it could still see, not all of them.
- **An `UPDATE` is versioned even outside a transaction.** Overwriting in place
  left no trace of the write, so a snapshot reader saw the new value and a
  serializable block could not tell that what it read had moved.
- Every write path undoes per row: `TRUNCATE` records each row it removes as a
  pre-image, `INSERT ... SELECT` predicts the rows it will add by running its
  select, and `COPY` records each copied line.
- **`CancelRequest` interrupts the statement running**, not only the one after
  it. The session's flag is checked every 512 rows in the storage fetch and in
  the filter, so a scan stops part-way: a cancel 0.3s into a 200,000-row scan
  ended it at 1.56s against 4.04s uncancelled, with no rows and
  "canceling statement due to user request". A wrong secret is ignored. What
  remains unchecked is the phase that formats and sends the result, so a cancel
  arriving after the last row is read is honoured only when the next statement
  starts.
- **GSSAPI encryption** is declined the way the protocol defines: a
  `GSSENCRequest` is answered with a single `N` and the client continues in the
  clear on the same connection, which is exactly what PostgreSQL built without
  `--with-gssapi` does. There is a conformance check for both halves — the
  answer and the fact that the session survives it.
  What is absent is the *Kerberos integration*, not a wire message: validating
  a ticket needs a KDC and a keytab. Shipping a handshake that cannot check a
  token would add an authentication path whose only honest outcome is failure,
  and whose dishonest outcome is accepting anyone.
- **Replication is logical only.** A connection opened with
  `replication=database` answers `IDENTIFY_SYSTEM`, `CREATE_REPLICATION_SLOT`,
  `DROP_REPLICATION_SLOT`, `TIMELINE_HISTORY` and `START_REPLICATION`, then
  streams each write as an `XLogData` frame whose payload is the change as
  JSON — an output plugin's job in PostgreSQL. Slots are persisted in the
  catalog and survive a restart; a standby status update records the confirmed
  position on the slot and answers a requested keepalive; a stream can replay
  from a named LSN or from the slot's confirmed position.
  The payload format follows the slot's plugin: `pgoutput` emits the binary
  `Begin`/`Relation`/`Insert`/`Update`/`Delete`/`Commit` messages a real
  subscriber decodes, and anything else gets JSON. Changes are written to a
  durable log as part of the write, so a replica that reconnects after a
  restart replays from disk, and positions continue where the last run left
  off rather than restarting at one. A block's statements arrive inside one
  `Begin`/`Commit` pair rather than as several transactions. A subscriber that
  passes `binary 'true'` gets values in binary rather than as text.
- **What replication still lacks:** physical replication is refused rather than
  served, with `0A000` (`feature_not_supported`) and a session that stays
  usable — reported as `XX000` a client could not tell a feature this server
  does not have from a backend that fell over.
  The contract of `START_REPLICATION ... PHYSICAL` is "send me your WAL". There
  is no PostgreSQL WAL here: storage is RocksDB plus a logical change log, with
  no page layout, no consistent checkpoint and no LSNs that mean what a standby
  reads them to mean. Synthesising records in that format would not be an
  approximation, it would be a stream that corrupts any standby that trusts
  it. Serving this is not a protocol gap to close but PostgreSQL's storage
  engine to reimplement.
  The change log is a table, not a WAL: it is trimmed to the slowest slot's
  confirmed position by `VACUUM` and by the background worker, and a slot that
  falls more than 100,000 changes behind is invalidated so one dead subscriber
  cannot hold the log open for ever — what `max_slot_wal_keep_size` protects
  against in PostgreSQL. The bound is
  `protocols.postgresql.max_slot_change_backlog`, defaulted so an existing
  configuration file keeps working.
  `TIMELINE_HISTORY` reports that the current timeline has no history file,
  because there is only ever one.
- **A write is logged only while a slot exists**, so a server nobody replicates
  from pays nothing. With a subscriber, a statement's changes are written as a
  single batched log row rather than one row each, so a 1000-row `INSERT` costs
  one row rather than a thousand, and a replay reads only the batches after the
  position it asks for rather than the whole log. A batch is retained until
  every change in it has been confirmed. What remains is that the underlying
  storage answers a predicate by scanning its rows, so the read is proportional
  to the log's size rather than to the answer — bounded by the trim and the
  backlog setting, but not indexed.
- **The fastpath `FunctionCall` message** executes a function this server
  published in `pg_proc`. Stored functions get OIDs in PostgreSQL's user range
  (at or above 16384), derived from the catalog key rather than from position,
  so an OID survives a restart and does not shift when another function is
  created or dropped — an OID that moved would make `pg_proc` useless for the
  thing OIDs are for. An OID this server did not publish is refused by number
  with `42883`, and the session stays usable; guessing which built-in a number
  meant would have the client silently calling something else.
  The message parser had to be fixed first: it read the argument count where
  the *format code* array sits, so it misread every call a real client sends.
  Arguments are read in either format. Binary is decoded against the
  parameter's declared type from the same `pg_proc` entry the client took the
  OID from: `int2`/`int4`/`int8`/`float4`/`float8` big-endian, `bool` as one
  byte, and the string types as their bytes. A value of the wrong length is
  `22P03` naming both lengths rather than a silently wrong number, and a type
  with no binary form here (`numeric`, whose binary shape is digit groups with
  a weight and a sign; `date`/`timestamp`, whose epoch is not the Unix one) is
  refused as `0A000` rather than read approximately. The result is returned in
  the format asked for.
- **The deferred pass re-reads every row of the tables the transaction wrote**,
  not only the rows it changed. Scoped to those tables rather than the whole
  database, but still proportional to their size.
- **Triggers run SQL statements, not a procedural function.**
  `CREATE TRIGGER ... EXECUTE <body>` supports `BEFORE`/`AFTER`/`INSTEAD OF`,
  `INSERT`/`UPDATE`/`DELETE`, `FOR EACH ROW`/`STATEMENT`, `WHEN (...)`,
  `NEW`/`OLD` column references, `SET NEW.col = <expr>` on both `BEFORE
  INSERT` and `BEFORE UPDATE`, a `$$BEGIN ... END$$` body of several
  statements, and `RAISE` to reject a write. A body that declares a variable, branches or loops is
  handed to the PL/pgSQL interpreter instead (see below); a `BEFORE UPDATE`
  rewrite is still applied only when the statement changes exactly one row.
- A recursive CTE is bounded at 1,000 rounds and fails loudly rather than
  running forever if it does not settle.
- **PL/pgSQL** (`orbit/server/src/protocols/postgres_wire/plpgsql.rs`) covers
  `DECLARE` with typed variables and defaults, assignment, `IF`/`ELSIF`/`ELSE`,
  `WHILE`, `FOR v IN [REVERSE] a..b`, bare `LOOP`, `EXIT`/`CONTINUE` with an
  optional `WHEN`, `RETURN`, `RAISE` at every level, `PERFORM`,
  `SELECT ... INTO`, and any SQL statement. It runs `DO $$ ... $$`,
  `CREATE FUNCTION ... LANGUAGE plpgsql` and calls to those functions, and any
  trigger body that needs more than a list of statements. Functions are stored
  in the catalog and survive a restart.
  No expression is evaluated here: an expression is captured as tokens,
  variable references are substituted, and the result is handed to the SQL
  engine as `SELECT <expr>` — one implementation of every operator rather than
  a second one that would drift. Substitution works on tokens, so a variable's
  name inside a string literal is left alone.
  A block is atomic. It runs in a transaction, and a failure removes what it
  wrote by stamp — rows carrying the block's id are deleted and rows it marked
  deleted are unmarked — before the id is retired, so nobody can read a row
  that is about to be removed. Inside an open transaction the block joins it
  rather than starting its own, and `ROLLBACK` covers it.
  `FOR rec IN SELECT ... LOOP` iterates a query's rows, with columns read as
  `rec.column`; a single-column row is also readable under the bare name.
  Substitution understands `rec.column` as one reference, so an ordinary
  `table.column` in SQL is left untouched.
  `RETURN QUERY` accumulates rows, which makes a function set-returning: the
  first query fixes the column names and later ones append.
  `BEGIN ... EXCEPTION WHEN OTHERS THEN ... END` catches. What the protected
  statements wrote is undone before the handler runs — catching without undoing
  would leave the half-finished write a handler exists to prevent — and
  `SQLERRM` carries the raised message. A sub-transaction that commits is
  registered with the block containing it, so if *that* fails later its rows go
  too. Variable values survive a caught exception; only database writes are
  undone, as in PostgreSQL.
  A parameter's declared type decides whether its value is quoted when
  substituted. Without that a `TEXT` argument was pasted in bare and
  `greet('world')` failed with `column "world" does not exist` — a defect in
  the first version of this module, found by testing a text argument rather
  than an integer one.

  `RETURN NEXT` appends one value at a time, alongside `RETURN QUERY`.
  Cursors are declared as `c CURSOR FOR <query>`, then `OPEN`, `FETCH [NEXT
  FROM] c INTO v[, v...]`, `CLOSE`, and `FOR r IN c LOOP`; `FOUND` reports
  whether the last `FETCH` returned a row, which is what ends a fetch loop.
  `FOUND` is spelled `TRUE`/`FALSE` rather than `t`/`f` — substituted bare into
  `EXIT WHEN NOT FOUND`, a `t` is an identifier and the statement failed with
  `column "t" does not exist`.
  `table.column%TYPE` takes that column's declared type, resolved against the
  catalog when the block runs, and `table%ROWTYPE` brings the table's columns
  into scope as `name.column`. An unknown column leaves the type unknown rather
  than guessing: quoting is then read off whatever the variable is assigned.
  Functions are keyed by name **and argument count**, so `f(a)` and `f(a, b)`
  coexist; `DROP FUNCTION f` removes every arity of the name.

  A named exception condition catches its own failure and no other:
  `WHEN unique_violation` catches a duplicate key and lets a missing table
  through, and `WHEN raise_exception` catches a `RAISE`. A condition name this
  server does not define matches nothing rather than everything.

  `OUT` and `INOUT` parameters are supported: a function with them answers with
  their values, named after them, rather than with whatever `RETURN` said.
  Overloading is resolved by argument **type**, following PostgreSQL's order.
  Types are reduced to canonical names, so `INTEGER` and `INT4` are one type
  and `INT8` another; a call keeps the candidates every argument converts to
  (same category, not narrowing — `int4` reaches `int8` but not `int2`), then
  prefers the candidate matching most arguments exactly, then resolves an
  untyped literal towards the string category, then towards each category's
  preferred type (`int4`, `text`, `float8`, `timestamptz`, `bool`). An
  argument's type comes from how it was written where that says — a quoted
  literal is `unknown` and fits either overload, an integer literal too large
  for `int4` is `int8` — and from its evaluated value otherwise.
  What survives all of that and is still tied is refused as `42725`
  (`ambiguous_function`) rather than resolved by a coin toss the caller cannot
  see; nothing viable is `42883` naming the argument types, as PostgreSQL also
  phrases it. An argument that cannot be its declared type is `22P02` naming
  the value and the type.
  Parameter parsing is paren-aware, so `NUMERIC(10, 2)` is one parameter — it
  had been split on every comma, and the stored form was joined on commas too,
  so a type containing one was corrupted in both directions.

  A parameter declared as a **domain** is the type the domain is built on,
  resolved against the catalogue per call so `ALTER DOMAIN` is seen by calls
  made after it. `pg_proc` reports the base type's OID for one, because this
  server assigns OIDs to functions and not to domains and reporting `text` for
  a domain over `INTEGER` would tell a client the wrong thing about how to call
  it. An **array** (`INTEGER[]`, `TEXT ARRAY`) carries its element type —
  `_int4` and `_text` are different types, so `f(int4[])` and `f(text[])`
  coexist — and never satisfies a scalar parameter. One array does not convert
  to another: widening `int4[]` to `int8[]` would mean rebuilding every
  element, which nothing here does. `pg_proc` reports the element type's array
  OID (`1007` for `int4[]`, `1009` for `text[]`).

  A type name the lattice does not recognise **keeps its own spelling** rather
  than becoming `text`. Collapsing it made every user-defined type — a
  composite, an enum, anything — the same type as `text`, so `f(mytype)` and
  `f(text)` could not both exist and a call to one could reach the other.

  A cast **to a domain** (`42::posint`) is a cast to what the domain is built
  on. `SqlValue::cast_to` is pure and has no catalogue, so it consults a
  registry the query engine keeps
  (`orbit/server/src/protocols/postgres_wire/domains.rs`): written when a
  domain is created, and loaded once at startup for those already stored, so a
  cast works after a restart and not only in the session that created the
  domain. A name that is *not* a known domain still fails — a typo'd type must
  not silently pass the value through, and there is a check for exactly that.

  An argument that **names** a type is typed from that name rather than from
  its value: `42::BIGINT` and `CAST(42 AS BIGINT)` are `int8` even though 42
  fits an `int4`, and a call to a function whose return type this server
  recorded is typed from the catalogue — `f(g())` where `g` returns `BIGINT`
  chooses the `int8` overload though the value it returns would read as
  `int4`. A nested call is only typed this way when one candidate could have
  been meant; two overloads may return different types, and picking one there
  would be a guess dressed as a lookup.

  A stored function can be **called from inside a query** —
  `SELECT f(id) FROM t`, `WHERE f(id) = 4`, `ORDER BY f(id)` — when its body
  needs no database. The expression evaluator is synchronous, so only a *pure*
  body is callable from it: assignments, conditionals, loops and `RETURN` over
  expressions, with no SQL statement anywhere in it, checked recursively so a
  branch or a nested block cannot smuggle one past
  (`orbit/server/src/protocols/postgres_wire/stored_functions.rs`). A body that
  does run SQL is refused there by name rather than run in a way that could
  block a runtime worker; it still works as a direct `SELECT f(...)`.

  Inside a query the argument's **own type** chooses the overload: a value
  reaching the evaluator carries it, so `SqlValue::BigInt` is not
  `SqlValue::Integer` and `f(a_bigint_column)` selects the `int8` form even
  though the value prints the same as an `int4` would. **Composite types** are created with
  `CREATE TYPE name AS (field type, ...)`, stored in the catalogue, and
  reported in `pg_type` with `typtype = 'c'` and an OID in the user range. A
  PL/pgSQL variable of one brings its fields into scope as `variable.field`,
  assignable and readable; a composite is its own type when choosing between
  overloads, and `pg_proc` reports the same OID `pg_type` gives it. `DROP TYPE`
  removes one and **refuses a name that was never there** with `42704` —
  `DROP TYPE IF EXISTS` is the form that may say nothing.
  Not every `CREATE TYPE` form is a composite: `AS ENUM` and the others are
  left unhandled rather than stored as something claiming to be one.
  A variable whose name matches a column of a table the block writes is
  substituted, so `INSERT INTO t (v) VALUES (v)` with a variable `v` rewrites
  the column name too. PostgreSQL resolves this ambiguity with
  `#variable_conflict`; here the rule is simply that a variable always wins, so
  name a variable something the statement does not also use as a column.
  A loop is bounded at 10,000,000 iterations and fails loudly. PostgreSQL lets
  one run forever, which is right for a dedicated backend process; here a
  statement that never finishes holds a connection and a share of the runtime.

Note on scope: 212/212 is 212 of *these 212 checks*. Each widening found real
defects — 62 checks found none, 93 found fourteen, 130 found fourteen more, 157
found fifteen including a reachable panic — so the number tracks the harness,
not the protocol. The two largest defects were invisible to every one of those
checks: nothing was persisted, because no check restarts the server, and every
scan was truncated at 10,000 rows, because no check used a table that large.
Treat the number as a floor that moves measurably, not a compatibility claim,
and keep asking what the harness cannot see.


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

```text
┌─────────────────────────────────────────────┐
│                Hot Tier                     │
│           (In-Memory / Redis)               │
│         < 100ms access latency              │
├─────────────────────────────────────────────┤
│               Warm Tier                     │
│          (RocksDB / LSM Tree)               │
│         < 10ms access latency               │
├─────────────────────────────────────────────┤
│               Cold Tier                     │
│        (Apache Iceberg / Parquet)           │
│         < 1s access latency                 │
└─────────────────────────────────────────────┘
```

### Current Tiered Storage Implementation

The tiered storage system operates at the storage layer, providing automatic data lifecycle management based on access patterns and age thresholds.

#### Storage Tier Definitions

| Tier | Storage Type | Age Threshold | Format | Workload |
|------|--------------|---------------|--------|----------|
| **Hot** | Row-based HashMap | < 48 hours | In-memory rows | OLTP |
| **Warm** | RocksDB/LSM | 48h - 30 days | Key-value | Mixed |
| **Cold** | Columnar (Iceberg) | > 30 days | Parquet | OLAP |

#### Key Implementation Files

| Component | File | Purpose |
|-----------|------|---------|
| TieredTableStorage | `protocols/common/storage/tiered.rs` | Protocol-agnostic tiered storage |
| HybridStorageManager | `postgres_wire/sql/execution/hybrid.rs` | PostgreSQL hybrid execution |
| SimpleLocalRegistry | `resp/commands/mod.rs` | Redis local storage (single-tier) |
| StorageTier enum | `execution/hybrid.rs:14-18` | Tier type definitions |

#### Architecture Diagram

```text
┌──────────────────────────────────────────────────────────────┐
│                     Protocol Layer                           │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│   │  Redis   │  │ Postgres │  │  MySQL   │  │   CQL    │     │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘     │
│        │             │             │             │           │
│        ▼             ▼             ▼             ▼           │
│   ┌──────────────────────────────────────────────────────┐   │
│   │              TieredTableStorage                      │   │
│   │  ┌────────────────────────────────────────────────┐  │   │
│   │  │              HybridStorageManager              │  │   │
│   │  │  ┌──────-──┐  ┌──────-──┐  ┌────────────────┐  │  │   │
│   │  │  │  Hot    │  │  Warm   │  │     Cold       │  │  │   │
│   │  │  │HashMap  │  │RocksDB  │  │ Columnar/Parq. │  │  │   │
│   │  │  │(<48h)   │  │(48h-30d)│  │    (>30d)      |  │  │   │
│   │  │  └───────-─┘  └────────-┘  └────────────────┘  │  │   │
│   │  └────────────────────────────────────────────────┘  │   │
│   └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘

           ╔═════════════════════════════════════╗
           ║    Actor System (SEPARATE)          ║
           ║  ┌───────────────────────────────┐  ║
           ║  │       OrbitClient             │  ║
           ║  │  (Distributed Computing)      │  ║
           ║  │   - Virtual Actors            │  ║
           ║  │   - Message Routing           │  ║
           ║  │   - Service Discovery         │  ║
           ║  └───────────────────────────────┘  ║
           ╚═════════════════════════════════════╝
```

#### Current Limitations

1. **Single-Node Storage**: Each tier operates independently per node; no cross-node data movement
2. **Actor/Storage Gap**: Actor system handles distributed compute but doesn't participate in storage tiering
3. **No Cluster-Aware Tiering**: Cold data doesn't migrate to specialized archive nodes
4. **Manual Configuration**: Tier thresholds are static, not workload-adaptive

### Cluster-Aware Tiered Storage (Proposed Architecture)

The following enhancements would integrate the actor system with tiered storage for true cluster-aware data lifecycle management. The key innovation is **multi-granularity actors** that manage data at different structural levels, enabling fine-grained control over tier placement and seamless cross-tier queries.

#### Multi-Granularity Actor Model

Actors exist at multiple levels of the data hierarchy, each responsible for managing lifecycle and tier placement at their scope:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Actor Granularity Hierarchy                          │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      TableActor                                     │   │
│   │  - Manages entire table lifecycle                                   │   │
│   │  - Coordinates child actors (extent, column)                        │   │
│   │  - Table-level statistics and tier recommendations                  │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│         ┌──────────────────────────┼──────────────────────────┐             │
│         ▼                          ▼                          ▼             │
│   ┌─────────────┐          ┌─────────────┐          ┌─────────────┐         │
│   │ ExtentActor │          │ ColumnActor │          │ IndexActor  │         │
│   │             │          │             │          │             │         │
│   │ • Page      │          │ • Columnar  │          │ • B-tree    │         │
│   │   groups    │          │   segments  │          │   segments  │         │
│   │ • 64KB-1MB  │          │ • Analytics │          │ • Lookup    │         │
│   │   blocks    │          │   workloads │          │   paths     │         │
│   └──────┬──────┘          └──────┬──────┘          └─────────────┘         │
│          │                        │                                         │
│          ▼                        ▼                                         │
│   ┌─────────────┐          ┌─────────────┐                                  │
│   │  RowActor   │          │ FieldActor  │                                  │
│   │             │          │             │                                  │
│   │ • Single    │          │ • Field     │                                  │
│   │   record    │          │   values    │                                  │
│   │ • Point     │          │ • BLOB/CLOB │                                  │
│   │   lookups   │          │ • Large     │                                  │
│   │             │          │   objects   │                                  │
│   └─────────────┘          └─────────────┘                                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Actor Granularity Definitions

| Level | Actor | Scope | Use Case | Typical Size |
|-------|-------|-------|----------|--------------|
| **Table** | TableActor | Entire table | DDL, schema, global stats | 1 per table |
| **Extent** | ExtentActor | Page groups | Bulk I/O, range scans | 64KB - 1MB |
| **Column** | ColumnActor | Column segments | Columnar analytics, aggregations | Variable |
| **Row** | RowActor | Single record | Point lookups, OLTP updates | ~1KB avg |
| **Field** | FieldActor | Field values | Large objects, BLOBs, versioning | Variable |
| **Index** | IndexActor | Index segments | Lookup acceleration | Variable |

#### Tier-Aware Actor Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Orbit Cluster - Tiered Actor System                  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                   StorageCoordinator Actor                          │   │
│   │  - Monitors data age and access patterns across all nodes           │   │
│   │  - Coordinates tier transitions (hot→warm→cold)                     │   │
│   │  - Manages actor placement and migration                            │   │
│   │  - Enforces replication and consistency policies                    │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│         ┌──────────────────────────┼───────────────────────────┐            │
│         ▼                          ▼                           ▼            │
│   ┌───────────────┐        ┌───────────────┐        ┌───────────────┐       │
│   │   Hot Nodes   │        │  Warm Nodes   │        │  Cold Nodes   │       │
│   │               │        │               │        │               │       │
│   │ ┌───────────┐ │        │ ┌───────────┐ │        │ ┌───────────┐ │       │
│   │ │ HotTier   │ │   ──►  │ │ WarmTier  │ │   ──►  │ │ ColdTier  │ │       │
│   │ │ Actor     │ │ migrate│ │ Actor     │ │ archive│ │ Actor     │ │       │
│   │ └───────────┘ │        │ └───────────┘ │        │ └───────────┘ │       │
│   │               │        │               │        │               │       │
│   │ Child Actors: │        │ Child Actors: │        │ Child Actors: │       │
│   │ • RowActor    │        │ • ExtentActor │        │ • ColumnActor │       │
│   │ • FieldActor  │        │ • RowActor    │        │ • ExtentActor │       │
│   │               │        │               │        │               │       │
│   │ • In-memory   │        │ • RocksDB     │        │ • Iceberg     │       │
│   │ • < 48 hours  │        │ • 48h - 30d   │        │ • > 30 days   │       │
│   │ • OLTP focus  │        │ • Mixed       │        │ • OLAP focus  │       │
│   │ • SSD/NVMe    │        │ • SSD         │        │ • HDD/Object  │       │
│   └───────────────┘        └───────────────┘        └───────────────┘       │
│         ▲                          ▲                           ▲            │
│         └──────────────────────────┼───────────────────────────┘            │
│                                    │                                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                     TierRouter Actor                                │   │
│   │  - Smart query routing based on data age and actor location         │   │
│   │  - Parallel query across tiers for range queries                    │   │
│   │  - Result merging from multiple tiers with minimal latency          │   │
│   │  - Actor reference caching for fast lookups                         │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Cross-Tier Query Execution

Queries transparently span all tiers with minimal performance impact:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Cross-Tier Query Flow                                   │
│                                                                             │
│   SELECT * FROM orders WHERE created_at > '2024-01-01'                      │
│                                    │                                        │
│                                    ▼                                        │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      QueryPlanner                                   │   │
│   │  1. Parse time range predicate                                      │   │
│   │  2. Identify tier coverage: Hot (last 48h), Warm (48h-30d), Cold    │   │
│   │  3. Generate parallel sub-queries for each tier                     │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│         ┌──────────────────────────┼──────────────────────────┐             │
│         ▼                          ▼                          ▼             │
│   ┌─────────────┐          ┌─────────────┐          ┌─────────────┐         │
│   │ Hot Query   │          │ Warm Query  │          │ Cold Query  │         │
│   │ (parallel)  │          │ (parallel)  │          │ (parallel)  │         │
│   │             │          │             │          │             │         │
│   │ RowActor    │          │ ExtentActor │          │ ColumnActor │         │
│   │ scan        │          │ range scan  │          │ columnar    │         │
│   │ < 1ms       │          │ < 10ms      │          │ scan        │         │
│   │             │          │             │          │ < 100ms     │         │
│   └──────┬──────┘          └──────┬──────┘          └──────┬──────┘         │
│          │                        │                        │                │
│          └────────────────────────┼────────────────────────┘                │
│                                   ▼                                         │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      ResultMerger                                   │   │
│   │  - Streaming merge as results arrive                                │   │
│   │  - Timestamp-ordered output                                         │   │
│   │  - Deduplication across tier boundaries                             │   │
│   │  - Total latency: max(tier latencies) + merge overhead              │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Actor Migration Between Tiers

Actors move between tiers based on age and access patterns:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Actor Tier Migration                                    │
│                                                                             │
│   RowActor (order_123)                                                      │
│   ├── Created: Hot tier (in-memory, row format)                             │
│   │   └── Age: 0-48 hours, Access: High frequency                           │
│   │                                                                         │
│   ├── Migrated: Hot → Warm                                                  │
│   │   └── Trigger: Age > 48 hours OR access frequency < threshold           │
│   │   └── Transform: Row → RocksDB key-value                                │
│   │   └── Actor type: RowActor → ExtentActor (grouped with similar rows)    │
│   │                                                                         │
│   └── Archived: Warm → Cold                                                 │
│       └── Trigger: Age > 30 days                                            │
│       └── Transform: Row → Columnar (Parquet segment)                       │
│       └── Actor type: ExtentActor → ColumnActor (analytical format)         │
│                                                                             │
│   Migration is transparent to queries - TierRouter maintains actor registry │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Proposed Implementation Phases

| Phase | Component | Description |
|-------|-----------|-------------|
| 1 | StorageCoordinatorActor | Central actor tracking data age and tier assignments |
| 2 | Multi-granularity actors | TableActor, ExtentActor, ColumnActor, RowActor, FieldActor, IndexActor |
| 3 | TierRouter | Query routing based on timestamps and actor location registry |
| 4 | DataMigrationWorker | Background actor for tier-to-tier data and actor movement |
| 5 | CrossTierQueryExecutor | Parallel query execution across tiers with result merging |
| 6 | ClusterAwareHybridStorage | Updated HybridStorageManager with actor integration |

#### Key Actor Messages

```rust
// Multi-granularity storage actor message types
enum StorageMessage {
    // Actor Lifecycle
    CreateActor { granularity: Granularity, id: ActorId, tier: StorageTier },
    MigrateActor { actor_id: ActorId, from_tier: StorageTier, to_tier: StorageTier },
    DeactivateActor { actor_id: ActorId },

    // Tier Coordination
    RegisterTierNode { node_id: NodeId, tier: StorageTier, capacity: u64 },
    ReportActorStats { actor_id: ActorId, access_count: u64, last_access: DateTime },

    // Cross-Tier Query
    RouteQuery { query: Query, time_range: TimeRange },
    QueryTier { tier: StorageTier, sub_query: Query },
    MergeResults { results: Vec<TierResult> },

    // Data Operations (granularity-aware)
    ReadRow { table: String, key: RowKey },
    ReadExtent { table: String, extent_id: ExtentId },
    ReadColumn { table: String, column: String, range: RowRange },
    ReadField { table: String, row: RowKey, field: String },
}

enum Granularity {
    Table,
    Extent,
    Column,
    Row,
    Field,
    Index,
}
```

#### Configuration (Proposed)

```toml
[storage.tiering]
enabled = true
hot_threshold_hours = 48
warm_threshold_days = 30

[storage.tiering.actors]
# Granularity-specific settings
row_actor_hot_threshold = 1000      # Access count before considering migration
extent_size_bytes = 65536           # 64KB extent grouping
column_segment_rows = 100000        # Rows per column segment
field_actor_min_size = 4096         # Min bytes to warrant separate FieldActor

[storage.tiering.cluster]
enabled = true
hot_nodes = ["node-1", "node-2"]      # Fast OLTP nodes
warm_nodes = ["node-3", "node-4"]      # General-purpose nodes
cold_nodes = ["node-5"]                 # Archive/OLAP node
replication_factor = 2                  # Copies per tier
migration_batch_size = 10000            # Rows per migration batch

[storage.tiering.routing]
prefer_hot_for_recent = true           # Route recent queries to hot tier
parallel_tier_query = true              # Query multiple tiers in parallel
merge_strategy = "streaming"            # "streaming" or "batch"
actor_cache_ttl_seconds = 300          # Cache actor locations for routing

[storage.tiering.query]
cross_tier_timeout_ms = 5000           # Max time for cross-tier query
hot_tier_priority = true               # Return hot results first (streaming)
cold_tier_pushdown = true              # Push predicates to columnar engine
```

#### Benefits of Multi-Granularity Actor Tiering

1. **Fine-Grained Control**: Move individual rows, columns, or extents between tiers based on actual access patterns
2. **Optimal Storage Format**: Row format in hot (OLTP), extent format in warm (mixed), columnar in cold (OLAP)
3. **Transparent Queries**: Applications query as if data is in one place; TierRouter handles distribution
4. **Minimal Query Latency**: Parallel tier queries with streaming merge; total time ≈ slowest tier
5. **Actor Locality**: Actors migrate with their data; no remote calls for local data
6. **Adaptive Placement**: AI subsystem can recommend actor migrations based on workload patterns
7. **Specialized Hardware**: Hot nodes with NVMe for row actors, cold nodes with HDD for column actors
8. **Cost Optimization**: Granular tiering moves only cold data to cheaper storage

---

## AI-Native Subsystems

### Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   AI Master Controller                      │
│              (10-second control loop)                       │
├──────────────┬──────────────┬──────────────┬────────────-───┤
│   Query      │   Resource   │   Storage    │  Transaction   │
│  Optimizer   │   Manager    │   Manager    │   Manager      │
├──────────────┼──────────────┼──────────────┼──────────────-─┤
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
| LLM Runtime | `server/src/llm/` | Shared model registry + router bootstrap |
| LLM Provider Layer | `orbit-llm/` | Provider abstraction, fallback, cost accounting |

---

## Feature Status Matrix

### Protocol Implementation Status

| Protocol | Status | Completion | Tests | Key Components |
|----------|--------|------------|-------|----------------|
| **OrbitQL** | Complete | 95% | 50+ | SurrealDB-style DEFINE/REMOVE, TRAVERSE/RELATE/MATCH, Vector KNN, Control flow (IF/FOR/LET), LIVE queries, ML functions |
| **Redis RESP** | Complete | 97% | 183 | String, Hash, List, Set, SortedSet, Stream, PubSub, Vector, TimeSeries, Graph, CLUSTER |
| **PostgreSQL** | Complete | 96% | 460+ | Wire protocol (v3/v3.2), SQL parser (complete DDL/DML/DCL/TCL), Query engine, JSONB, pgvector, CTEs, Window functions, PG18 features, Sequences, Full-text search types |
| **MySQL** | Complete | 80% | 32 | Wire protocol, Auth, Binary protocol (prepared statements) |
| **CQL (Cassandra)** | Complete | 75% | 23 | Wire protocol, CQL parser, BATCH operations, LWT |
| **AQL (ArangoDB)** | Production Ready | 85% | 250+ | Parser, Query engine, **Graph traversal (OUTBOUND/INBOUND/ANY, depth, SHORTEST_PATH, K_PATHS, METRICS)**, **PRUNE**, **OPTIONS**, **COLLECT**, **PROFILE**, SEARCH |
| **Cypher/Bolt** | Active | 75% | 107 | Bolt protocol, Cypher parser, Graph engine, db.* procedures, APOC, GDS algorithms |
| **MongoDB** | Active | 50% | 6 | Wire protocol (OP_MSG), CRUD, Aggregation pipeline (12 stages) |

### Detailed Feature Breakdown

| Feature | Status | Tests | Key Files |
|---------|--------|-------|-----------|
| **Core Systems** | | | |
| Core Actor System | Complete | 555 | `orbit-shared/src/lib.rs` |
| Distributed Transactions | Complete | 22 | `shared/src/transactions/` |
| REST API | Complete | 4 | `protocols/rest/` |
| **OrbitQL Features** | | | |
| Lexer/Parser | Complete | ~20 | `orbitql/lexer.rs`, `orbitql/ast.rs` |
| DEFINE/REMOVE (SurrealDB-style) | Complete | ~10 | `orbitql/parser.rs` |
| Graph (TRAVERSE/RELATE/MATCH) | Complete | ~10 | `orbitql/executor.rs` |
| Vector KNN Search | Complete | ~5 | `orbitql/executor.rs` |
| Control Flow (IF/FOR/LET/THROW) | Complete | ~5 | `orbitql/ast.rs` |
| LIVE Queries / KILL | Complete | - | `orbitql/streaming.rs` |
| Transaction SAVEPOINTs | Complete | - | `orbitql/executor.rs` |
| **Redis RESP Features** | | | |
| String/Hash/List/Set/SortedSet | Complete | ~80 | `resp/commands/*.rs` |
| Stream/PubSub | Complete | ~20 | `resp/commands/stream.rs`, `pubsub.rs` |
| Vector Commands | Complete | ~25 | `resp/commands/vector.rs` |
| Time Series | Complete | ~24 | `resp/commands/time_series.rs` |
| Graph Commands | Complete | ~20 | `resp/commands/graph.rs` |
| CLUSTER Commands | Complete | 7 | `resp/commands/cluster.rs` |
| ACL Commands | Complete | ~5 | `resp/commands/acl.rs` |
| **PostgreSQL Features** | | | |
| Wire Protocol (v3/v3.2) | Complete | - | `postgres_wire/protocol.rs` |
| SQL Parser (DDL/DML/DCL/TCL) | Complete | ~200 | `sql/parser/*.rs` (100+ DDL statements: CREATE/ALTER/DROP for all PG objects) |
| Query Engine | Complete | ~100 | `sql/query_engine.rs` |
| JSONB Operators | Complete | 12 | `jsonb/operators.rs` |
| pgvector Support | Complete | ~50 | `sql/pgvector*.rs` |
| Window Functions | Complete | ~30 | `sql/window_functions.rs` |
| **PostgreSQL 18 Features** | | | |
| NegotiateProtocolVersion | Complete | - | `postgres_wire/protocol.rs`, `messages.rs` |
| Variable-length Cancel Keys | Complete | - | `postgres_wire/protocol.rs` |
| UUIDv7 Functions | Complete | 6 | `sql/expression_evaluator.rs` |
| GENERATED Columns (STORED/VIRTUAL) | Complete | 10 | `sql/executor.rs` |
| OLD/NEW in RETURNING | Complete | 4 | `sql/executor.rs` |
| Temporal Constraints (WITHOUT OVERLAPS) | Complete | 13 | `sql/parser/ddl.rs`, `sql/executor.rs` |
| Sequence Functions (nextval/currval/setval/lastval) | Complete | 17 | `sql/executor.rs`, `sql/expression_evaluator.rs` |
| Math Functions (cbrt/div/factorial/gcd/lcm/sign) | Complete | - | `sql/expression_evaluator.rs` |
| MERGE with RETURNING | Partial | 3 | `sql/parser/dml.rs` |
| Comprehensive DDL Parser | Complete | - | `sql/parser/ddl.rs` (6300+ lines) |
| CREATE/ALTER/DROP: Foreign Tables, FDW, Servers | Complete | - | `sql/parser/ddl.rs` |
| CREATE/ALTER/DROP: Publications, Subscriptions | Complete | - | `sql/parser/ddl.rs` |
| CREATE/ALTER/DROP: Event Triggers, Access Methods | Complete | - | `sql/parser/ddl.rs` |
| CREATE/ALTER/DROP: Text Search (Config/Dict/Parser/Template) | Complete | - | `sql/parser/ddl.rs` |
| CREATE/ALTER/DROP: Transforms, Languages, Statistics | Complete | - | `sql/parser/ddl.rs` |
| CREATE/ALTER/DROP: Operators, Aggregates, Casts | Complete | - | `sql/parser/ddl.rs` |
| CREATE/ALTER/DROP: Collations, Conversions, Tablespaces | Complete | - | `sql/parser/ddl.rs` |
| **Time Travel & Data Recovery** | | | |
| Time Travel SQL Syntax (AT TIMESTAMP/VERSION/SNAPSHOT) | Complete | 7 | `sql/parser/select.rs`, `sql/lexer.rs` |
| Time Travel SQL:2011 (FOR SYSTEM_TIME AS OF) | Complete | 7 | `sql/parser/select.rs`, `sql/lexer.rs` |
| Time Travel Iceberg Storage (query_as_of, query_by_snapshot_id) | Complete | - | `execution/iceberg_cold.rs`, `engine/storage/iceberg.rs` |
| Time Travel Executor Integration | Complete | - | `sql/executor.rs`, `sql/execution_strategy.rs` (requires `storage-iceberg` feature) |
| UNDROP TABLE Syntax | Complete | 4 | `sql/parser/ddl.rs`, `sql/lexer.rs` |
| UNDROP TABLE Executor | Complete | - | `sql/execution_strategy.rs` (requires Iceberg catalog config) |
| **AQL (ArangoDB) Features** | | | |
| AQL Parser | Complete | ~20 | `aql/aql_parser.rs` (2941 lines) |
| AQL Query Engine | Complete | ~30 | `aql/query_engine.rs` (5300+ lines) |
| Graph Traversal (FOR...IN...OUTBOUND/INBOUND/ANY) | Complete | ~15 | `aql/query_engine.rs`, `common/graph_algorithms.rs` |
| Shortest Path (SHORTEST_PATH) | Complete | ~5 | `aql/query_engine.rs` (Dijkstra algorithm) |
| K Shortest Paths (K_SHORTEST_PATHS) | Complete | ~3 | `aql/query_engine.rs` (Yen's algorithm) |
| All Shortest Paths (ALL_SHORTEST_PATHS) | Complete | ~3 | `aql/query_engine.rs` |
| PRUNE Clause (condition evaluation) | Complete | ~5 | `aql/query_engine.rs` (custom_traversal with pruning) |
| Traversal Options (BFS/DFS, depth control) | Complete | ~5 | `aql/query_engine.rs` |
| Uniqueness Constraints (uniqueVertices, uniqueEdges) | Complete | ~5 | `aql/query_engine.rs` (None/Path/Global levels) |
| COLLECT Clause (grouping, aggregations) | Complete | ~8 | `aql/query_engine.rs` (COUNT, SUM, AVG, MIN, MAX, etc.) |
| PROFILE Feature (query profiling) | Complete | ~3 | `aql/query_engine.rs` (execution time, clause count) |
| Graph Functions | Complete | ~15 | `aql/query_engine.rs` (GRAPH_SHORTEST_PATH, GRAPH_DISTANCE_TO, GRAPH_NEIGHBORS, GRAPH_COMMON_NEIGHBORS, GRAPH_ECCENTRICITY, GRAPH_RADIUS, GRAPH_DIAMETER) |
| Document CRUD (INSERT/UPDATE/REPLACE/REMOVE) | Complete | ~15 | `aql/query_engine.rs` |
| UPSERT Operations | Complete | ~3 | `aql/query_engine.rs` |
| FILTER/SORT/LIMIT/LET | Complete | ~10 | `aql/query_engine.rs` |
| SEARCH (Full-text) | Complete | ~5 | `aql/query_engine.rs` |
| FULLTEXT Function | Complete | ~5 | `aql/query_engine.rs` |
| AQL Storage (RocksDB) | Complete | ~10 | `aql/storage.rs` |
| GraphRAG Integration | Complete | ~10 | `aql/graphrag_engine.rs` (10 GraphRAG functions) |
| **Cypher/Bolt Features** | | | |
| Bolt Protocol v4/v5 | Complete | - | `bolt_protocol.rs` |
| Cypher Parser | Complete | ~40 | `cypher_parser.rs` |
| Graph Engine | Complete | ~30 | `graph_engine.rs` |
| db.* Procedures | Complete | ~10 | `db_procedures.rs` |
| APOC Procedures | Complete | 11 | `apoc_procedures.rs` |
| GDS Algorithms | Complete | 16 | `graph_algorithms_procedures.rs` (PageRank, Betweenness, Louvain, LabelProp, HITS, WCC, SCC, etc.) |
| **MongoDB Features** | | | |
| Wire Protocol (OP_MSG) | Complete | - | `mongodb/protocol.rs` |
| Document Storage | Complete | - | `mongodb/storage.rs` |
| Aggregation Pipeline | Complete | - | 12 stages: $match, $project, $sort, $group, $unwind, $lookup, etc. |
| **AI/ML Features** | | | |
| AI-Native Features | Complete | 14 | `server/src/ai/` |
| Heterogeneous Compute | Complete | 83 | `orbit-compute/` |
| Machine Learning (core) | Partial | 283 | `orbit-ml/` — engine, inference, streaming |
| Machine Learning (industry verticals) | Scaffolding | 0 | `orbit-ml/industry_models/` — ~470 unimplemented stubs; feature-gated off by default behind `experimental-industry-models` |
| LLM Provider Layer | Complete | 160 | `orbit-llm/` — OpenAI, Anthropic, Ollama, OpenAI-compatible |
| LLM Registry & Router | Complete | (incl. above) | Runtime model switching, fallback, breaker, cost |
| LLM RESP Commands | Complete | 15 | `resp/commands/llm.rs` — `LLM.*` |
| LLM Streaming | Planned | 0 | Roadmap M6 |
| Semantic Cache | Planned | 0 | Roadmap M7 |
| Auto-embedding on write | Planned | 0 | Roadmap M8 |
| **Infrastructure** | | | |
| Kubernetes Operator | Active | 0 | `orbit-operator/` |

**Total Tests: 2,420+** (as of December 2025)

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

```text
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

```text
[ ] Read current PRD.md before making changes
[ ] Make code changes
[ ] Update relevant PRD.md sections
[ ] Update test counts if changed
[ ] Run: cargo fmt --all
[ ] Run: cargo clippy --workspace -- -D warnings
[ ] Run: cargo test --workspace
[ ] Commit code AND PRD.md together
```

### Commit Message Format

When updating this document along with code changes:

```text
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
