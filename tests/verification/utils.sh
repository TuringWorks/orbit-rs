#!/bin/bash

# Shared utilities for verification scripts

# Function to run commands with timeout (compatible with macOS)
run_with_timeout() {
    local timeout=$1
    shift
    if command -v timeout &> /dev/null; then
        timeout "$timeout" "$@"
    elif command -v gtimeout &> /dev/null; then
        gtimeout "$timeout" "$@"
    else
        # Fallback without timeout on macOS if coreutils not installed
        "$@"
    fi
}

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${CYAN}ℹ${NC} $1"
}

# Function to check if a command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Function to install cargo tool if not present
install_cargo_tool() {
    local tool_name=$1
    local install_args=${2:-""}
    
    if ! command_exists "$tool_name"; then
        print_info "Installing $tool_name..."
        if [ -n "$install_args" ]; then
            run_with_timeout 60 cargo install "$tool_name" $install_args
        else
            run_with_timeout 60 cargo install "$tool_name"
        fi
        
        if command_exists "$tool_name"; then
            print_success "$tool_name installed successfully"
        else
            print_warning "Failed to install $tool_name"
            return 1
        fi
    fi
}

# Function to check and warn about missing system dependencies
check_system_dependencies() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS specific checks
        if ! command_exists "protoc"; then
            print_warning "Protocol Buffers compiler not found. Install with: brew install protobuf"
            return 1
        fi
        
        if ! command_exists "timeout" && ! command_exists "gtimeout"; then
            print_warning "GNU timeout not found. Install with: brew install coreutils"
            print_info "Timeout functionality will be disabled"
        fi
    fi
    return 0
}