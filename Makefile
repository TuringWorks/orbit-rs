# Orbit-RS Development Makefile
# Common tasks for development workflow

.PHONY: conformance conformance-record conformance-probe conformance-parser conformance-reference help format check test build clean commit-ready commit-light \
        pre-commit-full pre-commit-light all \
        dev run cluster cluster-stop cluster-status cluster-lb cluster-lb-stop \
        test-ignored test-include-ignored test-quick test-server test-verbose \
        redis

help:
	@echo "🚀 Orbit-RS Development Tasks"
	@echo ""
	@echo "Build & Check:"
	@echo "  format              - Run cargo fmt --all to format code"
	@echo "  check               - Run cargo check and clippy"
	@echo "  test                - Run all workspace tests"
	@echo "  build               - Build all packages in workspace (debug)"
	@echo "  build-release       - Build all packages (release)"
	@echo "  clean               - Clean build artifacts"
	@echo ""
	@echo "Run & Develop:"
	@echo "  dev                 - Run orbit-server in dev mode (foreground)"
	@echo "  run                 - Alias for dev"
	@echo "  redis               - Run orbit-server with Redis on port 6379"
	@echo ""
	@echo "Cluster:"
	@echo "  cluster [N=3]       - Start N-node cluster (default: 3)"
	@echo "  cluster-stop        - Stop running cluster"
	@echo "  cluster-status      - Show cluster status"
	@echo "  cluster-lb [N=3]    - Start load balancer for N-node cluster"
	@echo "  cluster-lb-stop     - Stop load balancer"
	@echo ""
	@echo "Test Variants:"
	@echo "  test-ignored        - Run only ignored (slow) tests"
	@echo "  test-include-ignored - Run all tests including ignored"
	@echo "  test-quick          - Compile check only, no tests"
	@echo "  test-server         - Run orbit-server tests only"
	@echo "  test-verbose        - Run all tests with full output"
	@echo ""
	@echo "Pre-commit:"
	@echo "  commit-ready        - Format, check, and test (recommended)"
	@echo "  commit-light        - Format and check only (faster)"
	@echo "  pre-commit-full     - Enable full pre-commit hook with tests"
	@echo "  pre-commit-light    - Enable lightweight pre-commit hook"
	@echo ""
	@echo "Complete:"
	@echo "  all                 - Run format, check, test, and build"

format:
	@echo "🔧 Formatting code..."
	cargo fmt --all
	@echo "✅ Code formatting complete"

check:
	@echo "🔍 Running cargo check and clippy..."
	cargo check --workspace
	cargo clippy --all-targets -- -D warnings -A clippy::unnecessary-sort-by -A clippy::collapsible-match -A clippy::useless-conversion -A clippy::unnecessary-unwrap -A clippy::manual-checked-ops -A clippy::explicit-counter-loop
	@echo "✅ Code checks complete"

test:
	@echo "🧪 Running tests..."
	cargo test --workspace --verbose
	@echo "✅ Tests complete"

test-ignored:
	@echo "🧪 Running ignored (slow) tests..."
	cargo test --workspace -- --ignored
	@echo "✅ Ignored tests complete"

test-include-ignored:
	@echo "🧪 Running all tests including ignored..."
	cargo test --workspace -- --include-ignored
	@echo "✅ All tests complete"

test-quick:
	@echo "🧪 Quick compile check..."
	cargo check --workspace
	@echo "✅ Quick check complete"

test-server:
	@echo "🧪 Running orbit-server tests..."
	cargo test -p orbit-server
	@echo "✅ Server tests complete"

test-verbose:
	@echo "🧪 Running all tests with verbose output..."
	cargo test --workspace -- --nocapture
	@echo "✅ Verbose tests complete"

build:
	@echo "🏗️  Building workspace (debug)..."
	cargo build --workspace
	@echo "✅ Build complete"

build-release:
	@echo "🏗️  Building workspace (release)..."
	cargo build --workspace --release
	@echo "✅ Release build complete"

clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	@echo "✅ Clean complete"

# Development server (foreground)
dev: build
	@echo "🚀 Starting orbit-server in dev mode..."
	./target/debug/orbit-server --dev-mode

run: dev

# Redis-compatible mode
redis: build
	@echo "🔴 Starting orbit-server with Redis on port 6379..."
	./target/debug/orbit-server --dev-mode --redis-port 6379 --log-level info

# Cluster management
cluster: build-release
	@echo "🖥️  Starting $(or N,3)-node cluster..."
	./scripts/start-cluster.sh $(or N,3)

cluster-stop:
	./scripts/start-cluster.sh --stop

cluster-status:
	./scripts/start-cluster.sh --status

cluster-lb: build-release
	@echo "⚖️  Starting load balancer..."
	./scripts/start-cluster-lb.sh $(or N,3)

cluster-lb-stop:
	./scripts/start-cluster-lb.sh --stop

# PostgreSQL conformance: measure Orbit against the sqllogictest corpus (ratchet vs tests/conformance/baseline.json)
conformance:
	cargo test -p orbit-integration-tests --test postgres_conformance -- --ignored --nocapture orbit_does_not_regress

# Show what Orbit returns for ORBIT_PROBE_SQL (statements separated by ;;)
conformance-probe:
	@cargo test -q -p orbit-integration-tests --test postgres_conformance -- --ignored --nocapture probe 2>&1 | grep -A1 "^PROBE"

# Record a sqllogictest file from real PostgreSQL 18: make conformance-record IN=tests/conformance/sql/08_x.sql
conformance-record:
	@test -n "$(IN)" || { echo "usage: make conformance-record IN=tests/conformance/sql/NN_name.sql"; exit 2; }; \
	docker rm -f orbit-pg18-ref >/dev/null 2>&1; \
	docker run -d --rm --name orbit-pg18-ref -e POSTGRES_PASSWORD=pw -p 55432:5432 postgres:18 >/dev/null && \
	until docker exec orbit-pg18-ref pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done; sleep 2; \
	OUT=$(CURDIR)/tests/conformance/slt/$$(basename $(IN) .sql).slt; \
	CONFORMANCE_RECORD_IN=$(abspath $(IN)) CONFORMANCE_RECORD_OUT=$$OUT \
	ORBIT_CONFORMANCE_REFERENCE_URL="host=127.0.0.1 port=55432 user=postgres password=pw dbname=postgres" \
	cargo test -q -p orbit-integration-tests --test postgres_conformance -- --ignored record; \
	status=$$?; docker rm -f orbit-pg18-ref >/dev/null 2>&1; exit $$status

# Where the hand-rolled parser disagrees with PostgreSQL 18's grammar (libpg_query)
conformance-parser:
	cargo test -p orbit-integration-tests --features pg-query-parser --test postgres_conformance -- --ignored --nocapture hand_rolled_parser

# Validate the corpus itself against a real PostgreSQL 18 (needs docker)
conformance-reference:
	@docker rm -f orbit-pg18-ref >/dev/null 2>&1; \
	docker run -d --rm --name orbit-pg18-ref -e POSTGRES_PASSWORD=pw -p 55432:5432 postgres:18 >/dev/null && \
	until docker exec orbit-pg18-ref pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done; sleep 2; \
	ORBIT_CONFORMANCE_REFERENCE_URL="host=127.0.0.1 port=55432 user=postgres password=pw dbname=postgres" \
	cargo test -p orbit-integration-tests --test postgres_conformance -- --ignored --nocapture reference_postgres; \
	status=$$?; docker rm -f orbit-pg18-ref >/dev/null 2>&1; exit $$status

commit-ready: format check test
	@echo "🎉 Code is ready for commit!"
	@echo ""
	@echo "To commit your changes, run:"
	@echo "  git add ."
	@echo "  git commit -m 'your commit message'"

commit-light: format check
	@echo "✓ Code formatted and checked - ready for commit (lightweight)"

pre-commit-full:
	@echo "Installing full pre-commit hook (includes tests)..."
	@cp .git/hooks/pre-commit-full .git/hooks/pre-commit
	@chmod +x .git/hooks/pre-commit
	@echo "✓ Full pre-commit hook enabled"

pre-commit-light:
	@echo "Installing lightweight pre-commit hook (format + check only)..."
	@cp .git/hooks/pre-commit-light .git/hooks/pre-commit
	@chmod +x .git/hooks/pre-commit
	@echo "✓ Lightweight pre-commit hook enabled"

all: format check test build
	@echo "🎉 All tasks completed successfully!"
