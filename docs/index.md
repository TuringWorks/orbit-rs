---
layout: default
title: "Orbit-RS Documentation"
subtitle: "The Next-Generation Distributed Database System"
category: "home"
permalink: /
---

## Orbit-RS Documentation

### The Next-Generation Distributed Database System

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/TuringWorks/orbit-rs)
[![License](https://img.shields.io/badge/license-Apache%202.0-green.svg)](../LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/TuringWorks/orbit-rs/actions)
[![Coverage](https://img.shields.io/badge/coverage-79%25-yellow.svg)](https://codecov.io/gh/TuringWorks/orbit-rs)

---

##  Quick Start

**New to Orbit-RS?** Get up and running in 5 minutes:

```bash

# Install Orbit-RS
cargo install orbit-rs

# Start a local instance
orbit-rs start --port 5432

# Connect with any PostgreSQL client
psql -h localhost -p 5432 -U orbit
```

[ **Full Quick Start Guide**](quick_start.md) | [ **See Project Overview**](project_overview.md)

---

##  Documentation Sections

###  **Getting Started**

Perfect for developers new to Orbit-RS or distributed databases.

- [ **Quick Start Guide**](quick_start.md) - Get running in 5 minutes
- [ **Project Overview**](project_overview.md) - Architecture, features, and use cases
- [ **Project Structure**](PROJECT_STRUCTURE.md) - Codebase organization
- [ **Architecture Details**](overview.md) - Understanding Orbit-RS design
- [ **Development Guide**](contributing.md) - Setup and configuration

###  **Development & API**

For developers building applications with Orbit-RS.

- [ **API Reference**](https://turingworks.github.io/orbit-rs/api/) - Complete API documentation
- [ **Protocol Support**](NETWORK_LAYER.md) - Network layer and protocol details
- [ **Testing Guide**](contributing.md) - Writing and running tests
- [ **Contributing**](contributing.md) - How to contribute to Orbit-RS
- [ **Development Setup**](contributing.md) - Setting up development environment

###  **Operations & Deployment**

For system administrators and DevOps engineers.

- [ **Kubernetes Deployment**](kubernetes_deployment.md) - Production Kubernetes setup
- [ **Docker Guide**](DOCKER_REMOVAL_SUMMARY.md) - Docker considerations
- [ **Storage Architecture**](PERSISTENCE_COMPLETE_DOCUMENTATION.md) - Storage backends and persistence
- [ **Security**](SECURITY_COMPLETE_DOCUMENTATION.md) - Security policies and practices
- [ **Storage Guide**](KUBERNETES_STORAGE_GUIDE.md) - Kubernetes persistence
- [ **Performance**](LSM_TREE_IMPLEMENTATION.md) - Storage performance optimization

###  **Feature Guides**

Deep dives into Orbit-RS capabilities.

- [ **SQL Engine**](ORBITQL_COMPLETE_DOCUMENTATION.md) - OrbitQL and SQL capabilities
- [ **Time Series**](TIME_SERIES_ENGINE.md) - Time-series data management
- [ **Graph Database**](GRAPH_DATABASE.md) - Graph data models and queries
- [ **Vector Operations**](vector_commands.md) - Vector database operations
- [ **Hardware Acceleration**](COMPUTE_ACCELERATION_GUIDE.md) - GPU/Neural compute acceleration 
- [ **Compute Architecture**](rfcs/rfc_heterogeneous_compute.md) - Technical deep-dive on acceleration
- [ **AQL Reference**](AQL_REFERENCE.md) - ArangoDB Query Language support
- [ **GraphRAG Integration**](GRAPHRAG_COMPLETE_DOCUMENTATION.md) - AI and graph analytics

###  **Project Information**

Project status, roadmap, and community information.

- [ **Roadmap**](roadmap.md) - Development roadmap and upcoming features
- [ **Project Status**](PROJECT_STATUS.md) - Current implementation status  
- [ **Changelog**](CHANGELOG.md) - Version history and changes
- [ **Migration Guide**](MIGRATION_GUIDE.md) - Kotlin/JVM to Rust migration
- [ **Contributing**](contributing.md) - Community guidelines and support

---

##  Current Status: Phase 8 Complete

Orbit-RS has reached **Phase 8** completion with a fully functional SQL engine and multi-protocol support:

| Component | Status | Features |
|-----------|--------|----------|
| ** Actor System** |  **Complete** | Distributed actors, proxy generation, lifecycle management |
| ** Network Layer** |  **Complete** | gRPC services, Protocol Buffers, connection pooling |
| ** Transactions** |  **Complete** | 2PC, Saga pattern, distributed locks, ACID compliance |
| ** SQL Engine** |  **Complete** | Full DDL/DML/DCL/TCL, JOINs, aggregates, vector operations |
| ** Protocols** |  **Complete** | Redis RESP, PostgreSQL wire protocol, MCP server |
| ** Kubernetes** |  **Complete** | Operator, Helm charts, production deployment |
| ** Performance** |  **Planned** | Query optimization, vectorization, parallel processing |
| ** Graph Database** |  **Planned** | Neo4j Bolt protocol, Cypher language support |

**Performance Metrics:**

- **Throughput:** 500K+ messages/second per core
- **SQL Compatibility:** Full ANSI SQL with PostgreSQL extensions  
- **Test Coverage:** 79 passing tests with comprehensive scenarios
- **Code Quality:** 150,000+ lines of production-ready Rust

[ **View Detailed Status**](features.md) | [ **See Full Roadmap**](roadmap.md)

---

##  Key Features

<!-- features-grid START -->

###  **Distributed Actor System**

Built on a powerful actor model for scalable, fault-tolerant distributed computing.

- **Addressable Actors:** String and UUID-based actor addressing
- **Lifecycle Management:** Automatic registration, activation, and cleanup
- **Proxy Generation:** Type-safe client-side actor references
- **Message Routing:** Transparent cross-cluster message routing

###  **Multi-Protocol Support**

Native support for multiple database protocols and query languages.

- **PostgreSQL Wire Protocol:** Full compatibility with PostgreSQL clients
- **Redis RESP Protocol:** 50+ Redis commands with clustering support
- **Model Context Protocol:** AI agent integration for autonomous operations
- **Vector Operations:** pgvector compatibility with similarity search

###  **Advanced SQL Engine**

Enterprise-grade SQL engine with modern capabilities.

- **Complete SQL Support:** DDL, DML, DCL, TCL operations
- **Vector Database:** High-performance similarity search with HNSW/IVFFLAT
- **Complex Queries:** JOINs, subqueries, CTEs, window functions
- **Transaction Support:** ACID compliance with distributed transactions

###  **Cloud-Native**

Designed for modern cloud and Kubernetes deployments.

- **Kubernetes Operator:** Custom resources for cluster management
- **Helm Charts:** Production-ready deployment templates
- **Auto-scaling:** Dynamic cluster scaling based on workload
- **Multi-platform:** Support for linux/amd64 and linux/arm64

###  **Hardware Acceleration** 

Powerful heterogeneous compute engine for maximum performance.

- **CPU SIMD:** AVX-512, NEON optimization with 3-8x speedups
- **GPU Acceleration:** Metal, CUDA, OpenCL with 5-50x speedups  
- **Neural Engines:** Apple ANE, Snapdragon DSP for AI workloads
- **Auto-Detection:** Intelligent hardware discovery and workload routing
- **Cross-Platform:** macOS, Windows, Linux, Android, iOS support

<!-- features-grid END -->

---

##  Development Roadmap

Orbit-RS follows a structured development approach with clearly defined phases:

###  **Next Up: Phase 9 - Query Optimization** *(Q2 2024)*

- **Cost-Based Query Planner:** Intelligent query optimization
- **Vectorized Execution:** SIMD-optimized batch processing  
- **Parallel Processing:** Multi-threaded query execution
- **Index Intelligence:** Automatic index recommendations
- **Query Caching:** Multi-level intelligent caching

###  **Coming Soon**

- **Phase 10:** Production readiness with HA, monitoring, backup/recovery
- **Phase 11:** Advanced features with stored procedures and full-text search
- **Phase 12:** Time-series database with Redis TimeSeries compatibility
- **Phase 13:** Neo4j Bolt protocol with complete Cypher support

[ **View Complete Roadmap**](roadmap.md) | [ **Track Progress**](https://github.com/TuringWorks/orbit-rs/projects)

---

##  Learning Resources

###  **Tutorials**

- [ **Getting Started Tutorial**](quick_start.md) - Your first Orbit-RS application
- [ **SQL Tutorial**](ORBITQL_COMPLETE_DOCUMENTATION.md) - OrbitQL features and vector operations
- [ **Transaction Programming**](advanced_transaction_features.md) - Building with distributed transactions
- [ **Kubernetes Complete Documentation**](KUBERNETES_COMPLETE_DOCUMENTATION.md) - Production deployment guide

###  **Documentation**

- [ **Architecture Overview**](project_overview.md) - Understanding Orbit-RS design principles
- [ **Performance Guide**](LSM_TREE_IMPLEMENTATION.md) - Storage optimization and tuning  
- [ **Implementation Tracking**](ORBITQL_COMPLETE_DOCUMENTATION.md) - Development progress

###  **Development Resources**

- [ **Project Structure**](PROJECT_STRUCTURE.md) - Understanding the codebase
- [ **Migration Guide**](MIGRATION_GUIDE.md) - Moving from other databases
- [ **Security Guide**](SECURITY_COMPLETE_DOCUMENTATION.md) - Security best practices

##  Community & Support

###  **Get Help**

- [ **GitHub Issues**](https://github.com/TuringWorks/orbit-rs/issues) - Bug reports and questions
- [ **GitHub Discussions**](https://github.com/TuringWorks/orbit-rs/discussions) - Community Q&A (Enable in repository settings)
- [ **Contact Email**](mailto:contact@turingworks.com) - General inquiries
- [ **Support Portal**](support/) - Enterprise support options

###  **Report Issues**

- [ **Bug Reports**](https://github.com/TuringWorks/orbit-rs/issues/new?template=bug_report.md)
- [ **Feature Requests**](https://github.com/TuringWorks/orbit-rs/issues/new?template=feature_request.md)
- [ **Security Issues**](mailto:security@turingworks.com) - Responsible disclosure

###  **Contributing**

- [ **Contribution Guide**](contributing.md) - How to contribute code
- [ **Documentation**](README.md) - Documentation structure and guides
- [ **Testing**](contributing.md) - Adding tests and benchmarks
- [ **Development**](contributing.md) - Development workflow

##  License & Legal

Orbit-RS is released under the **Apache License 2.0**.

- [ **License**](https://github.com/TuringWorks/orbit-rs/blob/main/LICENSE)
- [ **Security Policy**](https://github.com/TuringWorks/orbit-rs/security/policy)  
- [ **Code of Conduct**](https://github.com/TuringWorks/orbit-rs/blob/main/CODE_OF_CONDUCT.md)
- [ **Terms of Service**](legal/terms/)
- [ **Privacy Policy**](legal/privacy/)

---

##  Quick Links

<div class="quick-links" markdown="1">

**For Developers:**

- [ API Documentation](https://turingworks.github.io/orbit-rs/api/)
- [ Development Setup](contributing.md)
- [ **Testing Guide**](contributing.md) - Adding tests and benchmarks

**For Operators:**  

- [ Kubernetes Guide](KUBERNETES_STORAGE_GUIDE.md)
- [ Storage Setup](KUBERNETES_STORAGE_GUIDE.md)
- [ Security Configuration](SECURITY_COMPLETE_DOCUMENTATION.md)

**For Decision Makers:**

- [ Project Status](features.md)
- [ Roadmap & Timeline](roadmap.md)
- [ Architecture Overview](project_overview.md)

</div>

---

<div class="footer-info" markdown="1">

** Star us on GitHub:** [TuringWorks/orbit-rs](https://github.com/TuringWorks/orbit-rs)

** Questions?** Reach out on [Discord](https://discord.gg/orbit-rs) or [GitHub Discussions](https://github.com/TuringWorks/orbit-rs/discussions)

** Enterprise?** Contact us at [enterprise@turingworks.com](mailto:enterprise@turingworks.com)

</div>
