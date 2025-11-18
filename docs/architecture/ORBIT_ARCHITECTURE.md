---
layout: default
title: Orbit-RS Architecture
category: architecture
---

## Orbit-RS Architecture

## Project Overview

Orbit-RS is a next-generation distributed actor system framework built in Rust, providing a comprehensive multi-model database platform with advanced query capabilities. It extends the original Orbit concept with native support for graph databases, time series analytics, and unified query processing.

## Key Features

- **Virtual Actor Model**: Addressable actors with persistent state and distributed execution
- **Multi-Model Database**: Native support for graphs, documents, time series, and relational data
- **Advanced Query Languages**: Cypher, AQL (ArrangoDB Query Language), and OrbitQL support
- **High-Performance Time Series**: Real-time analytics with advanced compression and partitioning
- **Distributed by Design**: Built for horizontal scaling and fault tolerance
- **Actor Integration**: Direct database access through the actor system

## System Architecture

### High-Level Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Query Layer                                    │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐   │
│  │   Cypher    │    │     AQL     │    │  OrbitQL    │    │ Protocol    │   │
│  │   Parser    │    │   Parser    │    │  Parser     │    │ Adapters    │   │
│  │             │    │             │    │             │    │ (Neo4j Bolt)│   │
│  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Query Engine Layer                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ Query Planner & │  │  Execution      │  │    Query Optimization &     │  │
│  │   Optimizer     │  │    Engine       │  │    Distributed Routing      │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Multi-Model Storage Layer                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │  Graph Database │  │  Time Series    │  │   Document & Key-Value      │  │
│  │                 │  │    Engine       │  │       Storage               │  │
│  │ • Node Storage  │  │ • In-Memory     │  │ • JSON Documents            │  │
│  │ • Relationship  │  │ • Redis TS      │  │ • Relational Tables         │  │
│  │   Storage       │  │ • TimescaleDB   │  │ • Actor State Storage       │  │
│  │ • Graph ML      │  │ • Compression   │  │                             │  │
│  │ • Analytics     │  │ • Partitioning  │  │                             │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Actor System Layer                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ Virtual Actors  │  │   Persistence   │  │    Cluster Management       │  │
│  │                 │  │                 │  │                             │  │
│  │ • Addressable   │  │ • COW B-Tree    │  │ • Node Discovery            │  │
│  │   Leasing       │  │ • LSM Tree      │  │ • Load Balancing            │  │
│  │ • State Mgmt    │  │ • RocksDB       │  │ • Fault Tolerance           │  │
│  │ • Lifecycle     │  │ • Memory        │  │ • Health Monitoring         │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Module Architecture

Orbit-RS is structured as a Rust workspace with the following main crates:

### Core Crates

#### orbit-shared

- **Purpose**: Shared data structures, traits, and utilities
- **Key Components**:
  - Graph database types (`GraphNode`, `GraphRelationship`, `GraphStorage`)
  - Time series engine (`TimeSeriesEngine`, compression algorithms)
  - Actor system primitives (`NodeId`, `AddressableReference`)
  - OrbitQL query language implementation
  - Transaction and persistence traits

#### orbit-server

- **Purpose**: Server-side actor hosting and cluster management
- **Key Components**:
  - `AddressableDirectory`: Actor location tracking
  - `ClusterNodeProvider`: Node management
  - Persistence backends (Memory, COW B-Tree, LSM Tree, RocksDB)
  - Kubernetes operator integration
  - Dynamic persistence configuration

#### orbit-protocols

- **Purpose**: Protocol implementations and query engines
- **Key Components**:
  - Cypher parser and execution engine
  - AQL parser and execution engine  
  - Neo4j Bolt protocol adapter
  - REST API server with WebSocket support
  - Protocol buffer definitions

### Specialized Modules

#### orbit-operator

- **Purpose**: Kubernetes operator for cluster management
- **Key Components**:
  - Custom Resource Definitions (CRDs)
  - Operator controller logic
  - Persistence configuration management

#### Examples and Testing

- **examples/**: Demonstration applications
  - Hello World actor example
  - OrbitQL usage examples
  - Time series analytics demo
  - Persistence configuration examples
- **tests/**: Integration and performance tests
  - Graph database tests
  - Time series engine tests
  - Protocol compatibility tests

## Core Concepts

### Addressables (Virtual Actors)

Addressables are the core abstraction in Orbit. They are virtual actors that:

- Have a unique identity (type + key)
- Are instantiated on-demand when first invoked
- Can be automatically migrated between nodes
- Support lifecycle callbacks (OnActivate, OnDeactivate)
- Can be either keyed (with identity) or keyless

```kotlin
interface GreeterActor : ActorWithStringKey {
    suspend fun greet(name: String): String
}

class GreeterActorImpl : GreeterActor {
    override suspend fun greet(name: String): String {
        return "Hello $name"
    }
}
```

### Node Management

Nodes in the cluster are identified by:

- `NodeId`: Combination of key and namespace
- `NodeCapabilities`: What services/actor types the node can host
- `NodeStatus`: Current operational status
- Lease-based lifecycle with automatic renewal

### Message System

Communication uses a structured message system:

- `Message`: Container with content, source, target, attempts
- `MessageContent`: Various message types (invocation requests/responses, errors, connection info)
- `MessageTarget`: Unicast or routed unicast delivery
- Protocol Buffer serialization for network transport

### Invocation Model

Actor method calls are handled through:

1. `AddressableProxy`: Intercepts method calls using Java dynamic proxies
2. `AddressableInvocation`: Structured representation of method calls
3. `InvocationSystem`: Routes calls to appropriate nodes
4. Serialization/deserialization of arguments and results

### Lease Management

Both actors and nodes use lease-based management:

- Automatic lease renewal to indicate liveness
- Configurable lease duration
- Cleanup of expired leases
- Grace periods for lease renewal failures

## Dependencies

### Core JVM Dependencies

- **Kotlin**: 1.3.72 (main language)
- **Kotlin Coroutines**: 1.3.5 (async programming)
- **GRPC**: 1.29.0 (inter-node communication)
- **Protocol Buffers**: 3.11.1 (serialization)
- **Jackson**: 2.10.2 (JSON serialization)
- **SLF4J**: 1.7.30 (logging)
- **Micrometer**: 1.3.5 (metrics)

### Build and Testing

- **Gradle**: 6.x with Kotlin DSL
- **Kotest**: 3.4.2 (testing framework)
- **Mockito**: 3.3.3 (mocking)
- **JMH**: Performance benchmarking

## Communication Flow

1. **Client Invocation**: Client calls method on actor proxy
2. **Proxy Interception**: `AddressableProxy` captures the call
3. **Message Creation**: Call is serialized into `AddressableInvocation`
4. **Routing**: System determines which node hosts the actor
5. **Network Transport**: Message sent via GRPC to target node
6. **Server Processing**: Target node deserializes and executes call
7. **Response**: Result serialized and sent back to client
8. **Completion**: Client receives response and completes the call

## Scalability Features

- **Horizontal Scaling**: Add nodes to increase capacity
- **Actor Migration**: Actors can move between nodes for load balancing
- **Cluster Discovery**: Automatic node discovery and membership
- **Health Monitoring**: Node health checks and failure detection
- **Backpressure**: Built-in flow control mechanisms

## Advanced Transaction Features

The Rust implementation extends the original architecture with a comprehensive transaction system:

### Transaction Module Architecture

The transaction system is organized into specialized modules:

```text
orbit/shared/src/transactions/
├── core.rs         - 2-Phase Commit protocol implementation
├── locks.rs        - Distributed locks with deadlock detection
├── metrics.rs      - Prometheus metrics integration
├── security.rs     - Authentication, authorization, audit logging
└── performance.rs  - Batching, connection pooling, resource management
```

### Distributed Lock System

**Components:**

- `DistributedLockManager`: Coordinates lock acquisition and release across the cluster
- `DeadlockDetector`: Performs wait-for graph analysis to detect and resolve deadlocks
- `LockMode`: Supports both Exclusive and Shared locking semantics
- `LockRequest`: Encapsulates lock acquisition with timeout and priority

**Deadlock Detection:**

- Wait-for graph construction tracking resource dependencies
- DFS-based cycle detection with O(N) complexity
- Automatic deadlock resolution with configurable policies
- Lock expiration and timeout handling

**Lock Lifecycle:**

```text
Request → Wait Queue → Deadlock Check → Acquire → Hold → Release → Cleanup
```

### Metrics and Observability

**Metric Types:**

1. **Transaction Metrics**
   - Counters: started, committed, aborted, failed, timeout
   - Gauges: active transactions, queued operations
   - Histograms: duration, prepare time, commit time, participant count

2. **Saga Metrics**
   - Counters: started, completed, failed, compensated, step execution
   - Gauges: active sagas, queued sagas
   - Histograms: saga duration, step duration, compensation duration

3. **Lock Metrics**
   - Counters: acquired, released, timeout, deadlock detected/resolved
   - Gauges: held locks, waiting requests
   - Histograms: wait duration, hold duration

**Prometheus Integration:**

- Automatic metric registration and collection
- Node-scoped metrics for cluster-wide aggregation
- Standard Prometheus metric naming conventions
- Compatible with Grafana dashboards

### Security Architecture

**Authentication:**

- Token-based authentication with JWT-style tokens
- Configurable token expiration and renewal
- Pluggable authentication providers (in-memory, external)

**Authorization:**

- Scope-based permission model
- Fine-grained transaction permissions:
  - Begin, Commit, Abort (lifecycle operations)
  - Read, Write (data operations)
  - Coordinate, Participate (coordination roles)
- Hierarchical scope inheritance

**Audit Logging:**

- Immutable audit trail for all transaction operations
- Query support for forensics and compliance
- Automatic retention management with configurable limits
- Structured log entries with timestamps and outcomes

**Security Context:**

```text
Request → Authenticate → Authorize → Execute → Audit Log
```

### Performance Optimization System

**Batch Processing:**

- Adaptive batch sizing based on load
- Priority queue for operation ordering
- Configurable flush triggers (size, time, or manual)
- Automatic batch optimization

**Connection Pooling:**

- Generic connection pool supporting any connection type
- Health checking with configurable intervals
- Automatic connection lifecycle management
- Connection reuse and cleanup

**Resource Management:**

- Memory usage tracking and limiting
- Concurrency control with semaphores
- RAII resource guards for automatic cleanup
- Backpressure handling under resource constraints

### Saga Pattern Implementation

**Orchestration:**

- Step-by-step execution with forward progress tracking
- Automatic compensation on failure (backward recovery)
- Persistent saga state for recovery after crashes
- Event-driven coordination between saga steps

**Compensation:**

- Declarative compensation actions per step
- Automatic rollback in reverse execution order
- Idempotent compensation handlers
- Compensation failure handling and retry logic

**State Management:**

```text
Saga States: NotStarted → Running → Completed | Compensating → Compensated | Failed
```

### Transaction Recovery

**Coordinator Failover:**

- Automatic detection of coordinator failures
- Transaction state recovery from persistent log
- Continuation of in-flight transactions
- Participant coordination after recovery

**Persistence:**

- SQLite-based transaction log with WAL journaling
- Durable state for all transaction phases
- Integrity verification and corruption detection
- Automatic cleanup of completed transactions

## Configuration and Deployment

- Containerized deployment with Docker support
- Kubernetes deployment with Helm charts
- Configuration via application properties
- Support for development with Tiltfile
- Transaction system configuration (timeouts, batch sizes, pool limits)
- Security configuration (authentication providers, token expiration)
- Metrics configuration (Prometheus endpoints, scrape intervals)

## Performance Characteristics

**Transaction System:**

- 2PC coordination: ~5-10ms overhead per transaction
- Lock acquisition: <1ms in uncontended scenarios
- Deadlock detection: O(N) where N = number of waiting transactions
- Batch processing: Up to 10x throughput improvement for write-heavy workloads

**Resource Usage:**

- Connection pooling: Reduces connection overhead by 80-90%
- Memory management: Configurable limits prevent OOM scenarios
- Metrics: Minimal overhead (<1% CPU) for metric collection

This architecture provides a solid foundation for building distributed, fault-tolerant, and scalable applications using the virtual actor model with production-ready transaction support.
