#!/bin/bash

# DSL Visualizer Startup Script
# This script manages all services needed for the DSL visualizer UI

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
RUST_DIR="rust"
API_PORT=8080
API_HOST="127.0.0.1"
POSTGRES_DB="ob-poc"
POSTGRES_URL="postgresql://localhost:5432/${POSTGRES_DB}"

# PID files for tracking running services
PID_DIR="./pids"
API_PID_FILE="${PID_DIR}/mock_api.pid"
DB_PID_FILE="${PID_DIR}/postgres.pid"

# Log files
LOG_DIR="./logs"
API_LOG_FILE="${LOG_DIR}/mock_api.log"
VISUALIZER_LOG_FILE="${LOG_DIR}/visualizer.log"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
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

# Function to create directories
create_dirs() {
    mkdir -p "${PID_DIR}" "${LOG_DIR}"
}

# Function to check if a service is running
is_service_running() {
    local pid_file=$1
    if [[ -f "$pid_file" ]]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            return 0  # Service is running
        else
            rm -f "$pid_file"  # Clean up stale PID file
            return 1  # Service is not running
        fi
    else
        return 1  # PID file doesn't exist
    fi
}

# Function to stop a service
stop_service() {
    local service_name=$1
    local pid_file=$2

    if is_service_running "$pid_file"; then
        local pid=$(cat "$pid_file")
        print_status "Stopping $service_name (PID: $pid)..."

        # Try graceful shutdown first
        kill "$pid" 2>/dev/null || true
        sleep 2

        # Force kill if still running
        if ps -p "$pid" > /dev/null 2>&1; then
            print_warning "Force killing $service_name..."
            kill -9 "$pid" 2>/dev/null || true
        fi

        rm -f "$pid_file"
        print_success "$service_name stopped"
    else
        print_status "$service_name is not running"
    fi
}

# Function to stop all services
stop_all() {
    print_status "Stopping all DSL visualizer services..."

    # Stop Mock API server
    stop_service "Mock API Server" "$API_PID_FILE"

    # Stop any running visualizer processes
    pkill -f "egui_dsl_visualizer" || true

    # Stop any cargo processes related to our binaries
    pkill -f "mock_rest_api_server" || true

    print_success "All services stopped"
}

# Function to check prerequisites
check_prerequisites() {
    print_status "Checking prerequisites..."

    # Check if we're in the right directory
    if [[ ! -d "$RUST_DIR" ]]; then
        print_error "Rust directory not found. Please run this script from the ob-poc root directory."
        exit 1
    fi

    # Check if cargo is installed
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo not found. Please install Rust and Cargo."
        exit 1
    fi

    # Check if PostgreSQL is accessible (optional)
    if command -v psql &> /dev/null; then
        if psql "$POSTGRES_URL" -c "SELECT 1;" &> /dev/null; then
            print_success "PostgreSQL database accessible"
        else
            print_warning "PostgreSQL database not accessible - using mock data only"
        fi
    else
        print_warning "psql not found - using mock data only"
    fi

    print_success "Prerequisites check complete"
}

# Function to build the binaries
build_binaries() {
    print_status "Building DSL visualizer binaries..."

    cd "$RUST_DIR"

    # Build mock API server
    print_status "Building mock API server..."
    if cargo build --features mock-api --bin mock_rest_api_server; then
        print_success "Mock API server built successfully"
    else
        print_error "Failed to build mock API server"
        exit 1
    fi

    # Build visualizer
    print_status "Building egui visualizer..."
    if cargo build --features visualizer --bin egui_dsl_visualizer; then
        print_success "Egui visualizer built successfully"
    else
        print_error "Failed to build egui visualizer"
        exit 1
    fi

    cd ..
    print_success "All binaries built successfully"
}

# Function to start the mock API server
start_api_server() {
    if is_service_running "$API_PID_FILE"; then
        print_warning "Mock API server is already running"
        return 0
    fi

    print_status "Starting Mock API server on ${API_HOST}:${API_PORT}..."

    cd "$RUST_DIR"

    # Set environment variables
    export API_HOST="$API_HOST"
    export API_PORT="$API_PORT"
    export RUST_LOG="info"

    # Start the server in background
    nohup cargo run --features mock-api --bin mock_rest_api_server \
        > "../${API_LOG_FILE}" 2>&1 &

    local pid=$!
    echo "$pid" > "../${API_PID_FILE}"

    cd ..

    # Wait for server to start
    print_status "Waiting for API server to start..."
    local max_attempts=10
    local attempt=1

    while [[ $attempt -le $max_attempts ]]; do
        if curl -s "http://${API_HOST}:${API_PORT}/api/health" > /dev/null 2>&1; then
            print_success "Mock API server started successfully (PID: $pid)"
            return 0
        fi

        sleep 1
        ((attempt++))
    done

    print_error "Failed to start Mock API server - check ${API_LOG_FILE}"
    return 1
}

# Function to test API endpoints
test_api() {
    print_status "Testing API endpoints..."

    local base_url="http://${API_HOST}:${API_PORT}"

    # Test health endpoint
    if curl -s "${base_url}/api/health" | grep -q "ok" 2>/dev/null; then
        print_success "Health endpoint working"
    else
        print_error "Health endpoint failed"
        return 1
    fi

    # Test DSL list endpoint
    if curl -s "${base_url}/api/dsls" | grep -q "entries" 2>/dev/null; then
        print_success "DSL list endpoint working"
    else
        print_error "DSL list endpoint failed"
        return 1
    fi

    print_success "API endpoints are responding correctly"
}

# Function to start the visualizer
start_visualizer() {
    print_status "Starting DSL Visualizer UI..."

    cd "$RUST_DIR"

    # Set environment variables
    export RUST_LOG="info"
    export DSL_API_BASE_URL="http://${API_HOST}:${API_PORT}"

    # Start visualizer (this will block)
    print_status "Launching egui DSL visualizer..."
    print_status "The visualizer window should open shortly..."

    cargo run --features visualizer --bin egui_dsl_visualizer 2>&1 | tee "../${VISUALIZER_LOG_FILE}"

    cd ..
}

# Function to show status
show_status() {
    print_status "DSL Visualizer Service Status:"
    echo

    if is_service_running "$API_PID_FILE"; then
        local api_pid=$(cat "$API_PID_FILE")
        print_success "Mock API Server: Running (PID: $api_pid) - http://${API_HOST}:${API_PORT}"
    else
        print_warning "Mock API Server: Not running"
    fi

    if pgrep -f "egui_dsl_visualizer" > /dev/null; then
        print_success "DSL Visualizer UI: Running"
    else
        print_warning "DSL Visualizer UI: Not running"
    fi

    echo
    print_status "Log files:"
    echo "  API Server: ${API_LOG_FILE}"
    echo "  Visualizer: ${VISUALIZER_LOG_FILE}"

    echo
    print_status "API Endpoints:"
    echo "  Health: http://${API_HOST}:${API_PORT}/api/health"
    echo "  DSL List: http://${API_HOST}:${API_PORT}/api/dsls"
    echo "  DSL Content: http://${API_HOST}:${API_PORT}/api/dsls/{id}/ast"
}

# Function to show logs
show_logs() {
    local service=$1

    case $service in
        "api")
            if [[ -f "$API_LOG_FILE" ]]; then
                print_status "Mock API Server logs:"
                tail -f "$API_LOG_FILE"
            else
                print_warning "API log file not found: $API_LOG_FILE"
            fi
            ;;
        "visualizer")
            if [[ -f "$VISUALIZER_LOG_FILE" ]]; then
                print_status "Visualizer logs:"
                tail -f "$VISUALIZER_LOG_FILE"
            else
                print_warning "Visualizer log file not found: $VISUALIZER_LOG_FILE"
            fi
            ;;
        *)
            print_error "Unknown service: $service"
            print_status "Available services: api, visualizer"
            ;;
    esac
}

# Function to show help
show_help() {
    echo "DSL Visualizer Management Script"
    echo
    echo "Usage: $0 [COMMAND]"
    echo
    echo "Commands:"
    echo "  start       Start all services and launch visualizer"
    echo "  stop        Stop all services"
    echo "  restart     Stop and start all services"
    echo "  status      Show service status"
    echo "  build       Build binaries"
    echo "  test        Test API endpoints"
    echo "  logs [svc]  Show logs (api|visualizer)"
    echo "  help        Show this help message"
    echo
    echo "Examples:"
    echo "  $0 start           # Start everything"
    echo "  $0 status          # Check what's running"
    echo "  $0 logs api        # Show API logs"
    echo "  $0 stop            # Stop everything"
}

# Main script logic
main() {
    local command=${1:-start}

    # Create necessary directories
    create_dirs

    case $command in
        "start")
            print_status "Starting DSL Visualizer System..."
            check_prerequisites
            build_binaries
            start_api_server
            test_api

            print_success "All services started successfully!"
            print_status "API Server running on: http://${API_HOST}:${API_PORT}"
            echo

            # Show status before starting visualizer
            show_status
            echo

            print_status "Press Ctrl+C to stop all services"
            print_status "Starting visualizer in 3 seconds..."
            sleep 3

            # Set up cleanup trap
            trap 'print_status "Shutting down..."; stop_all; exit 0' INT TERM

            start_visualizer
            ;;
        "stop")
            stop_all
            ;;
        "restart")
            stop_all
            sleep 2
            main start
            ;;
        "status")
            show_status
            ;;
        "build")
            check_prerequisites
            build_binaries
            ;;
        "test")
            if is_service_running "$API_PID_FILE"; then
                test_api
            else
                print_error "API server is not running. Start it first with: $0 start"
                exit 1
            fi
            ;;
        "logs")
            show_logs "$2"
            ;;
        "help"|"--help"|"-h")
            show_help
            ;;
        *)
            print_error "Unknown command: $command"
            echo
            show_help
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
