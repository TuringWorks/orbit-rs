# Orbit-RS Documentation

Welcome to the comprehensive documentation hub for Orbit-RS, a high-performance distributed virtual actor system framework written in Rust.

##  **Consolidated Documentation Index**

### ** Getting Started**

- **[Project Overview](project_overview.md)** - Comprehensive project overview with verified metrics and architecture
- **[Quick Start Guide](quick_start.md)** - Get up and running in minutes
- **[Development Guide](development/development.md)** - Complete setup and development instructions
- **[Contributing Guide](contributing.md)** - How to contribute to the project

### ** Architecture & Core Features**

- **[Virtual Actor Persistence](virtual_actor_persistence.md)** - Actor state persistence, activation, and lifecycle management
- **[Storage Backend Independence](STORAGE_BACKEND_INDEPENDENCE.md)** - Cloud vs local storage architecture
- **[Transaction Features](advanced_transaction_features.md)** - Distributed transactions, locks, saga patterns, and security
- **[Protocol Adapters](protocols/protocol_adapters.md)** - Redis, PostgreSQL, and MCP protocol support
- **[Persistence Architecture](PERSISTENCE_ARCHITECTURE.md)** - Storage backends and provider configuration
- **[Network Layer](NETWORK_LAYER.md)** - gRPC services, Protocol Buffers, and communication
- **[Cluster Management](CLUSTER_MANAGEMENT.md)** - Node discovery, load balancing, and fault tolerance

### ** Protocol Support**

- **[Graph Database](GRAPH_DATABASE.md)** - Complete graph operations and Cypher-like queries
- **[Graph Commands](graph_commands.md)** - Graph database command reference
- **[Vector Operations](vector_commands.md)** - AI/ML vector database capabilities
- **[Time Series Commands](timeseries_commands.md)** - Time-series data management and analytics
- **[ML Functions](ML_SQL_FUNCTIONS_DESIGN.md)** - Machine learning functions integrated with SQL
- **[AQL Reference](AQL_REFERENCE.md)** - ArangoDB Query Language compatibility

### ** Operations & Deployment**

- **[Kubernetes Deployment](kubernetes_deployment.md)** - Production Kubernetes deployment
- **[Kubernetes Storage Guide](KUBERNETES_STORAGE_GUIDE.md)** - Complete guide to persistence in Kubernetes
- **[Kubernetes Persistence](KUBERNETES_PERSISTENCE.md)** - Quick setup for K8s persistence backends
- **[CI/CD Pipeline](CICD.md)** - Continuous integration and deployment
- **[Advanced Transaction Features](advanced_transaction_features.md)** - Enterprise transaction capabilities

### ** Planning & Development**

- **[Strategic Roadmap](ROADMAP_CONSOLIDATED.md)** - Comprehensive development roadmap and future plans
- **[Feature Backlog](BACKLOG_CONSOLIDATED.md)** - Complete feature backlog with priorities and timelines
- **[Development Guide](DEVELOPMENT.md)** - Contributing, testing, and development setup
- **[Migration Guide](MIGRATION_GUIDE.md)** - Kotlin/JVM to Rust migration guide
- **[GitHub Project](https://github.com/orgs/TuringWorks/projects/1)** - Live project tracking with issues

### ** Advanced Topics**

- **[Query Languages Comparison](QUERY_LANGUAGES_COMPARISON.md)** - SQL, Cypher, AQL, and OrbitQL comparison
- **[GraphRAG Architecture](GraphRAG_ARCHITECTURE.md)** - Graph-based retrieval augmented generation
- **[LSM Tree Implementation](LSM_TREE_IMPLEMENTATION.md)** - Custom storage engine implementation
- **[OrbitQL Reference](ORBITQL_REFERENCE.md)** - Unified multi-model query language
- **[Persistence Alternatives](PERSISTENCE_ALTERNATIVES_ANALYSIS.md)** - Storage backend analysis
- **[Network Layer](NETWORK_LAYER.md)** - gRPC services and Protocol Buffers

### ** Security & Compliance**

- **[Security Guide](SECURITY.md)** - Security policies and vulnerability reporting
- **[Secrets Configuration](SECRETS_CONFIGURATION_GUIDE.md)** - Secure configuration management
- **[Advanced Transaction Features](advanced_transaction_features.md)** - Security and audit features

### ** Reference Documentation**

- **[Contributing Guide](contributing.md)** - How to contribute to the project
- **[Changelog](CHANGELOG.md)** - Version history and release notes
- **[RFC Documents](rfcs/)** - Request for Comments and architectural decisions
- **[API Documentation](https://turingworks.github.io/orbit-rs/api/)** - Generated API documentation

##  **Quick Navigation**

### ** New to Orbit-RS?**

1. Start with the **[Project Overview](project_overview.md)** to understand Orbit-RS capabilities and verified metrics
2. Follow the **[Quick Start Guide](quick_start.md)** to get a working system
3. Explore **[Advanced Transaction Features](advanced_transaction_features.md)** for distributed capabilities
4. Review **[Protocol Support](#-protocol-support)** to understand multi-model database features

### ** Looking to Deploy?**

1. Review the **[Strategic Roadmap](ROADMAP_CONSOLIDATED.md)** to understand current production-ready status
2. Check **[Kubernetes Deployment](kubernetes_deployment.md)** for production setup
3. Review **[CI/CD Pipeline](CICD.md)** for deployment automation
4. Explore **[Advanced Transaction Features](advanced_transaction_features.md)** for enterprise features

### ** Want to Contribute?**

1. Read the **[Development Guide](DEVELOPMENT.md)** for setup and guidelines
2. Check the **[Feature Backlog](BACKLOG_CONSOLIDATED.md)** for prioritized features and timelines
3. Review the **[Strategic Roadmap](ROADMAP_CONSOLIDATED.md)** for long-term development plans
4. Browse the **[GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues)** for specific tasks

### ** Enterprise Users**

1. Review **[Enterprise Features](#-security--compliance)** for security and compliance
2. Check **[Advanced Transaction Features](advanced_transaction_features.md)** for enterprise capabilities
3. Explore **[Migration Guide](MIGRATION_GUIDE.md)** for system migration
4. Contact **[Enterprise Support](mailto:enterprise@turingworks.com)** for professional services

##  External Resources

### Code & Development

- **[GitHub Repository](https://github.com/TuringWorks/orbit-rs)** - Source code and issues
- **[GitHub Project](https://github.com/orgs/TuringWorks/projects/1)** - Development roadmap tracking
- **[API Documentation](https://turingworks.github.io/orbit-rs/api/)** - Generated API documentation
- **[Contributing Guide](https://github.com/TuringWorks/orbit-rs/blob/main/CONTRIBUTING.md)** - How to contribute

### Community & Support

- **[Issue Tracker](https://github.com/TuringWorks/orbit-rs/issues)** - Bug reports and feature requests
- **[Discussions](https://github.com/TuringWorks/orbit-rs/discussions)** - Community discussions and Q&A

---

##  **Documentation Status**

### ** Recently Consolidated**

- **Project Documentation**: All overview, status, and structure documents merged into comprehensive [Project Overview](project_overview.md)
- **Strategic Planning**: All roadmap documents unified into [Strategic Roadmap](ROADMAP_CONSOLIDATED.md)
- **Feature Planning**: All backlog documents consolidated into [Feature Backlog](BACKLOG_CONSOLIDATED.md)
- **Link Verification**: All internal links updated and verified for consistency

### ** Current Stats**

- **Total Documents**: 50+ comprehensive markdown files
- **Lines of Documentation**: 25,000+ lines of technical documentation
- **Verified Metrics**: All project statistics audited and verified (October 2025)
- **Cross-References**: 200+ internal links verified and updated

### ** Maintenance**

- **Update Frequency**: Monthly review and quarterly major updates
- **Accuracy Audit**: Last completed October 7, 2025
- **Community Input**: Open documentation improvement process
- **Version Control**: All changes tracked in Git with detailed commit messages

---

** Orbit-RS: Production-ready multi-model distributed database platform**  
** Documentation: Comprehensive, accurate, and continuously maintained**  
** Happy building with Orbit-RS!**
