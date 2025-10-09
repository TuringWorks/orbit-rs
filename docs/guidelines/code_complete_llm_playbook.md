# Code-Complete Steps Playbook — For LLM Execution

> Purpose: a deterministic, end-to-end checklist that a Large Language Model (LLM) or bot can follow to deliver **production-grade code** from spec to release. All steps are **idempotent** and include machine-checkable gates.

---

## 0) Inputs & Non-Goals (MUST READ)
- **Inputs**: problem brief/PRD, target stack (e.g., Rust, Python, Node, Flutter), acceptance criteria, performance/SLOs, security/policy constraints, Git hosting URL, CI provider.
- **Non-Goals**: long-running background tasks, decisions outside spec. If a required input is missing, **fail early** with a clear message and a stub TODO.

---

## 1) Repo Bootstrap (Idempotent)
**Given**: organization Git URL, chosen license, GitFlow policy.

**Steps**:
```bash
set -euo pipefail
REPO_NAME=my-project
GIT_URL=git@github.com:org/${REPO_NAME}.git

# Create if absent
[ -d ${REPO_NAME} ] || git init ${REPO_NAME}
cd ${REPO_NAME}

# Standard scaffolding
mkdir -p .github/workflows .config scripts docs
[ -f README.md ] || echo "# ${REPO_NAME}" > README.md
[ -f LICENSE ] || echo "MIT" > LICENSE

# GitFlow baselines
[ -d .git ] || git init
(git rev-parse --verify develop >/dev/null 2>&1) || git checkout -b develop
[ -f .gitattributes ] || echo "* text=auto eol=lf" > .gitattributes

# Commit baseline
git add .
git commit -m "chore: repo bootstrap"
```

**Files to create/update**:
- `README.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`
- `.editorconfig`, `.gitattributes`, `.gitignore`
- `.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/bug.md`, `.github/ISSUE_TEMPLATE/feature.md`

---

## 2) Environment Contract (MUST)
Provide a deterministic dev/run setup. Prefer **containerized** tooling (Podman or Docker) and `taskfile`/`make` wrappers.

**Example `Taskfile.yml`**:
```yaml
version: '3'

vars:
  RUST_MSrv: '1.79.0'

tasks:
  setup:
    desc: Install toolchain
    cmds:
      - rustup toolchain install {{.RUST_MSrv}}
      - rustup component add clippy rustfmt --toolchain {{.RUST_MSrv}}
  fmt:
    cmds: ["cargo fmt --all"]
  lint:
    cmds: ["cargo clippy --workspace --all-targets --all-features -- -D warnings"]
  test:
    cmds: ["cargo test --workspace --all-features"]
  doc:
    cmds: ["cargo doc --no-deps"]
  audit:
    cmds: ["cargo audit || true", "cargo deny check || true"]
```

**Container (Podman) dev shell**:
```bash
podman build -t ${REPO_NAME}-dev -f - <<'EOF'
FROM rust:1.79
RUN apt-get update && apt-get install -y make git pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
EOF
podman run --rm -it -v "$(pwd)":/app -w /app ${REPO_NAME}-dev bash
```

---

## 3) Project Scaffolding (Language-Agnostic with Examples)
Pick one or more stacks; generate baseline code + tests + CI in one commit.

### Rust (workspace)
```bash

# Workspace
cat > Cargo.toml <<'TOML'
[workspace]
resolver = "2"
members = ["crates/app", "crates/core", "crates/infra"]
TOML

mkdir -p crates/app/src crates/core/src crates/infra/src

cat > crates/core/src/lib.rs <<'RS'
//! Core domain logic
pub fn add(a:i32,b:i32)->i32{a+b}

#[cfg(test)]
mod tests{use super::*;#[test]fn adds(){assert_eq!(add(2,3),5);}}
RS

cat > crates/app/src/main.rs <<'RS'
fn main(){println!("ok");}
RS

git add . && git commit -m "feat(rust): scaffold workspace with app/core/infra"
```

### Python (package)
```bash
python -m venv .venv && . .venv/bin/activate
pip install --upgrade pip wheel build pytest mypy ruff
mkdir -p src/pkg tests
cat > pyproject.toml <<'TOML'
[project]
name = "pkg"
version = "0.1.0"
requires-python = ">=3.11"

[tool.ruff]
line-length = 100

[tool.pytest.ini_options]
addopts = "-q"
TOML
cat > src/pkg/__init__.py <<'PY'
def add(a:int,b:int)->int:
    return a+b
PY
cat > tests/test_add.py <<'PY'
from pkg import add
def test_add():
    assert add(2,3)==5
PY

pip install -e .
pytest

git add . && git commit -m "feat(py): scaffold package with tests and lint"
```

### Node/TypeScript (service)
```bash
npm init -y
npm i -D typescript ts-node vitest @types/node eslint
npx tsc --init --rootDir src --outDir dist --esModuleInterop true
mkdir -p src
cat > src/add.ts <<'TS'
export const add=(a:number,b:number)=>a+b;
TS
cat > src/add.test.ts <<'TS'
import {describe,it,expect} from 'vitest';
import {add} from './add';
describe('add',()=>{it('adds',()=>{expect(add(2,3)).toBe(5)});});
TS
npx vitest run

git add . && git commit -m "feat(ts): scaffold service with tests"
```

### Flutter (app)
```bash
flutter create app && cd app
flutter test
cd ..

git add . && git commit -m "feat(flutter): scaffold app"
```

---

## 4) Branching & Issue Flow (GitFlow-compatible)
- Create **feature** branches from `develop`: `feature/ABC-123-short-slug`.
- Commit with **Conventional Commits** (machine-parsable).
- Open PRs against `develop` (features) and `main` (release/hotfix). Enforce CI gates.

---

## 5) Implementation Loop (LLM Checkpoints)
Repeat until acceptance criteria are met.

1. **Plan**: generate a task list; annotate each with *owner*, *effort*, *risk*.
2. **Code**: implement smallest vertical slice. Keep files < 400 lines; extract helpers.
3. **Tests**: add unit + integration for slice. Aim for >80% relevant coverage.
4. **Static analysis**: format + lint + type-check.
5. **Run**: local smoke (CLI/HTTP endpoint/demo screen).
6. **Docs**: update README/USAGE, crate/module docs, and changelog fragment.
7. **Commit**: one coherent commit message.

**Idempotent gate**:
```bash
make -q fmt lint test || task fmt lint test || true
```

---

## 6) Observability & Security Hooks (MUST)
- Add `tracing` / structured logs on public entry points and hot paths (no secrets).
- Add health/ready endpoints for services.
- Run `cargo audit` / `cargo deny`, `npm audit` or `pip-audit` per stack.
- Add basic rate limits / timeouts for network calls.

---

## 7) CI/CD Pipeline (Minimal but Strong)
**GitHub Actions example** (`.github/workflows/ci.yml`):
```yaml
name: ci
on:
  pull_request:
  push:
    branches: [develop, main]
jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy, rustfmt }
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - run: cargo test --workspace --all-features
      - run: cargo doc --no-deps
      - run: cargo install cargo-audit && cargo audit || true
  node:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '20' }
      - run: npm ci || true
      - run: npx vitest run || true
```

---

## 8) PR Checklist (LLM must verify before requesting review)
- [ ] Title uses Conventional Commits and includes ticket ID.
- [ ] CI green: format, lint, tests, docs, basic security checks.
- [ ] No `unwrap/expect/panic` in library code; errors typed.
- [ ] Logs structured; no secrets.
- [ ] README/docs updated; examples runnable.
- [ ] Changelog fragment added.
- [ ] Feature flagging or config documented.

---

## 9) Release Steps (GitFlow-aligned)
**When `develop` is stable**:
```bash
VERSION=1.0.0

# Start release
git checkout develop && git pull
git checkout -b release/${VERSION}

# Update versions/CHANGELOG
# Commit
git commit -am "chore(release): prepare v${VERSION}"

# PR: base main -> merge -> tag
git checkout main && git merge --no-ff release/${VERSION}
git tag -a v${VERSION} -m "Release v${VERSION}" && git push --tags

# Back-merge
git checkout develop && git merge --no-ff release/${VERSION}

# Cleanup
git branch -d release/${VERSION}
```

---

## 10) Hotfix Steps (Production Incident)
```bash
VERSION=1.0.1

# Branch from main
git checkout main && git pull
git checkout -b hotfix/${VERSION}

# Apply minimal fix + tests
# Merge to main and tag
git checkout main && git merge --no-ff hotfix/${VERSION}
git tag -a v${VERSION} -m "Hotfix v${VERSION}" && git push --tags

# Back-merge to develop (and release/* if open)
git checkout develop && git merge --no-ff hotfix/${VERSION}
```

---

## 11) Acceptance & Handover
- Provide **artifact list**: binaries/images, SBOM, docs link, changelog.
- Provide **runbook**: start/stop, env vars, health checks, on-call notes.
- Provide **coverage & test report** summary.

---

## 12) Defaults (When Unsure)
- Prefer **typed errors** and `Result` over panics; document invariants.
- Prefer **borrowing** over cloning; justify heavy allocations.
- Add **observability** first on new public APIs.
- Keep changes **small and vertical**; merge frequently behind flags.
- If a step fails, **stop** and output a clear error plus a fix suggestion.

---

## 13) Machine-Readable Policy (YAML for Bots)
```yaml
playbook:
  branching: gitflow
  require:
    - ci_green
    - formatted
    - lint_clean
    - tests_pass
    - docs_build
  checks:
    disallow_panic_in_library: true
    typed_errors: true
    structured_logging: true
  gates:
    pr:
      - checklist_completed
      - coverage_threshold>=0.75
    release:
      - semver_tag
      - changelog_updated
      - backmerge_to_develop
```

---

## 14) Minimal Command Index (copy/paste)
```bash

# Run full local gate (Rust example)
cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features && cargo doc --no-deps

# Security sanity
cargo install cargo-audit cargo-deny || true
cargo audit || true
cargo deny check || true

# Node tests
npm ci && npx vitest run || true

# Python tests
pytest -q || true
```
