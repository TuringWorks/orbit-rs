---
layout: default
title: Project Organization Rules
category: documentation
---

# Project Organization Rules

This document establishes clear guidelines for where different types of files should be placed within the Orbit-RS project to maintain consistency and organization.

## 📋 File Placement Rules

### 📚 Documentation Files

**Rule**: All documentation files MUST be placed in the `docs/` directory or its subdirectories.

**Applies to**:

- `*.md` files (except README.md in package roots)
- Architecture documents
- API documentation
- Design documents
- User guides
- Development guides
- RFCs (Request for Comments)

**Directory Structure**:

```
docs/
├── README.md                    # Documentation index
├── DEVELOPMENT.md              # Development guide (moved from root)
├── PROJECT_STRUCTURE.md        # Project architecture
├── architecture/               # Architecture documentation
├── development/                # Development guides and processes
├── deployment/                 # Deployment documentation
├── protocols/                  # Protocol specifications
├── rfcs/                       # Request for Comments documents  
├── wip/                        # Work-in-progress documents
└── features/                   # Feature-specific documentation
```

**Examples**:

- ✅ `docs/DEVELOPMENT.md`
- ✅ `docs/architecture/SYSTEM_DESIGN.md`
- ✅ `docs/rfcs/RFC-001-NEW_FEATURE.md`
- ❌ `DEVELOPMENT.md` (root level - should be in docs/)
- ❌ `orbit-shared/DESIGN.md` (package level - should be in docs/)

### 🏃‍♂️ Benchmark Files

**Rule**: All benchmark-related files MUST be placed in the `orbit-benchmarks/` package directory structure.

**Applies to**:

- Performance benchmarks
- Load testing code
- Benchmark configurations
- Performance measurement utilities
- Industry-standard benchmark implementations (TPC-H, TPC-C, TPC-DS)

**Directory Structure**:

```text
orbit-benchmarks/
├── Cargo.toml                  # Benchmark package configuration
├── README.md                   # Benchmark documentation
├── benches/                    # Criterion benchmarks
│   ├── actor_benchmarks.rs
│   ├── leader_election_benchmarks.rs
│   └── persistence_comparison.rs
├── src/                        # Benchmark library code
│   ├── lib.rs                  # Main benchmark exports
│   ├── orbitql/                # OrbitQL query benchmarks
│   │   ├── mod.rs
│   │   ├── benchmark.rs        # Query performance framework
│   │   └── comprehensive_benchmark.rs # TPC implementations
│   ├── persistence/            # Storage benchmarks
│   ├── compute/                # Compute acceleration benchmarks
│   └── performance/            # General performance utilities
├── scripts/                    # Benchmark utilities
│   ├── run_benchmarks.sh       # Benchmark runner
│   └── analyze_results.py      # Result analysis
└── examples/                   # Benchmark usage examples
```

**Examples**:

- ✅ `orbit-benchmarks/src/orbitql/benchmark.rs`
- ✅ `orbit-benchmarks/benches/new_benchmark.rs`  
- ✅ `orbit-benchmarks/scripts/performance_test.sh`
- ❌ `orbit-shared/src/benchmarks/` (should be in orbit-benchmarks)
- ❌ `benchmarks/` (root level - should be in orbit-benchmarks package)

### 🧪 Test Files

**Rule**: Test files should follow Rust conventions and be co-located with the code they test.

**Examples**:

- ✅ `orbit-shared/src/lib.rs` with `#[cfg(test)]` modules
- ✅ `orbit-shared/tests/integration_tests.rs`
- ✅ `tests/` (workspace-level integration tests)

### 🔧 Configuration Files

**Rule**: Configuration files should be at the appropriate scope level.

**Examples**:

- ✅ `Cargo.toml` (workspace root)
- ✅ `orbit-shared/Cargo.toml` (package level)
- ✅ `.github/workflows/` (CI/CD configurations)

### 📜 Scripts and Utilities

**Rule**: Scripts should be organized by purpose and scope.

**Directory Structure**:

```text
├── scripts/                    # Workspace-level scripts
├── orbit-benchmarks/scripts/   # Benchmark-specific scripts  
└── .github/workflows/          # CI/CD workflow scripts
```

## 🚫 Anti-Patterns to Avoid

### Documentation Anti-Patterns

- ❌ Placing `.md` files in package root directories (except package README.md)
- ❌ Creating documentation in random locations
- ❌ Mixing documentation with source code in inappropriate places

### Benchmark Anti-Patterns  

- ❌ Placing benchmark code in regular source packages
- ❌ Creating benchmark files outside the orbit-benchmarks package
- ❌ Including performance tests in unit test directories

### General Anti-Patterns

- ❌ Creating files at project root that should be organized in subdirectories
- ❌ Mixing different types of files in inappropriate locations
- ❌ Not following established directory hierarchies

## 🔄 Migration Guidelines

### When Moving Files

1. Use `git mv` to preserve file history
2. Update all references and imports
3. Update documentation links
4. Test that all builds still work
5. Update CI/CD pipelines if necessary

### For New Files

1. Determine the correct location based on these rules
2. Create files in the appropriate directory from the start
3. Follow naming conventions for the target directory

## ✅ Enforcement

### Pre-commit Hooks

Consider implementing pre-commit hooks to enforce these rules:

- Check that new `.md` files are in `docs/`
- Check that benchmark files are in `orbit-benchmarks/`
- Warn about files in inappropriate locations

### Code Review Guidelines

Reviewers should verify:

- New files are placed according to these rules
- File moves preserve git history using `git mv`
- Documentation links are updated after moves
- No broken references after reorganization

## 📝 Examples of Correct Organization

### Documentation Example

```text
✅ Correct:
docs/
├── development/
│   ├── CODING_STANDARDS.md
│   ├── TESTING_GUIDE.md
│   └── BENCHMARKING.md
└── architecture/
    ├── SYSTEM_OVERVIEW.md
    └── DATABASE_DESIGN.md
```

### Benchmark Example  

```text
✅ Correct:
orbit-benchmarks/
├── src/
│   ├── database/
│   │   ├── query_benchmarks.rs
│   │   └── transaction_benchmarks.rs
│   └── network/
│       └── latency_benchmarks.rs
└── benches/
    ├── end_to_end_benchmarks.rs
    └── load_testing.rs
```

## 🎯 Benefits of Following These Rules

1. **Consistency**: All team members know where to find and place files
2. **Maintainability**: Clear organization makes the project easier to maintain
3. **Discoverability**: New contributors can easily understand the project structure
4. **Tooling**: Build tools, IDEs, and CI/CD can rely on predictable file locations
5. **Separation of Concerns**: Different types of files are properly isolated

---

**Note**: These rules should be followed for all new files and enforced during code reviews. Existing files that violate these rules should be gradually migrated to the correct locations.
