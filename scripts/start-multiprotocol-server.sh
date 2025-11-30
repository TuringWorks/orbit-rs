#!/bin/bash

# Orbit-RS Multi-Protocol Database Server Startup Script
# Starts PostgreSQL + Redis + REST + gRPC in a single process

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Banner
echo -e "${BLUE}"
echo "🚀 Orbit-RS Multi-Protocol Database Server"
echo "=========================================="
echo "One Server, All Protocols:"
echo "• PostgreSQL (5432) - Full SQL + pgvector"
echo "• Redis (6379) - Key-value + vectors"
echo "• REST API (8080) - Web interface"
echo "• gRPC (50051) - Actor management"
echo -e "${NC}"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "orbit/server" ]; then
    echo -e "${RED}❌ Error: Please run this script from the orbit-rs root directory${NC}"
    exit 1
fi

# Build the project if needed
echo -e "${YELLOW}🔨 Building orbit-server...${NC}"
if ! cargo build --release --bin orbit-server; then
    echo -e "${RED}❌ Build failed. Please fix compilation errors.${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Build completed successfully${NC}"
echo

# Check for existing processes on the ports
check_port() {
    local port=$1
    local protocol=$2
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo -e "${YELLOW}⚠️  Port $port ($protocol) is already in use${NC}"
        echo "   Run: lsof -ti:$port | xargs kill -9  # to free the port"
        return 1
    fi
    return 0
}

echo -e "${CYAN}🔍 Checking for port conflicts...${NC}"
PORTS_OK=true
check_port 5432 "PostgreSQL" || PORTS_OK=false
check_port 6379 "Redis" || PORTS_OK=false  
check_port 8080 "REST API" || PORTS_OK=false
check_port 50051 "gRPC" || PORTS_OK=false

if [ "$PORTS_OK" = false ]; then
    echo -e "${YELLOW}"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
    echo -e "${NC}"
fi

# Determine config path
CONFIG_PATH=""
if [ -f "./config/orbit-server.toml" ]; then
    CONFIG_PATH="./config/orbit-server.toml"
    echo -e "${GREEN}📁 Found configuration: $CONFIG_PATH${NC}"
elif [ -f "./orbit-server.toml" ]; then
    CONFIG_PATH="./orbit-server.toml"
    echo -e "${GREEN}📁 Found configuration: $CONFIG_PATH${NC}"
fi

# Start the server
echo -e "${PURPLE}🎬 Starting Orbit-RS Multi-Protocol Server...${NC}"
echo

# Choose startup mode
if [ "$1" = "--prod" ] || [ "$1" = "--production" ]; then
    # Production mode
    if [ -n "$CONFIG_PATH" ]; then
        echo -e "${BLUE}🏢 Production Mode: Using configuration file${NC}"
        exec ./target/release/orbit-server --config "$CONFIG_PATH"
    else
        echo -e "${YELLOW}⚠️  No configuration file found, generating one...${NC}"
        ./target/release/orbit-server --generate-config > orbit-server.toml
        echo -e "${GREEN}📄 Generated orbit-server.toml - please edit and restart${NC}"
        echo -e "${CYAN}💡 Then run: $0 --prod${NC}"
        exit 0
    fi
elif [ "$1" = "--config" ] && [ -n "$2" ]; then
    # Custom config file
    echo -e "${BLUE}⚙️  Using custom configuration: $2${NC}"
    exec ./target/release/orbit-server --config "$2"
else
    # Development mode (default)
    echo -e "${GREEN}🔧 Development Mode: All protocols enabled${NC}"
    echo -e "${YELLOW}   For production, use: $0 --prod${NC}"
    echo
    echo -e "${CYAN}🎯 Server will be available at:${NC}"
    echo -e "   🐘 PostgreSQL: psql -h localhost -p 5432 -U postgres"
    echo -e "   🔴 Redis:      redis-cli -h localhost -p 6379"
    echo -e "   🌍 REST API:   curl http://localhost:8080/health"
    echo -e "   📡 gRPC:       localhost:50051 (actor management)"
    echo
    exec ./target/release/orbit-server --dev-mode
fi