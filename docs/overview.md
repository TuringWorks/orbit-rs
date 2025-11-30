---
layout: default
title: "Architecture Overview"
subtitle: "Understanding Orbit-RS design and capabilities"
category: "architecture"
permalink: /overview.html
---

## Production-Ready Multi-Protocol Database Platform

Orbit-RS is a high-performance, distributed multi-protocol database server written in Rust. It natively implements PostgreSQL, MySQL, CQL (Cassandra), Redis, HTTP REST, gRPC, and OrbitQL protocols from a single process, sharing a unified storage layer built on a virtual actor system.

[![Version](https://img.shields.io/badge/version-Production--Ready-brightgreen.svg)](https://github.com/TuringWorks/orbit-rs)
[![License](https://img.shields.io/badge/license-MIT%2FBSD--3--Clause-blue.svg)](https://github.com/TuringWorks/orbit-rs/blob/main/LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-red.svg)](https://www.rust-lang.org/)
[![148K+ Lines](https://img.shields.io/badge/lines-148K+-blue.svg)](project_overview.md)
[![1078+ Tests](https://img.shields.io/badge/tests-1078%2B-green.svg)](features.md)

## What is Orbit-RS?

Orbit-RS is a production-ready, multi-protocol database platform that combines the power of virtual actor architecture with comprehensive database capabilities. It's designed as a next-generation system that unifies:

- **Virtual Actor Framework** for distributed computing with in-process communication
- **Multi-Protocol Database** with SQL, vector, time-series, and graph support
- **AI/ML Integration** with 8 intelligent subsystems and neural engine acceleration
- **Cloud-Native Operations** with Kubernetes operators and enterprise features

### 🎯 **Current Status: Production Ready**

Orbit-RS has successfully delivered a **comprehensive multi-protocol database platform** with:

- **148,780+ lines** of production-ready Rust code
- **1,078+ tests** ensuring reliability and correctness
- **124+ Redis commands** with full compatibility
- **Complete PostgreSQL wire protocol** implementation
- **Advanced AI/ML capabilities** with 8 intelligent subsystems
- **In-process communication** for zero-overhead local operations

---

## 🏗️ Core Features

### 🎭 **Virtual Actor System**

- **Distributed Computing**: Actor-based distributed programming model
- **In-Process Communication**: Zero-overhead local actor invocations via channels
- **Automatic Lifecycle Management**: On-demand activation and transparent scaling
- **Type-Safe Interfaces**: Compile-time guarantees with Rust's type system
- **Location Transparency**: Actors can move between nodes seamlessly
- **State Persistence**: Automatic state management with RocksDB backend

### 📊 **Multi-Protocol Database**

- **SQL Database**: Full ANSI SQL support with PostgreSQL wire protocol compatibility
- **Vector Database**: High-performance similarity search with HNSW/IVFFLAT indexing
- **Time-Series Database**: RedisTimeSeries-compatible with 21 comprehensive tests
- **Graph Database**: Cypher-like queries with distributed graph operations
- **Document Store**: Flexible JSON document storage and querying
- **Key-Value Store**: Redis-compatible with 124+ commands

### 🤖 **AI-Native Subsystems (8 Components)**

1. **AI Master Controller** - Central orchestration (10-second control loop)
2. **Intelligent Query Optimizer** - Cost-based optimization with ML
3. **Predictive Resource Manager** - Workload forecasting
4. **Smart Storage Manager** - Hot/warm/cold tiering
5. **Adaptive Transaction Manager** - Dynamic concurrency control
6. **Learning Engine** - Model improvement
7. **Decision Engine** - Policy-based decisions
8. **Knowledge Base** - Pattern storage

### 🔌 **Protocol Support** (All with RocksDB Persistence)

| Protocol | Port | Status | Features |
|----------|------|--------|----------|
| **Redis RESP** | 6379 | ✅ Complete | 124+ commands, time series, vectors |
| **PostgreSQL** | 5432 | ✅ Complete | Full SQL, pgvector, JSONB, spatial |
| **MySQL** | 3306 | ✅ Complete | MySQL wire protocol compatibility |
| **CQL** | 9042 | ✅ Complete | Cassandra Query Language |
| **HTTP REST** | 8080 | ✅ Complete | JSON API, health, metrics |
| **gRPC** | 50051 | ✅ Complete | Actor communication, streaming |
| **Cypher/Bolt** | 7687 | 🔄 Active | Neo4j graph database protocol |

### ☁️ **Cloud-Native Architecture**

- **Kubernetes Operator**: Custom resources for cluster management
- **Helm Charts**: Production-ready deployment templates
- **Auto-Scaling**: Dynamic cluster scaling based on workload
- **Multi-Platform**: Support for linux/amd64, linux/arm64, and Apple Silicon
- **Enterprise Security**: RBAC, JWT authentication, audit trails

### ⚡ **Performance & Scale**

- **500k+ messages/second** per core throughput
- **Sub-millisecond latency** for actor message processing (1-5ms P99)
- **~10MB binary size** vs ~100MB JVM equivalents
- **Zero GC pauses** with predictable memory usage
- **In-process communication** eliminates network overhead for local actors
- **5-50x speedups** with heterogeneous compute acceleration

---

## 🏗️ System Architecture

Orbit-RS is built as a **comprehensive workspace** with **15 core modules**:

```text
orbit-rs/
├── 💭 Core Framework
│   ├── orbit-client/         # Client library with in-process support
│   ├── orbit-shared/         # Common types, clustering, transactions
│   ├── orbit-server/         # Multi-protocol server
│   ├── orbit-proto/          # gRPC services and Protocol Buffers
│   └── orbit-util/           # Utilities, RNG, metrics
├── 🔌 Storage & Compute
│   ├── orbit-engine/         # Storage engine (OrbitQL, adapters)
│   ├── orbit-compute/        # Hardware acceleration (SIMD, GPU)
│   └── orbit-ml/             # Machine learning inference
├── ☁️ Cloud & Operations
│   ├── orbit-operator/       # Kubernetes operator with CRDs
│   ├── orbit-server-etcd/    # etcd integration for clustering
│   └── orbit-server-prometheus/ # Metrics and monitoring
├── 🚀 Applications & Integration
│   ├── orbit-application/    # Application framework
│   ├── orbit-client-spring/  # Spring Boot integration
│   └── orbit-cli/            # Interactive CLI client
└── 🧪 Client SDKs
    ├── orbit-python-client/  # Python SDK
    └── orbit-vscode-extension/ # VS Code extension
```

### In-Process Communication Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   OrbitServer                           │
│  ┌──────────────────────────────────────────────────┐   │
│  │         Protocol Handlers                        │   │
│  │  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐         │   │
│  │  │Redis │  │Postgres│ │MySQL │  │ CQL  │         │   │
│  │  │:6379 │  │:5432  │  │:3306 │  │:9042 │         │   │
│  │  └───┬──┘  └───┬──┘  └───┬──┘  └───┬──┘         │   │
│  └──────┼─────────┼─────────┼─────────┼────────────┘   │
│         │         │         │         │                │
│         └─────────┴─────────┴─────────┘                │
│                     │                                   │
│         ┌───────────▼───────────┐                       │
│         │  OrbitClient (Local)  │                       │
│         │  In-Process Channels  │ ◄─── No gRPC!        │
│         └───────────┬───────────┘                       │
│                     │                                   │
│         ┌───────────▼───────────┐                       │
│         │  ServerConnectionSvc  │                       │
│         │  Message Processing   │                       │
│         └───────────┬───────────┘                       │
│                     │                                   │
│         ┌───────────▼───────────┐                       │
│         │    Actor Registry     │                       │
│         │   Virtual Actors      │                       │
│         └───────────┬───────────┘                       │
│                     │                                   │
│         ┌───────────▼───────────┐                       │
│         │   RocksDB Storage     │                       │
│         │  Persistent LSM-Tree  │                       │
│         └───────────────────────┘                       │
└─────────────────────────────────────────────────────────┘
```

### Component Overview

#### orbit-client

- **Actor Proxies**: Client-side representations of remote actors
- **In-Process Communication**: Zero-overhead local invocations via channels
- **Invocation System**: Message routing and remote procedure calls
- **Lease Management**: Actor lifetime and resource management

#### orbit-shared

- **Data Types**: Common data structures and type definitions
- **Messages**: Inter-actor communication protocols
- **Transactions**: Distributed transaction coordination (2PC, Saga)
- **Clustering**: Raft consensus, leader election
- **Persistence**: Storage backend abstractions and providers

#### orbit-server

- **Multi-Protocol Server**: PostgreSQL, Redis, MySQL, CQL, REST, gRPC
- **Cluster Management**: Node discovery and cluster membership
- **Load Balancer**: Request distribution and resource optimization
- **Health Checks**: System monitoring and failure detection
- **Persistence Providers**: RocksDB, Memory, COW B+Tree, LSM-Tree

#### orbit-engine

- **Storage Engine**: OrbitQL execution and storage adapters
- **Hybrid Storage**: Hot/warm/cold tiering
- **Query Planning**: Query optimization and execution

#### orbit-compute

- **SIMD Operations**: x86-64 AVX-512, ARM64 NEON/SVE
- **GPU Backends**: Metal, CUDA, Vulkan, ROCm
- **Neural Engines**: Apple Neural Engine integration

#### orbit-ml

- **ML Inference**: Model loading and execution
- **Statistical Functions**: Linear regression, correlation, z-score
- **SQL Integration**: ML functions callable from SQL

## Key Advantages

### Memory Safety

- Compile-time elimination of data races and memory errors
- Safe concurrent programming with Rust's ownership model
- No null pointer exceptions or memory leaks
- Zero unsafe code in core modules

### Performance Benefits

- Zero-cost abstractions for high-level programming
- Native performance without virtual machine overhead
- In-process communication eliminates network overhead
- Efficient memory usage and CPU utilization
- 85% memory reduction vs JVM equivalents

### Operational Excellence

- Single binary deployment with minimal dependencies
- Built-in observability and monitoring (100+ Prometheus metrics)
- Comprehensive error handling and debugging support
- Production-ready with enterprise features
- Zero compiler warnings policy

## Use Cases

Orbit-RS is ideal for building:

- **Unified Data Platform**: Replace PostgreSQL + MySQL + Redis + Cassandra with one server
- **AI/ML Applications**: Vector search, embeddings, semantic queries
- **Time Series Analytics**: IoT data, monitoring, real-time analytics
- **Graph Applications**: Social networks, knowledge graphs, recommendation engines
- **Microservices Architectures**: Distributed actor-based services
- **Real-time Applications**: Low-latency message processing
- **Financial Systems**: Transaction processing with ACID guarantees

---

## 📚 Next Steps & Documentation

### 🚀 **Getting Started**

- [🏃 **Quick Start Guide**](quick_start.md) - Get up and running in 30 seconds
- [📖 **Project Overview**](project_overview.md) - Complete project status and capabilities
- [📋 **Product Requirements Document**](PRD.md) - Complete architecture and module reference
- [🎯 **Feature Index**](features.md) - Comprehensive feature overview
- [🤝 **Contributing Guide**](contributing.md) - How to contribute to the project

### 🏗️ **Architecture & Core Systems**

- [🎭 **Virtual Actor Persistence**](content/architecture/virtual_actor_persistence.md) - Actor state management
- [💳 **Advanced Transaction Features**](content/features/advanced_transaction_features.md) - Distributed transactions
- [⚡ **Compute Acceleration Guide**](content/gpu-compute/COMPUTE_ACCELERATION_GUIDE.md) - GPU/Neural acceleration
- [🧠 **AI-Native Subsystems**](PRD.md#ai-native-subsystems) - Intelligent database features

### 🔌 **Protocols & Features**

- [🔴 **Redis Commands**](content/protocols/REDIS_COMMANDS_REFERENCE.md) - 124+ RESP commands
- [🐘 **PostgreSQL Integration**](content/protocols/POSTGRES_WIRE_IMPLEMENTATION.md) - PostgreSQL wire protocol
- [🐬 **MySQL Documentation**](content/protocols/MYSQL_COMPLETE_DOCUMENTATION.md) - MySQL wire protocol
- [📊 **Time Series**](content/server/TIMESERIES_IMPLEMENTATION_SUMMARY.md) - RedisTimeSeries compatibility
- [🔍 **Vector Operations**](content/protocols/vector_commands.md) - AI/ML vector database

### 🚀 **Operations & Deployment**

- [☸️ **Kubernetes Complete Documentation**](content/deployment/KUBERNETES_COMPLETE_DOCUMENTATION.md) - Production K8s setup
- [🔒 **Security Guide**](content/security/SECURITY_COMPLETE_DOCUMENTATION.md) - Security policies and best practices
- [📈 **Monitoring Guide**](content/operations/OPERATIONS_RUNBOOK.md) - Metrics and observability
- [⚙️ **Configuration Reference**](content/deployment/CONFIGURATION.md) - Complete configuration guide

### 🧪 **Client SDKs**

- [🐍 **Python SDK**](../orbit-python-client/) - Python client library
- [💻 **VS Code Extension**](../orbit-vscode-extension/) - Development tools

---

**Orbit-RS: One Server, All Protocols** 🚀
