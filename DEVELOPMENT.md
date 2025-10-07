# Orbit-RS Development Workflow

This document outlines the development workflow and best practices for contributing to Orbit-RS.

## 🔧 Pre-commit Checklist

Before committing any code, **always** run the following to ensure code quality and avoid CI failures:

### Option 1: Use Make (Recommended)
```bash
# Format, check, and test - all in one command
make commit-ready

# Or run individual steps
make format   # Format code with cargo fmt
make check    # Run cargo check and clippy  
make test     # Run all tests
make build    # Build all packages
```

### Option 2: Use the Pre-commit Script
```bash
# Run the comprehensive pre-commit script
./scripts/pre-commit.sh
```

### Option 3: Manual Commands
```bash
# 1. Format code (REQUIRED before every commit)
cargo fmt --all

# 2. Run clippy checks
cargo clippy --all-targets -- -D warnings

# 3. Run tests
cargo test --workspace --verbose

# 4. Build project
cargo build --workspace
```

## 📋 Available Make Targets

| Target | Description |
|--------|-------------|
| `make help` | Show all available targets |
| `make format` | Format code with `cargo fmt --all` |
| `make check` | Run cargo check and clippy |
| `make test` | Run all tests |
| `make build` | Build all packages |
| `make clean` | Clean build artifacts |
| `make commit-ready` | Format, check, and test (ready for commit) |
| `make all` | Run all checks and build |

## 🤖 Automated Pre-commit Hook

A pre-commit hook has been installed that will automatically run `cargo fmt --all` before each commit. If formatting changes are needed, the commit will be blocked and you'll need to review and re-add the changes.

## 🚨 CI/CD Pipeline

The CI/CD pipeline runs the following checks:
- ✅ `cargo fmt --all -- --check` (formatting)
- ✅ `cargo clippy --all-targets -- -D warnings` (linting)
- ✅ `cargo test --workspace --verbose` (testing)
- ✅ `cargo build --workspace` (building)

**Always run `cargo fmt --all` before committing to avoid CI failures!**

## 🎯 Best Practices

1. **Format First**: Always run `cargo fmt --all` before committing
2. **Test Locally**: Run tests locally before pushing to avoid CI failures  
3. **Use Make**: Use `make commit-ready` for a complete pre-commit workflow
4. **Check CI**: Monitor CI/CD pipeline status after pushing
5. **Clean Commits**: Make atomic commits with clear messages

## 📁 Project Structure

- `orbit-util/` - Utility functions and test helpers
- `orbit-shared/` - Shared types and protocols
- `orbit-client/` - Client library
- `orbit-server/` - Main server implementation
- `orbit-protocols/` - Protocol implementations (RESP, PostgreSQL, etc.)
- `examples/` - Example applications
- `scripts/` - Development scripts
- `docs/` - Documentation

## 🔗 Useful Commands

```bash
# Quick formatting (run this before every commit!)
cargo fmt --all

# Build all packages in workspace
cargo build --workspace

# Run all tests
cargo test --workspace --verbose

# Check for compilation errors
cargo check --workspace

# Run clippy linter
cargo clippy --all-targets -- -D warnings

# Generate documentation
cargo doc --no-deps --workspace --open
```

## 🆘 Getting Help

- Check the `Makefile` for common tasks: `make help`
- Review CI/CD pipeline configuration in `.github/workflows/`
- Run the pre-commit script for guidance: `./scripts/pre-commit.sh`