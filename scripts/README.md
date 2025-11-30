# Orbit-RS Scripts

Utility scripts for development, testing, and deployment.

## Available Scripts

### Development

#### run-tests.sh

Run tests for the workspace with various options.

```bash
./scripts/run-tests.sh              # Run all workspace tests
./scripts/run-tests.sh server       # Run orbit-server tests only
./scripts/run-tests.sh time-series  # Run time series tests
./scripts/run-tests.sh ignored      # Run slow/ignored tests
./scripts/run-tests.sh help         # Show all options
```

#### pre-commit.sh

Run before committing to ensure code quality.

```bash
./scripts/pre-commit.sh
```

This runs:
1. `cargo fmt --all` - Format code
2. `cargo clippy --all-targets -- -D warnings` - Lint checks
3. `cargo test --workspace --verbose` - Run tests
4. `cargo build --workspace` - Build project

### Server Startup

#### start-multiprotocol-server.sh

Start the full multi-protocol database server (PostgreSQL + Redis + REST + gRPC).

```bash
./scripts/start-multiprotocol-server.sh           # Development mode
./scripts/start-multiprotocol-server.sh --prod    # Production mode
./scripts/start-multiprotocol-server.sh --config path/to/config.toml
```

**Ports:**
- PostgreSQL: 5432
- Redis: 6379
- REST API: 8080
- gRPC: 50051

#### start-orbit-redis.sh

Quick start for Redis-compatible server only.

```bash
./scripts/start-orbit-redis.sh
```

Starts orbit-server with development mode, focusing on Redis (port 6379).

### CI/CD

#### run-ci.sh

Launcher for the manual CI/CD pipeline.

```bash
./scripts/run-ci.sh
```

#### manual-ci-cd.sh

Full manual CI/CD operations.

```bash
./scripts/manual-ci-cd.sh
```

### Deployment

#### prepare-secrets.sh

Prepare Kubernetes secrets for GitHub Actions CI/CD.

```bash
./scripts/prepare-secrets.sh staging     # For staging environment
./scripts/prepare-secrets.sh production  # For production environment
```

#### validate-k8s-deployments.sh

Validate Kubernetes deployment configurations.

```bash
./scripts/validate-k8s-deployments.sh
```

### Utilities

#### check-file-organization.sh

Check codebase file organization.

```bash
./scripts/check-file-organization.sh
```

#### add-jekyll-frontmatter.sh

Add Jekyll frontmatter to documentation files.

```bash
./scripts/add-jekyll-frontmatter.sh
```

## Adding New Scripts

When adding new scripts:

1. **Make executable:** `chmod +x scripts/your-script.sh`
2. **Add shebang:** Start with `#!/bin/bash`
3. **Add error handling:** Use `set -e` for bash scripts
4. **Document:** Add entry to this README
5. **Test:** Verify script works from project root

## Best Practices

- Use descriptive names: `verb-noun.sh` format
- Check for correct directory at start
- Provide helpful error messages
- Use colors for output clarity (but gracefully degrade)
- Test on both macOS and Linux when possible
