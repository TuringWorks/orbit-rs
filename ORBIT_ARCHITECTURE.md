# Orbit Architecture Analysis

## Project Overview

Orbit is a distributed actor system framework for building highly scalable real-time services. Originally developed by Electronic Arts, it's built on Kotlin/JVM and provides an actor model abstraction for distributed computing.

## Key Features

- **Virtual Actor Model**: Addressable actors that exist conceptually but are instantiated on-demand
- **Location Transparency**: Actors can be invoked regardless of their physical location in the cluster
- **Automatic Load Balancing**: Built-in distributed actor placement and migration
- **GRPC Communication**: High-performance communication between nodes using Protocol Buffers
- **Lease-based Management**: Actors and nodes use lease-based lifetime management
- **Spring Integration**: Plugin available for Spring Framework integration

## Module Architecture

The project is structured as a multi-module Gradle build with the following main components:

### Core Modules

#### orbit-util
- **Purpose**: Common utilities and base functionality
- **Dependencies**: kotlin-logging, micrometer-core
- **Key Components**: Logging utilities, metrics support, RNG utilities

#### orbit-shared
- **Purpose**: Shared data structures and interfaces used by both client and server
- **Dependencies**: orbit-util, kotlin-reflect
- **Key Components**: 
  - `Addressable` interfaces and types
  - `NodeId` and cluster management types
  - `Message` and communication protocols
  - Exception handling

#### orbit-proto
- **Purpose**: Protocol Buffer definitions and conversion utilities
- **Dependencies**: grpc, protobuf
- **Key Components**:
  - Proto definitions for messages, nodes, addressables
  - Kotlin converters between proto and domain objects
  - GRPC service definitions

### Client Module

#### orbit-client
- **Purpose**: Client-side actor system implementation
- **Dependencies**: orbit-util, orbit-shared, orbit-proto, coroutines, jackson, classgraph
- **Key Components**:
  - `AddressableProxy`: Dynamic proxy for actor invocations
  - `InvocationSystem`: Handles remote actor method calls
  - `AddressableLeaser`: Manages actor lease lifecycle
  - Actor lifecycle management (OnActivate, OnDeactivate)
  - Connection management and routing

### Server Module

#### orbit-server
- **Purpose**: Server-side cluster management and actor hosting
- **Dependencies**: orbit-util, orbit-shared, orbit-proto, grpc-netty
- **Key Components**:
  - `AddressableDirectory`: Tracks actor locations
  - `ClusterManager`: Manages cluster membership
  - Node discovery and health checking
  - Lease management and renewal

### Extension Modules

#### orbit-server-etcd
- **Purpose**: etcd-based distributed directory implementation
- **Key Components**:
  - `EtcdAddressableDirectory`
  - `EtcdNodeDirectory`

#### orbit-server-prometheus
- **Purpose**: Prometheus metrics integration

#### orbit-client-spring-plugin
- **Purpose**: Spring Framework integration
- **Key Components**: Spring-aware actor constructors

#### orbit-application
- **Purpose**: Application-level utilities and configuration

#### orbit-benchmarks
- **Purpose**: Performance benchmarks using JMH

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

## Configuration and Deployment

- Containerized deployment with Docker support
- Kubernetes deployment with Helm charts
- Configuration via application properties
- Support for development with Tiltfile

This architecture provides a solid foundation for building distributed, fault-tolerant, and scalable applications using the virtual actor model.