#!/bin/bash
# Build Docker images for ob-poc

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "Building ob-poc Docker images..."
echo ""

# Build all images
docker compose build

echo ""
echo "Build complete. Images created:"
docker images | grep ob-poc || true

echo ""
echo "Run './scripts/docker-run.sh' to start the services."
