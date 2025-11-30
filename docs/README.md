# Orbit-RS Documentation

Welcome to the comprehensive documentation hub for Orbit-RS, a high-performance distributed virtual actor system framework written in Rust.

## Directory Structure

```text
docs/
├── README.md                    # This file
├── PRD.md                       # Product Requirements Document
├── index.md                     # Documentation site index
├── overview.md                  # Project overview
├── project_overview.md          # Detailed project overview
├── quick_start.md               # Quick start guide
├── features.md                  # Feature documentation
├── status.md                    # Project status
├── contributing.md              # Contributing guide
│
├── content/                     # Core documentation content
│   ├── protocols/               # Protocol implementations
│   ├── storage/                 # Storage engine docs
│   ├── server/                  # Server configuration docs
│   ├── graph/                   # Graph database docs
│   ├── graph-rag/               # GraphRAG documentation
│   ├── geo/                     # Geospatial docs
│   ├── ai/                      # AI/ML features
│   ├── ml/                      # Machine learning docs
│   ├── gpu-compute/             # GPU acceleration
│   ├── architecture/            # Architecture docs
│   ├── operations/              # Operations guides
│   ├── deployment/              # Deployment guides
│   ├── development/             # Development resources
│   │   ├── github/              # GitHub templates
│   │   ├── guidelines/          # Coding standards
│   │   ├── issues/              # Issue tracking
│   │   ├── testing/             # Test coverage
│   │   ├── verification/        # Verification scripts
│   │   └── wip/                 # Work-in-progress
│   ├── rfcs/                    # RFC documents
│   ├── legal/                   # Legal documents
│   ├── desktop/                 # Desktop client docs
│   ├── examples/                # Code examples
│   ├── support/                 # Support resources
│   └── *.md                     # Feature documentation files
│
├── planning/                    # Project planning
│   ├── backlog/                 # Feature backlog items
│   ├── competitive/             # Competitive analysis
│   ├── features/                # Planned features
│   ├── phases/                  # Implementation phases
│   └── roadmap/                 # Strategic roadmap
│
├── archive/                     # Archived documentation
└── assets/                      # Static assets
```

## Consolidated Documentation Index

### Getting Started

- **[Project Overview](project_overview.md)** - Comprehensive project overview with verified metrics and architecture
- **[Quick Start Guide](quick_start.md)** - Get up and running in minutes
- **[Development Guide](content/development/development.md)** - Complete setup and development instructions
- **[Contributing Guide](contributing.md)** - How to contribute to the project

### Architecture & Core Features

- **[Virtual Actor Persistence](content/virtual_actor_persistence.md)** - Actor state persistence, activation, and lifecycle management
- **[Storage Backend Independence](content/storage/STORAGE_BACKEND_INDEPENDENCE.md)** - Cloud vs local storage architecture
- **[Transaction Features](content/advanced_transaction_features.md)** - Distributed transactions, locks, saga patterns, and security
- **[Protocol Adapters](content/protocols/)** - Redis, PostgreSQL, and MCP protocol support
- **[Network Layer](content/NETWORK_LAYER.md)** - gRPC services, Protocol Buffers, and communication
- **[Cluster Management](content/server/CLUSTER_MANAGEMENT.md)** - Node discovery, load balancing, and fault tolerance

### Protocol Support

- **[Graph Database](content/graph/GRAPH_DATABASE.md)** - Complete graph operations and Cypher-like queries
- **[Graph Commands](content/graph/graph_commands.md)** - Graph database command reference
- **[Vector Operations](content/vector_commands.md)** - AI/ML vector database capabilities
- **[Time Series Commands](content/timeseries_commands.md)** - Time-series data management and analytics
- **[OrbitQL Documentation](content/ORBITQL_COMPLETE_DOCUMENTATION.md)** - Unified multi-model query language
- **[AQL Reference](content/aql/)** - ArangoDB Query Language compatibility

### Operations & Deployment

- **[Kubernetes Deployment](content/KUBERNETES_COMPLETE_DOCUMENTATION.md)** - Production Kubernetes deployment and persistence
- **[CI/CD Pipeline](content/development/CICD.md)** - Continuous integration and deployment
- **[Deployment Guide](content/deployment/)** - Deployment configurations and guides

### Planning & Development

- **[Strategic Roadmap](content/roadmap/ROADMAP_CONSOLIDATED.md)** - Comprehensive development roadmap and future plans
- **[Feature Backlog](content/development/BACKLOG_CONSOLIDATED.md)** - Complete feature backlog with priorities and timelines
- **[Planning Documents](planning/)** - Detailed planning and phase documentation
- **[Development Resources](content/development/)** - Guidelines, testing, and verification
- **[GitHub Project](https://github.com/orgs/TuringWorks/projects/1)** - Live project tracking with issues

### Advanced Topics

- **[GPU Acceleration](content/gpu-compute/)** - CUDA, Metal, Vulkan GPU compute
- **[GraphRAG](content/graph-rag/)** - Graph-based retrieval augmented generation
- **[Query Languages Comparison](content/QUERY_LANGUAGES_COMPARISON.md)** - SQL, Cypher, AQL, and OrbitQL comparison
- **[LSM Tree Implementation](content/storage/LSM_TREE_IMPLEMENTATION.md)** - Custom storage engine implementation
- **[AI/ML Features](content/ai/)** - Machine learning and AI capabilities

### Security & Compliance

- **[Security Documentation](content/SECURITY_COMPLETE_DOCUMENTATION.md)** - Security policies and vulnerability reporting
- **[Secrets Configuration](content/SECRETS_CONFIGURATION_GUIDE.md)** - Secure configuration management

### Reference Documentation

- **[Contributing Guide](contributing.md)** - How to contribute to the project
- **[Changelog](content/development/CHANGELOG.md)** - Version history and release notes
- **[RFC Documents](content/rfcs/)** - Request for Comments and architectural decisions
- **[API Documentation](https://turingworks.github.io/orbit-rs/api/)** - Generated API documentation

## Quick Navigation

### New to Orbit-RS?

1. Start with the **[Project Overview](project_overview.md)** to understand Orbit-RS capabilities and verified metrics
2. Follow the **[Quick Start Guide](quick_start.md)** to get a working system
3. Explore **[Advanced Transaction Features](content/advanced_transaction_features.md)** for distributed capabilities
4. Review **[Protocol Support](#protocol-support)** to understand multi-model database features

### Looking to Deploy?

1. Review the **[Strategic Roadmap](content/roadmap/ROADMAP_CONSOLIDATED.md)** to understand current production-ready status
2. Check **[Kubernetes Complete Documentation](content/KUBERNETES_COMPLETE_DOCUMENTATION.md)** for production setup
3. Review **[CI/CD Pipeline](content/development/CICD.md)** for deployment automation
4. Explore **[Advanced Transaction Features](content/advanced_transaction_features.md)** for enterprise features

### Want to Contribute?

1. Read the **[Development Guide](content/development/DEVELOPMENT.md)** for setup and guidelines
2. Check the **[Feature Backlog](content/development/BACKLOG_CONSOLIDATED.md)** for prioritized features and timelines
3. Review the **[Strategic Roadmap](content/roadmap/ROADMAP_CONSOLIDATED.md)** for long-term development plans
4. Browse the **[GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues)** for specific tasks

### Enterprise Users

1. Review **[Security Documentation](#security--compliance)** for security and compliance
2. Check **[Advanced Transaction Features](content/advanced_transaction_features.md)** for enterprise capabilities
3. Browse the **[Planning Documents](planning/)** for migration and implementation guides
4. Contact **[Enterprise Support](mailto:enterprise@turingworks.com)** for professional services

## External Resources

### Code & Development

- **[GitHub Repository](https://github.com/TuringWorks/orbit-rs)** - Source code and issues
- **[GitHub Project](https://github.com/orgs/TuringWorks/projects/1)** - Development roadmap tracking
- **[API Documentation](https://turingworks.github.io/orbit-rs/api/)** - Generated API documentation
- **[Contributing Guide](https://github.com/TuringWorks/orbit-rs/blob/main/CONTRIBUTING.md)** - How to contribute

### Community & Support

- **[Issue Tracker](https://github.com/TuringWorks/orbit-rs/issues)** - Bug reports and feature requests
- **[Discussions](https://github.com/TuringWorks/orbit-rs/discussions)** - Community discussions and Q&A

---

## Documentation Status

### Recently Consolidated

- **Project Documentation**: All overview, status, and structure documents merged into comprehensive [Project Overview](project_overview.md)
- **Strategic Planning**: All roadmap documents unified into [planning/](planning/)
- **Feature Planning**: All backlog documents consolidated into [planning/backlog/](planning/backlog/)
- **Development Resources**: Guidelines, testing, and WIP docs organized in [development/](development/)
- **Core Content**: Protocol, storage, and feature docs organized in [content/](content/)
- **Folder Structure**: Reorganized into 3 main categories: content, development, planning

### Current Stats

- **Total Documents**: 100+ comprehensive markdown files
- **Lines of Documentation**: 30,000+ lines of technical documentation
- **Verified Metrics**: All project statistics audited and verified (November 2025)
- **Cross-References**: 200+ internal links verified and updated

### Maintenance

- **Update Frequency**: Monthly review and quarterly major updates
- **Last Reorganization**: November 2025
- **Community Input**: Open documentation improvement process
- **Version Control**: All changes tracked in Git with detailed commit messages

---

**Orbit-RS: Production-ready multi-model distributed database platform**
**Documentation: Comprehensive, accurate, and continuously maintained**
**Happy building with Orbit-RS!**
