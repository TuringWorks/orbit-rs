# Rust Coding Standards

## 0) Tooling Baseline
- **Edition**: `2021` (or `2024` when stable across toolchain).
- **Format**: `rustfmt` (enforced in CI).
- **Lints**: `clippy -D warnings` (deny warnings).
- **Docs**: `cargo doc --no-deps` must pass; public items documented.
- **MSRV**: Pin a **Minimum Supported Rust Version** in CI (e.g., 1.79).

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.79.0"
components = ["clippy", "rustfmt"]
profile = "minimal"
```

```toml
# .rustfmt.toml
edition = "2021"
use_small_heuristics = "Max"
normalize_doc_attributes = true
newline_style = "Unix"
group_imports = "StdExternalCrate"
imports_granularity = "Crate"
```

```toml
# clippy.toml
warns = ["clippy::pedantic", "clippy::nursery"]
allow-unwrap-in-tests = true
msrv = "1.79"
```

In CI:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
```

---

## 1) Project Structure
- **Workspaces**: Prefer workspace with clear `crates/` layout.
- **Binary vs Library**:
  - **Library crates**: Never panic for recoverable errors; stable API.
  - **Binary crates**: Can use `anyhow` for top-level error aggregation.
- **Modules**: Keep files < ~400 lines; split by domain not by layer.

```
repo/
  Cargo.toml
  crates/
    app/        # bin: CLI/service entrypoint
    core/       # lib: domain logic, pure Rust
    infra/      # lib: IO adapters (DB, FS, HTTP)
    macros/     # optional proc-macro crate
```

---

## 2) Naming & Style
- **Crates**: `kebab-case`.
- **Modules/files**: `snake_case`.
- **Types & Traits**: `UpperCamelCase`.
- **Functions/vars**: `snake_case`.
- **Constants**: `SCREAMING_SNAKE_CASE`.
- **Enum variants**: `UpperCamelCase`.
- Prefer *expressive* names over comments: `retry_backoff_ms` over `rb`.

---

## 3) Error Handling
- **Library crates**:
  - Create a typed error with `thiserror`.
  - Avoid `unwrap`, `expect`, and `panic!` in non-test code.
  - Return `Result<T, Error>`; attach context at boundaries.

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("config missing key: {0}")]
    MissingConfig(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

- **Binary crates**:
  - `anyhow::Result` at `main()` boundary; use `.context("...")`.

```rust
fn main() -> anyhow::Result<()> {
    let cfg = load_config().context("loading config")?;
    run(cfg).context("running service")?;
    Ok(())
}
```

- **Mapping**: Convert external errors at the **infrastructure boundary**; keep core errors domain-specific.

---

## 4) Panics & Unwraps
- **Allowed**:
  - In tests and examples.
  - For **truly unrecoverable invariants** (document why).
- **Not allowed**:
  - In library public surfaces for recoverable situations.
- Prefer `expect("why this cannot fail")` with a *clear* message if necessary.

---

## 5) Concurrency & Async
- **Runtime**: Prefer `tokio` for async; one runtime per process.
- **Traits**: Avoid blocking calls in async contexts; use `spawn_blocking` where needed.
- **Synchronization**: Prefer channels over shared locks.
- **Send/Sync**: Mark trait objects `dyn Trait + Send + Sync` where cross-thread.

---

## 6) API Design
- **Traits**: Define by capability. Keep small, cohesive method sets.
- **Generics**: Constrain with trait bounds in the where-clause for readability.
- **Newtypes**: Use newtypes for units/IDs to avoid parameter mixups.
- **Builders**: For structs with many optional fields, prefer the builder pattern.
- **Feature Flags**: Keep additive and orthogonal; avoid feature coupling.

---

## 7) Logging, Tracing & Metrics
- **Logging**: Use `tracing` with `tracing-subscriber`.
- **Structure**: Prefer structured fields over string interpolation.
- **Levels**: `error`, `warn`, `info` for ops; `debug`, `trace` for dev.
- **Spans**: Wrap request/Job lifecycles in spans.

---

## 8) Data & Serialization
- **Serde**: Derive in DTO crates only; domain types should be IO-agnostic.
- **Time**: Use UTC; consistent library (`time` or `chrono`).
- **IDs**: Use `uuid` or `ulid`; validate externally.

---

## 9) Unsafe & Performance
- **Unsafe**: Disallow by default; if required, isolate in `unsafe_` module with comments and tests.
- **Allocations**: Prefer borrowing (`&str`) over owned (`String`) when possible.
- **Benchmark**: Use `criterion` before micro-optimizations.

---

## 10) Testing Strategy
- **Unit tests** colocated with modules.
- **Integration tests** in `tests/` using public APIs.
- **Property tests** with `proptest` for core logic.
- **Fixtures**: Builder patterns for test data.

---

## 11) Security & Secrets
- No secrets in code. Use env vars or secret managers.
- Validate all external inputs.
- Use `Zeroize` for sensitive in-memory data.

---

## 12) Dependencies & Versions
- Prefer well-maintained crates.
- Pin direct dependencies carefully; no wildcards.
- Audit regularly with `cargo audit` and `cargo deny`.

---

## 13) Documentation
- Crate-level `//!` docs with purpose and examples.
- Public items must have `///` comments.
- Runnable examples in `examples/`.

---

## 14) CLI & Services Conventions
- **CLI**: Use `clap` derive.
- **Services**:
  - Graceful shutdown.
  - Health/ready endpoints.
  - Config layering (env + file + defaults).

---

## 15) Code Review Checklist
- No unnecessary clones, unwraps, or panics.
- Errors contextualized and typed.
- Tracing added at boundaries/hot paths.
- Tests cover happy path and edge cases.
- Docs and examples updated.

---

## 16) Starter Cargo.toml
- Library crates use `thiserror`; binaries may use `anyhow` and `clap`.

---

## 17) Example Patterns
- DTO ↔ domain adapters with `TryFrom`.
- Builder patterns for configs.

---

## 18) Migration to 2024 Edition
- Run `cargo fix --edition`.
- Audit async trait patterns.
