# AI Agent Instructions for Orbit-RS

This document provides instructions for AI coding assistants (Gemini, Copilot, Warp, Antigravity, Claude, and others) working with the Orbit-RS codebase.

## Required Reading

Before making any changes, read these files:
1. **`specifications/PRD.md`** — Single source of truth for architecture and modules
2. **`docs/content/architecture/ORBIT_ARCHITECTURE.md`** — Detailed architecture patterns and implementation details
3. **`CLAUDE.md`** — Quick development reference and commands

## Architecture Reference

**`specifications/PRD.md` is the authoritative reference** for:
- Workspace structure and all 15 crates
- Module directory layouts and file purposes
- Protocol implementations (PostgreSQL, MySQL, CQL, Redis, REST, gRPC)
- Storage architecture (RocksDB, Memory, Iceberg)
- AI-native subsystems
- Feature status matrix with test counts
- Development guidelines

**`docs/content/architecture/ORBIT_ARCHITECTURE.md` is the authoritative reference** for:
- Detailed architecture patterns and design decisions
- Transaction layer (MVCC, 2PC, deadlock detection, Saga pattern)
- Query execution (vectorized, SIMD, columnar format)
- Clustering and replication (Raft consensus, CDC)
- Network layer (gRPC services, Protocol Buffers, transport layer)
- Hybrid storage architecture (actor-based RESP vs direct storage)
- Performance characteristics and trade-offs

## PRD.md and ORBIT_ARCHITECTURE.md Maintenance (MANDATORY)

**When making architectural changes, you MUST update both `specifications/PRD.md` and `docs/content/architecture/ORBIT_ARCHITECTURE.md`.**

### Triggers for Updates
Update **PRD.md** when you:
- Add new modules, crates, or significant source files
- Change directory structures or file organization
- Add or modify protocol implementations or commands
- Update feature flags or cargo features
- Change API interfaces or add new endpoints
- Modify storage or compute backends
- Add new AI subsystems or capabilities
- Change test coverage significantly

Update **ORBIT_ARCHITECTURE.md** when you:
- Change transaction layer implementation (MVCC, 2PC, Saga)
- Modify query execution patterns (vectorization, SIMD)
- Update clustering or replication logic (Raft, CDC)
- Change network layer (gRPC services, transport)
- Alter storage architecture (tiering, actor patterns)
- Add new architectural patterns or design decisions
- Modify performance characteristics or trade-offs

### Client SDK & Extension Updates (Breaking Changes)

**When making breaking changes, also update external clients:**

| Change Type | Update Required |
|-------------|-----------------|
| New Redis/RESP commands | `orbit-python-client/orbit_client/client.py` |
| New PostgreSQL features | `orbit-python-client/orbit_client/protocols.py` |
| Protocol wire format | Both Python client and VS Code extension |
| New query languages | `orbit-vscode-extension/syntaxes/` |
| New connection types | `orbit-vscode-extension/src/connections/` |

### Change Management Workflow
```
1. Read specifications/PRD.md to understand current architecture
2. Make code changes
3. Update specifications/PRD.md to reflect changes
4. Run: make format
5. Run: make check
6. Run: make test
7. Commit code AND PRD.md changes together
8. Use descriptive commit messages
```

## Code Standards

### Build and Test (Make)

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
make commit-ready       # Format + check + test (recommended)
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

### Quality Requirements
- Zero compiler warnings
- Clippy must pass with `-D warnings` (plus targeted allows in Makefile)
- All tests must pass (except `#[ignore]` TLS integration tests)
- Code must be formatted with rustfmt (`make format`)

### Conventions
- **Crates**: `orbit-{name}` (orbit-server, orbit-engine, etc.)
- **Modules**: snake_case
- **Types**: PascalCase
- **Functions**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE

### Error Handling
- `anyhow::Result` for application errors
- `thiserror` for library error types
- Custom errors in `orbit-shared/src/error.rs`

### Async Runtime
- Tokio with `#[tokio::main]` for binaries
- `#[tokio::test]` for async tests

### Code Design Principles

Beyond passing `make check`, write code that is idiomatic, functional-leaning, and operable. These apply to all new and refactored Rust code.

#### Rust Idioms & Design Patterns
- **Model with the type system.** Use the newtype pattern (`struct NodeId(Uuid)`) to give primitives meaning; make illegal states unrepresentable with enums rather than boolean flags or sentinel values.
- **Builder / type-state patterns** for multi-step construction and configuration; prefer a builder over functions with many `Option` parameters.
- **Program to traits, not concretes.** Define behavior in traits (as `PersistenceProvider` does); use `impl Trait`/generic bounds for static dispatch on hot paths and `dyn Trait` behind `Arc` for pluggable backends.
- **RAII for resources.** Encode acquire/release of locks, connections, leases, and transactions in ownership and `Drop`; never expose manual `close()`-then-forget lifecycles.
- **Conversions via traits.** Implement `From`/`TryFrom` instead of ad-hoc `to_x`/`parse_x` helpers; accept `impl AsRef<str>` / `impl Into<T>` at API boundaries.
- **Keep public APIs evolvable** with sealed traits and `#[non_exhaustive]` on public enums and error types; add `#[must_use]` to guards, builders, and `Result`-like handles.

#### Functional Style
- **Immutability by default** — prefer `let` over `let mut`; use `mut` only when it measurably simplifies or speeds up the code.
- **Iterators over manual loops** — express transformations as `iter().map().filter().collect()` / `fold` / `try_fold` chains rather than index loops with mutable accumulators.
- **Combinators over branching** — use `Option`/`Result` combinators (`map`, `and_then`, `ok_or`, `unwrap_or_else`, `?`) and `match` instead of nested `if let` ladders.
- **Pure functions at the core, effects at the edges.** Keep business/query logic side-effect-free and testable; push I/O, logging, and mutation to the boundaries.
- **Avoid shared mutable state**; when unavoidable, isolate it behind an actor, a channel, or a single documented `Arc<Mutex<_>>`/`RwLock<_>`.

#### Reliability & Maintainability
- **Never `.unwrap()`/`.expect()`/`panic!` in non-test, non-`main` code** — propagate with `?`, model errors as `thiserror` variants, and add context via `anyhow::Context`.
- **Exhaustive `match`** — avoid catch-all `_ =>` arms on domain enums so new variants surface as compile errors.
- **Small, single-responsibility functions** that respect the cognitive-complexity-15 limit.
- **Document every public item** with `///` (with an example and `# Errors` / `# Panics` sections where relevant).
- **`unsafe` is a last resort** — justify each block with a `// SAFETY:` comment and cover it with tests.
- **Test the contract, not the implementation** — prefer property/table-driven tests for pure logic; keep async tests deterministic.

#### 12-Factor App Principles (where applicable)
Orbit-RS already uses `tracing` + `tracing-subscriber` (env-filter), `serde`/TOML config, `clap`, and graceful shutdown — build on these:
- **III. Config in the environment** — read tunables from env vars layered over `config/orbit-server.toml`; never hardcode ports, hosts, credentials, or paths; keep secrets out of source.
- **IV. Backing services as attached resources** — treat RocksDB, S3/Iceberg, etcd, TiKV, and peer nodes as swappable resources addressed by config/URL (the `PersistenceProvider` trait models this).
- **VI. Stateless, share-nothing processes** — keep durable state in backing services; a restart must be safe and actor/session state recoverable or replicated.
- **IX. Disposability** — fast startup, graceful shutdown on SIGTERM/ctrl-c (drain connections, flush WAL, release leases); make operations crash-safe and idempotent where possible.
- **XI. Logs as event streams** — emit structured events via `tracing` to stdout/stderr; never manage log files in-process; use spans for context and `RUST_LOG` for verbosity.
- **X. Dev/prod parity** — same binary and config schema across dev, cluster, and Kubernetes; express differences through config/env, not `#[cfg]` forks of behavior.

## Key Directories

```
orbit-rs/
├── orbit/                    # Source code (15 workspace crates)
│   ├── server/              # Main binary - all protocols + actor system
│   ├── engine/              # Storage engine (RocksDB, Iceberg, LSM)
│   ├── compute/             # Hardware acceleration (SIMD, GPU)
│   ├── ml/                  # ML inference pipeline
│   ├── shared/              # Shared types, traits, security, SQL
│   ├── client/              # Client library (OrbitClient)
│   ├── proto/               # Protocol Buffer definitions
│   ├── operator/            # Kubernetes operator
│   ├── application/         # Application configuration
│   ├── cli/                 # Interactive CLI client
│   ├── util/                # Core utilities
│   ├── client-spring/       # Spring framework integration
│   ├── server-etcd/         # etcd distributed directory
│   └── server-prometheus/   # Prometheus metrics
├── config/                   # Configuration + test TLS certs
├── docs/                     # Documentation (258 markdown files)
├── tests/                    # Integration tests
├── scripts/                  # Development and cluster scripts
├── orbit-python-client/      # Python SDK
├── orbit-vscode-extension/   # VS Code extension
└── benchmarks/               # Performance benchmarks (excluded from workspace)
```

## Protocol Ports

| Protocol   | Port  |
|------------|-------|
| PostgreSQL | 5432  |
| MySQL      | 3306  |
| CQL        | 9042  |
| Redis      | 6379  |
| REST       | 8080  |
| gRPC       | 50051 |

## Commit Message Format

```
type(scope): description

[body - optional]

🤖 Generated with [AI Assistant Name]
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`

## Important Reminders

1. **Always read PRD.md first** before making architectural decisions
2. **Always update PRD.md** when architecture changes
3. **Never commit without running tests** (`make commit-ready`)
4. **Keep PRD.md synchronized** with actual codebase
5. **Commit PRD.md changes together** with code changes
6. **Use `make` targets** for all build, test, and run operations

## Agent-Specific Files

- `AGENTS.md` — This file (generic AI agents)
- `CLAUDE.md` — Claude Code / Anthropic Claude instructions
- `.cursorrules` — Cursor AI instructions
