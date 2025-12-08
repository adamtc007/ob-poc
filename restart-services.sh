#!/bin/bash
# Restart all OB-POC services cleanly

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Stopping existing services ==="
pkill -f "dsl_api" 2>/dev/null || true
pkill -f "agentic_server" 2>/dev/null || true
pkill -f "webserver" 2>/dev/null || true
sleep 1

# Kill by port as fallback
lsof -ti:3000 | xargs kill -9 2>/dev/null || true
lsof -ti:3001 | xargs kill -9 2>/dev/null || true
lsof -ti:8181 | xargs kill -9 2>/dev/null || true
sleep 1

echo "=== Building services ==="
cd rust
cargo build --features server --bin dsl_api --bin agentic_server 2>&1 | grep -E "(Compiling|Finished|error)" || true
cd ..

cd go
GOEXPERIMENT=greenteagc,jsonv2 go build -o bin/webserver ./cmd/web
cd ..

echo "=== Starting DSL API (port 3001) ==="
cd rust
DATABASE_URL="postgresql:///data_designer" ./target/debug/dsl_api > /tmp/dsl_api.log 2>&1 &
DSL_PID=$!
cd ..
sleep 2

echo "=== Starting Agentic Server (port 3000) ==="
cd rust
DATABASE_URL="postgresql:///data_designer" ./target/debug/agentic_server > /tmp/agentic_server.log 2>&1 &
AGENT_PID=$!
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
if curl -s http://127.0.0.1:3001/health > /dev/null; then
    echo "✓ DSL API:        http://127.0.0.1:3001 (PID: $DSL_PID)"
else
    echo "✗ DSL API failed to start - check /tmp/dsl_api.log"
fi

if curl -s http://127.0.0.1:3000/api/agent/health > /dev/null; then
    echo "✓ Agentic Server: http://127.0.0.1:3000 (PID: $AGENT_PID)"
else
    echo "✗ Agentic Server failed to start - check /tmp/agentic_server.log"
fi

if curl -s http://127.0.0.1:8181/health > /dev/null; then
    echo "✓ Go Webserver:   http://127.0.0.1:8181 (PID: $GO_PID)"
else
    echo "✗ Go Webserver failed to start - check /tmp/go_webserver.log"
fi

echo ""
echo "Logs: /tmp/dsl_api.log, /tmp/agentic_server.log, /tmp/go_webserver.log"
echo "To stop: pkill -f dsl_api; pkill -f agentic_server; pkill -f webserver"
