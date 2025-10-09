# Rust Coding Standards — LLM Execution Playbook

> Purpose: Give a large language model (LLM) precise, deterministic rules to **write, modify, and review** Rust code in a way that a human team can trust. Follow the checklists and command recipes _exactly_. When unsure, use the **Defaults** section at the end.

---

## 0) Toolchain & Global Constraints (MUST)
- **Edition**: `2021` (upgrade to `2024` only if the repo explicitly says so).
- **MSRV** (Minimum Supported Rust Version): **pin in `rust-toolchain.toml`** (example: `1.79.0`). Never require features beyond MSRV.
- **Formatting**: `rustfmt` required. CI fails if not formatted.
- **Lints**: Run `clippy` on **all targets** and **all features**; fail on warnings.
- **Docs**: `cargo doc --no-deps` must succeed; all **public** items documented.
- **License & Security**: run `cargo deny` and `cargo audit` in CI; fail on vulnerable or yanked crates.

**Files to create/update at repo root** (idempotent):
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
format_code_in_doc_comments = true
```

```toml

# clippy.toml
warns = ["clippy::pedantic", "clippy::nursery"]
msrv = "1.79"
allow-unwrap-in-tests = true
```

**CI Commands (must pass):**
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --no-deps
cargo audit
cargo deny check
```

---

## 1) Repository & Crate Layout (MUST)
- Use a **workspace** with clear boundaries:
```
repo/
  Cargo.toml            # [workspace]
  crates/
    app/                # binary: CLI or service entrypoint
    core/               # library: domain logic (pure, no IO side effects)
    infra/              # library: IO adapters (DB, FS, HTTP, queues)
    macros/             # optional: proc-macro crate
```
- **Library crates** (e.g., `core`) MUST NOT panic for recoverable errors. Provide typed errors.
- **Binary crates** (e.g., `app`) MAY use `anyhow` at the top boundary to aggregate errors.
- Keep files **< 400 lines**; split by **domain concepts** not technical layers.

---

## 2) Naming, Style & Conventions (MUST)
- Crates: `kebab-case`; modules/files: `snake_case`; types/traits/enums: `UpperCamelCase`; functions/vars: `snake_case`.
- Prefer **expressive identifiers** over comments: `retry_backoff_ms` not `rbms`.
- Constants: `SCREAMING_SNAKE_CASE`.
- Public API items **must** include `///` docs with a short summary and example when relevant.

---

## 3) Error Handling (MUST)
- In libraries, define a typed error with `thiserror`; never `.unwrap()` or `.expect()` except for **proven invariants** (document why).
- Return `Result<T, Error>` from fallible functions.
- Add **context at boundaries** (e.g., IO, FFI, network) to preserve causality.
- In binaries, use `anyhow::Result` in `main()` and `.context("action")` at call sites.

```rust
// crates/core/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("missing config key: {0}")]
    MissingConfig(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

```rust
// crates/app/src/main.rs
use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let cfg = load_config().context("loading config")?;
    run(cfg).context("running service")?;
    Ok(())
}
```

**LLM rule**: If a function can fail, **never** return `Option<T>`—use `Result<T, E>` and define `E`.

---

## 4) Panics (MUST-NOT with Exceptions)
- **Never** panic in library public surfaces for recoverable issues.
- **Allowed**:
  - Tests and examples.
  - Truly unreachable states with a **documented** invariant (`debug_assert!`/`unreachable!()`).
- Use `.expect("why it cannot fail")` only when the reason is clear and documented.

---

## 5) Concurrency & Async (SHOULD)
- Use a single async runtime per process (`tokio` preferred).
- **No blocking calls in async contexts**. If unavoidable, use `tokio::task::spawn_blocking`.
- Prefer message passing (`tokio::sync::mpsc`) over shared mutexes.
- Trait objects crossing threads: annotate with `dyn Trait + Send + Sync` when required.

```rust

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            tracing::info!(%msg, "processing");
        }
    });
    tx.send("work".into()).await?;
    Ok(())
}
```

---

## 6) API & Type Design (SHOULD)
- Keep traits **small and focused** (Interface Segregation).
- Use **newtypes** for units/IDs to avoid parameter mix-ups: `struct Port(u16);`
- Prefer **builders** for complex config structs.
- Put **Serde** derives on **DTO/adapters**, not core domain structs (keep domain IO-agnostic).
- Time handling: use one library (`time` or `chrono`), UTC storage, explicit offsets in APIs.

```rust
pub struct Port(u16); // newtype

pub trait Repository {
    fn get(&self, id: Id) -> Result<Entity, RepoError>;
}
```

---

## 7) Logging, Tracing & Metrics (MUST)
- Use `tracing` and `tracing-subscriber` with structured fields.
- Wrap request/job lifecycles in `#[tracing::instrument]` or explicit spans.
- Do **not** log secrets or PII.
- Add metrics (`metrics` crate or OpenTelemetry) for hot paths when applicable.

```rust

#[tracing::instrument(skip(self), fields(user_id=%uid))]
pub async fn handle(&self, uid: UserId) -> anyhow::Result<()> {
    tracing::info!("handling request");
    Ok(())
}
```

---

## 8) Unsafe, Performance & Memory (MUST)
- **Unsafe** is disallowed by default. If needed: isolate in `unsafe_` module with proofs; add tests and fuzzing.
- Prefer borrowing (`&str`, `&[T]`) over owning (`String`, `Vec<T>`).
- Benchmark with `criterion` before micro-optimizing.

---

## 9) Tests (MUST)
- **Unit tests** near the code (`mod tests`).
- **Integration tests** in `tests/` using only public APIs.
- **Property tests** with `proptest` for core invariants.
- Async tests use `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.
- Test data via helper builders; avoid hidden global state.

```rust

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_id() {
        assert!(Id::parse("abc_123").is_ok());
    }
}
```

---

## 10) Dependencies (MUST)
- Prefer actively maintained crates. Avoid abandoned ones.
- Avoid wildcard versions. Turn off unnecessary default features:
```toml
some-crate = { version = "1", default-features = false, features = ["x"] }
```
- Periodically run `cargo tree -d` and remove duplicates/conflicts.

---

## 11) Documentation & Examples (MUST)
- Crate-level `//!` explains purpose, invariants, and a minimal example.
- Public items have `///` docs; include `# Examples` that compile (`doctest`).
- Place runnable examples in `examples/`. Keep them building in CI.
- Maintain a `CHANGELOG.md` (Keep a Changelog format).

---

## 12) Service & CLI Conventions (SHOULD)
- **CLI**: use `clap` derive; support `--config`, `-v/-q` for verbosity; print helpful `--help`.
- **Service**: graceful shutdown, health/readiness endpoints, layered config (env + file + defaults).

---

## 13) Code Review Checklist (LLM must enforce)
1. Ownership & borrowing correct; no unnecessary `clone()`.
2. No `unwrap/expect/panic` in library paths.
3. Errors are typed and add context at boundaries.
4. `tracing` present at hot paths; no secret leakage.
5. Features minimal and orthogonal; no coupling.
6. Tests cover happy paths + edge cases; docs updated.
7. CI passes: fmt, clippy (deny warnings), tests, docs, audit, deny.

---

## 14) Machine-Readable Policy (YAML for CI/bots)
```yaml
rust_policy:
  edition: "2021"
  msrv: "1.79.0"
  formatting:
    rustfmt: required
  lints:
    clippy:
      deny_warnings: true
      pedantic: true
      nursery: true
  docs:
    require_public_item_docs: true
  security:
    cargo_audit: required
    cargo_deny: required
  libraries:
    disallow_panic_in_public_api: true
    typed_errors: true
  async:
    single_runtime: tokio
    forbid_blocking_in_async: true
  logging:
    tracing_required: true
    forbid_secret_logging: true
  testing:
    unit: required
    integration: required
    property: recommended
  serialization:
    serde_in_adapters_only: true
```
Bots should fail the PR if any rule is violated.

---

## 15) Quick Command Cookbook (copy/paste)
```bash

# format & lint
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings

# test & doc
cargo test --workspace --all-features
cargo doc --no-deps --document-private-items

# security & licensing
cargo audit
cargo deny check

# dependency hygiene
cargo update -p <crate> --precise <version>
cargo tree -d
```

---

## 16) Defaults (When Unsure)
- Prefer **typed errors** + `Result` over `Option` or panics.
- Prefer **borrowing** over cloning; use `to_owned()` only with justification.
- Prefer **DTO adapters** at IO boundaries; keep domain types pure.
- Add **`tracing` instrumentation** on public async entry points.
- Write **docs + tests** for every new public function or type.

---

## 17) 2024 Edition Migration (Optional When Repo Opts In)
- Run `cargo fix --edition` first.
- Recheck async trait bounds and `impl Trait` changes.
- Keep MSRV policy aligned with edition bump; update `clippy.toml: msrv`.
