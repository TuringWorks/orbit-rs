#!/bin/bash

# =============================================================================
# run_demo.sh
# Interactive PostgreSQL Server Demo for Orbit-RS
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m' # No Color

# Configuration
SERVER_HOST="127.0.0.1"
SERVER_PORT="5433"
DB_NAME="orbit_demo"
DB_USER="orbit"

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SQL_DIR="$SCRIPT_DIR/sql"
LOG_FILE="$SCRIPT_DIR/server.log"

# Function to print colored output
print_header() {
    echo -e "${PURPLE}========================================${NC}"
    echo -e "${PURPLE}$1${NC}"
    echo -e "${PURPLE}========================================${NC}"
    echo ""
}

print_info() {
    echo -e "${CYAN}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_step() {
    echo -e "${BLUE}📍 $1${NC}"
}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check if psql is available
check_psql() {
    if ! command_exists psql; then
        print_error "psql is not installed or not in PATH"
        print_info "Please install PostgreSQL client tools:"
        print_info "  macOS: brew install postgresql"
        print_info "  Ubuntu/Debian: sudo apt install postgresql-client"
        print_info "  CentOS/RHEL: sudo yum install postgresql"
        exit 1
    fi
    print_success "psql is available"
}

# Function to check if cargo is available
check_cargo() {
    if ! command_exists cargo; then
        print_error "cargo is not installed or not in PATH"
        print_info "Please install Rust: https://rustup.rs/"
        exit 1
    fi
    print_success "cargo is available"
}

# Function to build the server
build_server() {
    print_step "Building PostgreSQL server..."
    cd "$SCRIPT_DIR"
    cargo build --release --bin postgres_server
    print_success "Server built successfully"
}

# Function to start the server in the background
start_server() {
    print_step "Starting PostgreSQL server on $SERVER_HOST:$SERVER_PORT..."
    cd "$SCRIPT_DIR"
    
    # Kill any existing server process
    pkill -f "postgres_server" || true
    sleep 1
    
    # Start server in background and redirect output to log file
    cargo run --release --bin postgres_server > "$LOG_FILE" 2>&1 &
    SERVER_PID=$!
    echo $SERVER_PID > "$SCRIPT_DIR/server.pid"
    
    # Wait for server to start
    print_info "Waiting for server to start..."
    sleep 3
    
    # Check if server is running
    if ! kill -0 $SERVER_PID 2>/dev/null; then
        print_error "Server failed to start. Check $LOG_FILE for details:"
        tail -n 20 "$LOG_FILE"
        exit 1
    fi
    
    print_success "Server started with PID $SERVER_PID"
    print_info "Server logs: $LOG_FILE"
}

# Function to stop the server
stop_server() {
    if [ -f "$SCRIPT_DIR/server.pid" ]; then
        SERVER_PID=$(cat "$SCRIPT_DIR/server.pid")
        if kill -0 $SERVER_PID 2>/dev/null; then
            print_step "Stopping server (PID: $SERVER_PID)..."
            kill $SERVER_PID
            sleep 2
            # Force kill if still running
            kill -9 $SERVER_PID 2>/dev/null || true
        fi
        rm -f "$SCRIPT_DIR/server.pid"
    fi
    print_success "Server stopped"
}

# Function to test server connection
test_connection() {
    print_step "Testing server connection..."
    local max_attempts=10
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
    if PGSSLMODE=disable psql -h "$SERVER_HOST" -p "$SERVER_PORT" -U "$DB_USER" -d "$DB_NAME" -c "SELECT 1;" >/dev/null 2>&1; then
            print_success "Connection test successful"
            return 0
        fi
        
        print_info "Attempt $attempt/$max_attempts: Connection failed, retrying in 2 seconds..."
        sleep 2
        ((attempt++))
    done
    
    print_error "Failed to connect to server after $max_attempts attempts"
    print_info "Check server logs: $LOG_FILE"
    return 1
}

# Function to run a SQL script
run_sql_script() {
    local script_name=$1
    local script_path="$SQL_DIR/$script_name"
    
    if [ ! -f "$script_path" ]; then
        print_error "Script not found: $script_path"
        return 1
    fi
    
    print_step "Running SQL script: $script_name"
    
    if PGSSLMODE=disable psql -h "$SERVER_HOST" -p "$SERVER_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$script_path"; then
        print_success "Script completed successfully"
    else
        print_warning "Script completed with warnings/errors"
    fi
}

# Function to open interactive psql session
interactive_session() {
    print_step "Opening interactive psql session..."
    print_info "You can now run SQL commands directly!"
    print_info "Type \\q to exit the session"
    print_info ""
    print_info "Try these commands:"
    print_info "  \\dt              -- List tables"
    print_info "  \\d table_name    -- Describe table"
    print_info "  SELECT * FROM users;"
    print_info ""
    
    PGSSLMODE=disable psql -h "$SERVER_HOST" -p "$SERVER_PORT" -U "$DB_USER" -d "$DB_NAME"
}

# Function to show the demo menu
show_menu() {
    echo ""
    print_header "Orbit-RS PostgreSQL Demo Menu"
    echo -e "${WHITE}1)${NC} Run Basic Operations Demo (CREATE, INSERT, SELECT, UPDATE, DELETE)"
    echo -e "${WHITE}2)${NC} Run JOINs and Indices Demo (All JOIN types, complex queries)"
    echo -e "${WHITE}3)${NC} Run Vector & Advanced Features Demo (Vector ops, JSON, transactions)"
    echo -e "${WHITE}4)${NC} Run Information Schema Demo (Meta-data queries, psql \\dt support)"
    echo -e "${WHITE}5)${NC} Run DVD Rental Database Demo (Real-world PostgreSQL sample database)"
    echo -e "${WHITE}6)${NC} Run All Demos Sequentially"
    echo -e "${WHITE}7)${NC} Interactive psql Session"
    echo -e "${WHITE}8)${NC} View Server Logs"
    echo -e "${WHITE}9)${NC} Restart Server"
    echo -e "${WHITE}q)${NC} Quit and Stop Server"
    echo ""
    echo -ne "${CYAN}Choose an option [1-9, q]: ${NC}"
}

# Main demo loop
run_demo_loop() {
    while true; do
        show_menu
        read -r choice
        
        case $choice in
            1)
                run_sql_script "01_basic_operations.sql"
                ;;
            2)
                run_sql_script "02_joins_and_indices.sql"
                ;;
            3)
                run_sql_script "03_vector_and_advanced.sql"
                ;;
            4)
                run_sql_script "04_information_schema.sql"
                ;;
            5)
                run_sql_script "05_dvd_rental_demo.sql"
                ;;
            6)
                print_step "Running all demo scripts sequentially..."
                run_sql_script "01_basic_operations.sql"
                echo ""
                run_sql_script "02_joins_and_indices.sql"
                echo ""
                run_sql_script "03_vector_and_advanced.sql"
                echo ""
                run_sql_script "04_information_schema.sql"
                echo ""
                run_sql_script "05_dvd_rental_demo.sql"
                ;;
            7)
                interactive_session
                ;;
            8)
                print_step "Showing recent server logs..."
                tail -n 50 "$LOG_FILE"
                ;;
            9)
                stop_server
                start_server
                test_connection || exit 1
                ;;
            q|Q)
                break
                ;;
            *)
                print_warning "Invalid option. Please choose 1-9 or q."
                ;;
        esac
        
        if [[ "$choice" != "q" && "$choice" != "Q" ]]; then
            echo ""
            print_info "Press Enter to return to menu..."
            read -r
        fi
    done
}

# Cleanup function
cleanup() {
    print_step "Cleaning up..."
    stop_server
    print_success "Demo completed. Thank you!"
}

# Main execution
main() {
    # Set trap for cleanup
    trap cleanup EXIT INT TERM
    
    print_header "🚀 Orbit-RS PostgreSQL Server Demo"
    print_info "This demo showcases the PostgreSQL wire protocol implementation"
    print_info "with support for tables, indices, JOINs, and vector operations."
    echo ""
    
    # Check prerequisites
    print_step "Checking prerequisites..."
    check_cargo
    check_psql
    echo ""
    
    # Build and start server
    build_server
    start_server
    
    # Test connection
    if ! test_connection; then
        exit 1
    fi
    
    echo ""
    print_success "🎉 Server is ready for demo!"
    print_info "Server is running at: postgresql://$DB_USER@$SERVER_HOST:$SERVER_PORT/$DB_NAME"
    
    # Run demo loop
    run_demo_loop
}

# Script usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  --build-only   Only build the server, don't run demo"
    echo "  --auto-demo    Run all demos automatically without menu"
    echo ""
    echo "Examples:"
    echo "  $0                 # Run interactive demo"
    echo "  $0 --auto-demo     # Run all demos automatically"
    echo "  $0 --build-only    # Just build the server"
}

# Parse command line arguments
case "${1:-}" in
    -h|--help)
        usage
        exit 0
        ;;
    --build-only)
        check_cargo
        build_server
        exit 0
        ;;
    --auto-demo)
        trap cleanup EXIT INT TERM
        print_header "🚀 Orbit-RS PostgreSQL Automated Demo"
        check_cargo
        check_psql
        build_server
        start_server
        test_connection || exit 1
        
        print_step "Running all demo scripts automatically..."
        run_sql_script "01_basic_operations.sql"
        echo ""
        run_sql_script "02_joins_and_indices.sql"
        echo ""
        run_sql_script "03_vector_and_advanced.sql"
        
        print_success "🎉 Automated demo completed!"
        exit 0
        ;;
    "")
        main
        ;;
    *)
        print_error "Unknown option: $1"
        usage
        exit 1
        ;;
esac