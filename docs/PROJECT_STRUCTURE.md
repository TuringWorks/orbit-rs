---
layout: default
title: Project Structure
category: documentation
---

# Project Structure

This document provides a comprehensive overview of the Orbit-RS project structure, including module organization, dependencies, and architectural layers.

## Overview

Orbit-RS is organized as a Rust workspace with multiple crates, each serving a specific purpose in the distributed actor system. The modular architecture allows for independent development, testing, and deployment of different components.

## Workspace Structure

```
orbit-rs/
├── Cargo.toml                    # Workspace configuration and dependencies
├── README.md                     # Project overview and quick start
├── LICENSE-MIT                   # MIT license
├── LICENSE-BSD                   # BSD 3-Clause license
│
├── orbit-util/                   # Core utilities and base functionality
├── orbit-shared/                 # Shared data structures and types
├── orbit-proto/                  # Protocol Buffer definitions and gRPC services
├── orbit-client/                 # Client-side actor system implementation
├── orbit-server/                 # Server-side cluster management with persistence
├── orbit-server-etcd/            # etcd-based distributed directory
├── orbit-server-prometheus/      # Prometheus metrics integration
├── orbit-protocols/              # Protocol adapters (Redis, PostgreSQL, MCP)
├── orbit-application/            # Application-level utilities
├── orbit-benchmarks/             # Performance benchmarks
├── orbit-operator/               # Kubernetes operator with custom CRDs
│
├── examples/                     # Example applications and demos
│   ├── hello-world/              # Basic actor greeting example
│   ├── distributed-counter/      # Shared counter with coordination
│   ├── distributed-transactions/ # Banking transaction example
│   ├── saga-example/             # Order processing workflow
│   ├── resp-server/              # Redis RESP protocol server
│   ├── vector-store/             # Vector database example
│   ├── pgvector-store/           # PostgreSQL vector extension
│   ├── timeseries-demo/          # Time series data handling
│   ├── orbitql-example/          # OrbitQL query examples
│   └── mcp-*/                    # Model Context Protocol examples
│
├── docs/                         # Comprehensive documentation (reorganized)
│   ├── rfc/                      # Request for Comments documents
│   ├── architecture/             # Architecture documentation
│   ├── development/              # Development guides
│   ├── deployment/               # Deployment documentation
│   ├── protocols/                # Protocol adapter docs
│   └── wip/                      # Work-in-progress documents
│
├── orbit-benchmarks/             # Consolidated performance benchmarks
├── verification/                 # Verification and formal methods
├── tools/                        # Development tools
│   └── vscode-orbitql/          # OrbitQL VS Code extension
├── tests/                        # Integration tests
├── scripts/                      # Development and deployment scripts
├── helm/                         # Helm charts for Kubernetes deployment
├── k8s/                         # Kubernetes manifests with persistence
└── .github/                     # GitHub Actions workflows and templates
```

## Core Crates

### orbit-util
**Purpose**: Foundational utilities and common functionality

```
orbit-util/
├── Cargo.toml
└── src/
    ├── lib.rs                   # Public API exports
    ├── config.rs                # Configuration management utilities
    ├── logging.rs               # Structured logging setup
    ├── metrics.rs               # Metrics collection utilities
    ├── time.rs                  # Time and duration utilities
    ├── network.rs               # Network utility functions
    └── test_utils.rs            # Testing utilities
```

**Key Features**:
- Configuration loading and validation
- Structured logging with tracing
- Metrics collection abstractions
- Common test utilities
- Network and time helpers

### orbit-shared
**Purpose**: Shared data structures, types, and core abstractions

```
orbit-shared/
├── Cargo.toml
└── src/
    ├── lib.rs                   # Public API and re-exports
    ├── actor.rs                 # Actor trait definitions
    ├── key.rs                   # Actor key types (String, UUID)
    ├── error.rs                 # Comprehensive error types
    ├── message.rs               # Message passing abstractions
    ├── lease.rs                 # Lease management types
    ├── node.rs                  # Node and cluster types
    ├── load_balancer.rs         # Load balancing strategies
    ├── health.rs                # Health check definitions
    ├── orbitql/                 # OrbitQL query language
    │   ├── mod.rs               # OrbitQL module exports
    │   ├── lexer.rs             # SQL-like lexer
    │   ├── parser.rs            # Query parser
    │   ├── ast.rs               # Abstract syntax tree
    │   ├── executor.rs          # Query executor
    │   └── lsp/                 # Language Server Protocol
    │       ├── mod.rs           # LSP implementation
    │       └── handlers.rs      # LSP request handlers
    ├── persistence/             # Persistence abstractions
    │   ├── mod.rs               # Persistence traits and types
    │   ├── provider.rs          # Storage provider interface
    │   └── config.rs            # Configuration types
    └── transactions/            # Transaction system
        ├── mod.rs               # Transaction module exports
        ├── types.rs             # Transaction types and IDs
        ├── coordinator.rs       # Transaction coordinator
        ├── participant.rs       # Participant implementation
        ├── saga.rs              # Saga pattern implementation
        ├── locks.rs             # Distributed locking
        ├── metrics.rs           # Transaction metrics
        └── security.rs          # Security and audit features
```

**Key Features**:
- Actor trait system with lifecycle management
- Comprehensive error handling
- Transaction system with 2PC and Saga patterns
- Distributed locks with deadlock detection
- Load balancing strategies
- Health monitoring abstractions
- OrbitQL query language with LSP support
- Persistence provider abstractions
- Multi-backend storage configuration

### orbit-proto
**Purpose**: Protocol Buffer definitions and gRPC service specifications

```
orbit-proto/
├── Cargo.toml
├── build.rs                     # Protocol Buffer compilation
├── proto/                       # Protocol Buffer definitions
│   ├── actor.proto             # Actor service definitions
│   ├── cluster.proto           # Cluster management services
│   ├── transaction.proto       # Transaction coordination
│   ├── lease.proto             # Lease management
│   ├── health.proto            # Health checking
│   └── metrics.proto           # Metrics collection
└── src/
    ├── lib.rs                  # Generated code exports
    └── generated/              # Generated Rust code (build-time)
        ├── actor.rs
        ├── cluster.rs
        └── ...
```

**Key Features**:
- Cross-language type definitions
- gRPC service specifications
- Efficient binary serialization
- Backwards compatibility support
- Service versioning

### orbit-client
**Purpose**: Client-side actor system implementation

```
orbit-client/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Public client API
    ├── client.rs               # Main OrbitClient implementation
    ├── builder.rs              # Client builder pattern
    ├── proxy.rs                # Actor proxy generation
    ├── invocation.rs           # Remote method invocation
    ├── lease.rs                # Client-side lease management
    ├── connection.rs           # Connection management and pooling
    ├── discovery.rs            # Service discovery client
    ├── load_balancer.rs        # Client-side load balancing
    └── error.rs                # Client-specific errors
```

**Key Features**:
- Actor proxy generation and management
- Connection pooling and management
- Client-side load balancing
- Service discovery integration
- Lease management
- Retry and circuit breaker patterns

### orbit-server
**Purpose**: Server-side cluster management and actor hosting

```
orbit-server/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Public server API
    ├── server.rs               # Main OrbitServer implementation
    ├── actor_manager.rs        # Actor lifecycle management
    ├── cluster.rs              # Cluster membership management
    ├── node.rs                 # Node management and registration
    ├── lease_manager.rs        # Server-side lease management
    ├── load_balancer.rs        # Request load balancing
    ├── health.rs               # Health monitoring
    ├── discovery.rs            # Service discovery server
    ├── grpc_server.rs          # gRPC server implementation
    ├── mesh.rs                 # Mesh networking and coordination
    └── persistence/            # Persistence layer with multiple backends
        ├── mod.rs              # Persistence abstractions and traits
        ├── config.rs           # Configuration for persistence backends
        ├── dynamic.rs          # Dynamic provider switching and monitoring
        ├── memory.rs           # In-memory storage provider
        ├── cow_btree.rs        # Copy-on-Write B+ Tree provider
        ├── lsm_tree.rs         # LSM-Tree storage provider
        └── rocksdb.rs          # RocksDB storage provider
```

**Key Features**:
- Actor lifecycle and state management with persistence
- Multiple storage backends (Memory, COW B+Tree, LSM-Tree, RocksDB)
- Cluster membership and coordination
- Health monitoring and failure detection
- Load balancing across cluster nodes
- gRPC service implementation
- Service discovery and registration
- Storage backend independence and hot-swapping

## Protocol and Integration Crates

### orbit-protocols
**Purpose**: Multi-protocol adapters for different client interfaces

```
orbit-protocols/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Protocol adapter exports
    ├── redis/                  # Redis RESP protocol
    │   ├── mod.rs
    │   ├── server.rs           # RESP server implementation
    │   ├── commands.rs         # Redis command handlers
    │   ├── actors.rs           # Redis-specific actors
    │   └── protocol.rs         # RESP protocol parsing
    ├── postgres_wire/          # PostgreSQL wire protocol
    │   ├── mod.rs
    │   ├── server.rs           # PostgreSQL server implementation
    │   ├── sql/                # SQL parsing and execution
    │   │   ├── lexer.rs        # SQL lexer
    │   │   ├── parser.rs       # SQL parser
    │   │   ├── ast.rs          # Abstract syntax tree
    │   │   ├── executor.rs     # Query executor
    │   │   └── optimizer.rs    # Query optimizer
    │   ├── types.rs            # PostgreSQL data types
    │   └── vector.rs           # Vector operations (pgvector)
    ├── mcp/                    # Model Context Protocol
    │   ├── mod.rs
    │   ├── server.rs           # MCP server implementation
    │   ├── types.rs            # MCP message types
    │   ├── handlers.rs         # Request handlers
    │   └── tools.rs            # AI agent tools
    └── rest/                   # REST API (planned)
        ├── mod.rs
        ├── server.rs
        └── handlers.rs
```

**Key Features**:
- Redis RESP protocol with 50+ commands
- PostgreSQL wire protocol with full SQL support
- Vector database capabilities (pgvector compatible)
- Model Context Protocol for AI agents
- Comprehensive SQL parser and executor
- Multi-protocol client compatibility

### orbit-operator
**Purpose**: Kubernetes operator for automated deployment and management

```
orbit-operator/
├── Cargo.toml
├── deploy/                     # Kubernetes deployment manifests
│   ├── crds.yaml              # Custom Resource Definitions
│   ├── rbac.yaml              # Role-based access control
│   ├── operator.yaml          # Operator deployment
│   └── examples.yaml          # Example cluster configurations
└── src/
    ├── main.rs                # Operator entry point
    ├── controller.rs          # Main controller logic
    ├── resources/             # CRD implementations
    │   ├── cluster.rs         # OrbitCluster CRD with persistence config
    │   ├── actor.rs           # OrbitActor CRD
    │   └── transaction.rs     # OrbitTransaction CRD
    ├── crd.rs                 # Custom resource definitions with persistence
    ├── reconciler.rs          # Resource reconciliation
    ├── config.rs              # Operator configuration
    └── error.rs               # Operator-specific errors
```

**Key Features**:
- Custom Resource Definitions for Orbit resources
- Automated cluster deployment and scaling with persistence
- Actor lifecycle management in Kubernetes
- Resource reconciliation and state management
- Storage backend configuration and management
- StatefulSet with PVC templates for persistence
- Integration with Kubernetes ecosystem
- Production-ready deployment automation

### orbit-benchmarks
**Purpose**: Comprehensive performance benchmarking and testing framework

```
orbit-benchmarks/                 # Performance benchmarks (excluded from workspace)
├── Cargo.toml                   # Independent crate configuration
├── README.md                    # Benchmark documentation and usage
├── benches/                     # Criterion benchmarks
│   ├── actor_benchmarks.rs      # Actor system performance
│   ├── leader_election_benchmarks.rs # Raft consensus performance
│   └── persistence_comparison.rs # Storage backend comparison (⚠️ WAL issues)
├── src/
│   ├── lib.rs                   # Benchmark framework exports
│   ├── orbitql/                 # OrbitQL query benchmarks (NEW)
│   │   ├── mod.rs               # OrbitQL benchmark exports
│   │   ├── benchmark.rs         # Query performance framework
│   │   └── comprehensive_benchmark.rs # TPC workload implementations
│   ├── persistence/             # Storage backend benchmarks
│   │   ├── mod.rs
│   │   ├── config.rs
│   │   ├── metrics.rs
│   │   ├── cow_btree.rs
│   │   ├── lsm_tree.rs
│   │   └── rocksdb_impl.rs
│   ├── compute/                 # Heterogeneous compute benchmarks
│   │   └── mod.rs
│   └── performance/             # General performance benchmarks
│       └── mod.rs
├── scripts/                     # Benchmark utilities
│   ├── run_benchmarks.sh       # Master benchmark runner
│   ├── analyze_results.py      # Result analysis and reporting
│   └── README.md               # Script documentation
├── examples/                    # Example benchmark usage
│   ├── cow_btree_demo.rs
│   ├── cow_btree_persistence_demo.rs
│   ├── rocksdb_demo.rs
│   └── configurable_backends_demo.rs
└── docs/                        # Benchmark-specific documentation
    ├── PERSISTENCE_ARCHITECTURE.md
    └── DISASTER_RECOVERY.md
```

**Key Features**:
- **OrbitQL Benchmarks**: TPC-H, TPC-C, TPC-DS industry-standard query benchmarks
- **Query Performance**: Comprehensive query optimization, vectorization, and parallelization testing
- **Actor System**: Virtual actor performance and message throughput benchmarks
- **Consensus**: Raft leader election and state management performance
- **Storage**: Multi-backend persistence performance comparison
- **Heterogeneous Compute**: GPU and SIMD acceleration benchmarks
- **CI/CD Integration**: Manual GitHub Actions workflow for performance validation
- **WAL Replay Issues**: Known issues with persistence comparison benchmarks
- **Independent Execution**: Excluded from workspace to prevent build interference

**Recent Changes**:
- ✅ **OrbitQL Benchmarks Moved**: Comprehensive query benchmarks relocated from `orbit-shared`
- ✅ **TPC Workloads**: Industry-standard TPC-H, TPC-C, TPC-DS benchmark implementations
- ✅ **Performance Framework**: Complete benchmark framework for query optimization validation
- ⚠️ **WAL Issues**: Persistence comparison benchmarks may hang due to WAL replay loops

## Supporting Infrastructure

### examples/
**Purpose**: Example applications and integration tests

```
examples/
├── hello-world/               # Basic actor example
│   ├── Cargo.toml
│   └── src/
│       └── main.rs           # Simple greeting actor
├── distributed-counter/       # Multi-actor coordination
│   ├── Cargo.toml
│   └── src/
│       └── main.rs           # Counter with coordination
├── banking-system/            # Transaction example
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── account.rs        # Bank account actor
│       └── transfer.rs       # Transfer coordination
└── tests/                     # Integration and BDD tests
    ├── features/              # Cucumber BDD scenarios
    │   ├── actor_lifecycle.feature
    │   ├── cluster_management.feature
    │   └── transactions.feature
    └── tests/                 # Integration test suites
        ├── integration.rs     # End-to-end tests
        └── bdd.rs            # BDD test runner
```

### docs/
**Purpose**: Comprehensive project documentation

```
docs/
├── README.md                  # Documentation index
├── OVERVIEW.md                # Project overview and architecture
├── QUICK_START.md             # Getting started guide
├── PROJECT_STRUCTURE.md       # This document
├── ROADMAP.md                 # Development roadmap
├── features/                  # Feature-specific documentation
│   └── TRANSACTION_FEATURES.md
├── protocols/                 # Protocol documentation
│   └── PROTOCOL_ADAPTERS.md
├── deployment/                # Deployment guides
│   └── DEPLOYMENT.md
└── development/               # Development documentation
    └── DEVELOPMENT.md
```

### scripts/
**Purpose**: Development and operational scripts

```
scripts/
├── prepare-secrets.sh         # CI/CD secret preparation
├── build-docker.sh           # Docker image building
├── run-benchmarks.sh         # Performance benchmarking
├── deploy-staging.sh         # Staging deployment
├── deploy-production.sh      # Production deployment
└── test-all.sh              # Comprehensive testing
```

## Dependency Relationships

### Core Dependencies
```
orbit-util (foundation)
    ↓
orbit-shared (core types)
    ↓
orbit-proto (networking)
    ↓  ↓
orbit-client  orbit-server
    ↓  ↓
orbit-protocols (adapters)
    ↓
orbit-operator (orchestration)
```

### Key External Dependencies

#### Core Dependencies
- **tokio**: Async runtime and utilities
- **serde**: Serialization framework
- **tracing**: Structured logging and instrumentation
- **anyhow/thiserror**: Error handling
- **uuid**: UUID generation and handling

#### Networking and Communication
- **tonic**: gRPC implementation
- **prost**: Protocol Buffer implementation
- **hyper**: HTTP client and server
- **tower**: Service abstractions and middleware

#### Storage and Persistence
- **rocksdb**: High-performance key-value store
- **sqlx**: Async SQL database driver
- **bloom**: Bloom filter implementation for LSM-Tree
- **base64**: Binary encoding for WAL operations
- **redis**: Redis client (for protocol adapter)

#### Kubernetes Integration
- **kube**: Kubernetes client library
- **k8s-openapi**: Kubernetes API types
- **schemars**: JSON schema generation

#### Testing and Development
- **tokio-test**: Async testing utilities
- **criterion**: Benchmarking framework
- **cucumber**: BDD testing framework
- **mockall**: Mock object generation

## Build Configuration

### Workspace Cargo.toml
The root `Cargo.toml` defines the workspace and shared dependencies:

```toml
[workspace]
members = [
    "orbit-util",
    "orbit-shared", 
    "orbit-proto",
    "orbit-client",
    "orbit-server",
    "orbit-protocols",
    "orbit-operator",
    "examples/*"
]

[workspace.dependencies]
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
tracing = "0.1"

# ... other shared dependencies

[workspace.lints.clippy]
pedantic = "warn"
nursery = "warn"
```

### Feature Flags
Conditional compilation features for different use cases:

```toml
[features]
default = ["client", "server"]
client = ["orbit-client"]
server = ["orbit-server", "orbit-protocols"]
kubernetes = ["orbit-operator"]
redis-protocol = ["orbit-protocols/redis"]
postgres-protocol = ["orbit-protocols/postgres"]
benchmarks = ["criterion"]
integration-tests = ["tokio-test"]
```

## Architecture Layers

### Layer 1: Foundation (orbit-util, orbit-shared)
- Core utilities and common functionality
- Shared types and abstractions
- Error handling and configuration
- No external service dependencies

### Layer 2: Communication (orbit-proto)
- Protocol definitions and code generation
- gRPC service specifications
- Cross-language compatibility
- Network serialization protocols

### Layer 3: Core Services (orbit-client, orbit-server)
- Actor system implementation
- Cluster management and coordination
- Client-server communication
- Service discovery and load balancing

### Layer 4: Protocol Adapters (orbit-protocols)
- Multi-protocol client support
- Redis, PostgreSQL, and MCP compatibility
- SQL parsing and execution
- Protocol-specific optimizations

### Layer 5: Orchestration (orbit-operator)
- Kubernetes integration
- Automated deployment and scaling
- Resource management
- Production operations

This layered architecture ensures clear separation of concerns, enables independent development and testing, and provides flexibility for different deployment scenarios.

## Related Documentation

- [Project Overview (Accurate)](PROJECT_OVERVIEW_ACCURATE.md) - High-level architecture and features
- [Development Guide](DEVELOPMENT.md) - Development setup and guidelines
- [Main README](../README.md) - Getting started with Orbit-RS
- [Deployment Guide](deployment/DEPLOYMENT.md) - Production deployment
- [Master Documentation](ORBIT_MASTER_DOCUMENTATION.md) - Comprehensive system documentation
- [Architecture Documentation](architecture/ORBIT_ARCHITECTURE.md) - Detailed system architecture
