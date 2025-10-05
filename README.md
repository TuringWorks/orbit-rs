# Orbit - Rust Implementation

[![CI](https://github.com/TuringWorks/orbit-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/TuringWorks/orbit-rs/actions/workflows/ci.yml)
[![CI/CD Pipeline](https://github.com/TuringWorks/orbit-rs/actions/workflows/ci-cd.yml/badge.svg)](https://github.com/TuringWorks/orbit-rs/actions/workflows/ci-cd.yml)
[![CodeQL](https://github.com/TuringWorks/orbit-rs/actions/workflows/codeql.yml/badge.svg)](https://github.com/TuringWorks/orbit-rs/actions/workflows/codeql.yml)
[![License](https://img.shields.io/badge/license-BSD--3--Clause%20OR%20MIT-blue.svg)](#license)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-red.svg)](https://www.rust-lang.org/)

A high-performance, distributed virtual actor system framework reimplemented in Rust, inspired by Microsoft Orleans and the original Java Orbit framework.

## 📚 Documentation

**Complete documentation is available in the [docs/](docs/) directory:**
- **[📖 Documentation Index](docs/README.md)** - Navigate all documentation
- **[🎯 Overview](docs/OVERVIEW.md)** - Architecture, features, and key benefits  
- **[🚀 Quick Start](docs/QUICK_START.md)** - Get up and running in minutes
- **[💎 Transaction Features](docs/features/TRANSACTION_FEATURES.md)** - Advanced distributed transactions
- **[🔌 Protocol Adapters](docs/protocols/PROTOCOL_ADAPTERS.md)** - Redis, PostgreSQL, MCP support
- **[☸️ Deployment](docs/deployment/DEPLOYMENT.md)** - Kubernetes, CI/CD, production setup
- **[👩‍💻 Development](docs/development/DEVELOPMENT.md)** - Contributing and development guide
- **[🗺️ Roadmap](docs/ROADMAP.md)** - Development roadmap and GitHub project

## What is Orbit-RS?

Orbit is a framework for building distributed systems using virtual actors. A virtual actor is an object that interacts with the world using asynchronous messages. Actors can be active or inactive - when inactive, their state resides in storage, and when a message is sent to an inactive actor, it automatically activates on an available server in the cluster.

### Key Features
- 🚀 **Virtual Actors**: Automatic lifecycle management with on-demand activation
- ⚡ **High Performance**: Up to 500k+ messages/second per core with Rust's memory safety
- 💎 **Distributed Transactions**: ACID-compliant with 2-phase commit, saga patterns, and distributed locks
- 🔌 **Protocol Adapters**: Redis (RESP), PostgreSQL wire protocol with vector operations, MCP for AI agents
- 🗄️ **SQL Database**: Complete PostgreSQL compatibility with advanced SQL features and pgvector support
- ☸️ **Kubernetes Native**: Custom operator with CRDs, Helm charts, and production-ready deployment
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

**📝 [Complete Quick Start Guide](docs/QUICK_START.md)** for detailed setup instructions.

## Current Status: Phase 8 Complete! 🎉

**✅ Production-Ready Features:**
- ✅ **Core Actor System** - Virtual actors with automatic lifecycle management
- ✅ **Distributed Transactions** - 2PC, Saga patterns, distributed locks, deadlock detection
- ✅ **Protocol Adapters** - Redis RESP (50+ commands), PostgreSQL wire protocol, MCP for AI
- ✅ **SQL Database** - Complete ANSI SQL with PostgreSQL compatibility and pgvector support
- ✅ **Kubernetes Integration** - Native operator, Helm charts, production deployment
- ✅ **Observability** - Prometheus metrics, Grafana dashboards, comprehensive monitoring

**🚀 What's Next:**
- **Phase 9**: Query Optimization & Performance ([5 GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-9))
- **Phase 10**: Production Readiness ([5 GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-10))
- **Phase 11**: Advanced Features ([5 GitHub Issues](https://github.com/TuringWorks/orbit-rs/issues?q=label%3Aphase-11))

**[📊 View Full Roadmap](docs/ROADMAP.md)** | **[📝 GitHub Project](https://github.com/orgs/TuringWorks/projects/1)**

## Contributing

We welcome contributions! See our **[Development Guide](docs/development/DEVELOPMENT.md)** for setup instructions and contributing guidelines.

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

