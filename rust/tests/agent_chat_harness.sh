#!/bin/bash
# Agent Chat API Test Harness
# Tests the unified DSL pipeline with various prompts

set -e

BASE_URL="${BASE_URL:-http://localhost:3000}"
SESSION_ID=""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0

# Create a session first
create_session() {
    echo -e "${CYAN}Creating session...${NC}"
    RESPONSE=$(curl -s -X POST "$BASE_URL/api/session" \
        -H "Content-Type: application/json" \
        -d '{}')
    SESSION_ID=$(echo "$RESPONSE" | jq -r '.session_id // .id // empty')
    if [ -z "$SESSION_ID" ]; then
        echo -e "${RED}Failed to create session${NC}"
        echo "$RESPONSE"
        exit 1
    fi
    echo -e "${GREEN}Session: $SESSION_ID${NC}"
    echo ""
}

# Send a chat message and check response
test_chat() {
    local prompt="$1"
    local expect_dsl="$2"        # "yes" or "no"
    local expect_execute="$3"    # "yes" or "no" - expects AgentCommand::Execute
    local description="$4"

    echo -e "${CYAN}TEST: $description${NC}"
    echo -e "  Prompt: \"$prompt\""

    RESPONSE=$(curl -s -X POST "$BASE_URL/api/session/$SESSION_ID/chat" \
        -H "Content-Type: application/json" \
        -d "{\"message\": \"$prompt\"}")

    # Check for error
    ERROR=$(echo "$RESPONSE" | jq -r '.error // empty')
    if [ -n "$ERROR" ]; then
        echo -e "  ${RED}FAIL: API Error - $ERROR${NC}"
        ((FAIL++))
        return
    fi

    # Extract fields
    MESSAGE=$(echo "$RESPONSE" | jq -r '.message // empty')
    DSL_SOURCE=$(echo "$RESPONSE" | jq -r '.dsl.source // empty')
    CAN_EXECUTE=$(echo "$RESPONSE" | jq -r '.dsl.can_execute // false')
    COMMANDS=$(echo "$RESPONSE" | jq -r '.commands // []')
    HAS_EXECUTE_CMD=$(echo "$COMMANDS" | jq 'map(select(. == "Execute")) | length > 0')

    echo -e "  Message: ${MESSAGE:0:60}..."

    # Check DSL expectation
    if [ "$expect_dsl" = "yes" ]; then
        if [ -n "$DSL_SOURCE" ] && [ "$DSL_SOURCE" != "null" ]; then
            echo -e "  ${GREEN}DSL generated: ${DSL_SOURCE:0:50}...${NC}"
        else
            echo -e "  ${RED}FAIL: Expected DSL but none generated${NC}"
            ((FAIL++))
            return
        fi
    elif [ "$expect_dsl" = "no" ]; then
        if [ -n "$DSL_SOURCE" ] && [ "$DSL_SOURCE" != "null" ]; then
            echo -e "  ${YELLOW}WARN: DSL generated when not expected: ${DSL_SOURCE:0:40}${NC}"
        fi
    fi

    # Check execute command expectation
    if [ "$expect_execute" = "yes" ]; then
        if [ "$HAS_EXECUTE_CMD" = "true" ]; then
            echo -e "  ${GREEN}Execute command returned${NC}"
        else
            echo -e "  ${RED}FAIL: Expected Execute command but not returned${NC}"
            echo -e "  Commands: $COMMANDS"
            ((FAIL++))
            return
        fi
    fi

    echo -e "  ${GREEN}PASS${NC}"
    ((PASS++))
    echo ""
}

echo "========================================"
echo "  Agent Chat API Test Harness"
echo "========================================"
echo ""

create_session

echo "========================================"
echo "  1. REPL Control Commands"
echo "========================================"

test_chat "run" "no" "no" "Run with no pending DSL"
test_chat "execute" "no" "no" "Execute with no pending DSL"
test_chat "undo" "no" "no" "Undo command"
test_chat "clear" "no" "no" "Clear command"

echo "========================================"
echo "  2. Natural Language Prompts"
echo "========================================"

test_chat "create a new CBU called Test Corp in Luxembourg" "yes" "no" "Create CBU - natural language"
test_chat "add custody product" "yes" "no" "Add product - short prompt"
test_chat "show me all CBUs in Germany" "yes" "no" "Query CBUs by jurisdiction"
test_chat "add fund accounting and transfer agency" "yes" "no" "Multiple products"
test_chat "create a fund called Alpha Fund" "yes" "no" "Create fund"
test_chat "assign John Smith as director" "yes" "no" "Role assignment"

echo "========================================"
echo "  3. Valid DSL Input"
echo "========================================"

test_chat "(cbu.create :name \"Direct DSL Corp\" :jurisdiction \"IE\")" "yes" "no" "Valid DSL - cbu.create"
test_chat "(cbu.add-product :product \"CUSTODY\")" "yes" "no" "Valid DSL - add product"
test_chat "(session.info)" "yes" "no" "Valid DSL - session info"
test_chat "(view.universe :jurisdiction [\"LU\"])" "yes" "no" "Valid DSL - view universe"

echo "========================================"
echo "  4. Malformed DSL (should recover intent)"
echo "========================================"

test_chat "(cbu create name Test)" "yes" "no" "Malformed - missing dots and colons"
test_chat "(product add custody)" "yes" "no" "Malformed - wrong verb structure"
test_chat "(add-product CUSTODY)" "yes" "no" "Malformed - no domain prefix"
test_chat "(cbu.create name=Test)" "yes" "no" "Malformed - wrong param syntax"

echo "========================================"
echo "  5. Edge Cases"
echo "========================================"

test_chat "help" "no" "no" "Help request"
test_chat "what can you do" "no" "no" "Capability query"
test_chat "" "no" "no" "Empty prompt"
test_chat "   " "no" "no" "Whitespace only"

echo "========================================"
echo "  6. DSL then Run sequence"
echo "========================================"

# Generate DSL first
test_chat "create a CBU called Sequence Test in Ireland" "yes" "no" "Generate DSL for sequence"
# Now run should trigger execute
test_chat "run" "no" "yes" "Run after DSL generated"

echo "========================================"
echo "  RESULTS"
echo "========================================"
echo -e "${GREEN}PASSED: $PASS${NC}"
echo -e "${RED}FAILED: $FAIL${NC}"
TOTAL=$((PASS + FAIL))
echo "TOTAL: $TOTAL"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
