# Orbit Engine

**A unified storage engine with tiered storage, MVCC transactions, distributed clustering, and protocol adapters for PostgreSQL, Redis, and REST APIs.**

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()

## 🚀 Features

### Storage Layer
- **Tiered Storage**: Automatic data lifecycle management across hot/warm/cold tiers
  - **Hot Tier**: In-memory row-based storage for recent data (last 24-48 hours)
  - **Warm Tier**: Hybrid format for medium-age data (2-30 days)
  - **Cold Tier**: Apache Iceberg columnar storage for historical data (>30 days)
- **Multi-Cloud Support**: S3, Azure Blob Storage, MinIO backends via OpenDAL
- **Columnar Format**: High-performance analytics with Apache Arrow and Parquet
- **SIMD Optimization**: Vectorized query execution with AVX2/AVX-512/NEON support

### Transaction Management
- **MVCC**: Multi-Version Concurrency Control with snapshot isolation
- **ACID Guarantees**: Full transaction support with 2-Phase Commit (2PC)
- **Distributed Locks**: Deadlock detection and resolution
- **Transaction Logging**: SQLite-based persistent transaction log
- **Isolation Levels**: Read Uncommitted, Read Committed, Repeatable Read, Serializable

### Clustering & Distribution
- **Raft Consensus**: Distributed consensus for cluster coordination
- **Replication**: Multi-node replication with configurable replication factor
- **Change Data Capture (CDC)**: Real-time data streaming and event capture
- **Transaction Recovery**: Automatic recovery after node failures
- **Cluster Management**: Dynamic node addition/removal

### Protocol Adapters
- **PostgreSQL**: Full SQL support with wire protocol compatibility
- **Redis (RESP)**: Key-value, hashes, lists, sets with TTL support
- **REST API**: JSON-based RESTful API for HTTP clients
- **Extensible**: Easy to add new protocol adapters (OrbitQL, AQL, Cypher, etc.)

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
orbit-engine = { path = "orbit/engine" }
tokio = { version = "1", features = ["full"] }
```

## 🎯 Quick Start

### Basic Usage with PostgreSQL Adapter

```rust
use orbit_engine::adapters::{AdapterContext, PostgresAdapter};
use orbit_engine::adapters::postgres::{PostgresColumnDef, PostgresDataType};
use orbit_engine::storage::HybridStorageManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage engine
    let storage = Arc::new(HybridStorageManager::new_in_memory());

    // Create adapter context
    let context = AdapterContext::new(
        storage as Arc<dyn orbit_engine::storage::TableStorage>
    );

    // Create PostgreSQL adapter
    let adapter = PostgresAdapter::new(context);

    // Create a table
    adapter.create_table(
        "users",
        vec![
            PostgresColumnDef {
                name: "id".to_string(),
                data_type: PostgresDataType::Integer,
                nullable: false,
            },
            PostgresColumnDef {
                name: "name".to_string(),
                data_type: PostgresDataType::Text,
                nullable: false,
            },
        ],
        vec!["id".to_string()],
    ).await?;

    println!("Table created successfully!");
    Ok(())
}
```

### Redis Adapter Example

```rust
use orbit_engine::adapters::{AdapterContext, RedisAdapter, ProtocolAdapter};
use orbit_engine::storage::HybridStorageManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = Arc::new(HybridStorageManager::new_in_memory());
    let context = AdapterContext::new(
        storage as Arc<dyn orbit_engine::storage::TableStorage>
    );

    let mut adapter = RedisAdapter::new(context);
    adapter.initialize().await?;

    // String operations
    adapter.set("mykey", "myvalue", None).await?;
    let value = adapter.get("mykey").await?;
    println!("Value: {:?}", value);

    // Hash operations
    adapter.hset("user:1000", "name", "Alice").await?;
    adapter.hset("user:1000", "email", "alice@example.com").await?;
    let fields = adapter.hgetall("user:1000").await?;
    println!("User fields: {:?}", fields);

    Ok(())
}
```

### REST API Example

```rust
use orbit_engine::adapters::{AdapterContext, RestAdapter};
use orbit_engine::adapters::rest::{CreateTableRequest, RestColumnDef};
use orbit_engine::storage::HybridStorageManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = Arc::new(HybridStorageManager::new_in_memory());
    let context = AdapterContext::new(
        storage as Arc<dyn orbit_engine::storage::TableStorage>
    );

    let adapter = RestAdapter::new(context);

    // Create table via REST API
    let request = CreateTableRequest {
        name: "products".to_string(),
        columns: vec![
            RestColumnDef {
                name: "id".to_string(),
                data_type: "integer".to_string(),
                nullable: false,
            },
            RestColumnDef {
                name: "name".to_string(),
                data_type: "string".to_string(),
                nullable: false,
            },
        ],
        primary_key: vec!["id".to_string()],
    };

    let response = adapter.create_table_request(request).await?;
    println!("Response: {:?}", response);

    Ok(())
}
```

## 📚 Examples

Run the complete example showcasing all features:

```bash
cargo run --example complete_example
```

Run the protocol adapters example:

```bash
cargo run --example protocol_adapters
```

## 🏗️ Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│         Protocol Layer (PostgreSQL, RESP, REST, etc.)       │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   Protocol Adapters                          │
│   PostgreSQL │ Redis │ REST │ OrbitQL │ AQL │ Cypher        │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Orbit Engine Core                         │
│  ┌──────────┐  ┌────────────┐  ┌──────────┐  ┌──────────┐ │
│  │ Storage  │  │Transaction │  │  Query   │  │ Cluster  │ │
│  │ Hot/Warm │  │    MVCC    │  │Vectorized│  │   Raft   │ │
│  │   Cold   │  │    2PC     │  │   SIMD   │  │Replicate │ │
│  └──────────┘  └────────────┘  └──────────┘  └──────────┘ │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│         Storage Backends (S3, Azure Blob, MinIO)            │
└─────────────────────────────────────────────────────────────┘
```

## 🔧 Configuration

### Storage Backends

#### S3 Configuration
```rust
use orbit_engine::storage::{StorageBackend, S3Config};

let backend = StorageBackend::S3(S3Config {
    endpoint: "s3.amazonaws.com".to_string(),
    region: "us-west-2".to_string(),
    bucket: "my-bucket".to_string(),
    access_key: "ACCESS_KEY".to_string(),
    secret_key: "SECRET_KEY".to_string(),
});
```

#### Azure Blob Storage Configuration
```rust
use orbit_engine::storage::{StorageBackend, AzureConfig};

let backend = StorageBackend::Azure(AzureConfig {
    account: "myaccount".to_string(),
    container: "mycontainer".to_string(),
    access_key: "ACCESS_KEY".to_string(),
});
```

### Transaction Configuration

```rust
use orbit_engine::transaction::IsolationLevel;

// Begin transaction with specific isolation level
let tx_id = tx_manager.begin(IsolationLevel::Serializable).await?;

// Perform operations...

// Commit
tx_manager.commit(tx_id).await?;
```

## 📊 Performance

### Tiered Storage Benefits

| Tier | Storage | Access Time | Cost | Use Case |
|------|---------|-------------|------|----------|
| Hot  | Memory  | < 1ms       | High | Recent data, OLTP |
| Warm | SSD (RocksDB) | < 10ms | Medium | Medium-age data |
| Cold | Object Storage | 10-100ms | Low | Historical analytics |

### SIMD Optimization

Vectorized query execution provides:
- **5-10x** faster aggregations (SUM, AVG, COUNT)
- **3-5x** faster filtering operations
- **2-3x** better compression ratios with Parquet

## 🧪 Testing

Run all tests:

```bash
cargo test
```

Run integration tests:

```bash
cargo test --features integration-tests
```

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **Apache Iceberg** - Table format for cold tier storage
- **Apache Arrow** - In-memory columnar data format
- **OpenDAL** - Unified data access layer for multi-cloud
- **RocksDB** - Embedded key-value store for warm tier
- **Raft** - Consensus algorithm for distributed clustering

## 📖 Documentation

- [API Documentation](https://docs.rs/orbit-engine)
- [Architecture Guide](docs/ARCHITECTURE.md)
- [Protocol Adapter Development](docs/ADAPTERS.md)
- [Deployment Guide](docs/DEPLOYMENT.md)

## 🗺️ Roadmap

- [x] Tiered storage (hot/warm/cold)
- [x] MVCC transactions
- [x] Distributed clustering (Raft)
- [x] Protocol adapters (PostgreSQL, Redis, REST)
- [ ] GraphQL adapter
- [ ] gRPC adapter
- [ ] Full-text search integration
- [ ] Time-series optimizations
- [ ] Geo-spatial indexing
- [ ] ML model integration

## 💬 Support

- **Issues**: [GitHub Issues](https://github.com/orbit-rs/orbit/issues)
- **Discussions**: [GitHub Discussions](https://github.com/orbit-rs/orbit/discussions)

---

**Built with ❤️ by the Orbit team**
