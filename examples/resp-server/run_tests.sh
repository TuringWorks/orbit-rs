#!/bin/bash

# RESP Server Test Runner
# This script starts the RESP server and runs comprehensive tests

set -e

PROJECT_ROOT="/Users/ravindraboddipalli/sources/orbit-rs"
LOG_FILE="/tmp/resp-server-test.log"

echo "🚀 RESP Server Test Runner"
echo "=========================="

# Check if server is already running
if lsof -Pi :6379 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "⚠️  Port 6379 is already in use. Stopping existing process..."
    pkill -f resp-server || true
    sleep 2
fi

echo "📦 Building RESP server..."
cd "$PROJECT_ROOT"
cargo build --package resp-server-example --release

echo "🌐 Starting RESP server..."
cargo run --package resp-server-example --bin resp-server > "$LOG_FILE" 2>&1 &
SERVER_PID=$!

echo "⏳ Waiting for server to start..."
sleep 3

# Check if server started successfully
if ! lsof -Pi :6379 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "❌ Failed to start RESP server on port 6379"
    echo "Server log:"
    cat "$LOG_FILE"
    exit 1
fi

echo "✅ RESP server started successfully (PID: $SERVER_PID)"

# Function to cleanup on exit
cleanup() {
    echo "🧹 Cleaning up..."
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    echo "✅ Cleanup complete"
}

# Set trap to cleanup on script exit
trap cleanup EXIT

echo "🧪 Running comprehensive tests..."
echo

# Run the comprehensive test suite
cargo run --package resp-server-example --bin resp-comprehensive-test

echo
echo "🔍 Test run completed!"
echo
echo "📊 Server log (last 20 lines):"
echo "================================"
tail -20 "$LOG_FILE"
echo
echo "💾 Full server log saved to: $LOG_FILE"