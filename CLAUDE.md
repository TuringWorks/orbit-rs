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

All development tasks use `make` targets — see the **orbit-dev-commands** skill for the full build / test / run / cluster / load-balancer reference (loaded on demand), or run `make help` / read the root `Makefile`. Most-used: `make dev`, `make test`, `make check`, `make commit-ready`, `make cluster`.

> Module layout and the crate list live in [`specifications/PRD.md`](specifications/PRD.md) (the single source of truth) — read it rather than duplicating them here.

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

## Code Design Principles

Beyond passing `make check`, write code that is idiomatic, functional-leaning, and operable. These apply to all new and refactored Rust code.

### Rust Idioms & Design Patterns
- **Model with the type system.** Use the newtype pattern (`struct NodeId(Uuid)`) to give primitives meaning and prevent mix-ups. Make illegal states unrepresentable with enums rather than boolean flags or sentinel values.
- **Builder / type-state patterns** for multi-step construction and configuration (server, cluster, connection, transaction builders). Prefer a builder over functions with many `Option` parameters.
- **Program to traits, not concretes.** Define behavior in traits (as `PersistenceProvider` already does); use `impl Trait`/generic bounds for static dispatch on hot paths and `dyn Trait` behind `Arc` for pluggable backends.
- **RAII for resources.** Encode acquire/release of locks, connections, leases, and transactions in ownership and `Drop`; never expose a manual `close()`-then-forget lifecycle.
- **Conversions via traits.** Implement `From`/`TryFrom` instead of ad-hoc `to_x`/`parse_x` helpers; accept `impl AsRef<str>` / `impl Into<T>` at API boundaries.
- **Keep public APIs evolvable** with sealed traits and `#[non_exhaustive]` on public enums and error types.
- Add `#[must_use]` to functions returning guards, builders, or `Result`-like handles that must not be silently dropped.

### Functional Style
- **Immutability by default.** Prefer `let` over `let mut`; reach for `mut` only when it measurably simplifies or speeds up the code.
- **Iterators over manual loops.** Express transformations as `iter().map().filter().collect()` / `fold` / `try_fold` chains rather than index loops with mutable accumulators.
- **Combinators over branching.** Use `Option`/`Result` combinators (`map`, `and_then`, `ok_or`, `unwrap_or_else`, `?`) and `match` instead of nested `if let` ladders.
- **Pure functions at the core, effects at the edges.** Keep business/query logic in side-effect-free functions that take inputs and return values; push I/O, logging, and mutation to the boundaries. Pure logic is trivially testable.
- **Avoid shared mutable state.** When it is unavoidable, isolate it behind an actor, a channel, or a single `Arc<Mutex<_>>`/`RwLock<_>` with a documented invariant.

### Reliability & Maintainability
- **Never `.unwrap()`/`.expect()`/`panic!` in non-test, non-`main` code.** Propagate with `?`, model errors as `thiserror` variants, and add context with `anyhow::Context` (`.with_context(|| ...)`).
- **Exhaustive `match`.** Avoid catch-all `_ =>` arms on domain enums so new variants surface as compile errors.
- **Small, single-responsibility functions** that respect the cognitive-complexity-15 limit. Extract helpers rather than nest deeply.
- **Document every public item** with `///`, including an example and `# Errors` / `# Panics` sections where relevant.
- **`unsafe` is a last resort** — justify each block with a `// SAFETY:` comment and cover it with tests.
- **Test the contract, not the implementation.** Prefer property/table-driven tests for pure logic; keep async tests deterministic.

### Modelling Honesty
A model that cannot be wrong is not a model. These are correctness rules, not style:
- **Ask what a default asserts.** `unwrap_or(0)` on a count asserts "none" — usually true. `unwrap_or_else(Utc::now)` on a record's timestamp asserts "this happened now" — a claim about the world nobody checked, and it stamps every imported row with the import time. Absent data stays absent: model it (`Option`, or a documented sentinel), refuse to derive from it, and surface it as unknown.
- **A clamp is not a value.** When a guard rail binds, say so in the type (return the clamped flag alongside the value) rather than silently substituting a bound that reads as a real measurement.
- **Decorative parameters invite false confidence.** If a config knob or tuning parameter can be removed without changing any output, it is not doing anything — delete it or wire it up.
- **Prefer unit-free derivations.** Where two provider/config fields meet in one expression, cross-check against a ratio that carries no units.

### Performance
Measure before optimizing; the order matters more than the micro-work:
1. **Measure** a number, not a hunch — CPU, memory, or latency? Idle CPU is the cheapest health check and almost nothing watches it.
2. **Attribute from the call tree**, not the leaf histogram. A flat "hottest functions" list names symptoms; only the tree says who asked for the work. Use `cargo flamegraph`, `dhat` for allocations, `tokio-console` for task stalls.
3. **Fix in yield order** — cadence (is this poll/tick running more often than the data changes?), eager work (built before it is needed?), reuse (rebuilt per iteration?), redundant notification (does it wake watchers when nothing changed?) — *then* algorithms and allocation.
4. **Re-measure like-for-like**, same protocol and warm-up; quote the stable extreme of a noisy counter and say it is noisy.
5. **Verify the feature still works.** A performance number that improved because a code path stopped doing its job is the easiest way to ship a regression while celebrating it.

Do not optimize what has not been measured as a problem. When leaving a latent issue alone, write down why.

### Allocation Discipline (hot paths only, after measuring)
- **Reuse over recreate** — hoist buffers/`Vec`s out of loops; keep scratch space on the struct.
- **`with_capacity`/`reserve`** whenever the size is known or estimable.
- **Borrow, don't clone.** Clone for ownership, never to quiet the borrow checker. Take `&str`/`&[T]`/`impl AsRef<_>` at boundaries.
- **Flat over pointer-chasing** — `Vec` and flat maps beat node-per-entry trees; store indices (`u32`) rather than pointers in transient containers; flatten nested maps behind a compound key.
- **Cheap reject before expensive check** — a length or first-byte test before a regex, hash, or allocation; fast path first, cold handling `#[cold]`/out-of-line.
- **Batch** to amortize per-call overhead, and **sample** high-frequency metrics (one in 32 via a power-of-two mask) so instrumentation does not dominate what it measures.
- **Bound anything that grows.** A cache that only ignores stale entries but never evicts them grows forever.

### Verification — a green build proves almost nothing
`make check` passing is the floor, not evidence the change works. In yield order:
- **Run it and read stderr.** `make dev` / start the server and watch the log.
- **Exercise the path you changed** with a real client (`psql`, `redis-cli`, `cqlsh`, `curl`) — code reachable only from an untested path is unverified no matter how green the build.
- **Reconcile one number against an external reference** — protocol conformance against the real server's behavior, not against your own expectation.
- **Prove the artifact carries the change** when packaging or deploying; verify the binary, not that files were copied.
- **Audit affordances.** For enum/registry dispatch, confirm every variant appears at a call site — the compiler will not tell you when the enum is data rather than control flow. A schema, a config key, or a trait impl that nothing calls is not a feature.
- **Duplicated contracts drift silently.** If a `.proto`, schema, or command table exists in two places with no codegen between them, diff them in CI.
- **"An error appeared after my change" ≠ "my change caused it."** Check provenance before assuming causation, and say which it was.

### 12-Factor App Principles (where applicable)
Orbit-RS already uses `tracing` + `tracing-subscriber` (env-filter), `serde`/TOML config, `clap`, and graceful shutdown — build on these:
- **III. Config in the environment.** Read tunables from env vars layered over `config/orbit-server.toml`; never hardcode ports, hosts, credentials, or paths. Secrets come from env or a secret store, never source.
- **IV. Backing services as attached resources.** Treat RocksDB, S3/Iceberg, etcd, TiKV, and peer nodes as swappable resources addressed by config/URL — swapping a backend requires no code change (the `PersistenceProvider` trait already models this).
- **VI. Stateless, share-nothing processes.** Keep durable state in backing services; a process restart must be safe, and actor/session state must be recoverable or replicated.
- **IX. Disposability.** Fast startup and graceful shutdown on SIGTERM/ctrl-c (drain connections, flush WAL, release leases); make operations crash-safe and idempotent where possible.
- **XI. Logs as event streams.** Emit structured events via `tracing` to stdout/stderr; never manage log files in-process. Use spans for request/transaction context and control verbosity with `RUST_LOG`.
- **X. Dev/prod parity.** Same binary and config schema across dev, cluster, and Kubernetes; express differences through config/env, not `#[cfg]` forks of behavior.

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
