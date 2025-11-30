# CLAUDE.md - AI Assistant Guide for Orbit-RS

This document provides essential context for AI assistants working with the Orbit-RS codebase.

> **Architecture Reference**: For detailed module structures, implementation patterns, and feature status, see [`docs/PRD.md`](docs/PRD.md) - the single source of truth for Orbit-RS architecture.

## Project Overview

**Orbit-RS** is a high-performance, distributed multi-protocol database server written in Rust. It natively implements PostgreSQL, MySQL, CQL (Cassandra), Redis, HTTP REST, gRPC, and OrbitQL protocols from a single process, sharing a unified storage layer.

- **Repository**: https://github.com/TuringWorks/orbit-rs
- **License**: BSD-3-Clause OR MIT
- **Rust Edition**: 2021
- **Minimum Rust Version**: 1.70+
- **Architecture**: See [`docs/PRD.md`](docs/PRD.md) for complete module reference

## Quick Reference Commands

```bash
# Build
cargo build --workspace              # Build all crates
cargo build --release                # Release build

# Test
cargo test --workspace               # Run all tests
cargo test --workspace --verbose     # Verbose test output
cargo test -p orbit-server           # Test specific package

# Lint & Format
cargo fmt --all                      # Format code (ALWAYS run before commit)
cargo clippy --workspace --all-targets -- -D warnings  # Lint with warnings as errors

# Combined workflows (recommended)
make commit-ready                    # Format + check + test (full pre-commit)
make commit-light                    # Format + check only (faster)
make all                             # Format + check + test + build
```

## Codebase Structure

```
orbit-rs/
├── orbit/                    # Main source code (15 workspace crates)
│   ├── server/              # Main server binary - all protocols
│   ├── protocols/           # Protocol implementations (RESP, PostgreSQL, MySQL, CQL, etc.)
│   ├── engine/              # Storage engine (RocksDB, LSM, Iceberg)
│   ├── compute/             # Hardware acceleration (SIMD, GPU, Neural)
│   ├── ml/                  # Machine learning inference
│   ├── shared/              # Shared types, traits, clustering
│   ├── client/              # Client library and actor system
│   ├── util/                # Core utilities
│   ├── proto/               # Protocol Buffer definitions
│   ├── cli/                 # Interactive CLI client
│   ├── operator/            # Kubernetes operator
│   ├── application/         # Application configuration
│   ├── client-spring/       # Spring framework integration
│   ├── server-etcd/         # etcd integration
│   └── server-prometheus/   # Prometheus metrics
├── orbit-python-client/      # Python SDK (non-Rust)
├── orbit-vscode-extension/   # VS Code extension (TypeScript)
├── config/                   # Configuration files
├── docs/                     # Documentation (258 markdown files)
├── tests/                    # Integration tests (Python + BDD)
├── benchmarks/               # Performance benchmarks (excluded from workspace)
├── helm/                     # Kubernetes Helm charts
└── k8s/                      # Kubernetes manifests
```

## Key Workspace Crates

| Crate | Purpose |
|-------|---------|
| `orbit-server` | Main binary - serves all protocols |
| `orbit-protocols` | Protocol implementations (RESP, PostgreSQL, MySQL, CQL, REST, gRPC) |
| `orbit-engine` | Unified storage (RocksDB, LSM, Iceberg, tiered storage) |
| `orbit-compute` | Hardware acceleration (SIMD/AVX-512, GPU/Metal/CUDA/Vulkan) |
| `orbit-ml` | ML inference pipeline |
| `orbit-shared` | Core traits, error types, clustering, pooling |
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
- **Clippy compliance**: `cargo clippy -- -D warnings` must pass
- **Formatting**: Always run `cargo fmt --all` before committing
- **Cognitive complexity**: Threshold of 15 (configured in `Cargo.toml`)

### Naming Conventions
- **Crates**: `orbit-{name}` (e.g., `orbit-server`, `orbit-engine`)
- **Modules**: snake_case
- **Types**: PascalCase
- **Functions/methods**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE

### Error Handling
- Use `anyhow::Result` for application errors
- Use `thiserror` for library error types
- Custom errors defined in `orbit-shared/src/error.rs`

### Async Runtime
- **Tokio** is the async runtime (version 1.48+, full features)
- Use `#[tokio::main]` for binaries
- Use `#[tokio::test]` for async tests

### Testing Patterns
- Unit tests: `#[cfg(test)]` modules in source files
- Integration tests: `orbit/*/tests/` directories
- Python integration tests: `tests/integration/`
- Use `mockall` for mocking traits
- Use `proptest` for property-based testing

## Architecture Patterns

> **Detailed Reference**: See [`docs/PRD.md`](docs/PRD.md) for complete module structures, directory layouts, and implementation details.

### Virtual Actor System
The core abstraction is virtual actors that:
- Activate on-demand
- Persist state automatically
- Distribute across cluster nodes
- Communicate via async messages

### Storage Tiers
- **Hot tier**: In-memory (fastest access)
- **Warm tier**: RocksDB/LSM (balanced)
- **Cold tier**: Apache Iceberg/Parquet (archival)

### Feature Flags
Key feature flags in `orbit-server`:
- `protocol-redis`, `protocol-postgres`, `protocol-mysql`, `protocol-cassandra`
- `storage-rocksdb`, `storage-memory`, `storage-iceberg`
- `ai-native-*` (8 AI subsystems)
- `gpu-acceleration`, `heterogeneous-compute`

## Running the Server

```bash
# Start with default config
cargo run --bin orbit-server

# Start with custom config
cargo run --bin orbit-server -- --config ./config/orbit-server.toml

# Development mode with debug logging
RUST_LOG=debug cargo run --bin orbit-server
```

## Testing

```bash
# All workspace tests (excludes slow tests)
cargo test --workspace

# Specific package tests
cargo test -p orbit-protocols
cargo test -p orbit-engine

# Run with output
cargo test --workspace -- --nocapture

# Run slow/ignored integration tests only
cargo test --workspace -- --ignored

# Run all tests including slow ones
cargo test --workspace -- --include-ignored

# Python integration tests
cd tests && python run_integration_tests.py
```

### Slow Tests
Some integration tests that start full server instances are marked with `#[ignore]` to keep regular test runs fast. These tests can be run explicitly using `--ignored` or `--include-ignored` flags.

## Important Notes

### Benchmarks Excluded
The `benchmarks/` directory is excluded from the workspace due to WAL replay issues. Run benchmarks separately:
```bash
cd benchmarks
cargo bench --bench actor_benchmarks
```

### Custom SQLx Fork
The project uses a patched SQLx from `github.com/ravituringworks/sqlx-no-rsa` to fix RSA vulnerability (RUSTSEC-2023-0071).

### Pre-commit Hooks
Install hooks for automatic quality checks:
```bash
make pre-commit-full   # Full checks including tests
make pre-commit-light  # Format + clippy only
```

### Runtime Data Directories
The following directories contain runtime data and are ignored by git:
- `orbit/server/data/` - Server runtime data (RocksDB, WAL, etc.)
- `/data/`, `data/*/` - Various data storage directories
- `demo_cow_data/`, `orbit_integrated_data*/` - Example/test data

Never commit database files (RocksDB manifests, WAL files, etc.) to the repository.

## Documentation

- **Architecture Reference**: [`docs/PRD.md`](docs/PRD.md) - Single source of truth for modules and architecture
- **Main docs**: `docs/` directory (258 files)
- **API docs**: `cargo doc --workspace --open`
- **Key documents**:
  - `docs/PRD.md` - **Complete architecture, modules, and feature status**
  - `docs/ORBITQL_COMPLETE_DOCUMENTATION.md` - SQL engine
  - `docs/GPU_ACCELERATION_COMPLETE.md` - Hardware acceleration
  - `docs/PERSISTENCE_COMPLETE_DOCUMENTATION.md` - Storage backends
  - `docs/PROTOCOL_ADAPTERS_INTEGRATION.md` - Protocol architecture

## PRD.md Maintenance (REQUIRED)

**IMPORTANT**: When making architectural changes, you MUST update `docs/PRD.md` to keep it synchronized with the codebase.

### When to Update PRD.md
Update PRD.md when you:
- Add new modules, crates, or significant files
- Change directory structures
- Add or modify protocol implementations
- Update feature flags or capabilities
- Change API interfaces or command support
- Modify storage or compute backends
- Add new AI subsystems or features

### What to Update
1. **Module Reference**: Update directory trees and file descriptions
2. **Feature Status Matrix**: Update implementation status and test counts
3. **Protocol Commands**: Add new commands or update existing ones
4. **Architecture Sections**: Reflect structural changes

### Update Workflow
```bash
# After making code changes:
1. Update relevant sections in docs/PRD.md
2. Run tests: cargo test --workspace
3. Commit both code and PRD.md changes together
4. Include "docs: update PRD.md" in commit message if PRD changes are significant
```

## Common Tasks

### Adding a New Feature
1. Create feature branch
2. Implement in appropriate crate
3. Add tests
4. **Update `docs/PRD.md`** if architecture changed
5. Run `make commit-ready`
6. Submit PR

### Debugging Protocol Issues
- PostgreSQL: `orbit/protocols/src/postgres_wire/`
- Redis: `orbit/protocols/src/resp/`
- MySQL: `orbit/protocols/src/mysql/`
- CQL: `orbit/protocols/src/cql/`

### Working with Storage
- RocksDB backend: `orbit/server/src/persistence/rocksdb.rs`
- Memory backend: `orbit/server/src/persistence/memory.rs`
- Engine adapters: `orbit/engine/src/adapters/`

## Contact & Resources

- **Issues**: https://github.com/TuringWorks/orbit-rs/issues
- **Discussions**: https://github.com/TuringWorks/orbit-rs/discussions
- **Roadmap**: `docs/roadmap.md` or GitHub Project
