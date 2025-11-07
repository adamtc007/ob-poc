#!/bin/bash

# Build script for DSL POC with greenteagc support

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
USE_GREENTEAGC=true
BINARY_NAME="dsl-poc"

# Function to print usage
print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -h, --help              Show this help message"
    echo "  --no-greenteagc         Build with standard Go GC (default: uses greenteagc)"
    echo "  -o, --output NAME       Specify output binary name (default: dsl-poc)"
    echo ""
    echo "Examples:"
    echo "  $0                      # Build with greenteagc"
    echo "  $0 --no-greenteagc      # Build with standard GC"
    echo "  $0 -o my-binary         # Build as 'my-binary' with greenteagc"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            print_usage
            exit 0
            ;;
        --no-greenteagc)
            USE_GREENTEAGC=false
            shift
            ;;
        -o|--output)
            BINARY_NAME="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            print_usage
            exit 1
            ;;
    esac
done

# Ensure Go is installed
if ! command -v go &> /dev/null; then
    echo -e "${RED}Error: Go is not installed or not in PATH${NC}"
    exit 1
fi

echo -e "${YELLOW}Building DSL POC...${NC}"
echo "Go version: $(go version)"

# Download dependencies
echo -e "${YELLOW}Downloading dependencies...${NC}"
go mod download
go mod tidy

# Build with or without greenteagc
if [ "$USE_GREENTEAGC" = true ]; then
    echo -e "${YELLOW}Building with experimental greenteagc garbage collector...${NC}"
    GOEXPERIMENT=greenteagc go build -v -o "$BINARY_NAME" .
else
    echo -e "${YELLOW}Building with standard Go garbage collector...${NC}"
    go build -v -o "$BINARY_NAME" .
fi

if [ $? -eq 0 ]; then
    echo -e "${GREEN}Build successful!${NC}"
    echo -e "${GREEN}Binary: ${BINARY_NAME}${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Set DB connection: export DB_CONN_STRING=\"postgres://user:password@localhost:5432/db?sslmode=disable\""
    echo "  2. Initialize database: ./$BINARY_NAME init-db"
    echo "  3. Create a case: ./$BINARY_NAME create --cbu=\"CBU-1234\""
else
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi
