# Orbit-RS Development Makefile
# Common tasks for development workflow

.PHONY: help format check test build clean commit-ready all

help:
	@echo "🚀 Orbit-RS Development Tasks"
	@echo ""
	@echo "Available targets:"
	@echo "  format        - Run cargo fmt --all to format code"
	@echo "  check         - Run cargo check and clippy"
	@echo "  test          - Run all tests"
	@echo "  build         - Build all packages in workspace"
	@echo "  clean         - Clean build artifacts"
	@echo "  commit-ready  - Format, check, and test (ready for commit)"
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

all: format check test build
	@echo "🎉 All tasks completed successfully!"