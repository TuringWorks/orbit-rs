---
layout: default
title: "Contributing Guide"
subtitle: "How to contribute to the Orbit-RS project"
category: "contributing"
---

# Contributing to Orbit-RS

Thank you for your interest in contributing to Orbit-RS! This document provides guidelines and information about contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Contributing Process](#contributing-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Performance Considerations](#performance-considerations)
- [Submitting Changes](#submitting-changes)

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct. Please treat all contributors and users with respect and create a welcoming environment for everyone.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/yourusername/orbit-rs.git
   cd orbit-rs
   ```
3. **Add the upstream remote**:
   ```bash
   git remote add upstream https://github.com/original/orbit-rs.git
   ```

## Development Setup

### Prerequisites

- **Rust 1.70+**: Install via [rustup](https://rustup.rs/)
- **Protocol Buffers Compiler**: Install `protoc`
  ```bash
  # macOS
  brew install protobuf
  
  # Ubuntu/Debian
  sudo apt-get install protobuf-compiler
  
  # Windows
  # Download from https://github.com/protocolbuffers/protobuf/releases
  ```
- **SQLite development libraries** (for transaction logging)

### Building the Project

```bash
# Build all crates
cargo build

# Build with all features
cargo build --all-features

# Build specific crate
cargo build --package orbit-shared
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test --package orbit-shared

# Run tests with output
cargo test -- --nocapture

# Run ignored tests
cargo test -- --ignored
```

### Running Examples

```bash
# Hello World example
cargo run --example hello-world

# Distributed Transactions example
cargo run --example distributed-transactions
```

## Contributing Process

### 1. Choose What to Work On

- Check the [Issues](https://github.com/yourusername/orbit-rs/issues) page for open issues
- Look for issues labeled `good first issue` for newcomers
- Check the [CHANGELOG.md](CHANGELOG.md) for planned features
- Discuss new features in issues before starting work

### 2. Create a Branch

```bash
git checkout -b feature/your-feature-name
```

Branch naming conventions:
- `feature/feature-name` - New features
- `fix/issue-description` - Bug fixes
- `docs/documentation-update` - Documentation changes
- `refactor/component-name` - Code refactoring

### 3. Make Changes

Follow the coding standards and guidelines outlined below.

### 4. Test Your Changes

Ensure all tests pass and add new tests for your changes:

```bash
cargo test
cargo clippy
cargo fmt --all
```

### 5. Commit Your Changes

Write clear, descriptive commit messages:

```bash
git commit -m "feat: add distributed transaction recovery mechanism

- Implement coordinator failover detection
- Add transaction checkpoint management  
- Include recovery event handlers
- Add comprehensive tests for recovery scenarios
"
```

Commit message format:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `style:` - Code style changes
- `refactor:` - Code refactoring
- `test:` - Adding tests
- `chore:` - Maintenance tasks

## Coding Standards

### Rust Style Guide

- **Follow standard Rust formatting**: Use `cargo fmt`
- **Use clippy**: Run `cargo clippy` and address warnings
- **Documentation**: Document all public APIs with `///` comments
- **Error handling**: Use `OrbitResult<T>` for recoverable errors
- **Async/await**: Use async/await for I/O operations
- **Memory safety**: Avoid `unsafe` code unless absolutely necessary

### Code Organization

```rust
// File structure within modules
use std::...;           // Standard library
use external_crate::...;  // External dependencies  
use crate::...;         // Internal crate dependencies

// Constants
const MAX_RETRIES: u32 = 3;

// Type definitions
pub type TransactionId = String;

// Structs and enums
#[derive(Debug, Clone)]
pub struct MyStruct {
    // fields
}

// Trait definitions
pub trait MyTrait {
    // methods
}

// Implementations
impl MyStruct {
    // methods
}

// Tests (at bottom of file)
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_my_function() {
        // test code
    }
}
```

### Transaction System Guidelines

When working on transaction-related code:

- **ACID Properties**: Ensure atomicity, consistency, isolation, durability
- **Error Handling**: Handle network failures, timeouts, and participant failures gracefully
- **Logging**: Use structured logging with `tracing` crate
- **Testing**: Include both unit tests and integration tests
- **Performance**: Consider batch operations and connection pooling
- **Recovery**: Design for coordinator failure and transaction recovery

### Documentation Standards

- **Public APIs**: Document with examples
- **Complex algorithms**: Explain the approach and trade-offs
- **Configuration**: Document all configuration options
- **Error conditions**: Document when functions return errors

Example:
```rust
/// Begins a new distributed transaction across multiple participants.
///
/// This method initiates a 2-phase commit protocol that ensures ACID
/// properties across distributed actors. The transaction will timeout
/// after the specified duration.
///
/// # Arguments
///
/// * `timeout` - Optional timeout for the transaction. If None, uses
///   the configured default timeout.
///
/// # Returns
///
/// Returns a `TransactionId` that can be used to add operations and
/// commit the transaction.
///
/// # Errors
///
/// Returns `OrbitError::Internal` if the maximum number of concurrent
/// transactions has been exceeded.
///
/// # Example
///
/// ```rust
/// let tx_id = coordinator.begin_transaction(Some(Duration::from_secs(30))).await?;
/// // Add operations and commit...
/// ```
pub async fn begin_transaction(&self, timeout: Option<Duration>) -> OrbitResult<TransactionId> {
    // implementation
}
```

## Testing

### Types of Tests

1. **Unit Tests**: Test individual functions and modules
2. **Integration Tests**: Test component interactions
3. **Example Tests**: Ensure examples compile and run
4. **Property Tests**: Use `proptest` for property-based testing
5. **Benchmark Tests**: Performance regression testing

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_transaction_creation() {
        // Arrange
        let config = TransactionConfig::default();
        let coordinator = setup_test_coordinator(config).await;
        
        // Act
        let result = coordinator.begin_transaction(None).await;
        
        // Assert
        assert!(result.is_ok());
        let tx_id = result.unwrap();
        assert!(!tx_id.id.is_empty());
    }
    
    // Helper functions
    async fn setup_test_coordinator(config: TransactionConfig) -> TransactionCoordinator {
        // setup code
    }
}
```

### Mocking and Test Utilities

- Use mock implementations for external dependencies
- Create test utilities in `tests/common/` for reusable test code
- Use `tempfile` for temporary file system operations

## Documentation

### API Documentation

- Use `cargo doc` to generate documentation
- Include examples in doc comments
- Document error conditions and edge cases
- Keep documentation up to date with code changes

### README and Guides

- Update README.md for user-facing changes
- Add examples for new features
- Update architecture documentation for significant changes

## Performance Considerations

### Transaction System Performance

- **Connection Pooling**: Reuse gRPC connections
- **Batch Operations**: Process multiple operations together
- **Async I/O**: Use tokio for non-blocking operations
- **Memory Usage**: Consider memory allocations in hot paths
- **Database Operations**: Use prepared statements and transactions

### Benchmarking

```bash
# Run benchmarks
cargo bench --package orbit-benchmarks

# Profile with perf (Linux)
cargo build --release
perf record --call-graph=dwarf target/release/your-binary
perf report
```

## Submitting Changes

### Pull Request Process

1. **Update documentation** for any user-facing changes
2. **Add tests** for new functionality
3. **Update CHANGELOG.md** with your changes
4. **Ensure CI passes** - all tests, linting, and formatting
5. **Request review** from maintainers

### Pull Request Template

```markdown
## Description
Brief description of changes.

## Type of Change
- [ ] Bug fix
- [ ] New feature  
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Refactoring

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Added new tests for this change
- [ ] Manual testing completed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
```

### Review Process

- **Code review**: At least one maintainer review required
- **CI checks**: All automated checks must pass
- **Documentation**: Ensure docs are updated
- **Breaking changes**: Must be discussed and approved

## Getting Help

- **GitHub Issues**: For bug reports and feature requests
- **Discussions**: For questions and general discussion
- **Discord/Slack**: Real-time chat (if available)
- **Email**: Contact maintainers directly for sensitive issues

## Recognition

Contributors will be recognized in:
- CHANGELOG.md for significant contributions
- README.md contributors section
- GitHub contributors page

Thank you for contributing to Orbit-RS! 🚀