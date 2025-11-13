#!/bin/bash

# End-to-End Database Demo Setup and Run Script
# This script sets up PostgreSQL and runs the complete demonstration

set -e

echo "🚀 End-to-End Database Demo Setup Script"
echo "========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
DB_NAME="ob_poc"
DB_USER="${POSTGRES_USER:-postgres}"
DB_PASSWORD="${POSTGRES_PASSWORD:-password}"
DB_HOST="${POSTGRES_HOST:-localhost}"
DB_PORT="${POSTGRES_PORT:-5432}"
DATABASE_URL="postgresql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

# Functions
print_step() {
    echo -e "${BLUE}📋 Step $1: $2${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check if PostgreSQL is running
check_postgres() {
    print_step 1 "Checking PostgreSQL availability"

    if command -v pg_isready >/dev/null 2>&1; then
        if pg_isready -h $DB_HOST -p $DB_PORT -U $DB_USER >/dev/null 2>&1; then
            print_success "PostgreSQL is running on $DB_HOST:$DB_PORT"
            return 0
        else
            print_error "PostgreSQL is not responding on $DB_HOST:$DB_PORT"
            return 1
        fi
    else
        print_warning "pg_isready not found, attempting direct connection test"
        # Try a simple connection test
        if psql "$DATABASE_URL" -c "SELECT 1;" >/dev/null 2>&1; then
            print_success "PostgreSQL connection successful"
            return 0
        else
            print_error "Could not connect to PostgreSQL"
            return 1
        fi
    fi
}

# Create database if it doesn't exist
create_database() {
    print_step 2 "Creating database '$DB_NAME'"

    # Connect to postgres database to create our target database
    POSTGRES_URL="postgresql://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/postgres"

    # Check if database exists
    DB_EXISTS=$(psql "$POSTGRES_URL" -tAc "SELECT 1 FROM pg_database WHERE datname='$DB_NAME';" 2>/dev/null || echo "")

    if [ "$DB_EXISTS" = "1" ]; then
        print_success "Database '$DB_NAME' already exists"
    else
        echo "Creating database '$DB_NAME'..."
        psql "$POSTGRES_URL" -c "CREATE DATABASE \"$DB_NAME\";" 2>/dev/null || {
            print_error "Failed to create database '$DB_NAME'"
            exit 1
        }
        print_success "Database '$DB_NAME' created successfully"
    fi
}

# Initialize database schema
init_schema() {
    print_step 3 "Initializing database schema"

    # Check if demo_setup.sql exists
    if [ -f "sql/demo_setup.sql" ]; then
        echo "Running schema initialization..."
        psql "$DATABASE_URL" -f sql/demo_setup.sql 2>/dev/null || {
            print_error "Failed to initialize schema from sql/demo_setup.sql"
            exit 1
        }
        print_success "Schema initialized successfully"
    else
        print_error "Schema file sql/demo_setup.sql not found"
        print_warning "Please run this script from the ob-poc root directory"
        exit 1
    fi
}

# Clean up previous demo data
cleanup_demo_data() {
    print_step 4 "Cleaning up previous demo data"

    echo "Removing previous demo data..."
    psql "$DATABASE_URL" -c "SELECT \"ob-poc\".cleanup_demo_data();" >/dev/null 2>&1 || {
        print_warning "Cleanup function not available or failed (this is normal for first run)"
    }
    print_success "Demo data cleanup completed"
}

# Build the Rust project
build_project() {
    print_step 5 "Building Rust project with database features"

    echo "Compiling project..."
    cargo build --example real_database_end_to_end_demo --features="database" || {
        print_error "Failed to build Rust project"
        print_warning "Make sure you have Rust installed and all dependencies available"
        exit 1
    }
    print_success "Project built successfully"
}

# Run the demo
run_demo() {
    print_step 6 "Running End-to-End Database Demo"

    export DATABASE_URL="$DATABASE_URL"
    echo "Database URL: $DATABASE_URL"
    echo ""
    echo "🎬 Starting demo execution..."
    echo "==============================================="

    cargo run --example real_database_end_to_end_demo --features="database" || {
        print_error "Demo execution failed"
        exit 1
    }

    print_success "Demo completed successfully!"
}

# Verify demo results
verify_results() {
    print_step 7 "Verifying demo results in database"

    echo "Checking database for demo data..."

    # Check CBUs
    CBU_COUNT=$(psql "$DATABASE_URL" -tAc "SELECT COUNT(*) FROM \"ob-poc\".cbus;" 2>/dev/null || echo "0")
    echo "  📊 CBU records: $CBU_COUNT"

    # Check DSL instances
    DSL_COUNT=$(psql "$DATABASE_URL" -tAc "SELECT COUNT(*) FROM \"ob-poc\".dsl_instances;" 2>/dev/null || echo "0")
    echo "  📊 DSL instances: $DSL_COUNT"

    # Check parsed ASTs
    AST_COUNT=$(psql "$DATABASE_URL" -tAc "SELECT COUNT(*) FROM \"ob-poc\".parsed_asts;" 2>/dev/null || echo "0")
    echo "  📊 Parsed ASTs: $AST_COUNT"

    # Check entities
    ENTITY_COUNT=$(psql "$DATABASE_URL" -tAc "SELECT COUNT(*) FROM \"ob-poc\".entities;" 2>/dev/null || echo "0")
    echo "  📊 Entities: $ENTITY_COUNT"

    if [ "$DSL_COUNT" -gt "0" ] || [ "$AST_COUNT" -gt "0" ] || [ "$ENTITY_COUNT" -gt "0" ]; then
        print_success "Demo data found in database - End-to-End flow verified!"
    else
        print_warning "Limited demo data found - check logs for any issues"
    fi
}

# Display final summary
show_summary() {
    echo ""
    echo "🎉 End-to-End Database Demo Summary"
    echo "=================================="
    echo "✅ PostgreSQL connection: $DB_HOST:$DB_PORT"
    echo "✅ Database: $DB_NAME"
    echo "✅ Schema: ob-poc"
    echo "✅ Demo execution: Completed"
    echo ""
    echo "🔍 To explore the data:"
    echo "  psql \"$DATABASE_URL\""
    echo "  \\dt \"ob-poc\".*"
    echo "  SELECT * FROM \"ob-poc\".dsl_instances LIMIT 5;"
    echo ""
    echo "🧹 To clean up demo data:"
    echo "  psql \"$DATABASE_URL\" -c \"SELECT \\\"ob-poc\\\".cleanup_demo_data();\""
}

# Handle script arguments
case "${1:-}" in
    "cleanup-only")
        echo "🧹 Running cleanup only..."
        check_postgres || exit 1
        cleanup_demo_data
        exit 0
        ;;
    "schema-only")
        echo "🏗️  Running schema setup only..."
        check_postgres || exit 1
        create_database
        init_schema
        exit 0
        ;;
    "verify-only")
        echo "🔍 Running verification only..."
        check_postgres || exit 1
        verify_results
        exit 0
        ;;
esac

# Main execution flow
main() {
    echo "Environment:"
    echo "  Database Host: $DB_HOST:$DB_PORT"
    echo "  Database Name: $DB_NAME"
    echo "  Database User: $DB_USER"
    echo ""

    # Execute all steps
    check_postgres || exit 1
    create_database
    init_schema
    cleanup_demo_data
    build_project
    run_demo
    verify_results
    show_summary

    echo ""
    print_success "🎉 Complete End-to-End Database Demo finished successfully!"
}

# Help information
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    echo "End-to-End Database Demo Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  (no args)     Run complete demo setup and execution"
    echo "  cleanup-only  Clean up demo data only"
    echo "  schema-only   Set up database schema only"
    echo "  verify-only   Verify demo results only"
    echo "  --help, -h    Show this help"
    echo ""
    echo "Environment Variables:"
    echo "  POSTGRES_HOST     PostgreSQL host (default: localhost)"
    echo "  POSTGRES_PORT     PostgreSQL port (default: 5432)"
    echo "  POSTGRES_USER     PostgreSQL user (default: postgres)"
    echo "  POSTGRES_PASSWORD PostgreSQL password (default: password)"
    echo ""
    echo "Prerequisites:"
    echo "  - PostgreSQL running and accessible"
    echo "  - Rust toolchain installed"
    echo "  - Run from ob-poc root directory"
    exit 0
fi

# Run main function
main
