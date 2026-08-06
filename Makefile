# Orbit-RS Development Makefile
# Common tasks for development workflow

.PHONY: help format check test build clean commit-ready commit-light \
        pre-commit-full pre-commit-light all \
        dev run cluster cluster-stop cluster-status cluster-lb cluster-lb-stop \
        test-ignored test-include-ignored test-quick test-server test-verbose \
        redis desktop-check desktop-test desktop-build

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

check: desktop-check
	@echo "🔍 Running cargo check and clippy..."
	cargo check --workspace
	cargo clippy --all-targets -- -D warnings -A clippy::unnecessary-sort-by -A clippy::collapsible-match -A clippy::useless-conversion -A clippy::unnecessary-unwrap -A clippy::manual-checked-ops -A clippy::explicit-counter-loop
	@echo "✅ Code checks complete"

# orbit/desktop declares its own [workspace], so the root cargo commands above
# do not reach it. Without these targets it was possible — and did happen — for
# the desktop app to accumulate compile errors while `make check` stayed green.
DESKTOP_MANIFEST := orbit/desktop/src-tauri/Cargo.toml

desktop-check:
	@echo "🔍 Checking orbit-desktop (separate workspace)..."
	cargo clippy --manifest-path $(DESKTOP_MANIFEST) --all-targets -- -D warnings
	@if [ -d orbit/desktop/node_modules ]; then \
		cd orbit/desktop && npm run typecheck; \
	else \
		echo "⚠️  orbit/desktop/node_modules missing - run 'cd orbit/desktop && npm install' to typecheck the UI"; \
	fi
	@echo "✅ orbit-desktop checks complete"

desktop-test:
	@echo "🧪 Testing orbit-desktop..."
	cargo test --manifest-path $(DESKTOP_MANIFEST)
	@if [ -d orbit/desktop/node_modules ]; then \
		cd orbit/desktop && npm test; \
	else \
		echo "⚠️  orbit/desktop/node_modules missing - skipping UI tests"; \
	fi
	@echo "✅ orbit-desktop tests complete"

desktop-build:
	@echo "🏗️  Building orbit-desktop..."
	cd orbit/desktop && npm install && npm run build
	@echo "✅ orbit-desktop build complete"

test: desktop-test
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
