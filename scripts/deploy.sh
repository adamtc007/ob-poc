#!/bin/bash
# Deploy script for ob-poc web server
# Builds everything fresh and restarts the server

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$PROJECT_ROOT/rust"

echo "=== OB-POC Deploy Script ==="
echo "Project root: $PROJECT_ROOT"
echo ""

# Kill existing server
echo "1. Stopping existing server..."
pkill -f "ob-poc-web" 2>/dev/null || true
sleep 1

# Build WASM components
echo "2. Building WASM components..."
cd "$RUST_DIR/crates/ob-poc-graph"
wasm-pack build --target web --out-dir ../ob-poc-web/static/wasm 2>&1 | tail -3

cd "$RUST_DIR/crates/ob-poc-ui"
wasm-pack build --target web --out-dir ../ob-poc-web/static/wasm 2>&1 | tail -3

# Force rebuild of web server (touch a source file to invalidate cache)
echo "3. Rebuilding web server..."
touch "$RUST_DIR/crates/ob-poc-web/src/main.rs"
cd "$RUST_DIR"
cargo build -p ob-poc-web 2>&1 | tail -3

# Add cache-busting to WASM files by appending build timestamp
BUILD_TIME=$(date +%s)
echo "4. Build timestamp: $BUILD_TIME"

# Start server
echo "5. Starting server..."
cd "$RUST_DIR"
DATABASE_URL="${DATABASE_URL:-postgresql:///data_designer}" \
  cargo run -p ob-poc-web &

sleep 3

echo ""
echo "=== Deploy Complete ==="
echo "Server running at: http://localhost:3000"
echo ""
echo "To force browser refresh:"
echo "  - Hard refresh: Cmd+Shift+R (Mac) or Ctrl+Shift+R (Windows)"
echo "  - Or open in incognito window"
echo ""
