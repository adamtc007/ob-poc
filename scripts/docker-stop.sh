#!/bin/bash
# Stop ob-poc Docker services

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "Stopping ob-poc services..."

docker compose down

echo ""
echo "Services stopped."
echo ""
echo "Note: Database data is preserved in the 'postgres_data' volume."
echo "To remove all data: docker compose down -v"
