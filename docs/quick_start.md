---
layout: default
title: "Quick Start Guide"
subtitle: "Get up and running with Orbit-RS in minutes"
category: "getting-started"
---

# Orbit-RS Quick Start Guide

Get up and running with Orbit-RS in just a few minutes.

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

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs
```

### 2. Build the Project

```bash
# Build in release mode for optimal performance
cargo build --release

# Or build in debug mode for development
cargo build
```

### 3. Run Tests

```bash
# Run all tests
cargo test

# Run tests for a specific workspace
cargo test --workspace

# Run tests with output
cargo test -- --nocapture
```

### 4. Verify Installation

```bash
# Check that examples compile
cargo check --examples

# Run a simple example
cargo run --example hello-world
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
    
    println!("🎉 Transaction {} committed successfully!", tx_id);
    Ok(())
}
```

## Running Examples

Orbit-RS includes several examples to demonstrate different features:

### Hello World Example
```bash
cargo run --example hello-world
```

### Distributed Counter Example
```bash
cargo run --example distributed-counter
```

### Redis Protocol Example
```bash
# Start the RESP server
cargo run --example resp-server

# In another terminal, connect with redis-cli
redis-cli -h 127.0.0.1 -p 6380
```

### PostgreSQL Protocol Example
```bash
# Start the PostgreSQL-compatible server
cargo run --example pgvector-store

# In another terminal, connect with psql
psql -h 127.0.0.1 -p 5433 -d orbit
```

## Configuration

### Basic Configuration

Create a configuration file `orbit-config.toml`:

```toml
[cluster]
namespace = "my-app"
port = 8080

[storage]
backend = "sqlite"
connection_string = "orbit.db"

[metrics]
enabled = true
port = 9090
```

### Environment Variables

You can also configure Orbit-RS using environment variables:

```bash
export ORBIT_NAMESPACE="my-app"
export ORBIT_PORT="8080"
export ORBIT_STORAGE_BACKEND="sqlite"
export ORBIT_METRICS_ENABLED="true"
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
```
error: Could not find `protoc` installation
```
**Solution**: Install Protocol Buffers compiler as described in prerequisites.

#### Rust Version Too Old
```
error: package requires Rust 1.70 or newer
```
**Solution**: Update Rust using `rustup update`

#### Port Already in Use
```
error: Address already in use (os error 48)
```
**Solution**: Use a different port or stop the conflicting service.

### Getting Help

- 📖 [Full Documentation](README.md)
- 🐛 [Issue Tracker](https://github.com/TuringWorks/orbit-rs/issues)
- 💬 [Discussions](https://github.com/TuringWorks/orbit-rs/discussions)

## Next Steps

Now that you have Orbit-RS running, explore these advanced features:

- [Transaction Features](features/TRANSACTION_FEATURES.md) - Advanced distributed transactions
- [Protocol Adapters](protocols/PROTOCOL_ADAPTERS.md) - Redis and PostgreSQL compatibility
- [Deployment Guide](deployment/DEPLOYMENT.md) - Production deployment with Kubernetes
- [Development Guide](development/DEVELOPMENT.md) - Contributing to Orbit-RS
- [Project Roadmap](ROADMAP.md) - Future development plans