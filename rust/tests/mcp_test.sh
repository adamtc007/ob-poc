#!/bin/bash
# MCP Server Protocol Tests

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$ROOT_DIR/target/release/dsl_mcp"
DB_URL="${DATABASE_URL:-postgresql:///data_designer}"

echo "==============================================="
echo "        MCP Server Protocol Tests              "
echo "==============================================="
echo "Database: $DB_URL"
echo ""

# Check binary exists
if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    echo "Run: cargo build --release --features mcp --bin dsl_mcp"
    exit 1
fi

PASS=0
FAIL=0

echo "==============================================="
echo "            SINGLE REQUEST TESTS               "
echo "==============================================="

# Test 1: Initialize
echo ""
echo "Test 1: Initialize"
INIT_REQ='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
INIT_RESP=$(echo "$INIT_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$INIT_RESP" | jq -e '.result.protocolVersion' > /dev/null 2>&1; then
    echo "  Protocol: $(echo "$INIT_RESP" | jq -r '.result.protocolVersion')"
    echo "  Server: $(echo "$INIT_RESP" | jq -r '.result.serverInfo.name')"
    echo "  Initialize                                 PASS"
    PASS=$((PASS + 1))
else
    echo "  Initialize                                 FAIL"
    echo "  Response: $INIT_RESP"
    FAIL=$((FAIL + 1))
fi

# Test 2: Tools List
echo ""
echo "Test 2: Tools List"
TOOLS_REQ='{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
TOOLS_RESP=$(echo "$TOOLS_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

TOOL_COUNT=$(echo "$TOOLS_RESP" | jq -r '.result.tools | length' 2>/dev/null)
if [ "$TOOL_COUNT" -ge 8 ]; then
    echo "  Tools: $(echo "$TOOLS_RESP" | jq -r '.result.tools[].name' | tr '\n' ' ')"
    echo "  Tools List ($TOOL_COUNT tools)                    PASS"
    PASS=$((PASS + 1))
else
    echo "  Tools List                                 FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 3: verbs_list tool
echo ""
echo "Test 3: verbs_list Tool"
VERBS_REQ='{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"verbs_list","arguments":{}}}'
VERBS_RESP=$(echo "$VERBS_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$VERBS_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    VERBS_TEXT=$(echo "$VERBS_RESP" | jq -r '.result.content[0].text')
    if echo "$VERBS_TEXT" | jq -e '.domains' > /dev/null 2>&1; then
        DOMAIN_COUNT=$(echo "$VERBS_TEXT" | jq -r '.domains | length')
        VERB_COUNT=$(echo "$VERBS_TEXT" | jq -r '.verb_count')
        echo "  verbs_list ($DOMAIN_COUNT domains, $VERB_COUNT verbs)      PASS"
        PASS=$((PASS + 1))
    else
        echo "  verbs_list                                 FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  verbs_list                                 FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 4: schema_info tool
echo ""
echo "Test 4: schema_info Tool"
SCHEMA_REQ='{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"schema_info","arguments":{"category":"all"}}}'
SCHEMA_RESP=$(echo "$SCHEMA_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$SCHEMA_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    SCHEMA_TEXT=$(echo "$SCHEMA_RESP" | jq -r '.result.content[0].text')
    if echo "$SCHEMA_TEXT" | jq -e '.entity_types' > /dev/null 2>&1; then
        ET_COUNT=$(echo "$SCHEMA_TEXT" | jq -r '.entity_types | length')
        ROLE_COUNT=$(echo "$SCHEMA_TEXT" | jq -r '.roles | length')
        DOC_COUNT=$(echo "$SCHEMA_TEXT" | jq -r '.document_types | length')
        echo "  schema_info ($ET_COUNT types, $ROLE_COUNT roles, $DOC_COUNT docs)  PASS"
        PASS=$((PASS + 1))
    else
        echo "  schema_info                                FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  schema_info                                FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 5: dsl_validate with valid DSL
echo ""
echo "Test 5: dsl_validate (valid DSL)"
VALID_DSL='(cbu.create :name \"Test\" :as @cbu)'
VALIDATE_REQ="{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"dsl_validate\",\"arguments\":{\"source\":\"$VALID_DSL\"}}}"
VALIDATE_RESP=$(echo "$VALIDATE_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$VALIDATE_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    VALIDATE_TEXT=$(echo "$VALIDATE_RESP" | jq -r '.result.content[0].text')
    if echo "$VALIDATE_TEXT" | jq -e '.valid == true' > /dev/null 2>&1; then
        echo "  dsl_validate (valid)                       PASS"
        PASS=$((PASS + 1))
    else
        echo "  dsl_validate (valid)                       FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  dsl_validate (valid)                       FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 6: dsl_validate with invalid DSL
echo ""
echo "Test 6: dsl_validate (invalid DSL)"
INVALID_DSL='(invalid.verb :foo bar)'
VALIDATE_REQ2="{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"dsl_validate\",\"arguments\":{\"source\":\"$INVALID_DSL\"}}}"
VALIDATE_RESP2=$(echo "$VALIDATE_REQ2" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$VALIDATE_RESP2" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    VALIDATE_TEXT2=$(echo "$VALIDATE_RESP2" | jq -r '.result.content[0].text')
    if echo "$VALIDATE_TEXT2" | jq -e '.valid == false' > /dev/null 2>&1; then
        echo "  dsl_validate (invalid - rejected)          PASS"
        PASS=$((PASS + 1))
    else
        echo "  dsl_validate (invalid)                     FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  dsl_validate (invalid)                     FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 7: dsl_plan
echo ""
echo "Test 7: dsl_plan"
PLAN_DSL='(cbu.create :name \"PlanTest\" :as @cbu)(entity.create-proper-person :cbu-id @cbu :name \"John\" :as @person)'
PLAN_REQ="{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"dsl_plan\",\"arguments\":{\"source\":\"$PLAN_DSL\"}}}"
PLAN_RESP=$(echo "$PLAN_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$PLAN_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    PLAN_TEXT=$(echo "$PLAN_RESP" | jq -r '.result.content[0].text')
    STEP_COUNT=$(echo "$PLAN_TEXT" | jq -r '.step_count' 2>/dev/null)
    if [ "$STEP_COUNT" = "2" ]; then
        echo "  dsl_plan ($STEP_COUNT steps)                        PASS"
        PASS=$((PASS + 1))
    else
        echo "  dsl_plan                                   FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  dsl_plan                                   FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 8: cbu_list
echo ""
echo "Test 8: cbu_list"
LIST_REQ='{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"cbu_list","arguments":{"limit":5}}}'
LIST_RESP=$(echo "$LIST_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$LIST_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    LIST_TEXT=$(echo "$LIST_RESP" | jq -r '.result.content[0].text')
    if echo "$LIST_TEXT" | jq -e '.cbus' > /dev/null 2>&1; then
        CBU_COUNT=$(echo "$LIST_TEXT" | jq -r '.cbus | length')
        echo "  cbu_list ($CBU_COUNT CBUs)                        PASS"
        PASS=$((PASS + 1))
    else
        echo "  cbu_list                                   FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  cbu_list                                   FAIL"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "==============================================="
echo "             EXECUTION TESTS                   "
echo "==============================================="

# Test 9: dsl_execute dry run
echo ""
echo "Test 9: dsl_execute (dry run)"
EXEC_DSL='(cbu.create :name \"DryRunTest\" :client-type \"corporate\" :as @cbu)'
EXEC_REQ="{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\"params\":{\"name\":\"dsl_execute\",\"arguments\":{\"source\":\"$EXEC_DSL\",\"dry_run\":true}}}"
EXEC_RESP=$(echo "$EXEC_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$EXEC_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    EXEC_TEXT=$(echo "$EXEC_RESP" | jq -r '.result.content[0].text')
    if echo "$EXEC_TEXT" | jq -e '.dry_run == true and .success == true' > /dev/null 2>&1; then
        echo "  dsl_execute (dry run)                      PASS"
        PASS=$((PASS + 1))
    else
        echo "  dsl_execute (dry run)                      FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  dsl_execute (dry run)                      FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 10: dsl_execute real execution
echo ""
echo "Test 10: dsl_execute (real)"
TIMESTAMP=$(date +%s)
REAL_DSL="(cbu.create :name \\\"MCPTest_${TIMESTAMP}\\\" :client-type \\\"corporate\\\" :jurisdiction \\\"GB\\\" :as @cbu)"
REAL_REQ="{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"tools/call\",\"params\":{\"name\":\"dsl_execute\",\"arguments\":{\"source\":\"$REAL_DSL\",\"dry_run\":false}}}"
REAL_RESP=$(echo "$REAL_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

CBU_ID=""
if echo "$REAL_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    REAL_TEXT=$(echo "$REAL_RESP" | jq -r '.result.content[0].text')
    if echo "$REAL_TEXT" | jq -e '.success == true' > /dev/null 2>&1; then
        CBU_ID=$(echo "$REAL_TEXT" | jq -r '.bindings.cbu // .bindings.cbu_id // empty')
        echo "  Created CBU: $CBU_ID"
        echo "  dsl_execute (real)                         PASS"
        PASS=$((PASS + 1))
    else
        echo "  dsl_execute (real)                         FAIL"
        echo "  Response: $REAL_TEXT"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  dsl_execute (real)                         FAIL"
    FAIL=$((FAIL + 1))
fi

# Test 11: cbu_get with the created CBU
echo ""
echo "Test 11: cbu_get"
if [ -n "$CBU_ID" ]; then
    GET_REQ="{\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"tools/call\",\"params\":{\"name\":\"cbu_get\",\"arguments\":{\"cbu_id\":\"$CBU_ID\"}}}"
    GET_RESP=$(echo "$GET_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

    if echo "$GET_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
        GET_TEXT=$(echo "$GET_RESP" | jq -r '.result.content[0].text')
        if echo "$GET_TEXT" | jq -e '.cbu.name' > /dev/null 2>&1; then
            CBU_NAME=$(echo "$GET_TEXT" | jq -r '.cbu.name')
            echo "  Retrieved: $CBU_NAME"
            echo "  cbu_get                                    PASS"
            PASS=$((PASS + 1))
        else
            echo "  cbu_get                                    FAIL"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "  cbu_get                                    FAIL"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  cbu_get                                    SKIP (no CBU)"
fi

# Test 12: Full scenario
echo ""
echo "Test 12: Full Scenario Execute"
TIMESTAMP2=$(date +%s)
FULL_DSL="(cbu.create :name \\\"MCPFull_${TIMESTAMP2}\\\" :client-type \\\"corporate\\\" :as @cbu)(entity.create-limited-company :cbu-id @cbu :name \\\"MCPCompany_${TIMESTAMP2}\\\" :as @company)(entity.create-proper-person :cbu-id @cbu :first-name \\\"John\\\" :last-name \\\"MCPPerson_${TIMESTAMP2}\\\" :as @ubo)(cbu.assign-role :cbu-id @cbu :entity-id @ubo :target-entity-id @company :role \\\"BENEFICIAL_OWNER\\\" :ownership-percentage 100.0)"
FULL_REQ="{\"jsonrpc\":\"2.0\",\"id\":12,\"method\":\"tools/call\",\"params\":{\"name\":\"dsl_execute\",\"arguments\":{\"source\":\"$FULL_DSL\"}}}"
FULL_RESP=$(echo "$FULL_REQ" | DATABASE_URL="$DB_URL" "$BINARY" 2>/dev/null | head -1)

if echo "$FULL_RESP" | jq -e '.result.content[0].text' > /dev/null 2>&1; then
    FULL_TEXT=$(echo "$FULL_RESP" | jq -r '.result.content[0].text')
    if echo "$FULL_TEXT" | jq -e '.success == true' > /dev/null 2>&1; then
        STEPS=$(echo "$FULL_TEXT" | jq -r '.steps_executed')
        BINDINGS=$(echo "$FULL_TEXT" | jq -r '.bindings | keys | length')
        echo "  Full scenario ($STEPS steps, $BINDINGS bindings)        PASS"
        PASS=$((PASS + 1))
    else
        echo "  Full scenario                              FAIL"
        echo "  Response: $FULL_TEXT"
        FAIL=$((FAIL + 1))
    fi
else
    echo "  Full scenario                              FAIL"
    FAIL=$((FAIL + 1))
fi

# Cleanup
echo ""
echo "Cleaning up test data..."
psql "$DB_URL" -c "DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id IN (SELECT cbu_id FROM \"ob-poc\".cbus WHERE name LIKE 'MCPTest_%' OR name LIKE 'MCPFull_%');" 2>/dev/null || true
psql "$DB_URL" -c "DELETE FROM \"ob-poc\".entities WHERE name LIKE 'MCPCompany_%' OR name LIKE 'MCPPerson_%';" 2>/dev/null || true
psql "$DB_URL" -c "DELETE FROM \"ob-poc\".cbus WHERE name LIKE 'MCPTest_%' OR name LIKE 'MCPFull_%';" 2>/dev/null || true

echo ""
echo "==============================================="
echo "                  RESULTS                      "
echo "==============================================="
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ $FAIL -gt 0 ]; then
    echo "SOME TESTS FAILED"
    exit 1
fi

echo "ALL TESTS PASSED!"
echo ""
echo "MCP Server is ready for Claude Desktop integration!"
