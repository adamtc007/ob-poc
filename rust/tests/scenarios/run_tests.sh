#!/bin/bash
# DSL Test Runner
# Runs all scenario files and reports pass/fail

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
CLI="$ROOT_DIR/rust/target/release/dsl_cli"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
DIM='\033[2m'
NC='\033[0m' # No Color

# Counters
VALID_PASS=0
VALID_FAIL=0
ERROR_PASS=0
ERROR_FAIL=0
ERROR_SKIP=0

echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}       DSL Test Runner${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""

# Check if CLI exists
if [ ! -f "$CLI" ]; then
    echo -e "${YELLOW}Building dsl_cli...${NC}"
    cd "$ROOT_DIR/rust"
    cargo build --features cli,database --bin dsl_cli --release 2>/dev/null
fi

# Mode: validate, plan, or execute
MODE="${1:-plan}"
DB_URL="${DATABASE_URL:-postgresql:///data_designer}"

echo -e "Mode: ${CYAN}$MODE${NC}"
if [ "$MODE" = "execute" ]; then
    echo -e "Database: ${CYAN}$DB_URL${NC}"
fi
echo ""

# ========================================
# VALID SCENARIOS - Should Pass
# ========================================
echo -e "${CYAN}--- Valid Scenarios (should PASS) ---${NC}"
echo ""

for file in "$SCRIPT_DIR/valid"/*.dsl; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .dsl)
        printf "  %-35s " "$name"

        if [ "$MODE" = "execute" ]; then
            if "$CLI" execute --dry-run --db-url "$DB_URL" --file "$file" --format json > /tmp/dsl_test_output.json 2>&1; then
                success=$(jq -r '.success // .dry_run // false' /tmp/dsl_test_output.json 2>/dev/null || echo "false")
                if [ "$success" = "true" ]; then
                    echo -e "${GREEN}PASS${NC}"
                    ((VALID_PASS++))
                else
                    echo -e "${RED}FAIL${NC}"
                    ((VALID_FAIL++))
                fi
            else
                echo -e "${RED}FAIL${NC}"
                ((VALID_FAIL++))
            fi
        elif [ "$MODE" = "plan" ]; then
            if "$CLI" plan --file "$file" --format json > /tmp/dsl_test_output.json 2>&1; then
                echo -e "${GREEN}PASS${NC}"
                ((VALID_PASS++))
            else
                echo -e "${RED}FAIL${NC}"
                ((VALID_FAIL++))
            fi
        else
            # validate mode
            if "$CLI" validate --file "$file" --format json > /tmp/dsl_test_output.json 2>&1; then
                errors=$(jq -r '.errors // 0' /tmp/dsl_test_output.json 2>/dev/null || echo "0")
                if [ "$errors" = "0" ]; then
                    echo -e "${GREEN}PASS${NC}"
                    ((VALID_PASS++))
                else
                    echo -e "${RED}FAIL${NC} (errors: $errors)"
                    ((VALID_FAIL++))
                fi
            else
                echo -e "${RED}FAIL${NC}"
                ((VALID_FAIL++))
            fi
        fi
    fi
done

echo ""

# ========================================
# ERROR SCENARIOS - Should Fail
# ========================================
echo -e "${CYAN}--- Error Scenarios (should FAIL) ---${NC}"
echo ""

# Define which errors are detectable at which stage
# CSG errors (01, 05) require database
# Compile errors (03) require plan/execute
# Runtime errors (04) require execute
# Symbol errors (02) are caught at validate

for file in "$SCRIPT_DIR/error"/*.dsl; do
    if [ -f "$file" ]; then
        name=$(basename "$file" .dsl)
        printf "  %-35s " "$name"

        # Check if this is a CSG error (requires DB)
        is_csg_error=false
        if [[ "$name" == "01_passport_for_company" ]] || [[ "$name" == "05_trust_deed_for_company" ]]; then
            is_csg_error=true
        fi

        # Check if this is a runtime error (requires execute)
        # Note: undefined symbols are resolved at runtime, not compile time
        is_runtime_error=false
        if [[ "$name" == "02_undefined_symbol" ]] || [[ "$name" == "04_missing_required_arg" ]]; then
            is_runtime_error=true
        fi

        # Skip CSG errors if not in execute mode with DB
        if [ "$is_csg_error" = true ] && [ "$MODE" != "execute" ]; then
            echo -e "${DIM}SKIP${NC} ${DIM}(needs database)${NC}"
            ((ERROR_SKIP++))
            continue
        fi

        # Skip runtime errors if not in execute mode
        if [ "$is_runtime_error" = true ] && [ "$MODE" != "execute" ]; then
            echo -e "${DIM}SKIP${NC} ${DIM}(needs execute)${NC}"
            ((ERROR_SKIP++))
            continue
        fi

        if [ "$MODE" = "execute" ]; then
            if "$CLI" execute --dry-run --db-url "$DB_URL" --file "$file" --format json > /tmp/dsl_test_output.json 2>&1; then
                success=$(jq -r '.success // .dry_run // false' /tmp/dsl_test_output.json 2>/dev/null || echo "false")
                if [ "$success" = "false" ]; then
                    echo -e "${GREEN}PASS${NC} (correctly rejected)"
                    ((ERROR_PASS++))
                else
                    echo -e "${RED}FAIL${NC} (should have been rejected)"
                    ((ERROR_FAIL++))
                fi
            else
                echo -e "${GREEN}PASS${NC} (correctly rejected)"
                ((ERROR_PASS++))
            fi
        elif [ "$MODE" = "plan" ]; then
            if "$CLI" plan --file "$file" --format json > /tmp/dsl_test_output.json 2>&1; then
                echo -e "${RED}FAIL${NC} (should have been rejected)"
                ((ERROR_FAIL++))
            else
                echo -e "${GREEN}PASS${NC} (correctly rejected)"
                ((ERROR_PASS++))
            fi
        else
            # validate mode
            if "$CLI" validate --file "$file" --format json > /tmp/dsl_test_output.json 2>&1; then
                errors=$(jq -r '.errors // 0' /tmp/dsl_test_output.json 2>/dev/null || echo "0")
                if [ "$errors" != "0" ]; then
                    echo -e "${GREEN}PASS${NC} (correctly rejected)"
                    ((ERROR_PASS++))
                else
                    echo -e "${RED}FAIL${NC} (should have been rejected)"
                    ((ERROR_FAIL++))
                fi
            else
                echo -e "${GREEN}PASS${NC} (correctly rejected)"
                ((ERROR_PASS++))
            fi
        fi
    fi
done

echo ""

# ========================================
# SUMMARY
# ========================================
echo -e "${CYAN}========================================${NC}"
echo -e "${CYAN}       Summary${NC}"
echo -e "${CYAN}========================================${NC}"
echo ""

TOTAL_PASS=$((VALID_PASS + ERROR_PASS))
TOTAL_FAIL=$((VALID_FAIL + ERROR_FAIL))
TOTAL=$((TOTAL_PASS + TOTAL_FAIL))

echo -e "  Valid scenarios:  ${GREEN}$VALID_PASS passed${NC}, ${RED}$VALID_FAIL failed${NC}"
echo -e "  Error scenarios:  ${GREEN}$ERROR_PASS passed${NC}, ${RED}$ERROR_FAIL failed${NC}, ${DIM}$ERROR_SKIP skipped${NC}"
echo ""
echo -e "  Total: ${GREEN}$TOTAL_PASS${NC}/${TOTAL} passed"

if [ $ERROR_SKIP -gt 0 ]; then
    echo ""
    echo -e "  ${DIM}Note: $ERROR_SKIP error tests skipped (require database or execute mode)${NC}"
    echo -e "  ${DIM}Run with 'execute' mode and DATABASE_URL for full coverage${NC}"
fi

echo ""

if [ $TOTAL_FAIL -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
