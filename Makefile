# Orbit-RS Development Makefile
# Common tasks for development workflow

.PHONY: help format check test build clean commit-ready commit-light pre-commit-full pre-commit-light all

help:
	@echo "🚀 Orbit-RS Development Tasks"
	@echo ""
	@echo "Build & Check:"
	@echo "  format        - Run cargo fmt --all to format code"
	@echo "  check         - Run cargo check and clippy"
	@echo "  test          - Run all tests"
	@echo "  build         - Build all packages in workspace"
	@echo "  clean         - Clean build artifacts"
	@echo ""
	@echo "Pre-commit:"
	@echo "  commit-ready  - Format, check, and test (recommended)"
	@echo "  commit-light  - Format and check only (faster)"
	@echo "  pre-commit-full  - Enable full pre-commit hook with tests"
	@echo "  pre-commit-light - Enable lightweight pre-commit hook"
	@echo ""
	@echo "Complete:"
	@echo "  all           - Run format, check, test, and build"
	@echo ""

format:
	@echo "🔧 Formatting code..."
	cargo fmt --all
	@echo "✅ Code formatting complete"

check:
	@echo "🔍 Running cargo check and clippy..."
	cargo check --workspace
	cargo clippy --all-targets -- -D warnings
	@echo "✅ Code checks complete"

test:
	@echo "🧪 Running tests..."
	cargo test --workspace --verbose
	@echo "✅ Tests complete"

build:
	@echo "🏗️  Building workspace..."
	cargo build --workspace
	@echo "✅ Build complete"

clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	@echo "✅ Clean complete"

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