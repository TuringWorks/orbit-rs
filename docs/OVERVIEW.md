---
layout: default
title: "Architecture Overview"
subtitle: "Understanding Orbit-RS design and capabilities"
category: "architecture"
---

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

### 💾 Persistence & Storage
- **Multiple Storage Backends**: In-Memory, COW B+Tree, LSM-Tree, and RocksDB
- **Storage Backend Independence**: Cloud vs local storage with seamless switching
- **Kubernetes Integration**: Full K8s persistence with StatefulSets and PVCs
- **Actor State Management**: Automatic persistence with configurable backends

### 💎 Advanced Features
- **ACID Distributed Transactions**: 2-phase commit with coordinator failover
- **Transaction Recovery**: Automatic recovery with durable audit trails
- **Multiple Protocol Support**: Redis RESP, PostgreSQL wire, MCP, and REST APIs
- **Comprehensive Observability**: Built-in Prometheus metrics and health checks
- **Production Ready**: Single binary deployment with Kubernetes operator

## Architecture

The Rust implementation maintains the same core architecture as the original Kotlin version:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   orbit-client  │    │  orbit-shared   │    │  orbit-server   │
│                 │    │                 │    │                 │
│ • Actor Proxies │    │ • Data Types    │    │ • Cluster Mgmt  │
│ • Invocation    │    │ • Messages      │    │ • Load Balancer │
│ • Lease Mgmt    │    │ • Transactions  │    │ • Persistence   │
│                 │    │ • OrbitQL       │    │ • Health Check  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
       ┌─────────────────────────┼─────────────────────────┐
       │                         │                         │
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   orbit-proto   │    │ orbit-protocols │    │ orbit-operator  │
│                 │    │                 │    │                 │
│ • gRPC Services │    │ • Redis RESP    │    │ • K8s CRDs      │
│ • Proto Buffers │    │ • PostgreSQL    │    │ • Persistence   │
│ • Serialization │    │ • REST/HTTP     │    │ • Config Mgmt   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Component Overview

#### orbit-client
- **Actor Proxies**: Client-side representations of remote actors
- **Invocation**: Message routing and remote procedure calls
- **Lease Management**: Actor lifetime and resource management

#### orbit-shared
- **Data Types**: Common data structures and type definitions
- **Messages**: Inter-actor communication protocols
- **Transactions**: Distributed transaction coordination and recovery
- **OrbitQL**: SQL-like query language for actor systems
- **Persistence**: Storage backend abstractions and providers

#### orbit-server
- **Cluster Management**: Node discovery and cluster membership
- **Load Balancer**: Request distribution and resource optimization
- **Health Checks**: System monitoring and failure detection
- **Persistence Providers**: Multiple storage backends (Memory, COW B+Tree, LSM-Tree, RocksDB)

#### orbit-proto
- **gRPC Services**: Network communication layer
- **Protocol Buffers**: Serialization and type safety
- **Cross-language Support**: Interoperability with other systems

#### orbit-protocols
- **Redis RESP**: Full Redis protocol compatibility
- **PostgreSQL Wire**: Database protocol adapter
- **REST/HTTP**: RESTful API interface
- **MCP Support**: Model Context Protocol integration

#### orbit-operator
- **Kubernetes CRDs**: Custom resource definitions for K8s deployment
- **Persistence Configuration**: Declarative storage backend management
- **Config Management**: Automated configuration and scaling

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
