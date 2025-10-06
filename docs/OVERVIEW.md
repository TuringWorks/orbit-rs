# Orbit-RS Overview

A high-performance, distributed virtual actor system framework reimplemented in Rust, inspired by Microsoft Orleans and the original Java Orbit framework.

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](#)
[![License](https://img.shields.io/badge/license-BSD--3--Clause%20OR%20MIT-blue.svg)](../LICENSE-MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-red.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-79%20passing-green)]()
[![Coverage](https://img.shields.io/badge/coverage-comprehensive-blue)]()
[![CI/CD](https://img.shields.io/badge/CI%2FCD-verified-brightgreen)]()

## What is Orbit-RS?

Orbit is a framework for building distributed systems using virtual actors. A virtual actor is an object that interacts with the world using asynchronous messages. Actors can be active or inactive - when inactive, their state resides in storage, and when a message is sent to an inactive actor, it automatically activates on an available server in the cluster.

## Core Features

### 🚀 Virtual Actors
- Automatic lifecycle management with on-demand activation
- Transparent location and state management
- Type-safe actor interfaces with compile-time guarantees

### 🌐 Distributed Architecture
- Seamless clustering with automatic load balancing  
- Multiple load balancing strategies: round-robin, least connections, resource-aware, hash-based
- Fault tolerant with health checks, timeouts, and automatic cleanup

### ⚡ High Performance
- Built with Rust for maximum performance and memory safety
- Up to 500k+ messages/second per core
- Zero GC pauses with consistent sub-microsecond latency
- Small footprint: ~10MB statically linked binaries vs ~100MB JVM deployments

### 🔧 Protocol Integration
- Protocol Buffers for type-safe cross-language communication via gRPC
- High-performance gRPC transport with connection pooling
- Multiple protocol adapters: Redis (RESP), PostgreSQL wire protocol, REST API, and Neo4j Bolt support

### 💎 Advanced Features
- ACID-compliant distributed transactions with 2-phase commit
- Coordinator failover and transaction recovery mechanisms
- Persistent logging with durable transaction audit trails using SQLite
- Built-in metrics and comprehensive observability with Prometheus integration
- Easy deployment with single binary and minimal dependencies

## Architecture

The Rust implementation maintains the same core architecture as the original Kotlin version:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   orbit-client  │    │  orbit-shared   │    │  orbit-server   │
│                 │    │                 │    │                 │
│ • Actor Proxies │    │ • Data Types    │    │ • Cluster Mgmt  │
│ • Invocation    │    │ • Messages      │    │ • Load Balancer │
│ • Lease Mgmt    │    │ • Errors        │    │ • Health Check  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   orbit-proto   │
                    │                 │
                    │ • gRPC Services │
                    │ • Proto Buffers │
                    │ • Serialization │
                    └─────────────────┘
```

### Component Overview

#### orbit-client
- **Actor Proxies**: Client-side representations of remote actors
- **Invocation**: Message routing and remote procedure calls
- **Lease Management**: Actor lifetime and resource management

#### orbit-shared
- **Data Types**: Common data structures and type definitions
- **Messages**: Inter-actor communication protocols
- **Errors**: Standardized error handling across the system

#### orbit-server
- **Cluster Management**: Node discovery and cluster membership
- **Load Balancer**: Request distribution and resource optimization
- **Health Checks**: System monitoring and failure detection

#### orbit-proto
- **gRPC Services**: Network communication layer
- **Protocol Buffers**: Serialization and type safety
- **Cross-language Support**: Interoperability with other systems

## Key Advantages

### Memory Safety
- Compile-time elimination of data races and memory errors
- Safe concurrent programming with Rust's ownership model
- No null pointer exceptions or memory leaks

### Performance Benefits
- Zero-cost abstractions for high-level programming
- Native performance without virtual machine overhead
- Efficient memory usage and CPU utilization

### Operational Excellence
- Single binary deployment with minimal dependencies
- Built-in observability and monitoring
- Comprehensive error handling and debugging support
- Production-ready with enterprise features

## Use Cases

Orbit-RS is ideal for building:

- **Microservices Architectures**: Distributed actor-based services
- **Real-time Applications**: Low-latency message processing
- **IoT Systems**: Device management and data processing
- **Game Backends**: Player state management and matchmaking
- **Financial Systems**: Transaction processing and audit trails
- **Data Processing**: Stream processing and ETL pipelines

## Next Steps

- [Quick Start Guide](QUICK_START.md) - Get up and running quickly
- [Virtual Actor Persistence](VIRTUAL_ACTOR_PERSISTENCE.md) - Actor state management and lifecycle
- [Transaction Features](features/TRANSACTION_FEATURES.md) - Advanced transaction capabilities
- [Protocol Adapters](protocols/PROTOCOL_ADAPTERS.md) - Multi-protocol support
- [Development Guide](development/DEVELOPMENT.md) - Contributing and development
- [Deployment Guide](deployment/DEPLOYMENT.md) - Production deployment
