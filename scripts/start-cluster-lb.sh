#!/bin/bash

# Orbit-RS Cluster Load Balancer
# Uses the built-in orbit-lb binary for lightweight TCP load balancing
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
LOG_FILE="$LB_DIR/lb.log"

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

# Build the load balancer
build_lb() {
    print_status "Building orbit-lb..."
    cargo build --release --bin orbit-lb 2>&1 | tail -5
    if [ ! -f "./target/release/orbit-lb" ]; then
        print_error "Failed to build orbit-lb"
        exit 1
    fi
    print_success "orbit-lb built successfully"
}

# Start the load balancer
start_lb() {
    check_directory
    mkdir -p "$LB_DIR"

    # Check if already running
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            print_warning "Load balancer already running (PID $pid)"
            return
        fi
    fi

    # Build if needed
    if [ ! -f "./target/release/orbit-lb" ]; then
        build_lb
    fi

    print_status "Starting orbit-lb for $CLUSTER_SIZE-node cluster"
    echo "=============================================="

    # Start orbit-lb in background
    ./target/release/orbit-lb --nodes "$CLUSTER_SIZE" --verbose > "$LOG_FILE" 2>&1 &
    local pid=$!
    echo $pid > "$PID_FILE"

    # Wait a moment and verify it started
    sleep 1
    if ! kill -0 "$pid" 2>/dev/null; then
        print_error "Failed to start orbit-lb. Check $LOG_FILE"
        cat "$LOG_FILE"
        exit 1
    fi

    print_success "orbit-lb started (PID $pid)"
    echo ""
    print_status "Load Balancer Endpoints (default ports):"
    echo "  Redis:      redis-cli -p 6379"
    echo "  PostgreSQL: psql -h 127.0.0.1 -p 5432 -U orbit"
    echo "  MySQL:      mysql -h 127.0.0.1 -P 3306 -u orbit"
    echo "  CQL:        cqlsh 127.0.0.1 9042"
    echo "  HTTP:       curl http://127.0.0.1:8080/health"
    echo "  gRPC:       grpcurl -plaintext 127.0.0.1:50051 list"
    echo ""
    echo "Logs: $LOG_FILE"
    echo "=============================================="
}

# Stop load balancer
stop_lb() {
    print_status "Stopping load balancer..."

    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true

            # Wait for graceful shutdown
            local count=0
            while kill -0 "$pid" 2>/dev/null && [ $count -lt 10 ]; do
                sleep 0.2
                count=$((count + 1))
            done

            if kill -0 "$pid" 2>/dev/null; then
                kill -9 "$pid" 2>/dev/null || true
            fi

            print_success "Stopped orbit-lb (PID $pid)"
        else
            print_warning "orbit-lb not running"
        fi
        rm -f "$PID_FILE"
    else
        print_warning "No PID file found"
    fi
}

# Show status
show_status() {
    print_status "Load Balancer Status"
    echo "=============================================="

    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            echo -e "${GREEN}[RUNNING]${NC} orbit-lb (PID $pid)"

            # Show listening ports
            if command -v lsof &> /dev/null; then
                echo ""
                echo "Listening ports:"
                lsof -i -P -n 2>/dev/null | grep "$pid" | grep LISTEN | awk '{print "  " $9}' | sort -u
            fi
        else
            echo -e "${RED}[STOPPED]${NC} orbit-lb (stale PID file)"
        fi
    else
        echo -e "${YELLOW}[NOT RUNNING]${NC} No load balancer found"
    fi

    echo "=============================================="
}

# Tail logs
tail_logs() {
    if [ -f "$LOG_FILE" ]; then
        print_status "Tailing load balancer logs..."
        tail -f "$LOG_FILE"
    else
        print_error "No log file found: $LOG_FILE"
    fi
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
        --logs|-l)
            tail_logs
            ;;
        --build|-b)
            check_directory
            build_lb
            ;;
        --help|-h)
            echo "Usage: $0 [OPTION] [CLUSTER_SIZE]"
            echo ""
            echo "Options:"
            echo "  <number>      Configure LB for N-node cluster (default: 3)"
            echo "  --stop, -s    Stop load balancer"
            echo "  --status, -S  Show LB status"
            echo "  --logs, -l    Tail LB logs"
            echo "  --build, -b   Build orbit-lb only"
            echo "  --help, -h    Show this help"
            echo ""
            echo "Uses orbit-lb (Rust binary) - no external dependencies!"
            echo ""
            echo "Load balancer listens on default ports:"
            echo "  Redis: 6379 -> 16379, 16380, ..."
            echo "  PostgreSQL: 5432 -> 15432, 15433, ..."
            echo "  MySQL: 3306 -> 13306, 13307, ..."
            echo "  CQL: 9042 -> 19042, 19043, ..."
            echo "  HTTP: 8080 -> 18080, 18081, ..."
            echo "  gRPC: 50051 -> 60051, 60052, ..."
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
