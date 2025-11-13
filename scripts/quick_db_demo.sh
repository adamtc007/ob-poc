#!/bin/bash

# Quick Database Demo Setup Script
# Sets up PostgreSQL and runs the simple real database demo

set -e

echo "🚀 Quick Database Demo Setup"
echo "============================"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Configuration
DB_NAME="ob_poc"
DB_USER="${POSTGRES_USER:-postgres}"
DB_PASSWORD="${POSTGRES_PASSWORD:-password}"
DB_HOST="${POSTGRES_HOST:-localhost}"
DB_PORT="${POSTGRES_PORT:-5432}"
DATABASE_URL="postgresql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

echo -e "${YELLOW}Configuration:${NC}"
echo "  Database: $DB_HOST:$DB_PORT/$DB_NAME"
echo "  User: $DB_USER"
echo ""

# Check PostgreSQL connection
echo "🔌 Testing PostgreSQL connection..."
if psql "$DATABASE_URL" -c "SELECT 1;" >/dev/null 2>&1; then
    echo -e "${GREEN}✅ PostgreSQL connection successful${NC}"
else
    echo -e "${RED}❌ Cannot connect to PostgreSQL${NC}"
    echo "   Make sure PostgreSQL is running:"
    echo "   - macOS: brew services start postgresql"
    echo "   - Linux: sudo systemctl start postgresql"
    echo "   - Or set DATABASE_URL environment variable"
    exit 1
fi

# Create database if needed
echo "🗄️  Creating database if needed..."
POSTGRES_URL="postgresql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/postgres"
psql "$POSTGRES_URL" -c "CREATE DATABASE \"$DB_NAME\";" 2>/dev/null || echo "Database already exists"

# Build the demo
echo "🔨 Building Rust project..."
cd rust
cargo build --example simple_real_database_demo --features="database" --quiet || {
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
}
echo -e "${GREEN}✅ Build successful${NC}"

# Set environment and run
echo "🚀 Running simple database demo..."
export DATABASE_URL="$DATABASE_URL"
echo "   Using DATABASE_URL: $DATABASE_URL"
echo ""

cargo run --example simple_real_database_demo --features="database" --quiet || {
    echo -e "${RED}❌ Demo failed${NC}"
    exit 1
}

echo ""
echo -e "${GREEN}🎉 Demo completed successfully!${NC}"
echo ""
echo "💡 To explore the database:"
echo "   psql \"$DATABASE_URL\""
echo "   \\dt \"ob-poc\".*"
echo "   SELECT * FROM \"ob-poc\".demo_dsl_instances;"
