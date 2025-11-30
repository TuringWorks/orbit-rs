#!/bin/bash
#
# Orbit-RS Redis Server Startup Script
#
# This script starts the Orbit server with Redis (RESP) protocol enabled.
# Redis functionality is now built into orbit-server (no separate resp-server).
#
# Usage: ./start-orbit-redis.sh
#
# Documentation:
# - Complete guide: docs/protocols/RESP_PRODUCTION_GUIDE.md
# - Protocol docs: docs/protocols/protocol_adapters.md
#
# Features:
# - 100% Redis compatibility (124+ commands)
# - Full redis-cli support
# - RocksDB persistence
# - High performance (10,000+ ops/sec)
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Starting Orbit Redis-compatible server...${NC}"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "orbit/server" ]; then
    echo -e "${RED}❌ Error: Please run this script from the orbit-rs root directory${NC}"
    exit 1
fi

# Build if not exists
if [ ! -f "target/release/orbit-server" ]; then
    echo -e "${YELLOW}🔨 Building orbit-server...${NC}"
    cargo build --release --bin orbit-server
fi

# Check if port 6379 is available
if lsof -Pi :6379 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo -e "${YELLOW}⚠️  Port 6379 is already in use${NC}"
    echo "   Run: lsof -ti:6379 | xargs kill -9  # to free the port"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo -e "${GREEN}🎯 Starting Redis-compatible server on port 6379...${NC}"
echo ""
echo -e "${BLUE}Connect using:${NC}"
echo "   redis-cli -h 127.0.0.1 -p 6379"
echo ""
echo -e "${BLUE}Try these commands:${NC}"
echo "   > PING"
echo "   > SET mykey 'Hello Orbit!'"
echo "   > GET mykey"
echo "   > HSET user:1 name 'Alice' age '25'"
echo "   > HGETALL user:1"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
echo ""

# Start orbit-server (Redis is built-in on port 6379)
exec ./target/release/orbit-server --dev-mode --log-level info
