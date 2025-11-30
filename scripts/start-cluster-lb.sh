#!/bin/bash

# Orbit-RS Cluster Load Balancer
# Provides load balancing for cluster nodes on default ports
#
# Usage:
#   ./scripts/start-cluster-lb.sh              # Start LB for 3-node cluster
#   ./scripts/start-cluster-lb.sh 5            # Start LB for 5-node cluster
#   ./scripts/start-cluster-lb.sh --stop       # Stop load balancer
#   ./scripts/start-cluster-lb.sh --status     # Show LB status

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
CLUSTER_SIZE=${1:-3}
LB_DIR="./cluster-data/lb"
PID_FILE="$LB_DIR/lb.pid"
HAPROXY_CFG="$LB_DIR/haproxy.cfg"
SOCAT_PIDS="$LB_DIR/socat_pids"

# Default ports (load balancer listens on these)
LB_REDIS_PORT=6379
LB_POSTGRES_PORT=5432
LB_MYSQL_PORT=3306
LB_CQL_PORT=9042
LB_HTTP_PORT=8080
LB_GRPC_PORT=50051

# Base ports for cluster nodes
BASE_REDIS_PORT=16379      # Nodes use 16379, 16380, etc.
BASE_POSTGRES_PORT=15432
BASE_MYSQL_PORT=13306
BASE_CQL_PORT=19042
BASE_HTTP_PORT=18080
BASE_GRPC_PORT=60051

print_status() { echo -e "${BLUE}[LB]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

check_directory() {
    if [ ! -f "Cargo.toml" ] || [ ! -d "orbit" ]; then
        print_error "Please run this script from the orbit-rs root directory"
        exit 1
    fi
}

# Check if haproxy is available
has_haproxy() {
    command -v haproxy &> /dev/null
}

# Check if socat is available
has_socat() {
    command -v socat &> /dev/null
}

# Generate HAProxy configuration
generate_haproxy_config() {
    mkdir -p "$LB_DIR"

    cat > "$HAPROXY_CFG" << EOF
# Orbit-RS Cluster Load Balancer Configuration
# Auto-generated - do not edit

global
    daemon
    maxconn 4096
    pidfile $LB_DIR/haproxy.pid

defaults
    mode tcp
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 50000ms
    option tcplog

# Redis Load Balancer
frontend redis_frontend
    bind *:$LB_REDIS_PORT
    default_backend redis_backend

backend redis_backend
    balance roundrobin
EOF

    for i in $(seq 1 $CLUSTER_SIZE); do
        local port=$((BASE_REDIS_PORT + i - 1))
        echo "    server node$i 127.0.0.1:$port check" >> "$HAPROXY_CFG"
    done

    cat >> "$HAPROXY_CFG" << EOF

# PostgreSQL Load Balancer
frontend postgres_frontend
    bind *:$LB_POSTGRES_PORT
    default_backend postgres_backend

backend postgres_backend
    balance roundrobin
EOF

    for i in $(seq 1 $CLUSTER_SIZE); do
        local port=$((BASE_POSTGRES_PORT + i - 1))
        echo "    server node$i 127.0.0.1:$port check" >> "$HAPROXY_CFG"
    done

    cat >> "$HAPROXY_CFG" << EOF

# MySQL Load Balancer
frontend mysql_frontend
    bind *:$LB_MYSQL_PORT
    default_backend mysql_backend

backend mysql_backend
    balance roundrobin
EOF

    for i in $(seq 1 $CLUSTER_SIZE); do
        local port=$((BASE_MYSQL_PORT + i - 1))
        echo "    server node$i 127.0.0.1:$port check" >> "$HAPROXY_CFG"
    done

    cat >> "$HAPROXY_CFG" << EOF

# CQL Load Balancer
frontend cql_frontend
    bind *:$LB_CQL_PORT
    default_backend cql_backend

backend cql_backend
    balance roundrobin
EOF

    for i in $(seq 1 $CLUSTER_SIZE); do
        local port=$((BASE_CQL_PORT + i - 1))
        echo "    server node$i 127.0.0.1:$port check" >> "$HAPROXY_CFG"
    done

    cat >> "$HAPROXY_CFG" << EOF

# HTTP/REST Load Balancer
frontend http_frontend
    bind *:$LB_HTTP_PORT
    default_backend http_backend

backend http_backend
    balance roundrobin
EOF

    for i in $(seq 1 $CLUSTER_SIZE); do
        local port=$((BASE_HTTP_PORT + i - 1))
        echo "    server node$i 127.0.0.1:$port check" >> "$HAPROXY_CFG"
    done

    cat >> "$HAPROXY_CFG" << EOF

# gRPC Load Balancer
frontend grpc_frontend
    bind *:$LB_GRPC_PORT
    default_backend grpc_backend

backend grpc_backend
    balance roundrobin
EOF

    for i in $(seq 1 $CLUSTER_SIZE); do
        local port=$((BASE_GRPC_PORT + i - 1))
        echo "    server node$i 127.0.0.1:$port check" >> "$HAPROXY_CFG"
    done

    cat >> "$HAPROXY_CFG" << EOF

# Stats interface
listen stats
    bind *:9999
    mode http
    stats enable
    stats uri /
    stats refresh 5s
EOF
}

# Start HAProxy-based load balancer
start_haproxy_lb() {
    print_status "Starting HAProxy load balancer..."
    generate_haproxy_config

    haproxy -f "$HAPROXY_CFG" -D

    print_success "HAProxy load balancer started"
    print_status "Stats available at: http://127.0.0.1:9999"
}

# Start socat-based load balancer (fallback)
start_socat_lb() {
    print_status "Starting socat-based load balancer..."
    mkdir -p "$LB_DIR"
    rm -f "$SOCAT_PIDS"
    touch "$SOCAT_PIDS"

    # Simple round-robin using multiple socat instances
    # Note: This is basic - just forwards to first node for simplicity
    local first_node_offset=0

    start_socat_proxy() {
        local name=$1
        local lb_port=$2
        local backend_port=$3

        print_status "  $name: $lb_port -> $backend_port"
        socat TCP-LISTEN:$lb_port,fork,reuseaddr TCP:127.0.0.1:$backend_port &
        echo $! >> "$SOCAT_PIDS"
    }

    # Start proxies (connecting to first node for simplicity)
    start_socat_proxy "Redis" $LB_REDIS_PORT $((BASE_REDIS_PORT))
    start_socat_proxy "PostgreSQL" $LB_POSTGRES_PORT $((BASE_POSTGRES_PORT))
    start_socat_proxy "MySQL" $LB_MYSQL_PORT $((BASE_MYSQL_PORT))
    start_socat_proxy "CQL" $LB_CQL_PORT $((BASE_CQL_PORT))
    start_socat_proxy "HTTP" $LB_HTTP_PORT $((BASE_HTTP_PORT))
    start_socat_proxy "gRPC" $LB_GRPC_PORT $((BASE_GRPC_PORT))

    print_success "Socat load balancer started (single-node forwarding)"
    print_warning "Note: socat LB forwards to node 1 only. Install HAProxy for full load balancing."
}

# Start the load balancer
start_lb() {
    check_directory
    mkdir -p "$LB_DIR"

    print_status "Starting load balancer for $CLUSTER_SIZE-node cluster"
    echo "=============================================="

    if has_haproxy; then
        start_haproxy_lb
    elif has_socat; then
        start_socat_lb
    else
        print_error "Neither HAProxy nor socat found!"
        print_status "Install one of:"
        print_status "  macOS: brew install haproxy"
        print_status "  macOS: brew install socat"
        print_status "  Ubuntu: apt install haproxy"
        print_status "  Ubuntu: apt install socat"
        exit 1
    fi

    echo ""
    print_status "Load Balancer Endpoints (default ports):"
    echo "  Redis:      redis-cli -p $LB_REDIS_PORT"
    echo "  PostgreSQL: psql -h 127.0.0.1 -p $LB_POSTGRES_PORT -U orbit"
    echo "  MySQL:      mysql -h 127.0.0.1 -P $LB_MYSQL_PORT -u orbit"
    echo "  CQL:        cqlsh 127.0.0.1 $LB_CQL_PORT"
    echo "  HTTP:       curl http://127.0.0.1:$LB_HTTP_PORT/health"
    echo "=============================================="
}

# Stop load balancer
stop_lb() {
    print_status "Stopping load balancer..."

    # Stop HAProxy
    if [ -f "$LB_DIR/haproxy.pid" ]; then
        local pid=$(cat "$LB_DIR/haproxy.pid")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            print_status "Stopped HAProxy (PID $pid)"
        fi
        rm -f "$LB_DIR/haproxy.pid"
    fi

    # Stop socat processes
    if [ -f "$SOCAT_PIDS" ]; then
        while read pid; do
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid" 2>/dev/null || true
            fi
        done < "$SOCAT_PIDS"
        rm -f "$SOCAT_PIDS"
        print_status "Stopped socat proxies"
    fi

    print_success "Load balancer stopped"
}

# Show status
show_status() {
    print_status "Load Balancer Status"
    echo "=============================================="

    local running=false

    if [ -f "$LB_DIR/haproxy.pid" ]; then
        local pid=$(cat "$LB_DIR/haproxy.pid")
        if kill -0 "$pid" 2>/dev/null; then
            echo -e "${GREEN}[RUNNING]${NC} HAProxy (PID $pid)"
            echo "  Stats: http://127.0.0.1:9999"
            running=true
        fi
    fi

    if [ -f "$SOCAT_PIDS" ]; then
        local count=0
        while read pid; do
            if kill -0 "$pid" 2>/dev/null; then
                count=$((count + 1))
            fi
        done < "$SOCAT_PIDS"
        if [ $count -gt 0 ]; then
            echo -e "${GREEN}[RUNNING]${NC} Socat ($count proxies)"
            running=true
        fi
    fi

    if [ "$running" = false ]; then
        echo -e "${YELLOW}[STOPPED]${NC} No load balancer running"
    fi

    echo "=============================================="
}

# Main
main() {
    case "${1:-}" in
        --stop|-s)
            stop_lb
            ;;
        --status|-S)
            show_status
            ;;
        --help|-h)
            echo "Usage: $0 [OPTION] [CLUSTER_SIZE]"
            echo ""
            echo "Options:"
            echo "  <number>      Configure LB for N-node cluster (default: 3)"
            echo "  --stop, -s    Stop load balancer"
            echo "  --status, -S  Show LB status"
            echo "  --help, -h    Show this help"
            echo ""
            echo "Load balancer listens on default ports:"
            echo "  Redis: $LB_REDIS_PORT, PostgreSQL: $LB_POSTGRES_PORT"
            echo "  MySQL: $LB_MYSQL_PORT, CQL: $LB_CQL_PORT"
            echo "  HTTP: $LB_HTTP_PORT, gRPC: $LB_GRPC_PORT"
            echo ""
            echo "Requires HAProxy (recommended) or socat"
            ;;
        [0-9]*)
            CLUSTER_SIZE=$1
            start_lb
            ;;
        *)
            start_lb
            ;;
    esac
}

main "$@"
