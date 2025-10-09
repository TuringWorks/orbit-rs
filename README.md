# Orbit - Rust Implementation

![License](https://img.shields.io/badge/license-BSD--3--Clause%20OR%20MIT-blue.svg)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-red.svg)](https://www.rust-lang.org/)

A high-performance, distributed virtual actor system framework reimplemented in Rust, inspired by Microsoft Orleans and the original Java Orbit framework.

## 📚 Documentation

**Complete documentation is available in the [docs/](docs/) directory:**
- **[📚 Documentation Index](docs/README.md)** - Navigate all documentation
- **[🏆 Feature Index](docs/features.md)** - Complete feature list with implementation status
- **[🎯 Overview](docs/overview.md)** - Architecture, features, and key benefits  
- **[🚀 Quick Start](docs/quick_start.md)** - Get up and running in minutes
- **[⚙️ Virtual Actor Persistence](docs/virtual_actor_persistence.md)** - Actor state management and lifecycle
- **[💎 Transaction Features](docs/features/transaction_features.md)** - Advanced distributed transactions
- **[🔌 Protocol Adapters](docs/protocols/protocol_adapters.md)** - Redis, PostgreSQL, MCP support
- **[☸️ Deployment](docs/kubernetes_deployment.md)** - Kubernetes, CI/CD, production setup
- **[👩‍💻 Development](docs/development/development.md)** - Contributing and development guide
- **[🗺️ Roadmap](docs/roadmap.md)** - Development roadmap and GitHub project

## What is Orbit-RS?

Orbit is a framework for building distributed systems using virtual actors. A virtual actor is an object that interacts with the world using asynchronous messages. Actors can be active or inactive - when inactive, their state resides in storage, and when a message is sent to an inactive actor, it automatically activates on an available server in the cluster.

### Key Features
- 🚀 **Virtual Actors**: Automatic lifecycle management with on-demand activation
- ⚡ **High Performance**: Up to 500k+ messages/second per core with Rust's memory safety
- 🏎️ **Heterogeneous Compute**: **NEW!** Automatic hardware acceleration (CPU SIMD, GPU, Neural Engines) with 5-50x speedups for parallelizable workloads
- 💎 **Distributed Transactions**: ACID-compliant with 2-phase commit, saga patterns, and distributed locks
- 🔌 **Protocol Adapters**: **Complete Redis compatibility (124+ commands)** including Vector/AI, Time Series, Graph DB, ML functions + PostgreSQL wire protocol + MCP for AI agents
- 🤖 **AI/ML Ready**: Native vector similarity search, embeddings storage, and semantic search capabilities
- 📊 **Time Series**: Full RedisTimeSeries compatibility with aggregation, retention policies, and real-time analytics
- 🕸️ **Graph Database**: Cypher-like queries with execution planning, profiling, and distributed graph operations
- 🗄️ **SQL Database**: Complete PostgreSQL compatibility with advanced SQL features and pgvector support
- ☘️ **Kubernetes Native**: Custom operator with CRDs, Helm charts, and production-ready deployment
- 📊 **Observability**: Built-in Prometheus metrics, Grafana dashboards, and comprehensive monitoring
- 🛡️ **Enterprise Security**: Authentication, authorization, audit logging, and compliance features

## Quick Start

### Installation
```bash
# Clone and build
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs
cargo build --release
cargo test
```

### Basic Actor Example
```rust
use orbit_client::OrbitClient;
use orbit_shared::{ActorWithStringKey, Key};
use async_trait::async_trait;

#[async_trait]
trait GreeterActor: ActorWithStringKey {
    async fn greet(&self, name: String) -> Result<String, orbit_shared::OrbitError>;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OrbitClient::builder().with_namespace("demo").build().await?;
    let greeter = client.actor_reference::<dyn GreeterActor>(
        Key::StringKey { key: "my-greeter".to_string() }
    ).await?;
    
    let greeting = greeter.greet("World".to_string()).await?;
    println!("{}", greeting); // "Hello, World!"
    Ok(())
}
```

**📝 [Complete Quick Start Guide](docs/quick_start.md)** for detailed setup instructions.

## Current Status: Phase 8.5 Complete! 🎉

**✅ Production-Ready Features:**
- ✅ **Core Actor System** - Virtual actors with automatic lifecycle management
- ✅ **Heterogeneous Compute Engine** - **NEW!** Automatic hardware acceleration with 5-50x speedups
- ✅ **Distributed Transactions** - 2PC, Saga patterns, distributed locks, deadlock detection
- ✅ **Redis Protocol** - **Complete compatibility with 124+ commands** including:
  - ✅ All core Redis data types (String, Hash, List, Set, Sorted Set, Pub/Sub)
  - ✅ **Vector Operations (VECTOR.*, FT.*)** - AI/ML similarity search with multiple metrics
  - ✅ **Time Series (TS.*)** - Full RedisTimeSeries compatibility (18+ commands)
  - ✅ **Graph Database (GRAPH.*)** - Cypher-like queries with execution planning
  - ✅ **Machine Learning (ML_*)** - Statistical functions integrated with SQL
  - ✅ **Search Engine (FT.*)** - RedisSearch-compatible indexing and search
- ✅ **PostgreSQL Protocol** - Complete wire protocol with complex SQL parsing and pgvector support
- ✅ **AI Agent Integration** - Model Context Protocol (MCP) with comprehensive tool support
- ✅ **Kubernetes Integration** - Native operator, Helm charts, production deployment
- ✅ **Observability** - Prometheus metrics, Grafana dashboards, comprehensive monitoring

**🚀 What's Next:**
- **Phase 9**: Query Optimization & Performance ([5 GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-9))
- **Phase 10**: Production Readiness ([5 GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-10))
- **Phase 11**: Advanced Features ([5 GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-11))

**[📊 View Full Roadmap](docs/roadmap.md)** | **[📝 GitHub Project](https://github.com/orgs/TuringWorks/projects/1)**

## Contributing

We welcome contributions! See our **[Development Guide](DEVELOPMENT.md)** for setup instructions and contributing guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-new-feature`)
3. Make your changes and add tests
4. Run tests (`cargo test --workspace`)
5. Submit a pull request

**Priority areas for contributors:**
- Query optimization algorithms ([Phase 9 issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-9))
- Production readiness features ([Phase 10 issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-10))
- Advanced SQL features ([Phase 11 issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-11))

## License

This project is dual licensed under your choice of:

* **[MIT License](LICENSE-MIT)** - Very permissive, widely compatible
* **[BSD 3-Clause License](LICENSE-BSD)** - Also permissive with endorsement clause

Choose the license that best fits your project's needs.

## Credits

- Original Orbit project by [Electronic Arts](https://www.ea.com/)
- Rust implementation by [TuringWorks](https://github.com/TuringWorks)
- AI development engineering by [Warp.dev](https://warp.dev) - The AI-powered terminal for modern development workflows

## Support

- 📖 **[Full Documentation](docs/)** - Complete documentation hub
- 🐛 **[Issue Tracker](https://github.com/TuringWorks/orbit-rs/issues)** - Bug reports and feature requests
- 💬 **[Discussions](https://github.com/TuringWorks/orbit-rs/discussions)** - Community Q&A
- 📈 **[GitHub Project](https://github.com/orgs/TuringWorks/projects/1)** - Development roadmap

---

**Built with ❤️ in Rust for production-scale distributed systems.**

