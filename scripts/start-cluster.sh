#!/bin/bash

# Orbit-RS Local Cluster Startup Script
# Starts a multi-node cluster for integration and BDD testing
#
# Usage:
#   ./scripts/start-cluster.sh              # Start 3-node cluster (default)
#   ./scripts/start-cluster.sh 5            # Start 5-node cluster
#   ./scripts/start-cluster.sh --stop       # Stop running cluster
#   ./scripts/start-cluster.sh --status     # Show cluster status
#   ./scripts/start-cluster.sh --logs 1     # Tail logs for node 1

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
CLUSTER_SIZE=${1:-3}
USE_LB_PORTS=${USE_LB_PORTS:-false}  # Set to true when using load balancer

# Configure ports based on USE_LB_PORTS setting
# This is called as a function so it can be re-evaluated after USE_LB_PORTS is set
configure_ports() {
    # When using load balancer, nodes use offset ports (LB uses default ports)
    # Without LB, nodes use default ports directly
    if [ "$USE_LB_PORTS" = "true" ]; then
        BASE_GRPC_PORT=60051
        BASE_HTTP_PORT=18080
        BASE_POSTGRES_PORT=15432
        BASE_REDIS_PORT=16379
        BASE_MYSQL_PORT=13306
        BASE_CQL_PORT=19042
        BASE_METRICS_PORT=19090
    else
        BASE_GRPC_PORT=50051
        BASE_HTTP_PORT=8080
        BASE_POSTGRES_PORT=5432
        BASE_REDIS_PORT=6379
        BASE_MYSQL_PORT=3306
        BASE_CQL_PORT=9042
        BASE_METRICS_PORT=9090
    fi
}

# Initialize ports with default values
configure_ports

BASE_DATA_DIR="./cluster-data"
PID_DIR="./cluster-data/pids"
LOG_DIR="./cluster-data/logs"

print_status() {
    echo -e "${BLUE}[CLUSTER]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_node() {
    echo -e "${CYAN}[NODE $1]${NC} $2"
}

# Check if we're in the right directory
check_directory() {
    if [ ! -f "Cargo.toml" ] || [ ! -d "orbit" ]; then
        print_error "Please run this script from the orbit-rs root directory"
        exit 1
    fi
}

# Build the server if needed
build_server() {
    print_status "Building orbit-server (release mode)..."
    cargo build --release --bin orbit-server
    print_success "Build complete"
}

# Calculate port for a node
get_port() {
    local base_port=$1
    local node_num=$2
    echo $((base_port + node_num - 1))
}

# Generate seed nodes list
generate_seeds() {
    local exclude_node=$1
    local seeds=""
    for i in $(seq 1 $CLUSTER_SIZE); do
        if [ $i -ne $exclude_node ]; then
            local grpc_port=$(get_port $BASE_GRPC_PORT $i)
            if [ -n "$seeds" ]; then
                seeds="${seeds},"
            fi
            seeds="${seeds}http://127.0.0.1:${grpc_port}"
        fi
    done
    echo "$seeds"
}

# Start a single node
start_node() {
    local node_num=$1
    local node_id="node-${node_num}"
    local data_dir="${BASE_DATA_DIR}/${node_id}"
    local log_file="${LOG_DIR}/${node_id}.log"
    local pid_file="${PID_DIR}/${node_id}.pid"

    # Calculate ports
    local grpc_port=$(get_port $BASE_GRPC_PORT $node_num)
    local http_port=$(get_port $BASE_HTTP_PORT $node_num)
    local postgres_port=$(get_port $BASE_POSTGRES_PORT $node_num)
    local redis_port=$(get_port $BASE_REDIS_PORT $node_num)
    local mysql_port=$(get_port $BASE_MYSQL_PORT $node_num)
    local cql_port=$(get_port $BASE_CQL_PORT $node_num)
    local metrics_port=$(get_port $BASE_METRICS_PORT $node_num)

    # Get seed nodes (excluding self)
    local seeds=$(generate_seeds $node_num)

    # Create data directory
    mkdir -p "$data_dir"

    print_node $node_num "Starting..."
    print_node $node_num "  gRPC: $grpc_port | HTTP: $http_port | PostgreSQL: $postgres_port | Redis: $redis_port"
    print_node $node_num "  MySQL: $mysql_port | CQL: $cql_port | Metrics: $metrics_port"
    print_node $node_num "  Data: $data_dir"

    # Build command
    local cmd="./target/release/orbit-server"
    cmd="$cmd --node-id $node_id"
    cmd="$cmd --grpc-port $grpc_port"
    cmd="$cmd --http-port $http_port"
    cmd="$cmd --postgres-port $postgres_port"
    cmd="$cmd --redis-port $redis_port"
    cmd="$cmd --mysql-port $mysql_port"
    cmd="$cmd --cql-port $cql_port"
    cmd="$cmd --metrics-port $metrics_port"
    cmd="$cmd --data-dir $data_dir"
    cmd="$cmd --dev-mode"

    if [ -n "$seeds" ]; then
        cmd="$cmd --seed-nodes $seeds"
    fi

    # Start the server in background
    RUST_LOG=info $cmd > "$log_file" 2>&1 &
    local pid=$!
    echo $pid > "$pid_file"

    print_node $node_num "Started with PID $pid"
}

# Stop all nodes
stop_cluster() {
    print_status "Stopping cluster..."

    if [ ! -d "$PID_DIR" ]; then
        print_warning "No cluster found to stop"
        return
    fi

    for pid_file in "$PID_DIR"/*.pid; do
        if [ -f "$pid_file" ]; then
            local node_name=$(basename "$pid_file" .pid)
            local pid=$(cat "$pid_file")

            if kill -0 "$pid" 2>/dev/null; then
                print_status "Stopping $node_name (PID $pid)..."
                kill "$pid" 2>/dev/null || true

                # Wait for graceful shutdown
                local count=0
                while kill -0 "$pid" 2>/dev/null && [ $count -lt 10 ]; do
                    sleep 0.5
                    count=$((count + 1))
                done

                # Force kill if still running
                if kill -0 "$pid" 2>/dev/null; then
                    print_warning "Force killing $node_name..."
                    kill -9 "$pid" 2>/dev/null || true
                fi
            fi

            rm -f "$pid_file"
        fi
    done

    print_success "Cluster stopped"
}

# Show cluster status
show_status() {
    print_status "Cluster Status"
    echo "=============================================="

    if [ ! -d "$PID_DIR" ]; then
        print_warning "No cluster found"
        return
    fi

    local running=0
    local stopped=0

    for pid_file in "$PID_DIR"/*.pid; do
        if [ -f "$pid_file" ]; then
            local node_name=$(basename "$pid_file" .pid)
            local pid=$(cat "$pid_file")
            local node_num=${node_name#node-}

            local grpc_port=$(get_port $BASE_GRPC_PORT $node_num)
            local redis_port=$(get_port $BASE_REDIS_PORT $node_num)
            local postgres_port=$(get_port $BASE_POSTGRES_PORT $node_num)

            if kill -0 "$pid" 2>/dev/null; then
                echo -e "${GREEN}[RUNNING]${NC} $node_name (PID $pid)"
                echo "          Redis: $redis_port | PostgreSQL: $postgres_port | gRPC: $grpc_port"
                running=$((running + 1))
            else
                echo -e "${RED}[STOPPED]${NC} $node_name (PID $pid - not running)"
                stopped=$((stopped + 1))
            fi
        fi
    done

    echo "=============================================="
    echo "Running: $running | Stopped: $stopped"
}

# Tail logs for a node
tail_logs() {
    local node_num=$1
    local log_file="${LOG_DIR}/node-${node_num}.log"

    if [ ! -f "$log_file" ]; then
        print_error "Log file not found: $log_file"
        exit 1
    fi

    print_status "Tailing logs for node-${node_num}..."
    tail -f "$log_file"
}

# Wait for cluster to be ready
wait_for_cluster() {
    print_status "Waiting for cluster to be ready..."

    local max_wait=30
    local wait_count=0

    while [ $wait_count -lt $max_wait ]; do
        local ready_count=0

        for i in $(seq 1 $CLUSTER_SIZE); do
            local redis_port=$(get_port $BASE_REDIS_PORT $i)
            if nc -z 127.0.0.1 $redis_port 2>/dev/null; then
                ready_count=$((ready_count + 1))
            fi
        done

        if [ $ready_count -eq $CLUSTER_SIZE ]; then
            print_success "All $CLUSTER_SIZE nodes are ready!"
            return 0
        fi

        echo -ne "\r  Waiting... ($ready_count/$CLUSTER_SIZE nodes ready)"
        sleep 1
        wait_count=$((wait_count + 1))
    done

    echo ""
    print_warning "Timeout waiting for cluster. Some nodes may still be starting."
    return 1
}

# Print connection info
print_connection_info() {
    echo ""
    print_status "Cluster Connection Information"
    echo "=============================================="
    echo ""
    echo "Redis connections:"
    for i in $(seq 1 $CLUSTER_SIZE); do
        local redis_port=$(get_port $BASE_REDIS_PORT $i)
        echo "  Node $i: redis-cli -p $redis_port"
    done
    echo ""
    echo "PostgreSQL connections:"
    for i in $(seq 1 $CLUSTER_SIZE); do
        local postgres_port=$(get_port $BASE_POSTGRES_PORT $i)
        echo "  Node $i: psql -h 127.0.0.1 -p $postgres_port -U orbit"
    done
    echo ""
    echo "HTTP/REST endpoints:"
    for i in $(seq 1 $CLUSTER_SIZE); do
        local http_port=$(get_port $BASE_HTTP_PORT $i)
        echo "  Node $i: http://127.0.0.1:$http_port"
    done
    echo ""
    echo "Logs:"
    echo "  Directory: $LOG_DIR"
    echo "  Tail node 1: ./scripts/start-cluster.sh --logs 1"
    echo ""
    echo "Stop cluster:"
    echo "  ./scripts/start-cluster.sh --stop"
    echo "=============================================="
}

# Start the cluster
start_cluster() {
    check_directory

    print_status "Starting $CLUSTER_SIZE-node Orbit cluster"
    echo "=============================================="

    # Create directories
    mkdir -p "$PID_DIR"
    mkdir -p "$LOG_DIR"

    # Check if cluster is already running
    if [ -d "$PID_DIR" ] && [ "$(ls -A $PID_DIR 2>/dev/null)" ]; then
        print_warning "Cluster may already be running. Stopping first..."
        stop_cluster
        sleep 2
    fi

    # Build server
    build_server

    echo ""

    # Start each node with a small delay
    for i in $(seq 1 $CLUSTER_SIZE); do
        start_node $i
        sleep 1  # Small delay between nodes for orderly startup
    done

    echo ""

    # Wait for cluster to be ready
    wait_for_cluster

    # Print connection info
    print_connection_info
}

# Clean cluster data
clean_cluster() {
    print_status "Cleaning cluster data..."

    stop_cluster

    if [ -d "$BASE_DATA_DIR" ]; then
        rm -rf "$BASE_DATA_DIR"
        print_success "Cluster data cleaned"
    else
        print_warning "No cluster data found"
    fi
}

# Main entry point
main() {
    case "${1:-}" in
        --stop|-s)
            stop_cluster
            ;;
        --status|-S)
            show_status
            ;;
        --logs|-l)
            if [ -z "${2:-}" ]; then
                print_error "Please specify node number: --logs <node_num>"
                exit 1
            fi
            tail_logs "$2"
            ;;
        --clean|-c)
            clean_cluster
            ;;
        --with-lb)
            # Start cluster with load balancer (nodes use offset ports)
            export USE_LB_PORTS=true
            configure_ports  # Re-configure with offset ports
            shift
            CLUSTER_SIZE=${1:-3}
            start_cluster
            print_status ""
            print_status "Now start the load balancer:"
            print_status "  ./scripts/start-cluster-lb.sh $CLUSTER_SIZE"
            ;;
        --help|-h)
            echo "Usage: $0 [OPTION] [CLUSTER_SIZE]"
            echo ""
            echo "Options:"
            echo "  <number>      Start cluster with specified number of nodes (default: 3)"
            echo "  --with-lb [N] Start cluster configured for load balancer"
            echo "  --stop, -s    Stop running cluster"
            echo "  --status, -S  Show cluster status"
            echo "  --logs, -l N  Tail logs for node N"
            echo "  --clean, -c   Stop cluster and clean all data"
            echo "  --help, -h    Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0              # Start 3-node cluster (direct access)"
            echo "  $0 5            # Start 5-node cluster"
            echo "  $0 --with-lb    # Start cluster + use with load balancer"
            echo "  $0 --stop       # Stop cluster"
            echo "  $0 --logs 1     # Tail node 1 logs"
            echo ""
            echo "Direct access (no load balancer):"
            echo "  Node 1: Redis=6379, PostgreSQL=5432, MySQL=3306, CQL=9042"
            echo "  Node 2: Redis=6380, PostgreSQL=5433, MySQL=3307, CQL=9043"
            echo ""
            echo "With load balancer (--with-lb):"
            echo "  LB:     Redis=6379, PostgreSQL=5432 (default ports)"
            echo "  Node 1: Redis=16379, PostgreSQL=15432 (offset ports)"
            echo "  Node 2: Redis=16380, PostgreSQL=15433"
            ;;
        [0-9]*)
            CLUSTER_SIZE=$1
            start_cluster
            ;;
        *)
            start_cluster
            ;;
    esac
}

main "$@"
