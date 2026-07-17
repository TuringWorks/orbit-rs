---
name: orbit-dev-commands
description: Orbit-RS build, test, run, cluster, and load-balancer commands — make targets and ./scripts invocations. Use when you need the exact command to build, check, format, test, run the server, or start/stop a cluster.
---

# Orbit-RS Development Commands

All development tasks use `make` targets. (You can also run `make help` or read the root `Makefile`.)

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
make commit-ready       # Format + check + test (recommended before push)
make commit-light       # Format + check only (faster)
make all                # Full pipeline: format, check, test, build
```

## Direct Script Usage

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
