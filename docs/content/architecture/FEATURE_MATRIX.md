# Orbit-RS Feature Matrix and Configuration Guide

**Last Updated**: November 23, 2025
**Version**: 1.0.0

## Overview

This document provides a comprehensive matrix of all Orbit-RS features, their implementation status, dependencies, and configuration options through Cargo feature flags. Use this guide to build custom configurations optimized for specific use cases.

---

## Quick Reference

### Build Configurations

```bash
# Minimal build (core actor system only)
cargo build --no-default-features

# Default build (all production-ready features)
cargo build

# Full build (all features including experimental)
cargo build --all-features

# Custom build (specific features)
cargo build --features "postgres redis ai-native"
```

---

## Feature Categories

### 1. Core System Features

| Feature Flag | Status | Default | Description | Dependencies |
|-------------|---------|---------|-------------|--------------|
| `actor-system` | ✅ Complete | Yes | Virtual actor system with persistence | - |
| `clustering` | ✅ Complete | Yes | Distributed cluster management | `actor-system` |
| `persistence` | ✅ Complete | Yes | Actor state persistence | `actor-system` |
| `networking` | ✅ Complete | Yes | gRPC and internal networking | - |
| `metrics` | ✅ Complete | Yes | Prometheus metrics collection | - |

**Build Example**:
```bash
# Core only (no protocols)
cargo build --no-default-features --features "actor-system,clustering"
```

---

### 2. Protocol Support

| Feature Flag | Status | Default | Description | Protocol Port | Dependencies |
|-------------|---------|---------|-------------|--------------|--------------|
| `protocol-redis` | ✅ Complete | Yes | Redis RESP protocol | 6379 | `actor-system` |
| `protocol-postgres` | ✅ Complete | Yes | PostgreSQL wire protocol | 5432 | `actor-system` |
| `protocol-mysql` | ⚠️ Partial | No | MySQL wire protocol | 3306 | `actor-system` |
| `protocol-cassandra` | ⚠️ Partial | No | CQL/Cassandra protocol | 9042 | `actor-system` |
| `protocol-neo4j` | ⚠️ Partial | No | Cypher/Bolt protocol | 7687 | `actor-system,graph` |
| `protocol-arangodb` | ⚠️ Partial | No | AQL protocol | 8529 | `actor-system,graph` |
| `protocol-grpc` | ✅ Complete | Yes | gRPC actor management | 50051 | `networking` |
| `protocol-rest` | ✅ Complete | Yes | HTTP/REST API | 8080 | `networking` |
| `protocol-mcp` | ✅ Complete | Yes | Model Context Protocol | - | `networking` |

**Build Example**:
```bash
# PostgreSQL + Redis only
cargo build --features "protocol-postgres,protocol-redis"

# Graph database focus
cargo build --features "protocol-neo4j,protocol-arangodb,graph"
```

---

### 3. Storage Engines

| Feature Flag | Status | Default | Description | Use Case | Dependencies |
|-------------|---------|---------|-------------|----------|--------------|
| `storage-memory` | ✅ Complete | Yes | In-memory storage | Development, testing | - |
| `storage-rocksdb` | ✅ Complete | Yes | RocksDB LSM-tree | Production | - |
| `storage-lsm` | ✅ Complete | No | Custom LSM implementation | Specialized workloads | - |
| `storage-btree` | ✅ Complete | No | COW B+Tree | Concurrent reads | - |
| `storage-iceberg` | ✅ Complete | No | Apache Iceberg (cold tier) | Data lake integration | - |
| `storage-cloud` | ✅ Complete | No | Cloud storage (S3/Azure/GCP/DO Spaces) | Cloud deployments | - |

**Build Example**:
```bash
# Production with RocksDB + Iceberg
cargo build --features "storage-rocksdb,storage-iceberg"

# Cloud deployment with multi-provider support
cargo build --features "storage-cloud,storage-iceberg"
```

---

### 4. AI-Native Database Features

| Feature Flag | Status | Default | Description | Dependencies |
|-------------|---------|---------|-------------|--------------|
| `ai-native` | ✅ Complete | Yes | Enable all AI features | `ai-*` |
| `ai-query-optimizer` | ✅ Complete | Yes | Intelligent query optimization | `actor-system` |
| `ai-resource-manager` | ✅ Complete | Yes | Predictive resource management | `metrics` |
| `ai-storage-manager` | ✅ Complete | Yes | Smart storage tiering | `persistence` |
| `ai-transaction-manager` | ✅ Complete | Yes | Adaptive transaction management | `actor-system` |
| `ai-learning-engine` | ✅ Complete | Yes | Continuous learning system | `ai-*` |
| `ai-decision-engine` | ✅ Complete | Yes | Autonomous decision making | `ai-*` |
| `ai-knowledge-base` | ✅ Complete | Yes | Pattern storage and retrieval | `persistence` |

**Build Example**:
```bash
# With AI features (default)
cargo build --features "ai-native"

# Without AI features (reduced footprint)
cargo build --no-default-features --features "actor-system,protocol-redis"
```

---

### 5. Data Model Support

| Feature Flag | Status | Default | Description | Dependencies |
|-------------|---------|---------|-------------|--------------|
| `model-relational` | ✅ Complete | Yes | Relational/SQL support | - |
| `model-document` | ✅ Complete | Yes | JSON document storage | - |
| `model-kv` | ✅ Complete | Yes | Key-value operations | - |
| `model-graph` | ✅ Complete | Yes | Graph database operations | - |
| `model-timeseries` | ✅ Complete | Yes | Time-series analytics | - |
| `model-vector` | ✅ Complete | Yes | Vector/embeddings storage | - |
| `model-spatial` | ✅ Complete | Yes | Geospatial operations | - |
| `model-columnar` | ✅ Complete | Yes | Columnar analytics | - |

**Build Example**:
```bash
# Time-series + Vector focus
cargo build --features "model-timeseries,model-vector"

# Graph database only
cargo build --features "model-graph,protocol-neo4j"
```

---

### 6. Advanced Features

| Feature Flag | Status | Default | Description | Dependencies |
|-------------|---------|---------|-------------|--------------|
| `transactions` | ✅ Complete | Yes | ACID transactions | `actor-system` |
| `distributed-tx` | ✅ Complete | Yes | 2PC distributed transactions | `transactions,clustering` |
| `sagas` | ✅ Complete | Yes | Saga pattern workflows | `transactions` |
| `cdc` | ✅ Complete | Yes | Change data capture | `actor-system` |
| `streaming` | ✅ Complete | Yes | Real-time stream processing | `actor-system` |
| `replication` | ✅ Complete | Yes | Data replication | `clustering` |
| `sharding` | ⚠️ Partial | No | Automatic data sharding | `clustering` |
| `compression` | ✅ Complete | Yes | Data compression | - |
| `encryption` | ⚠️ Partial | No | At-rest encryption | - |
| `gpu-graph-traversal` | ✅ Complete | Yes | GPU-accelerated BFS/DFS | `heterogeneous-compute,model-graph` |
| `gpu-vector-similarity` | ✅ Complete | Yes | GPU-accelerated vector similarity search | `gpu-acceleration,model-vector` |
| `gpu-spatial-operations` | ✅ Complete | Yes | GPU-accelerated spatial distance calculations | `gpu-acceleration,model-spatial` |
| `gpu-columnar-analytics` | ✅ Complete | Yes | GPU-accelerated columnar aggregations | `gpu-acceleration,model-columnar` |
| `gpu-timeseries-operations` | ✅ Complete | Yes | CPU-parallel time-series operations | `gpu-acceleration,model-timeseries` |
| `gpu-columnar-joins` | ✅ Complete | Yes | CPU-parallel columnar hash joins | `gpu-acceleration,model-columnar` |

**Build Example**:
```bash
# Full transactional system
cargo build --features "transactions,distributed-tx,sagas"

# Streaming analytics
cargo build --features "streaming,cdc,model-timeseries"
```

---

### 7. Integration Features

| Feature Flag | Status | Default | Description | Dependencies |
|-------------|---------|---------|-------------|--------------|
| `kubernetes` | ✅ Complete | Yes | K8s operator and CRDs | `clustering` |
| `prometheus` | ✅ Complete | Yes | Prometheus metrics export | `metrics` |
| `etcd` | ✅ Complete | Yes | etcd integration | `clustering` |
| `graphrag` | ✅ Complete | Yes | Graph RAG for LLMs | `model-graph` |
| `neural-engine` | 🚧 Experimental | No | Hardware acceleration | - |
| `heterogeneous-compute` | ✅ Complete | Yes | Multi-device compute (CPU/GPU/Neural) | `neural-engine` |
| `gpu-acceleration` | ✅ Complete | Yes | GPU compute acceleration (Metal/Vulkan) | `heterogeneous-compute` |

**Build Example**:
```bash
# Kubernetes deployment
cargo build --features "kubernetes,prometheus,etcd"

# AI/ML workloads

# GPU-accelerated graph operations
cargo build --features "gpu-acceleration,gpu-graph-traversal,model-graph"
cargo build --features "graphrag,model-vector,ai-native"
```

---

### 8. Query Languages

| Feature Flag | Status | Default | Description | Dependencies |
|-------------|---------|---------|-------------|--------------|
| `query-sql` | ✅ Complete | Yes | SQL query support | `model-relational` |
| `query-cypher` | ✅ Complete | Yes | Cypher graph queries | `model-graph` |
| `query-aql` | ✅ Complete | Yes | ArangoDB Query Language | `model-graph` |
| `query-orbitql` | ✅ Complete | Yes | Native OrbitQL | - |
| `query-redis` | ✅ Complete | Yes | Redis commands | `protocol-redis` |
| `query-natural` | ✅ Complete | Yes | Natural language (MCP) | `protocol-mcp` |

---

### 9. Development & Testing

| Feature Flag | Status | Default | Description | Dependencies |
|-------------|---------|---------|-------------|--------------|
| `benchmarks` | ✅ Complete | No | Performance benchmarks | - |
| `testing` | ✅ Complete | No | Test utilities | - |
| `tracing` | ✅ Complete | Yes | Distributed tracing | `metrics` |
| `debug-tools` | ✅ Complete | No | Debug utilities | - |

---

## Feature Status Legend

- ✅ **Complete**: Production-ready, fully tested
- ⚠️ **Partial**: Partially implemented, may have limitations
- 🚧 **Experimental**: Under development, not production-ready
- 🔜 **Planned**: On roadmap, not yet implemented

---

## Default Features

The following features are enabled by default:

```toml
default = [
    "actor-system",
    "clustering",
    "persistence",
    "networking",
    "metrics",
    "protocol-redis",
    "protocol-postgres",
    "protocol-grpc",
    "protocol-rest",
    "protocol-mcp",
    "storage-memory",
    "storage-rocksdb",
    "ai-native",
    "model-relational",
    "model-document",
    "model-kv",
    "model-graph",
    "model-timeseries",
    "model-vector",
    "model-spatial",
    "model-columnar",
    "transactions",
    "distributed-tx",
    "sagas",
    "cdc",
    "streaming",
    "replication",
    "compression",
    "query-sql",
    "query-cypher",
    "query-aql",
    "query-orbitql",
    "query-redis",
    "query-natural",
    "kubernetes",
    "prometheus",
    "etcd",
    "graphrag",
    "gpu-graph-traversal",
    "gpu-acceleration",
    "heterogeneous-compute",
    "tracing",
]
```

---

## Common Build Configurations

### Minimal Configuration (Core Only)
```bash
cargo build --no-default-features --features "actor-system"
# Binary size: ~5MB
# Use case: Embedded systems, minimal footprint
```

### Redis-Compatible Server
```bash
cargo build --no-default-features --features "actor-system,protocol-redis,storage-rocksdb,model-kv"
# Binary size: ~15MB
# Use case: Redis replacement
```

### PostgreSQL-Compatible Database
```bash
cargo build --no-default-features --features "actor-system,protocol-postgres,storage-rocksdb,model-relational,transactions,query-sql"
# Binary size: ~25MB
# Use case: PostgreSQL alternative
```

### Graph Database
```bash
cargo build --features "protocol-neo4j,model-graph,query-cypher,storage-rocksdb"
# Use case: Neo4j alternative
```

### AI-Native Database (Full)
```bash
cargo build --features "ai-native,protocol-postgres,protocol-redis,model-relational,model-vector"
# Use case: AI-powered database with multiple protocols
```

### Time-Series Analytics
```bash
cargo build --features "model-timeseries,streaming,cdc,compression,storage-rocksdb,storage-iceberg"
# Use case: Time-series data platform
```

### Kubernetes Deployment
```bash
cargo build --features "kubernetes,prometheus,etcd,clustering,replication"
# Use case: Cloud-native deployment
```

---

## Performance Impact

### Feature Impact on Binary Size

| Configuration | Binary Size | Memory Overhead |
|--------------|-------------|-----------------|
| Minimal (core only) | ~5 MB | ~20 MB |
| Redis-compatible | ~15 MB | ~50 MB |
| PostgreSQL-compatible | ~25 MB | ~100 MB |
| Full (all features) | ~50 MB | ~200 MB |

### Feature Impact on Runtime Performance

| Feature Category | CPU Overhead | Memory Overhead | I/O Impact |
|-----------------|--------------|-----------------|------------|
| AI-Native Features | 5-10% | 50-100 MB | Minimal |
| Clustering | 2-5% | 20-50 MB | Network dependent |
| Transactions | 1-3% | 10-20 MB | Disk dependent |
| Streaming/CDC | 3-7% | 30-60 MB | Moderate |

---

## Feature Dependencies Graph

```text
┌─────────────────────────────────────────────────┐
│               Actor System (Core)               │
└─────────────────────┬───────────────────────────┘
                        │
        ┌───────────────┼───────────────┐
        ▼               ▼               ▼
   ┌────────-─┐    ┌────────-──┐   ┌──────────┐
   │Clustering│    │Persistence│   │Networking│
   └────┬───-─┘    └─────┬─────┘   └────┬─────┘
        │                │              │
        ├─────--──┬───-──┼───────┬──-───┤
        ▼         ▼      ▼       ▼      ▼
    ┌────-────┐ ┌───┐ ┌──-──┐ ┌────┐ ┌─────┐
    │Protocols│ │AI │ │Data │ │Txn │ │Query│
    │         │ │   │ │Model│ │    │ │Lang │
    └─────────┘ └───┘ └────-┘ └────┘ └─────┘
```

---

## Troubleshooting

### Common Issues

**Issue**: Build fails with "feature not enabled"
- **Solution**: Add the required feature flag: `cargo build --features "feature-name"`

**Issue**: Binary size too large
- **Solution**: Use `--no-default-features` and only enable needed features

**Issue**: Missing protocol support at runtime
- **Solution**: Rebuild with protocol feature flags enabled

**Issue**: AI features not working
- **Solution**: Ensure `ai-native` feature is enabled (on by default)

---

## Migration Guide

### Upgrading from Pre-Feature-Flag Versions

If upgrading from a version before feature flags:

1. **Default behavior unchanged**: All previously available features are enabled by default
2. **Opt-out approach**: Use `--no-default-features` to disable unwanted features
3. **No code changes required**: Feature flags only affect compilation

### Example Migration

**Before** (all features always compiled):
```bash
cargo build
```

**After** (same result, all features):
```bash
cargo build  # Default features (same as before)
```

**After** (optimized build):
```bash
cargo build --no-default-features --features "actor-system,protocol-redis,storage-rocksdb"
```

---

## Feature Roadmap

### Upcoming Features

| Feature | Target Version | Status |
|---------|---------------|--------|
| `encryption-at-rest` | 1.1.0 | 🚧 In Progress |
| `auto-sharding` | 1.1.0 | 🔜 Planned |
| `multi-region` | 1.2.0 | 🔜 Planned |
| `query-optimizer-v2` | 1.2.0 | 🔜 Planned |
| `protocol-mongodb` | 1.3.0 | 🔜 Planned |
| `gpu-cuda` | 1.1.0 | 🔜 Planned | NVIDIA CUDA support for graph traversal |
| `gpu-vector-similarity` | 1.0.0 | ✅ Complete | GPU-accelerated vector similarity (50-200x speedup) |
| `gpu-spatial-operations` | 1.0.0 | ✅ Complete | GPU-accelerated spatial distance (20-100x speedup) |
| `gpu-columnar-analytics` | 1.0.0 | ✅ Complete | GPU-accelerated columnar aggregations (20-100x speedup) |
| `vulkan-graph-shaders` | 1.1.0 | 🚧 In Progress | SPIR-V compilation for Vulkan shaders |

---

## Runtime Feature Detection

Orbit-RS provides a runtime feature detection API through the `Features` module:

```rust
use orbit_server::Features;

// Create features instance
let features = Features::new();

// Check if specific features are enabled
if features.has_ai_native() {
    println!("AI-native features are enabled");
}

if features.has_redis() {
    println!("Redis protocol support is available");
}

// Get list of all enabled features
let enabled = features.enabled();
println!("Enabled features: {:?}", enabled);

// Check feature count
println!("Total enabled features: {}", features.count());

// Check specific feature by name
if features.is_enabled("graphrag") {
    println!("GraphRAG is available");
}
```

### Available Methods

- `has_actor_system()` - Check if actor system is enabled
- `has_clustering()` - Check if clustering is enabled
- `has_persistence()` - Check if persistence is enabled
- `has_ai_native()` - Check if AI-native features are enabled
- `has_redis()` - Check if Redis protocol is enabled
- `has_postgres()` - Check if PostgreSQL protocol is enabled
- `has_transactions()` - Check if transactions are enabled
- `has_distributed_tx()` - Check if distributed transactions are enabled
- `has_graphrag()` - Check if GraphRAG is enabled
- `has_gpu_acceleration()` - Check if GPU acceleration is enabled
- `has_gpu_graph_traversal()` - Check if GPU graph traversal is enabled
- `is_enabled(name)` - Check if a named feature is enabled
- `enabled()` - Get list of all enabled features
- `count()` - Get count of enabled features

---

## Support and Documentation

- **Feature Requests**: [GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues)
- **Build Documentation**: [docs/deployment/DEPLOYMENT.md](../deployment/DEPLOYMENT.md)
- **Architecture Guide**: [ORBIT_ARCHITECTURE.md](ORBIT_ARCHITECTURE.md)
- **Performance Tuning**: [docs/operations/PERFORMANCE_TUNING.md](../operations/PERFORMANCE_TUNING.md)

---

**This feature matrix is maintained as part of the Orbit-RS project. For the latest updates, see the GitHub repository.**
