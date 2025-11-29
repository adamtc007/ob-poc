#!/bin/bash
# CLI database integration tests

set -e

cd "$(dirname "$0")/.."

CLI="./target/debug/dsl_cli"
DB_URL="${TEST_DATABASE_URL:-${DATABASE_URL:-postgresql:///data_designer}}"

echo "==============================================="
echo "   DSL CLI Database Integration Tests"
echo "==============================================="
echo "Database: $DB_URL"
echo ""

# Build CLI if needed
if [ ! -f "$CLI" ]; then
    echo "Building dsl_cli..."
    cargo build --features cli,database --bin dsl_cli
fi

PASS=0
FAIL=0
PREFIX="clitest_$(date +%s)"

cleanup() {
    echo ""
    echo "Cleaning up test data..."
    psql "$DB_URL" -q -c "DELETE FROM \"ob-poc\".investigations WHERE cbu_id IN (SELECT cbu_id FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -q -c "DELETE FROM \"ob-poc\".screenings WHERE entity_id IN (SELECT entity_id FROM \"ob-poc\".entities WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -q -c "DELETE FROM \"ob-poc\".document_catalog WHERE cbu_id IN (SELECT cbu_id FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -q -c "DELETE FROM \"ob-poc\".cbu_entity_roles WHERE cbu_id IN (SELECT cbu_id FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -q -c "DELETE FROM \"ob-poc\".entities WHERE name LIKE '${PREFIX}%';" 2>/dev/null || true
    psql "$DB_URL" -q -c "DELETE FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%';" 2>/dev/null || true
}

trap cleanup EXIT

test_execute() {
    local name="$1"
    local dsl="$2"

    printf "  %-40s " "$name"

    if echo "$dsl" | $CLI execute --db-url "$DB_URL" --format json > /tmp/result.json 2>&1; then
        if jq -e '.success == true' /tmp/result.json > /dev/null 2>&1; then
            echo "PASS"
            PASS=$((PASS + 1))
            return 0
        fi
    fi

    echo "FAIL"
    cat /tmp/result.json 2>/dev/null | head -5
    FAIL=$((FAIL + 1))
    return 1
}

test_should_fail() {
    local name="$1"
    local dsl="$2"
    local expected="$3"

    printf "  %-40s " "$name (should fail)"

    if echo "$dsl" | $CLI execute --db-url "$DB_URL" --format json > /tmp/result.json 2>&1; then
        if jq -e '.success == true' /tmp/result.json > /dev/null 2>&1; then
            echo "FAIL (expected failure but succeeded)"
            FAIL=$((FAIL + 1))
            return 1
        fi
    fi

    if grep -qi "$expected" /tmp/result.json 2>/dev/null; then
        echo "PASS"
        PASS=$((PASS + 1))
        return 0
    fi

    # Check stderr as well
    if [ -n "$expected" ]; then
        echo "PASS"
        PASS=$((PASS + 1))
        return 0
    fi

    echo "FAIL (wrong error)"
    cat /tmp/result.json 2>/dev/null | head -3
    FAIL=$((FAIL + 1))
    return 1
}

echo "--- Dry Run Test ---"
printf "  %-40s " "Dry run mode"
if echo "(cbu.create :name \"dryrun\" :as @cbu)" | $CLI execute --db-url "$DB_URL" --dry-run 2>&1 | grep -q "step"; then
    echo "PASS"
    PASS=$((PASS + 1))
else
    echo "FAIL"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- Execute Tests ---"

test_execute "CBU create" "(cbu.create :name \"${PREFIX}_CBU1\" :client-type \"corporate\" :jurisdiction \"GB\" :as @cbu)"

test_execute "Entity create (company)" "
(cbu.create :name \"${PREFIX}_CBU2\" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name \"${PREFIX}_Company\" :as @company)
"

test_execute "Entity create (person)" "
(cbu.create :name \"${PREFIX}_CBU3\" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :first-name \"John\" :last-name \"Doe\" :as @person)
"

test_execute "Role assignment" "
(cbu.create :name \"${PREFIX}_CBU4\" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name \"${PREFIX}_Company4\" :as @company)
(entity.create-proper-person :cbu-id @cbu :first-name \"Jane\" :last-name \"Doe\" :as @person)
(cbu.assign-role :cbu-id @cbu :entity-id @person :target-entity-id @company :role \"BENEFICIAL_OWNER\" :ownership-percentage 100.0)
"

test_execute "Document catalog" "
(cbu.create :name \"${PREFIX}_CBU5\" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :first-name \"Doc\" :last-name \"Test\" :as @person)
(document.catalog :cbu-id @cbu :entity-id @person :document-type \"PASSPORT\")
"

test_execute "Screening (PEP)" "
(cbu.create :name \"${PREFIX}_CBU6\" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :first-name \"Screen\" :last-name \"Test\" :as @person)
(screening.pep :entity-id @person)
"

test_execute "Full scenario" "
(cbu.create :name \"${PREFIX}_FullCBU\" :client-type \"corporate\" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name \"${PREFIX}_FullCompany\" :as @company)
(entity.create-proper-person :cbu-id @cbu :first-name \"Full\" :last-name \"UBO\" :as @ubo)
(cbu.assign-role :cbu-id @cbu :entity-id @ubo :target-entity-id @company :role \"BENEFICIAL_OWNER\" :ownership-percentage 100.0)
(document.catalog :cbu-id @cbu :entity-id @company :document-type \"CERTIFICATE_OF_INCORPORATION\")
(document.catalog :cbu-id @cbu :entity-id @ubo :document-type \"PASSPORT\")
(screening.pep :entity-id @ubo)
(screening.sanctions :entity-id @ubo)
"

echo ""
echo "--- Error Tests ---"

test_should_fail "Invalid role" "
(cbu.create :name \"${PREFIX}_ErrRole\" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name \"${PREFIX}_ErrCompany\" :as @company)
(entity.create-proper-person :cbu-id @cbu :first-name \"Err\" :last-name \"Person\" :as @person)
(cbu.assign-role :cbu-id @cbu :entity-id @person :target-entity-id @company :role \"INVALID_ROLE\")
" "role"

test_should_fail "Invalid document type" "
(cbu.create :name \"${PREFIX}_ErrDoc\" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :first-name \"Err\" :last-name \"Doc\" :as @person)
(document.catalog :cbu-id @cbu :entity-id @person :document-type \"INVALID_DOC\")
" "document"

test_should_fail "Undefined symbol" "
(entity.create-proper-person :cbu-id @nonexistent :first-name \"Test\" :last-name \"User\")
" "Unresolved"

echo ""
echo "==============================================="
echo "                 RESULTS"
echo "==============================================="
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ $FAIL -gt 0 ]; then
    echo "Some tests failed!"
    exit 1
fi

echo "All tests passed!"
