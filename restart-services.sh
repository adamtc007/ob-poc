#!/bin/bash
# Restart all OB-POC services cleanly.
#
# Slice 4.1 (2026-04-22): dsl_api binary retired. ob-poc-web
# (`runbook-gate-vnext` ON) is the sole authoritative HTTP server.
# This script previously started dsl_api on :3001 and agentic_server
# on :3000; both are gone. Use `cargo x deploy` for ob-poc-web.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Stopping existing services ==="
pkill -f "ob-poc-web" 2>/dev/null || true
pkill -f "webserver" 2>/dev/null || true
sleep 1

# Kill by port as fallback
lsof -ti:3000 | xargs kill -9 2>/dev/null || true
lsof -ti:8181 | xargs kill -9 2>/dev/null || true
sleep 1

echo "=== Building services ==="
cd rust
cargo build -p ob-poc-web 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

cd go
GOEXPERIMENT=greenteagc,jsonv2 go build -o bin/webserver ./cmd/web
cd ..

echo "=== Starting ob-poc-web (port 3000) ==="
cd rust
DATABASE_URL="postgresql:///data_designer" ./target/debug/ob-poc-web > /tmp/ob-poc-web.log 2>&1 &
WEB_PID=$!
cd ..
sleep 2

echo "=== Starting Go Webserver (port 8181) ==="
cd go
GOEXPERIMENT=greenteagc,jsonv2 ./bin/webserver > /tmp/go_webserver.log 2>&1 &
GO_PID=$!
cd ..
sleep 1

echo ""
echo "=== Checking services ==="
if curl -s http://127.0.0.1:3000/api/health > /dev/null 2>&1 || curl -s http://127.0.0.1:3000/ > /dev/null 2>&1; then
    echo "✓ ob-poc-web:    http://127.0.0.1:3000 (PID: $WEB_PID)"
else
    echo "✗ ob-poc-web failed to start - check /tmp/ob-poc-web.log"
fi

if curl -s http://127.0.0.1:8181/health > /dev/null; then
    echo "✓ Go Webserver:  http://127.0.0.1:8181 (PID: $GO_PID)"
else
    echo "✗ Go Webserver failed to start - check /tmp/go_webserver.log"
fi

echo ""
echo "Logs: /tmp/ob-poc-web.log, /tmp/go_webserver.log"
echo "To stop: pkill -f ob-poc-web; pkill -f webserver"
