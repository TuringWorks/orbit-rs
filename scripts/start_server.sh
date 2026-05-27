#!/bin/bash
# Orbit-RS Development Server Startup Script
# Starts orbit-server in dev mode with all protocols on non-standard ports
# (to avoid conflicts with any locally-running databases)
#
# Usage: ./scripts/start_server.sh
#        ./scripts/start_server.sh --stop

set -e

# Resolve the project root from the script location
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Check for stop argument
if [ "$1" = "--stop" ]; then
    echo "Stopping orbit-server..."
    pkill -f "orbit-server --dev-mode" 2>/dev/null || true
    echo "Stopped."
    exit 0
fi

# Build if needed
if [ ! -f "./target/release/orbit-server" ]; then
    echo "Building orbit-server (release)..."
    cargo build --release --bin orbit-server
fi

echo "Starting orbit-server in dev mode..."
echo "  PostgreSQL: 5433 | Redis: 6380 | MySQL: 3307 | CQL: 9043"
echo "  Press Ctrl+C to stop"

./target/release/orbit-server --data-dir ./test-data --postgres-port 5433 --bind 127.0.0.1 --redis-port 6380 --mysql-port 3307 --cql-port 9043 --dev-mode
