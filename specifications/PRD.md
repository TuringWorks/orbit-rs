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
