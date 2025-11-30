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

### Cluster Testing

#### start-cluster.sh

Start a multi-node local cluster for integration and BDD testing.

```bash
./scripts/start-cluster.sh              # Start 3-node cluster (default)
./scripts/start-cluster.sh 5            # Start 5-node cluster
./scripts/start-cluster.sh --stop       # Stop running cluster
./scripts/start-cluster.sh --status     # Show cluster status
./scripts/start-cluster.sh --logs 1     # Tail logs for node 1
./scripts/start-cluster.sh --clean      # Stop and clean all data
./scripts/start-cluster.sh --help       # Show all options
```

**Port Allocation (per node):**

| Node | Redis | PostgreSQL | MySQL | CQL  | HTTP | gRPC  | Metrics |
|------|-------|------------|-------|------|------|-------|---------|
| 1    | 6379  | 5432       | 3306  | 9042 | 8080 | 50051 | 9090    |
| 2    | 6380  | 5433       | 3307  | 9043 | 8081 | 50052 | 9091    |
| 3    | 6381  | 5434       | 3308  | 9044 | 8082 | 50053 | 9092    |
| N    | +N-1  | +N-1       | +N-1  | +N-1 | +N-1 | +N-1  | +N-1    |

**Cluster Data:**
- Data directory: `./cluster-data/`
- Logs: `./cluster-data/logs/`
- PIDs: `./cluster-data/pids/`

**Usage with Integration Tests:**

```bash
# 1. Start the cluster
./scripts/start-cluster.sh

# 2. Run integration tests against the cluster
python3 tests/run_integration_tests.py --host localhost --resp-port 6379

# 3. Run BDD tests
cd tests && behave

# 4. Stop the cluster when done
./scripts/start-cluster.sh --stop
```

#### start-cluster-lb.sh

Lightweight load balancer for cluster testing. Listens on default ports and distributes to cluster nodes.

```bash
./scripts/start-cluster-lb.sh              # Start LB for 3-node cluster
./scripts/start-cluster-lb.sh --stop       # Stop load balancer
./scripts/start-cluster-lb.sh --status     # Show LB status
```

**Requires:** HAProxy (recommended) or socat

```bash
# macOS
brew install haproxy

# Ubuntu
apt install haproxy
```

**Usage with Load Balancer:**

```bash
# 1. Start cluster with LB-compatible ports
./scripts/start-cluster.sh --with-lb

# 2. Start load balancer
./scripts/start-cluster-lb.sh

# 3. Connect using default ports (LB handles routing)
redis-cli -p 6379                    # Connects via LB
psql -h 127.0.0.1 -p 5432 -U orbit   # Connects via LB

# 4. HAProxy stats available at http://127.0.0.1:9999

# 5. Stop everything
./scripts/start-cluster-lb.sh --stop
./scripts/start-cluster.sh --stop
```

**Architecture:**
```
Client (redis-cli :6379) --> Load Balancer --> Node 1 (:16379)
                                           --> Node 2 (:16380)
                                           --> Node 3 (:16381)
```

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
