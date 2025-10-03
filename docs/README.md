# Orbit-RS Documentation Index

## Getting Started

- [README](../README.md) - Project overview and quick start
- [Installation Guide](../README.md#installation) - Setup instructions
- [Basic Usage Examples](../README.md#basic-usage) - Simple actor examples

## Architecture

- [Architecture Documentation](../ORBIT_ARCHITECTURE.md) - Detailed system architecture
- [Module Structure](../README.md#project-structure) - Workspace organization
- [Migration Guide](../MIGRATION_GUIDE.md) - Kotlin/JVM to Rust migration

## Core Features

### Actor System
- [Basic Actor Usage](../README.md#basic-usage) - Creating and invoking actors
- [Actor Lifecycle](../ORBIT_ARCHITECTURE.md#addressables-virtual-actors) - Activation and deactivation
- [Examples](../examples/) - Working example implementations

### Distributed Transactions
- [2-Phase Commit](../README.md#distributed-transactions-example) - Basic transaction example
- [Transaction Coordination](../ORBIT_ARCHITECTURE.md#transaction-recovery) - Coordinator and participants
- [Persistence](../ORBIT_ARCHITECTURE.md#persistence) - Transaction logging

### Advanced Transaction Features
- **[Advanced Transaction Features Guide](ADVANCED_TRANSACTION_FEATURES.md)** - Comprehensive guide
- [Distributed Locks](#distributed-locks) - Lock management with deadlock detection
- [Metrics Integration](#metrics-integration) - Prometheus observability
- [Security Features](#security-features) - Authentication and audit logging
- [Performance Optimizations](#performance-optimizations) - Batching and pooling
- [Saga Pattern](#saga-pattern) - Long-running workflows

### Network Layer
- **[Network Layer Guide](NETWORK_LAYER.md)** - Complete gRPC infrastructure documentation
- [Protocol Buffers](#protocol-buffers) - Message serialization
- [gRPC Services](#grpc-services) - Service definitions and usage
- [Transport Layer](#transport-layer) - Connection pooling and retry logic
- [Raft Transport](#raft-transport) - Consensus communication

## Deployment

### CI/CD and Secrets
- **[Secrets Configuration Guide](SECRETS_CONFIGURATION_GUIDE.md)** - Complete guide for configuring GitHub Actions secrets
- [Secrets Preparation Script](../scripts/prepare-secrets.sh) - Automated kubeconfig preparation
- [CI/CD Pipeline]../.github/workflows/ci-cd.yml) - GitHub Actions workflow

### Kubernetes
- [Kubernetes Deployment Guide](KUBERNETES_DEPLOYMENT.md) - Complete K8s deployment
- [Operator Usage](KUBERNETES_DEPLOYMENT.md#operator-usage) - Custom resource definitions
- [Helm Charts](../helm/orbit-rs/) - Production-ready Helm deployment

### Docker
- [Docker Compose](../docker-compose.yml) - Local development environment
- [Dockerfile](../Dockerfile) - Container build configuration

## Reference

- [API Documentation](https://docs.rs/orbit-rs) - Complete API reference
- [Dependency Mapping](../DEPENDENCY_MAPPING.md) - JVM to Rust dependencies
- [Project Status](../PROJECT_STATUS.md) - Current implementation status
- [Change Log](../CHANGELOG.md) - Version history

## Examples

### Basic Examples
- [Hello World](../examples/hello-world/) - Simple actor greeting
- [Distributed Counter](../examples/distributed-counter/) - Shared counter with actors
- [Chat Room](../examples/chat-room/) - Multi-user chat application

### Advanced Examples
- [Distributed Transactions](../examples/distributed-transactions/) - Banking transaction example
- [Saga Pattern](../examples/saga-example/) - Order processing workflow

## Advanced Topics

### Network Layer

High-performance gRPC-based network infrastructure with connection pooling, retry logic, and comprehensive monitoring.

**Key Features:**
- Protocol Buffer integration with `tonic`
- Bidirectional streaming for actor messages
- Connection pooling with automatic cleanup
- Exponential backoff retry logic
- Health check service

**Quick Example:**
```rust
use orbit_shared::transport::{GrpcTransactionMessageSender, TransportConfig};

let sender = GrpcTransactionMessageSender::new(
    node_id,
    node_resolver,
    TransportConfig::default(),
);

sender.start_background_tasks().await?;
sender.send_message(&target, message).await?;
```

**Documentation:** [Network Layer Guide](NETWORK_LAYER.md)

### Protocol Buffers

Complete Protocol Buffer definitions for all core types and services.

**Defined Protocols:**
- `messages.proto` - Actor invocation and routing
- `node.proto` - Cluster node information
- `addressable.proto` - Actor references and leases
- `connection.proto` - Connection service
- `health.proto` - Health monitoring

**Documentation:** [Protocol Buffers Section](NETWORK_LAYER.md#protocol-buffer-definitions)

### gRPC Services

Four fully implemented gRPC services for distributed communication:

1. **ConnectionService** - Bidirectional actor message streaming
2. **HealthService** - Standard health checks and monitoring
3. **RaftConsensusService** - Consensus protocol messages
4. **TransactionService** - Distributed transaction coordination

**Documentation:** [gRPC Services Section](NETWORK_LAYER.md#grpc-services)

### Transport Layer

Production-ready transport with advanced features:

**Connection Pooling:**
- Automatic connection caching and reuse
- Health-based cleanup
- Per-connection metrics tracking

**Retry Logic:**
- Exponential backoff strategy
- Smart error classification
- Timeout enforcement

**Performance:**
- HTTP/2 with adaptive flow control
- TCP keepalive
- Concurrent request handling
- Broadcast optimization

**Documentation:** [Transport Layer Section](NETWORK_LAYER.md#transport-layer)

### Raft Transport

Specialized gRPC transport for Raft consensus protocol:

**Features:**
- Vote request/response handling
- Log replication via append entries
- Concurrent heartbeat broadcasting
- Dynamic node address updates
- Automatic reconnection

**Documentation:** [Raft Transport Section](NETWORK_LAYER.md#raft-consensus-transport-orbit-sharedsrcraft_transportrs)

### Distributed Locks

Coordinate access to shared resources across the cluster with automatic deadlock detection.

**Key Features:**
- Exclusive and shared lock modes
- Wait-for graph based deadlock detection
- Automatic timeout and expiration
- Priority-based fair acquisition

**Quick Example:**
```rust
use orbit_shared::transactions::{DistributedLockManager, LockMode};

let lock = lock_manager.acquire_lock(
    "resource-123",
    LockMode::Exclusive,
    Duration::from_secs(30)
).await?;

// Lock automatically released when dropped
```

**Documentation:** [Distributed Locks Guide](ADVANCED_TRANSACTION_FEATURES.md#distributed-locks)

### Metrics Integration

Comprehensive Prometheus metrics for transaction monitoring and observability.

**Exported Metrics:**
- Transaction counters (started, committed, aborted)
- Active transaction gauges
- Duration histograms
- Lock acquisition and deadlock metrics
- Saga step execution tracking

**Quick Example:**
```rust
use orbit_shared::transactions::TransactionMetrics;

let metrics = TransactionMetrics::new(node_id);
metrics.record_transaction_started(&tx_id).await;
metrics.record_transaction_committed(&tx_id).await;
```

**Documentation:** [Metrics Integration Guide](ADVANCED_TRANSACTION_FEATURES.md#metrics-integration)

### Security Features

Token-based authentication, scope-based authorization, and comprehensive audit logging.

**Key Features:**
- JWT-style authentication tokens
- Fine-grained permission control
- Immutable audit trail
- Pluggable authentication providers

**Quick Example:**
```rust
use orbit_shared::transactions::{TransactionSecurityManager, TransactionPermission};

let security_mgr = TransactionSecurityManager::new(config);
security_mgr.authenticate(&token).await?;
security_mgr.authorize(&token, &tx_id, TransactionPermission::Commit).await?;
```

**Documentation:** [Security Features Guide](ADVANCED_TRANSACTION_FEATURES.md#security-features)

### Performance Optimizations

Adaptive batch processing, connection pooling, and resource management.

**Key Features:**
- Adaptive batch sizing with priority queues
- Generic connection pooling with health checks
- Memory and concurrency limiting
- RAII resource guards

**Quick Example:**
```rust
use orbit_shared::transactions::{BatchProcessor, ConnectionPool};

// Batch processing
let processor = BatchProcessor::new(config);
processor.add_operation(op, priority).await?;
let batch = processor.flush().await?;

// Connection pooling
let pool = ConnectionPool::new(config, factory);
let conn = pool.acquire().await?;
```

**Documentation:** [Performance Optimizations Guide](ADVANCED_TRANSACTION_FEATURES.md#performance-optimizations)

### Saga Pattern

Long-running distributed transactions with automatic compensation on failure.

**Key Features:**
- Step-by-step execution with forward progress
- Automatic compensation on failure
- Persistent saga state
- Event-driven coordination

**Quick Example:**
```rust
use orbit_shared::saga::SagaOrchestrator;

let saga = SagaOrchestrator::new("order-processing")
    .add_step("reserve-inventory", compensation: "release-inventory")
    .add_step("charge-payment", compensation: "refund-payment")
    .add_step("ship-order", compensation: "cancel-shipment");

saga.execute().await?;
```

**Documentation:** [Saga Pattern Guide](ADVANCED_TRANSACTION_FEATURES.md#saga-pattern)

## Development

### Building
```bash
cargo build --workspace          # Build all modules
cargo test --workspace           # Run all tests
cargo clippy --all-targets       # Lint code
cargo fmt                        # Format code
```

### Testing
```bash
cargo test --lib                 # Unit tests
cargo test --test integration    # Integration tests
cargo test --test bdd            # BDD scenarios
cargo bench                      # Benchmarks
```

### Running Examples
```bash
cargo run --package hello-world
cargo run --package distributed-counter
cargo run --package distributed-transactions
```

## Troubleshooting

### Common Issues

**Build Errors:**
- Ensure Rust 1.70+ is installed: `rustc --version`
- Install Protocol Buffers compiler: `brew install protobuf` (macOS)
- Clear build cache: `cargo clean`

**Deadlock Issues:**
- Enable debug logging: `RUST_LOG=orbit_shared::transactions::locks=debug`
- Check lock acquisition order
- Review timeout configurations

**Performance Issues:**
- Monitor metrics in Prometheus
- Check batch processor statistics
- Review connection pool health

See [Advanced Features Troubleshooting](ADVANCED_TRANSACTION_FEATURES.md#troubleshooting) for detailed guidance.

## Contributing

- GitHub Repository: https://github.com/TuringWorks/orbit-rs
- Issue Tracker: https://github.com/TuringWorks/orbit-rs/issues
- [Contributing Guidelines](../CONTRIBUTING.md)
- [Security Policy](../SECURITY.md)

## Resources

### Documentation
- [Rust Documentation](https://doc.rust-lang.org/)
- [Tokio Documentation](https://tokio.rs/)
- [Tonic/gRPC Documentation](https://github.com/hyperium/tonic)
- [Kubernetes Documentation](https://kubernetes.io/docs/)

### Related Projects
- [Microsoft Orleans](https://github.com/dotnet/orleans) - Original inspiration
- [Akka](https://akka.io/) - JVM actor system
- [Orbit (Java/Kotlin)](https://github.com/orbit/orbit) - Original implementation

## License

This project is licensed under the BSD-3-Clause License. See [LICENSE](../LICENSE) for details.
