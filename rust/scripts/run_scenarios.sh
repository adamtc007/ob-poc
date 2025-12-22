#!/bin/bash
# =============================================================================
# Trading Matrix Test Scenario Runner
# Executes all DSL test scenarios and reports results
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SCENARIOS_DIR="$PROJECT_ROOT/examples/scenarios"
DSL_CLI="$PROJECT_ROOT/target/debug/dsl_cli"
DB_URL="${DATABASE_URL:-postgresql:///data_designer}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0
SKIPPED=0

echo -e "${BLUE}==============================================================================${NC}"
echo -e "${BLUE}   Trading Matrix Test Scenario Runner${NC}"
echo -e "${BLUE}==============================================================================${NC}"
echo ""
echo -e "Project Root: $PROJECT_ROOT"
echo -e "Scenarios Dir: $SCENARIOS_DIR"
echo -e "Database: $DB_URL"
echo ""

# Check if dsl_cli exists
if [ ! -f "$DSL_CLI" ]; then
    echo -e "${YELLOW}Building dsl_cli...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --features database,cli --bin dsl_cli
fi

# Verify CLI works
if ! "$DSL_CLI" --help > /dev/null 2>&1; then
    echo -e "${RED}ERROR: dsl_cli not working${NC}"
    exit 1
fi

echo -e "${GREEN}dsl_cli ready${NC}"
echo ""

# List of scenarios to run
SCENARIOS=(
    "01_simple_equity_fund"
    "02_multi_manager_global"
    "03_fund_with_otc"
    "04_transition_manager"
    "05_sub_advised_fund"
)

# Run each scenario
for scenario in "${SCENARIOS[@]}"; do
    SCENARIO_FILE="$SCENARIOS_DIR/${scenario}.dsl"

    echo -e "${BLUE}------------------------------------------------------------------------------${NC}"
    echo -e "${BLUE}Running: ${scenario}${NC}"
    echo -e "${BLUE}------------------------------------------------------------------------------${NC}"

    if [ ! -f "$SCENARIO_FILE" ]; then
        echo -e "${YELLOW}SKIPPED: File not found${NC}"
        ((SKIPPED++))
        continue
    fi

    # Execute the scenario
    START_TIME=$(date +%s.%N)

    if "$DSL_CLI" execute -f "$SCENARIO_FILE" --db-url "$DB_URL" 2>&1; then
        END_TIME=$(date +%s.%N)
        DURATION=$(echo "$END_TIME - $START_TIME" | bc)
        echo -e "${GREEN}PASSED${NC} (${DURATION}s)"
        ((PASSED++))
    else
        END_TIME=$(date +%s.%N)
        DURATION=$(echo "$END_TIME - $START_TIME" | bc)
        echo -e "${RED}FAILED${NC} (${DURATION}s)"
        ((FAILED++))

        # Optionally continue on failure (comment out to stop on first failure)
        # exit 1
    fi

    echo ""
done

# Summary
echo -e "${BLUE}==============================================================================${NC}"
echo -e "${BLUE}   Summary${NC}"
echo -e "${BLUE}==============================================================================${NC}"
echo ""
echo -e "Total Scenarios: ${#SCENARIOS[@]}"
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo -e "${YELLOW}Skipped: $SKIPPED${NC}"
echo ""

if [ $FAILED -eq 0 ] && [ $SKIPPED -eq 0 ]; then
    echo -e "${GREEN}All scenarios passed!${NC}"
    exit 0
elif [ $FAILED -eq 0 ]; then
    echo -e "${YELLOW}All executed scenarios passed (some skipped)${NC}"
    exit 0
else
    echo -e "${RED}Some scenarios failed${NC}"
    exit 1
fi
