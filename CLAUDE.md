# CLAUDE.md - AI Assistant Guide for Orbit-RS

This document provides essential context for AI assistants working with the Orbit-RS codebase.

> **Architecture References**: 
> - [`specifications/PRD.md`](specifications/PRD.md) - Single source of truth for modules, directory structures, and feature status
> - [`docs/content/architecture/ORBIT_ARCHITECTURE.md`](docs/content/architecture/ORBIT_ARCHITECTURE.md) - Detailed architecture patterns, transaction layer, query execution, network layer, and storage architecture

## Project Overview

**Orbit-RS** is a high-performance, distributed multi-protocol database server written in Rust. It natively implements PostgreSQL, MySQL, CQL (Cassandra), Redis, HTTP REST, gRPC, and OrbitQL protocols from a single process, sharing a unified storage layer.

- **Repository**: https://github.com/TuringWorks/orbit-rs
- **License**: BSD-3-Clause OR MIT
- **Rust Edition**: 2021
- **Minimum Rust Version**: 1.70+
- **Architecture**: See [`specifications/PRD.md`](specifications/PRD.md) for complete module reference

## Quick Reference Commands

All development tasks use `make` targets:

```bash
# Build & Check
make build              # Debug build (all workspace crates)
make build-release      # Release build (optimized)
make check              # cargo check + clippy (with pedantic allows)
make format             # cargo fmt --all
make clean              # cargo clean

# Run the server
make dev                # Run orbit-server in dev mode (foreground, debug)
make run                # Alias for make dev
make redis              # Run with Redis on default port 6379

# Testing
make test               # All workspace tests
make test-ignored       # Run only ignored (slow/integration) tests
make test-include-ignored  # Run all tests including ignored
make test-quick         # Compile check only, no test execution
make test-server        # orbit-server tests only
make test-verbose       # All tests with full output

# Cluster management
make cluster            # Start 3-node cluster (make cluster N=5)
make cluster-stop       # Stop running cluster
make cluster-status     # Show cluster node status
make cluster-lb         # Start load balancer for 3-node cluster
make cluster-lb-stop    # Stop load balancer

# Pre-commit
make commit-ready       # Format + check + test (recommended before push)
make commit-light       # Format + check only (faster)
make all                # Full pipeline: format, check, test, build
```

### Direct Script Usage

```bash
# Development server (non-standard ports)
./scripts/start_server.sh

# Multi-protocol server (all default ports)
./scripts/start-multiprotocol-server.sh           # dev mode
./scripts/start-multiprotocol-server.sh --prod     # production mode

# Cluster operations
./scripts/start-cluster.sh              # 3-node cluster
./scripts/start-cluster.sh 5            # 5-node cluster
./scripts/start-cluster.sh --with-lb 3  # Cluster with LB-compatible ports
./scripts/start-cluster.sh --stop       # Stop cluster
./scripts/start-cluster.sh --status     # Show status

# Load balancer
./scripts/start-cluster-lb.sh 3         # LB for 3-node cluster
./scripts/start-cluster-lb.sh --stop    # Stop LB

# Test runner (granular control)
./scripts/run-tests.sh              # All workspace tests
./scripts/run-tests.sh server       # orbit-server only
./scripts/run-tests.sh engine       # orbit-engine only
./scripts/run-tests.sh shared       # orbit-shared only
./scripts/run-tests.sh time-series  # Time series tests
./scripts/run-tests.sh ignored      # Slow integration tests
./scripts/run-tests.sh quick        # Compile check only
```

## Codebase Structure

```
orbit-rs/
├── orbit/                    # Main source code (15 workspace crates)
│   ├── server/              # Main server binary - all protocols + actor system
│   ├── engine/              # Storage engine (RocksDB, Iceberg, LSM)
│   ├── compute/             # Hardware acceleration (SIMD, GPU, Neural)
│   ├── ml/                  # ML inference pipeline
│   ├── shared/              # Shared types, traits, security, SQL
│   ├── client/              # Client library (OrbitClient)
│   ├── proto/               # Protocol Buffer definitions
│   ├── operator/            # Kubernetes operator
│   ├── application/         # Application configuration
│   ├── cli/                 # Interactive CLI client
│   ├── util/                # Core utilities
│   ├── client-spring/       # Spring framework integration
│   ├── server-etcd/         # etcd integration
│   └── server-prometheus/   # Prometheus metrics
├── config/                   # Configuration + test TLS certs
├── docs/                     # Documentation (258 markdown files)
├── tests/                    # Integration tests (Python + BDD)
├── scripts/                  # Development and cluster scripts
├── orbit-python-client/      # Python SDK (non-Rust)
├── orbit-vscode-extension/   # VS Code extension (TypeScript)
├── benchmarks/               # Performance benchmarks (excluded from workspace)
├── helm/                     # Kubernetes Helm charts
└── k8s/                      # Kubernetes manifests
```

## Key Workspace Crates

| Crate | Purpose |
|-------|---------|
| `orbit-server` | Main binary — serves all protocols, actor system, load balancer |
| `orbit-engine` | Unified storage (RocksDB, LSM, Iceberg, tiered storage) |
| `orbit-compute` | Hardware acceleration (SIMD/AVX-512, GPU/Metal/CUDA/Vulkan) |
| `orbit-ml` | ML inference pipeline |
| `orbit-shared` | Core traits, error types, clustering, pooling, security |
| `orbit-client` | Actor messaging and service discovery |
| `orbit-cli` | Interactive CLI with syntax highlighting |
| `orbit-operator` | Kubernetes operator |

## Protocol Ports (Default)

| Protocol | Port | Usage |
|----------|------|-------|
| PostgreSQL | 5432 | SQL with pgvector support |
| MySQL | 3306 | MySQL-compatible SQL |
| CQL (Cassandra) | 9042 | Wide-column queries |
| Redis RESP | 6379 | Key-value + vector ops |
| HTTP REST | 8080 | JSON API |
| gRPC | 50051 | Actor management |

## Development Conventions

### Code Style
- **Zero warnings policy**: Code must compile with no warnings
- **Clippy compliance**: `make check` must pass (includes targeted allows)
- **Formatting**: Always run `make format` before committing
- **Cognitive complexity**: Threshold of 15 (configured in `Cargo.toml`)

### Naming Conventions
- **Crates**: `orbit-{name}` (e.g., `orbit-server`, `orbit-engine`)
- **Modules**: snake_case
- **Types**: PascalCase
- **Functions/methods**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE

### Error Handling
- `anyhow::Result` for application errors
- `thiserror` for library error types
- Custom errors in `orbit-shared/src/error.rs`

### Async Runtime
- Tokio with `#[tokio::main]` for binaries
- `#[tokio::test]` for async tests

## TLS Integration Tests

TLS tests require self-signed certificates in `config/certs/`. These tests are marked `#[ignore]` and must be run explicitly:

```bash
make test-ignored
# Or directly:
cargo test -p orbit-server --test lb_tls_test -- --ignored
```

## Important Notes

### Benchmarks Excluded
The `benchmarks/` directory is excluded from the workspace. Run separately:
```bash
cd benchmarks
cargo bench --bench actor_benchmarks
```

### Custom SQLx Fork (Removed)
The project previously used a patched SQLx from `github.com/TuringWorks/sqlx-no-rsa` to fix RSA vulnerability (RUSTSEC-2023-0071). As of the 2026-05-25 dependency update, this fork has been removed in favor of `sqlx 0.9` from crates.io, which includes the fix upstream.

### Pre-commit Hooks
Install hooks for automatic quality checks:
```bash
make pre-commit-full   # Full checks including tests
make pre-commit-light  # Format + clippy only
```

### Runtime Data Directories
The following directories contain runtime data and are git-ignored:
- `orbit/server/data/` — Server runtime data (RocksDB, WAL, etc.)
- `/data/`, `data/*/` — Various data storage directories
- `demo_cow_data/`, `orbit_integrated_data*/` — Example/test data

Never commit database files to the repository.

## Documentation

- **Architecture Reference**: [`specifications/PRD.md`](specifications/PRD.md) — Single source of truth for modules and architecture
- **Architecture Details**: [`docs/content/architecture/ORBIT_ARCHITECTURE.md`](docs/content/architecture/ORBIT_ARCHITECTURE.md) — Detailed patterns and design decisions
- **Main docs**: `docs/` directory (258 files)
- **API docs**: `cargo doc --workspace --open`
- **Obsidian**: `/Volumes/fast01/obsidian/documents/myWork/Projects/orbit-rs/`

## PRD.md and ORBIT_ARCHITECTURE.md Maintenance (REQUIRED)

**IMPORTANT**: When making architectural changes, you MUST update the appropriate documentation files.

### When to Update PRD.md
- Add new modules, crates, or significant files
- Change directory structures
- Add or modify protocol implementations
- Update feature flags or capabilities
- Change API interfaces or command support
- Modify storage or compute backends
- Add new AI subsystems or features

### When to Update ORBIT_ARCHITECTURE.md
- Change transaction layer implementation (MVCC, 2PC, Saga)
- Modify query execution patterns (vectorization, SIMD)
- Update clustering or replication logic (Raft, CDC)
- Change network layer (gRPC services, transport)
- Alter storage architecture (tiering, actor patterns)
- Add new architectural patterns or design decisions

### Client SDK & Extension Updates (Breaking Changes)

When making breaking changes to protocols or APIs, also update:
- `orbit-python-client/` — Python SDK (new commands, API changes)
- `orbit-vscode-extension/` — VS Code extension (syntax, connections)

| Change Type | Files to Update |
|-------------|-----------------|
| New Redis commands | `orbit-python-client/orbit_client/client.py` |
| New query syntax | `orbit-vscode-extension/syntaxes/*.tmLanguage.json` |
| Protocol changes | Both Python client and VS Code extension |
| New connection types | `orbit-vscode-extension/src/connections/` |

### Update Workflow
```bash
# After making code changes:
1. Update relevant sections in specifications/PRD.md
2. Run: make commit-ready
3. Commit both code and PRD.md changes together
4. Include "docs: update PRD.md" in commit message if PRD changes are significant
```

## Common Tasks

### Adding a New Feature
1. Create feature branch
2. Implement in appropriate crate
3. Add tests
4. **Update `specifications/PRD.md`** if architecture changed
5. Run `make commit-ready`
6. Submit PR

### Debugging Protocol Issues
- PostgreSQL: `orbit/server/src/protocols/postgres_wire/`
- Redis: `orbit/server/src/protocols/resp/`
- MySQL: `orbit/server/src/protocols/mysql/`
- CQL: `orbit/server/src/protocols/cql/`

### Working with Storage
- RocksDB backend: `orbit/server/src/persistence/rocksdb.rs`
- Memory backend: `orbit/server/src/persistence/memory.rs`
- Engine adapters: `orbit/engine/src/adapters/`

## Contact & Resources

- **Issues**: https://github.com/TuringWorks/orbit-rs/issues
- **Discussions**: https://github.com/TuringWorks/orbit-rs/discussions
- **Roadmap**: `docs/roadmap.md` or GitHub Project
