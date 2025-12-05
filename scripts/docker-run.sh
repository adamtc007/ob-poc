#!/bin/bash
# Start ob-poc Docker services

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Check for ANTHROPIC_API_KEY
if [ -z "$ANTHROPIC_API_KEY" ]; then
    # Try to load from .env file
    if [ -f ".env" ]; then
        export $(grep -v '^#' .env | xargs)
    fi

    if [ -z "$ANTHROPIC_API_KEY" ]; then
        echo "Error: ANTHROPIC_API_KEY not set"
        echo ""
        echo "Either:"
        echo "  1. export ANTHROPIC_API_KEY='sk-ant-...'"
        echo "  2. Create a .env file with: ANTHROPIC_API_KEY=sk-ant-..."
        exit 1
    fi
fi

echo "Starting ob-poc services..."
echo ""

# Start services in detached mode
docker compose up -d

echo ""
echo "Services starting..."
echo ""

# Wait for health checks
echo "Waiting for services to be healthy..."
sleep 5

# Show status
docker compose ps

echo ""
echo "Service URLs:"
echo "  PostgreSQL:  localhost:5432"
echo "  Rust Server: http://localhost:3000"
echo "  Go UI:       http://localhost:8181"
echo ""
echo "View logs:  docker compose logs -f"
echo "Stop:       ./scripts/docker-stop.sh"
