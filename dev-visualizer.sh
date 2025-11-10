#!/bin/bash

# Development DSL Visualizer Quick Start
# Simple script for development and testing

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}[DEV]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_info "🚀 DSL Visualizer - Development Mode"
echo

# Check if we're in the right directory
if [[ ! -d "rust" ]]; then
    print_error "Please run from ob-poc root directory"
    exit 1
fi

# Kill any existing processes
print_info "Cleaning up existing processes..."
pkill -f "mock_rest_api_server" || true
pkill -f "egui_dsl_visualizer" || true
sleep 1

print_info "Building binaries..."
cd rust

# Build quickly in dev mode
if ! cargo build --features mock-api --bin mock_rest_api_server; then
    print_error "Failed to build mock API server"
    exit 1
fi

if ! cargo build --features visualizer --bin egui_dsl_visualizer; then
    print_error "Failed to build visualizer"
    exit 1
fi

print_success "Binaries built"

# Start API server in background
print_info "Starting mock API server..."
export API_HOST="127.0.0.1"
export API_PORT="8080"
export RUST_LOG="info"

cargo run --features mock-api --bin mock_rest_api_server &
API_PID=$!

# Wait a bit for server to start
sleep 3

# Test if API is responding
if curl -s "http://127.0.0.1:8080/api/health" >/dev/null 2>&1; then
    print_success "API server started (PID: $API_PID)"
else
    print_error "API server failed to start"
    kill $API_PID 2>/dev/null || true
    exit 1
fi

print_info "API available at: http://127.0.0.1:8080"
print_info "Endpoints:"
echo "  - GET /api/health"
echo "  - GET /api/dsls"
echo "  - GET /api/dsls/{id}/ast"
echo

# Set up cleanup on exit
cleanup() {
    print_info "Cleaning up..."
    kill $API_PID 2>/dev/null || true
    pkill -f "egui_dsl_visualizer" || true
    print_success "Cleanup complete"
}
trap cleanup EXIT INT TERM

# Start visualizer
print_info "Starting DSL Visualizer UI..."
print_info "The egui window should open shortly..."
echo

# Run visualizer in foreground
export DSL_API_BASE_URL="http://127.0.0.1:8080"
cargo run --features visualizer --bin egui_dsl_visualizer

cd ..
