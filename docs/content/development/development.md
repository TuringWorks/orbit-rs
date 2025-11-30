---
layout: default
title: Development Guide
category: development
---

## Development Guide

This guide covers development setup, contributing guidelines, testing procedures, and architectural considerations for Orbit-RS development.

## Development Environment Setup

### Prerequisites

- **Rust 1.70+** - [Install via rustup](https://rustup.rs/)
- **Protocol Buffers compiler (`protoc`)** - Required for gRPC code generation
- **Git** - Version control system
- **IDE/Editor** with Rust support (VS Code, IntelliJ IDEA, or Neovim)
- **CMake** - Required for building RocksDB native dependencies
- **Clang/LLVM** - C++ compiler for native dependencies

### Setting Up the Development Environment

```bash

# Clone the repository
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs

# Install system dependencies (macOS)
brew install protobuf cmake llvm

# Install system dependencies (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install -y protobuf-compiler cmake clang

# Install Rust toolchain components
rustup component add clippy rustfmt

# Install development tools
cargo install cargo-audit
cargo install cargo-deny
cargo install cargo-tarpaulin  # For code coverage

# Build all modules (including persistence backends)
cargo build --workspace

# Run tests to verify setup
cargo test --workspace

# Verify persistence modules specifically
cargo test --lib --package orbit-server
```

### IDE Configuration

#### Visual Studio Code

Recommended extensions:

- **rust-analyzer** - Rust language server with excellent IntelliSense
- **CodeLLDB** - Debugger support for Rust
- **crates** - Cargo dependency management
- **Better TOML** - TOML file syntax highlighting
- **GitLens** - Enhanced Git capabilities

VS Code settings (`.vscode/settings.json`):

```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.checkOnSave.extraArgs": ["--", "-D", "warnings"],
    "rust-analyzer.cargo.buildScripts.enable": true,
    "files.watcherExclude": {
        "**/target/**": true
    }
}
```

#### IntelliJ IDEA

1. Install the **Rust** plugin
2. Configure the Rust toolchain in **Settings → Languages & Frameworks → Rust**
3. Enable **Use Clippy instead of Cargo Check**
4. Configure **Code Style → Rust** to use rustfmt

### Development Commands

```bash

# Fast compilation check (no codegen) - includes persistence backends
cargo check --workspace

# Full compilation with all features
cargo build --workspace --all-features

# Release build with optimizations
cargo build --workspace --release

# Run all tests including persistence layer
cargo test --workspace

# Test specific modules
cargo test --lib --package orbit-server  # Persistence modules
cargo test --lib --package orbit-shared  # Core types

# Code formatting
cargo fmt --all

# Linting (persistence modules now pass)
cargo clippy --workspace --all-targets

# Security audit
cargo audit

# License and dependency checking
cargo deny check

# Generate documentation
cargo doc --workspace --open

# Run persistence benchmarks
cargo run --package orbit-benchmarks

# Run protocol-specific benchmarks
cargo bench --workspace
```

## Project Structure

### Workspace Layout

```text
orbit-rs/
├── Cargo.toml              # Workspace configuration
├── README.md               # Project overview
├── LICENSE-MIT             # MIT license
├── LICENSE-BSD             # BSD license
│
├── orbit-util/             # Utilities and base functionality
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── config.rs       # Configuration management
│       ├── logging.rs      # Logging utilities
│       └── metrics.rs      # Metrics utilities
│
├── orbit-shared/           # Shared data structures and types
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── actor.rs        # Actor trait definitions
│       ├── error.rs        # Error types
│       ├── key.rs          # Actor key types
│       └── transactions/   # Transaction system
│           ├── mod.rs
│           ├── coordinator.rs
│           ├── participant.rs
│           └── saga.rs
│
├── orbit-proto/            # Protocol Buffer definitions
│   ├── Cargo.toml
│   ├── build.rs           # Proto compilation
│   ├── proto/
│   │   ├── actor.proto    # Actor service definitions
│   │   ├── cluster.proto  # Cluster management
│   │   └── transaction.proto
│   └── src/
│       ├── lib.rs
│       └── generated/     # Generated gRPC code
│
├── orbit-client/           # Client-side actor system
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── client.rs      # Main client implementation
│       ├── proxy.rs       # Actor proxy generation
│       └── lease.rs       # Lease management
│
├── orbit-server/           # Server-side cluster management
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── server.rs      # Main server implementation
│       ├── cluster.rs     # Cluster management
│       ├── load_balancer.rs
│       └── persistence/   # Multiple storage backends
│           ├── mod.rs     # Persistence traits and config
│           ├── memory.rs  # In-memory provider
│           ├── cow_btree.rs  # COW B+Tree provider
│           ├── lsm_tree.rs   # LSM-Tree provider
│           └── rocksdb.rs    # RocksDB provider
│
├── orbit-protocols/        # Protocol adapters
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── redis/         # Redis RESP protocol
│       ├── postgres_wire/ # PostgreSQL wire protocol
│       └── mcp/          # Model Context Protocol
│
├── orbit-operator/         # Kubernetes operator
│   ├── Cargo.toml
│   ├── deploy/            # Kubernetes manifests
│   └── src/
│       ├── main.rs
│       ├── controller.rs
│       └── resources/
│

│   ├── hello-world/       # Basic actor example
│   ├── distributed-counter/  # Multi-actor coordination
│   └── tests/
│       ├── features/      # BDD scenarios
│       └── tests/         # Integration tests
│
├── docs/                   # Documentation
│   ├── README.md          # Documentation index
│   ├── OVERVIEW.md        # Project overview
│   ├── QUICK_START.md     # Quick start guide
│   ├── features/          # Feature documentation
│   ├── protocols/         # Protocol documentation
│   ├── deployment/        # Deployment guides
│   └── development/       # Development guides
│
└── scripts/                # Development scripts
    ├── prepare-secrets.sh  # CI/CD secret preparation
    ├── build-docker.sh     # Docker image building
    └── run-benchmarks.sh   # Benchmark execution
```

### Architectural Layers

#### Core Layer (`orbit-shared`, `orbit-util`)

- Fundamental types and utilities
- Actor trait definitions
- Error handling
- Configuration management
- Logging and metrics utilities

#### Network Layer (`orbit-proto`)

- Protocol Buffer definitions
- gRPC service specifications
- Message serialization/deserialization
- Cross-language type definitions

#### Client Layer (`orbit-client`)

- Actor proxy generation
- Remote invocation handling
- Lease management
- Connection pooling

#### Server Layer (`orbit-server`)

- Cluster node management
- Actor lifecycle management
- Load balancing
- Health monitoring

#### Protocol Layer (`orbit-protocols`)

- Redis RESP protocol adapter
- PostgreSQL wire protocol adapter
- SQL parser and executor
- MCP server implementation

#### Orchestration Layer (`orbit-operator`)

- Kubernetes Custom Resource Definitions
- Operator controller logic
- Resource management
- Deployment automation

## Persistence Development

### Storage Backends

Orbit-RS supports multiple pluggable storage backends:

#### In-Memory Provider (`memory.rs`)

- Hash-map based storage for development and testing
- No persistence across restarts
- Fastest performance for temporary data

#### COW B+Tree Provider (`cow_btree.rs`)

- Copy-on-Write B+ Tree with Write-Ahead Logging
- Optimized for read-heavy workloads
- Snapshots and versioning support

#### LSM-Tree Provider (`lsm_tree.rs`)

- Log-Structured Merge Tree implementation
- Optimized for write-heavy workloads
- Background compaction and bloom filters

#### RocksDB Provider (`rocksdb.rs`)

- Production-grade key-value store
- ACID transactions and crash recovery
- High-performance with tunable parameters

### Persistence Configuration

```rust
use orbit_server::persistence::*;

// Configure persistence backend
let config = PersistenceConfig {
    backend: BackendType::RocksDB,
    rocksdb: Some(RocksDbConfig {
        data_dir: "./orbit_data".to_string(),
        enable_wal: true,
        block_cache_size: 256 * 1024 * 1024, // 256MB
        ..Default::default()
    }),
    ..Default::default()
};

let provider = DynamicPersistenceProvider::new(config).await?;
```

### Adding New Storage Backends

1. **Implement Required Traits**:

   ```rust
   impl PersistenceProvider for MyCustomProvider {
       async fn initialize(&self) -> OrbitResult<()> { /* ... */ }
       async fn shutdown(&self) -> OrbitResult<()> { /* ... */ }
       // ... other methods
   }
   
   impl AddressableDirectoryProvider for MyCustomProvider {
       async fn store_lease(&self, lease: &AddressableLease) -> OrbitResult<()> { /* ... */ }
       // ... other methods
   }
   ```

2. **Add Configuration Support**:

   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MyCustomConfig {
       pub connection_string: String,
       pub pool_size: usize,
   }
   ```

3. **Register Backend**:

   ```rust
   // In persistence/mod.rs
   pub enum BackendType {
       Memory,
       CowBTree,
       LsmTree,
       RocksDB,
       MyCustom, // Add here
   }
   ```

4. **Write Tests**:

   ```rust
   #[tokio::test]
   async fn test_my_custom_provider() {
       let config = MyCustomConfig { /* ... */ };
       let provider = MyCustomProvider::new(config)?;
       
       provider.initialize().await?;
       
       // Test CRUD operations
       let lease = AddressableLease { /* ... */ };
       provider.store_lease(&lease).await?;
       
       let retrieved = provider.get_lease(&lease.reference).await?;
       assert_eq!(retrieved.unwrap(), lease);
   }
   ```

### Persistence Testing

```bash

# Test all persistence backends
cargo test --lib --package orbit-server persistence

# Test specific backend
cargo test --lib --package orbit-server persistence::rocksdb

# Benchmark persistence performance
cargo run --package orbit-benchmarks

# Test with different configurations
ORBIT_PERSISTENCE_BACKEND=rocksdb cargo test persistence_integration
```

## Testing Framework

### Test Structure

```text
tests/
├── unit/                  # Unit tests (in src/ directories)
├── integration/           # Integration tests
│   ├── actor_lifecycle.rs
│   ├── cluster_management.rs
│   └── transaction_tests.rs
├── bdd/                   # Behavior-driven tests
│   ├── features/
│   │   ├── actor_lifecycle.feature
│   │   └── messaging.feature
│   └── step_definitions/
└── benchmarks/            # Performance benchmarks
    ├── actor_throughput.rs
    └── transaction_latency.rs
```

### Running Tests

```bash

# All tests
cargo test --workspace

# Unit tests only
cargo test --workspace --lib

# Integration tests
cargo test --workspace --test integration

# BDD scenarios
cargo test --workspace --test bdd

# Specific test module
cargo test --package orbit-client --test client_tests

# Tests with output
cargo test --workspace -- --nocapture

# Parallel test execution
cargo test --workspace --jobs 4
```

### Test Configuration

Test configuration in `Cargo.toml`:

```toml
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.11"
cucumber = "0.19"
criterion = "0.5"

[[test]]
name = "integration"
path = "tests/integration/mod.rs"
required-features = ["integration-tests"]

[[bench]]
name = "actor_throughput"
harness = false
```

### Writing Tests

#### Unit Test Example

```rust

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_actor_invocation() {
        let client = OrbitClient::builder()
            .with_namespace("test")
            .build()
            .await
            .unwrap();

        let actor = client.actor_reference::<dyn TestActor>(
            Key::StringKey { key: "test-actor".to_string() }
        ).await.unwrap();

        let result = actor.test_method("input").await.unwrap();
        assert_eq!(result, "expected_output");
    }
}
```

#### Integration Test Example

```rust
use orbit_client::OrbitClient;
use orbit_server::OrbitServer;
use std::time::Duration;

#[tokio::test]
async fn test_cluster_communication() {
    // Start server
    let server = OrbitServer::builder()
        .with_port(8080)
        .build()
        .await
        .unwrap();

    // Start client
    let client = OrbitClient::builder()
        .with_endpoint("http://localhost:8080")
        .build()
        .await
        .unwrap();

    // Test communication
    // ... test logic
}
```

#### BDD Test Example

Feature file (`tests/features/actor_lifecycle.feature`):

```gherkin
Feature: Actor Lifecycle Management
  
  Scenario: Actor activation and deactivation
    Given a cluster with 3 nodes
    When I request an actor "test-actor"
    Then the actor should be activated on one node
    And the actor should respond to messages
    When the actor is idle for 5 minutes
    Then the actor should be deactivated
```

Step definitions:

```rust
use cucumber::{given, when, then, World};

#[derive(Debug, Default, World)]
pub struct ActorWorld {
    cluster: Option<TestCluster>,
    actor: Option<ActorReference>,
}

#[given("a cluster with {int} nodes")]
async fn given_cluster(world: &mut ActorWorld, node_count: i32) {
    world.cluster = Some(TestCluster::new(node_count as usize).await);
}

#[when("I request an actor {string}")]
async fn when_request_actor(world: &mut ActorWorld, actor_id: String) {
    let client = world.cluster.as_ref().unwrap().client();
    world.actor = Some(client.actor_reference(&actor_id).await.unwrap());
}
```

### Benchmarking

#### Performance Benchmarks

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use tokio::runtime::Runtime;

fn actor_throughput_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_with_input(
        BenchmarkId::new("actor_invocation", "1000_messages"),
        &1000,
        |b, &size| {
            b.to_async(&rt).iter(|| async {
                // Benchmark actor invocation throughput
                for _ in 0..size {
                    // Invoke actor method
                }
            });
        },
    );
}

criterion_group!(benches, actor_throughput_benchmark);
criterion_main!(benches);
```

## Code Quality Standards

### Code Formatting

Use rustfmt for consistent code formatting:

```bash

# Format all code
cargo fmt --all

# Check formatting without changing files
cargo fmt --all -- --check
```

Rustfmt configuration (`.rustfmt.toml`):

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
newline_style = "Unix"
use_small_heuristics = "Default"
reorder_imports = true
reorder_modules = true
remove_nested_parens = true
```

### Linting

Use Clippy for comprehensive linting:

```bash

# Run all lints
cargo clippy --workspace --all-targets -- -D warnings

# Fix automatically fixable lints
cargo clippy --workspace --all-targets --fix
```

Clippy configuration in `Cargo.toml`:

```toml
[workspace.lints.clippy]
pedantic = "warn"
nursery = "warn"
cargo = "warn"
missing_docs_in_private_items = "allow"
module_name_repetitions = "allow"
```

### Documentation Standards

- **Public APIs**: All public functions, structs, enums, and modules must have documentation
- **Examples**: Include usage examples in documentation
- **Error Handling**: Document error conditions and return types
- **Safety**: Document unsafe code with safety requirements

Documentation example:

```rust
/// Manages the lifecycle of actors in the Orbit cluster.
///
/// The `ActorManager` coordinates actor activation, deactivation, and
/// message routing across cluster nodes. It implements load balancing
/// and health monitoring for optimal performance.
///
/// # Examples
///
/// ```
/// use orbit_server::ActorManager;
/// 
/// let manager = ActorManager::new(config).await?;
/// let actor = manager.get_or_create_actor("my-actor").await?;
/// ```
///
/// # Errors
///
/// Returns [`ActorError::CreationFailed`] if actor creation fails due to
/// resource constraints or configuration issues.
pub struct ActorManager {
    // Implementation details
}
```

### Error Handling

- Use `Result<T, E>` for fallible operations
- Create specific error types for different failure modes
- Provide context in error messages
- Use `anyhow` for application errors, custom error types for library APIs

Error type example:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ActorError {
    #[error("Actor not found: {actor_id}")]
    NotFound { actor_id: String },
    
    #[error("Actor creation failed: {reason}")]
    CreationFailed { reason: String },
    
    #[error("Network error: {0}")]
    NetworkError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
```

## Contributing Guidelines

### Development Workflow

1. **Fork and Clone**

   ```bash
   git clone https://github.com/your-username/orbit-rs.git
   cd orbit-rs
   git remote add upstream https://github.com/TuringWorks/orbit-rs.git
   ```

2. **Create Feature Branch**

   ```bash
   git checkout -b feature/amazing-new-feature
   ```

3. **Make Changes**
   - Write code following the style guide
   - Add tests for new functionality
   - Update documentation
   - Run tests and ensure they pass

4. **Commit Changes**

   ```bash
   git add .
   git commit -m "feat: add amazing new feature"
   ```

5. **Push and Create PR**

   ```bash
   git push origin feature/amazing-new-feature
   # Create pull request through GitHub UI
   ```

### Commit Message Format

Follow conventional commit format:

```text
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

Types:

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Formatting changes
- `refactor`: Code refactoring
- `test`: Adding tests
- `chore`: Maintenance tasks

Examples:

```text
feat(client): add connection pooling support
fix(server): resolve actor lifecycle race condition
docs(readme): update installation instructions
```

### Pull Request Process

1. **Pre-submission Checklist**
   - [ ] Tests pass locally
   - [ ] Code is formatted (`cargo fmt`)
   - [ ] No linting errors (`cargo clippy`)
   - [ ] Documentation updated
   - [ ] CHANGELOG.md updated (for significant changes)

2. **PR Description**
   - Clear description of changes
   - Link to related issues
   - Screenshots/examples if applicable
   - Breaking changes noted

3. **Review Process**
   - Automated CI checks must pass
   - At least one maintainer approval required
   - Address review feedback
   - Squash commits before merge (if requested)

### Code Review Guidelines

#### For Authors

- Keep PRs focused and reasonably sized
- Provide clear description and context
- Respond to feedback promptly
- Test edge cases and error conditions

#### For Reviewers

- Check for correctness and clarity
- Verify test coverage
- Consider performance implications
- Review documentation updates
- Be constructive in feedback

## Architecture Guidelines

### Design Principles

1. **Separation of Concerns**: Each module has a clear, well-defined responsibility
2. **Modularity**: Components can be developed and tested independently
3. **Extensibility**: Easy to add new features without breaking existing functionality
4. **Performance**: Optimize for common use cases while maintaining flexibility
5. **Safety**: Leverage Rust's type system for memory safety and thread safety

### Async Programming

- Use `tokio` for async runtime
- Prefer `async`/`await` over manual `Future` implementations
- Use structured concurrency patterns
- Handle cancellation gracefully

Example:

```rust
use tokio::time::{timeout, Duration};

pub async fn process_with_timeout<T>(
    future: impl Future<Output = T>,
    timeout_duration: Duration,
) -> Result<T, TimeoutError> {
    timeout(timeout_duration, future)
        .await
        .map_err(|_| TimeoutError::Elapsed)
}
```

### Error Handling Strategy

- Use specific error types for different modules
- Implement `From` traits for error conversion
- Provide helpful error messages with context
- Log errors at appropriate levels

### Performance Considerations

- Minimize allocations in hot paths
- Use zero-copy techniques where possible
- Profile performance-critical code
- Document performance characteristics

## Debugging and Profiling

### Logging Configuration

```rust
use tracing::{info, warn, error, debug, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn init_logging() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
}

// Usage
info!("Actor {} activated", actor_id);
warn!("High memory usage detected: {}MB", memory_usage);
error!("Failed to connect to database: {}", error);
```

### Performance Profiling

```bash

# CPU profiling with perf
cargo build --release
perf record --call-graph=dwarf ./target/release/orbit-server
perf report

# Memory profiling with valgrind
cargo build
valgrind --tool=memcheck --leak-check=full ./target/debug/orbit-server

# Benchmarking
cargo bench
```

### Debugging Tools

- **gdb/lldb**: Traditional debuggers
- **rust-analyzer**: IDE integration for debugging
- **tokio-console**: Async runtime debugging
- **tracing**: Structured logging and instrumentation

## Related Documentation

- [Quick Start Guide](../QUICK_START.md) - Getting started with Orbit-RS
- [Overview](../OVERVIEW.md) - Architecture and features overview
- [Transaction Features](../features/TRANSACTION_FEATURES.md) - Advanced transaction development
- [Protocol Adapters](../protocols/PROTOCOL_ADAPTERS.md) - Protocol development
- [Deployment Guide](../deployment/DEPLOYMENT.md) - Production deployment
