---
layout: default
title: "Orbit-RS Documentation"
subtitle: "The Next-Generation Distributed Database System"
category: "home"
permalink: /
---

## Orbit-RS Documentation

### The Next-Generation Multi-Protocol Database Platform

[![Version](https://img.shields.io/badge/version-Production--Ready-brightgreen.svg)](https://github.com/TuringWorks/orbit-rs)
[![License](https://img.shields.io/badge/license-MIT%2FBSD--3--Clause-blue.svg)](../LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/TuringWorks/orbit-rs/actions)
[![148K+ Lines](https://img.shields.io/badge/lines-148K+-blue.svg)](project_overview.md)
[![1078+ Tests](https://img.shields.io/badge/tests-1078%2B-green.svg)](features.md)

---

## Quick Start

**New to Orbit-RS?** Get up and running in 30 seconds:

```bash
# Clone and build
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs
cargo build --release

# Start multi-protocol server
./target/release/orbit-server --dev-mode

# Connect with any client:
# PostgreSQL: psql -h localhost -p 5432 -U orbit -d actors
# Redis: redis-cli -h localhost -p 6379
# MySQL: mysql -h localhost -P 3306 -u orbit
```

[**Full Quick Start Guide**](quick_start.md) | [**See Project Overview**](project_overview.md) | [**Architecture (PRD)**](PRD.md)

---

## Documentation Sections

### **Getting Started**

Perfect for developers new to Orbit-RS or distributed databases.

- [**Quick Start Guide**](quick_start.md) - Get running in 30 seconds
- [**Project Overview**](project_overview.md) - Architecture, features, and use cases
- [**Product Requirements Document (PRD)**](PRD.md) - Complete architecture and module reference
- [**Architecture Details**](overview.md) - Understanding Orbit-RS design
- [**Development Guide**](contributing.md) - Setup and configuration

### **Development & API**

For developers building applications with Orbit-RS.

- [**API Reference**](https://turingworks.github.io/orbit-rs/api/) - Complete API documentation
- [**Protocol Support**](content/server/NETWORK_LAYER.md) - Network layer and protocol details
- [**Testing Guide**](contributing.md) - Writing and running tests
- [**Contributing**](contributing.md) - How to contribute to Orbit-RS
- [**Development Setup**](content/DEVELOPMENT.md) - Setting up development environment

### **Operations & Deployment**

For system administrators and DevOps engineers.

- [**Kubernetes Deployment**](content/server/KUBERNETES_COMPLETE_DOCUMENTATION.md) - Production Kubernetes setup
- [**Configuration Reference**](content/deployment/CONFIGURATION.md) - Complete configuration guide
- [**Storage Architecture**](PRD.md#storage-architecture) - Storage backends and tiering
- [**Security**](content/server/SECURITY_COMPLETE_DOCUMENTATION.md) - Security policies and practices
- [**Operations Runbook**](content/operations/OPERATIONS_RUNBOOK.md) - Monitoring and troubleshooting
- [**Performance Tuning**](content/server/PETABYTE_SCALE_PERFORMANCE.md) - Optimization guide

### **Feature Guides**

Deep dives into Orbit-RS capabilities.

- [**SQL Engine**](content/protocols/POSTGRES_WIRE_IMPLEMENTATION.md) - PostgreSQL wire protocol and SQL
- [**Time Series**](content/server/TIMESERIES_IMPLEMENTATION_SUMMARY.md) - RedisTimeSeries compatibility
- [**Graph Database**](content/graph/GRAPH_DATABASE.md) - Graph data models and queries
- [**Vector Operations**](content/server/vector_commands.md) - AI/ML vector database
- [**Redis Commands**](content/protocols/REDIS_COMMANDS_REFERENCE.md) - 124+ RESP commands
- [**MySQL Protocol**](content/protocols/MYSQL_COMPLETE_DOCUMENTATION.md) - MySQL wire protocol
- [**CQL Protocol**](content/protocols/CQL_COMPLETE_DOCUMENTATION.md) - Cassandra Query Language
- [**Hardware Acceleration**](content/gpu-compute/COMPUTE_ACCELERATION_GUIDE.md) - GPU/Neural compute
- [**AQL Reference**](content/aql/AQL_REFERENCE.md) - ArangoDB Query Language support
- [**GraphRAG Integration**](content/graph-rag/GRAPHRAG_COMPLETE_DOCUMENTATION.md) - AI and graph analytics

### **Project Information**

Project status, roadmap, and community information.

- [**Features**](features.md) - Complete feature list
- [**Project Status**](status.md) - Current implementation status  
- [**Changelog**](content/development/CHANGELOG.md) - Version history and changes
- [**Migration Guide**](content/migration/MIGRATION_GUIDE.md) - Migrating from other databases
- [**Contributing**](contributing.md) - Community guidelines and support

---

## Current Status: Production Ready

Orbit-RS has delivered a **comprehensive multi-protocol database platform**:

| Component | Status | Features |
|-----------|--------|----------|
| **Actor System** | Complete | In-process & distributed actors, persistence |
| **Network Layer** | Complete | gRPC, in-process communication, connection pooling |
| **Transactions** | Complete | 2PC, Saga pattern, distributed locks, ACID compliance |
| **SQL Engine** | Complete | Full DDL/DML/DCL/TCL, JOINs, aggregates, vector operations |
| **Protocols** | Complete | Redis, PostgreSQL, MySQL, CQL, REST, gRPC |
| **Kubernetes** | Complete | Operator, Helm charts, production deployment |
| **AI Subsystems** | Complete | 8 intelligent subsystems for autonomous optimization |
| **Time Series** | Complete | RedisTimeSeries compatible with 21 comprehensive tests |

**Performance Metrics:**

- **Throughput:** 500K+ messages/second per core
- **Code Quality:** 148,780+ lines of production-ready Rust
- **Test Coverage:** 1,078+ tests ensuring reliability
- **Protocols:** 7 complete protocols with RocksDB persistence
- **Zero Warnings:** Strict code quality policy enforced

[**View Detailed Status**](features.md) | [**See Full PRD**](PRD.md)

---

## Key Features

### **Multi-Protocol Database**

One server, all protocols - replace PostgreSQL + MySQL + Redis + Cassandra with a single process.

- **PostgreSQL Wire Protocol:** Full SQL with pgvector, JSONB, spatial functions
- **Redis RESP Protocol:** 124+ commands with time series and vectors
- **MySQL Wire Protocol:** MySQL client compatibility
- **CQL Protocol:** Cassandra Query Language support
- **HTTP REST API:** JSON API with health and metrics
- **gRPC Services:** High-performance actor communication

### **AI-Native Subsystems**

8 intelligent subsystems for autonomous database optimization.

- **AI Master Controller:** Central orchestration (10-second control loop)
- **Intelligent Query Optimizer:** Cost-based optimization with ML
- **Predictive Resource Manager:** Workload forecasting
- **Smart Storage Manager:** Hot/warm/cold tiering
- **Adaptive Transaction Manager:** Dynamic concurrency control
- **Learning Engine:** Model improvement
- **Decision Engine:** Policy-based decisions
- **Knowledge Base:** Pattern storage

### **Advanced SQL Engine**

Enterprise-grade SQL engine with modern capabilities.

- **Complete SQL Support:** DDL, DML, DCL, TCL operations
- **Vector Database:** High-performance similarity search with HNSW/IVFFLAT
- **Complex Queries:** JOINs, subqueries, CTEs, window functions
- **Transaction Support:** ACID compliance with distributed transactions
- **JSONB Support:** JSON storage, indexing, and querying
- **Spatial Functions:** PostGIS-compatible spatial operations

### **Cloud-Native**

Designed for modern cloud and Kubernetes deployments.

- **Kubernetes Operator:** Custom resources for cluster management
- **Helm Charts:** Production-ready deployment templates
- **Auto-scaling:** Dynamic cluster scaling based on workload
- **Multi-platform:** Support for linux/amd64, linux/arm64, and Apple Silicon
- **Enterprise Security:** RBAC, JWT authentication, audit trails

### **Hardware Acceleration**

Powerful heterogeneous compute engine for maximum performance.

- **CPU SIMD:** AVX-512, NEON optimization with 3-8x speedups
- **GPU Acceleration:** Metal, CUDA, Vulkan, ROCm with 5-50x speedups  
- **Neural Engines:** Apple ANE integration for AI workloads
- **Auto-Detection:** Intelligent hardware discovery and workload routing
- **Cross-Platform:** macOS, Windows, Linux support

---

## Learning Resources

### **Tutorials**

- [**Getting Started Tutorial**](quick_start.md) - Your first Orbit-RS application
- [**SQL Tutorial**](content/protocols/POSTGRES_WIRE_IMPLEMENTATION.md) - SQL features and vector operations
- [**Time Series Tutorial**](content/server/TIMESERIES_IMPLEMENTATION_SUMMARY.md) - Time series data management
- [**Transaction Programming**](planning/features/transaction_features.md) - Building with distributed transactions
- [**Kubernetes Deployment**](content/server/KUBERNETES_COMPLETE_DOCUMENTATION.md) - Production deployment guide

### **Documentation**

- [**Architecture Overview**](project_overview.md) - Understanding Orbit-RS design principles
- [**Product Requirements Document**](PRD.md) - Complete architecture and module reference
- [**Performance Guide**](content/server/PETABYTE_SCALE_PERFORMANCE.md) - Optimization and tuning  
- [**Storage Architecture**](PRD.md#storage-architecture) - Storage backends and tiering

### **Development Resources**

- [**Development Guide**](content/DEVELOPMENT.md) - Understanding the codebase
- [**Migration Guide**](content/engine/MIGRATION.md) - Moving from other databases
- [**Security Guide**](content/server/SECURITY_COMPLETE_DOCUMENTATION.md) - Security best practices
- [**Operations Runbook**](content/operations/OPERATIONS_RUNBOOK.md) - Monitoring and troubleshooting

## Community & Support

### **Get Help**

- [**GitHub Issues**](https://github.com/TuringWorks/orbit-rs/issues) - Bug reports and questions
- [**GitHub Discussions**](https://github.com/TuringWorks/orbit-rs/discussions) - Community Q&A
- [**Contact Email**](mailto:contact@turingworks.com) - General inquiries

### **Report Issues**

- [**Bug Reports**](https://github.com/TuringWorks/orbit-rs/issues/new?template=bug_report.md)
- [**Feature Requests**](https://github.com/TuringWorks/orbit-rs/issues/new?template=feature_request.md)
- [**Security Issues**](mailto:security@turingworks.com) - Responsible disclosure

### **Contributing**

- [**Contribution Guide**](contributing.md) - How to contribute code
- [**Documentation**](PRD.md#document-maintenance) - Documentation structure and guides
- [**Testing**](contributing.md) - Adding tests and benchmarks
- [**Development**](content/DEVELOPMENT.md) - Development workflow

## License & Legal

Orbit-RS is released under **dual license: MIT or BSD-3-Clause**.

- [**License**](https://github.com/TuringWorks/orbit-rs/blob/main/LICENSE)
- [**Security Policy**](https://github.com/TuringWorks/orbit-rs/security/policy)  
- [**Code of Conduct**](https://github.com/TuringWorks/orbit-rs/blob/main/CODE_OF_CONDUCT.md)

---

## Quick Links

<div class="quick-links" markdown="1">

**For Developers:**

- [API Documentation](https://turingworks.github.io/orbit-rs/api/)
- [Development Setup](content/DEVELOPMENT.md)
- [Testing Guide](contributing.md)

**For Operators:**  

- [Kubernetes Guide](content/server/KUBERNETES_COMPLETE_DOCUMENTATION.md)
- [Configuration Reference](content/deployment/CONFIGURATION.md)
- [Security Configuration](content/server/SECURITY_COMPLETE_DOCUMENTATION.md)
- [Operations Runbook](content/operations/OPERATIONS_RUNBOOK.md)

**For Decision Makers:**

- [Project Overview](project_overview.md)
- [Product Requirements Document](PRD.md)
- [Features](features.md)
- [Architecture Overview](overview.md)

</div>

---

<div class="footer-info" markdown="1">

**Star us on GitHub:** [TuringWorks/orbit-rs](https://github.com/TuringWorks/orbit-rs)

**Questions?** Reach out on [GitHub Discussions](https://github.com/TuringWorks/orbit-rs/discussions)

**Enterprise?** Contact us at [enterprise@turingworks.com](mailto:enterprise@turingworks.com)

</div>
