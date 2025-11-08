#!/usr/bin/env bash
set -euo pipefail

# Dev harness to build and run the Rust AST visualization tool.
# - Ensures PostgreSQL is reachable via $DATABASE_URL (defaults to local ob-poc)
# - Applies schema + migrations if needed
# - Optionally seeds dictionary/CBU sample data
# - Builds and runs `rust/src/bin/ast_viz.rs`
#
# Usage examples:
#   scripts/run_ast_viz.sh --create-ob              # create an OB request and visualize its AST
#   scripts/run_ast_viz.sh --version <UUID>         # visualize existing version's AST
#   DATABASE_URL=postgresql://localhost:5432/ob-poc scripts/run_ast_viz.sh --create-ob

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SQL_DIR="$ROOT_DIR/sql"
RUST_DIR="$ROOT_DIR/rust"

# 1) Check tools
command -v psql >/dev/null 2>&1 || { echo "ERROR: psql is required in PATH"; exit 1; }
command -v pg_isready >/dev/null 2>&1 || echo "WARN: pg_isready not found; skipping readiness probe"
command -v protoc >/dev/null 2>&1 || echo "WARN: protoc (protobuf) not found; only needed for gRPC builds"
command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo is required in PATH"; exit 1; }

# 2) Database URL
export DATABASE_URL=${DATABASE_URL:-"postgresql://localhost:5432/ob-poc"}
echo "Using DATABASE_URL=$DATABASE_URL"

# 3) Wait for PostgreSQL
if command -v pg_isready >/dev/null 2>&1; then
  echo "Checking PostgreSQL readiness..."
  ATTEMPTS=30
  until pg_isready -d "$DATABASE_URL" -q || [ $ATTEMPTS -eq 0 ]; do
    ATTEMPTS=$((ATTEMPTS-1))
    sleep 1
  done
  pg_isready -d "$DATABASE_URL" -q || { echo "ERROR: PostgreSQL not reachable at $DATABASE_URL"; exit 1; }
fi

# 4) Ensure schema + migrations (idempotent)
echo "Applying base schema and migrations (idempotent)..."
if [ -f "$SQL_DIR/00_init_schema.sql" ]; then
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$SQL_DIR/00_init_schema.sql"
fi
if [ -d "$SQL_DIR/migrations" ]; then
  for f in "$SQL_DIR"/migrations/*.sql; do
    [ -f "$f" ] || continue
    echo "Running migration: $f"
    psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"
  done
fi

# Optional seeds (safe to re-run)
if [ -f "$SQL_DIR/03_seed_dictionary_attributes.sql" ]; then
  echo "Seeding dictionary attributes..."
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$SQL_DIR/03_seed_dictionary_attributes.sql" || true
fi
if [ -f "$SQL_DIR/05_seed_simplified_cbus.sql" ]; then
  echo "Seeding simplified CBU samples..."
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$SQL_DIR/05_seed_simplified_cbus.sql" || true
fi

# 5) Build the dev harness binary
echo "Building Rust ast_viz binary..."
(
  cd "$RUST_DIR"
  cargo build --bin ast_viz
)

# 6) Run the visualization harness, forwarding all args
echo "Running ast_viz $*"
(
  cd "$RUST_DIR"
  cargo run --bin ast_viz -- "$@"
)

