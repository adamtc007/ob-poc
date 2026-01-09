#!/bin/bash
# Test incremental DSL accumulation for CBU domain

API_URL="${API_URL:-http://localhost:3001}"
PASS=0
FAIL=0

echo "=== Incremental DSL Accumulation Tests - CBU Domain ==="
echo "API: $API_URL"
echo ""

# Create a session for the test run
echo "Creating test session..."
SESSION_RESPONSE=$(curl -s -X POST "$API_URL/api/session" \
    -H "Content-Type: application/json" \
    -d '{"source": "test_script"}')
SESSION_ID=$(echo "$SESSION_RESPONSE" | jq -r '.id')

if [ "$SESSION_ID" = "null" ] || [ -z "$SESSION_ID" ]; then
    echo "Failed to create session: $SESSION_RESPONSE"
    exit 1
fi
echo "Session ID: $SESSION_ID"
echo ""

run_dsl() {
    local dsl="$1"
    local desc="$2"
    echo "--- $desc ---"
    echo "$dsl"
    echo ""

    # Write DSL to temp file for proper JSON encoding
    echo "$dsl" > /tmp/test_dsl.txt
    local json_dsl=$(jq -Rs '.' /tmp/test_dsl.txt)

    result=$(curl -s -X POST "$API_URL/api/session/$SESSION_ID/execute" \
        -H "Content-Type: application/json" \
        -d "{\"dsl\": $json_dsl}")

    success=$(echo "$result" | jq -r '.success')

    if [ "$success" = "true" ]; then
        echo "✓ PASS"
        ((PASS++))
        echo "Bindings: $(echo "$result" | jq -c '.bindings')"
    else
        echo "✗ FAIL"
        echo "Errors: $(echo "$result" | jq -r '.errors[]?' 2>/dev/null || echo "$result")"
        ((FAIL++))
    fi
    echo ""
}

echo "=== TEST 1: Create CBU only ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)' "Create CBU"

echo "=== TEST 2: Re-run same DSL (idempotent) ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)' "Re-run CBU"

echo "=== TEST 3: CBU + Entity ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)
(entity.create-limited-company :name "Test Holdings Ltd" :jurisdiction "LU" :as @company)' "CBU + Entity"

echo "=== TEST 4: Re-run CBU + Entity (idempotent) ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)
(entity.create-limited-company :name "Test Holdings Ltd" :jurisdiction "LU" :as @company)' "Re-run CBU + Entity"

echo "=== TEST 5: CBU + Entity + Role ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)
(entity.create-limited-company :name "Test Holdings Ltd" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")' "Add Role"

echo "=== TEST 6: Re-run full (idempotent) ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)
(entity.create-limited-company :name "Test Holdings Ltd" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")' "Re-run full"

echo "=== TEST 7: Add person ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)
(entity.create-limited-company :name "Test Holdings Ltd" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)' "Add person"

echo "=== TEST 8: Add UBO role ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)
(entity.create-limited-company :name "Test Holdings Ltd" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)
(cbu.assign-role :cbu-id @cbu :entity-id @john :role "BENEFICIAL_OWNER" :ownership-percentage 100)' "Add UBO role"

echo "=== TEST 9: Final re-run (idempotent) ==="
run_dsl '(cbu.ensure :name "Incremental Test Fund" :jurisdiction "LU" :client-type "fund" :as @cbu)
(entity.create-limited-company :name "Test Holdings Ltd" :jurisdiction "LU" :as @company)
(cbu.assign-role :cbu-id @cbu :entity-id @company :role "PRINCIPAL")
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)
(cbu.assign-role :cbu-id @cbu :entity-id @john :role "BENEFICIAL_OWNER" :ownership-percentage 100)' "Final re-run"

echo "==========================================="
echo "RESULTS: $PASS passed, $FAIL failed"
echo "Session: $SESSION_ID"
echo "==========================================="

[ $FAIL -eq 0 ]
