# DSL Test Scenarios

**Purpose**: Comprehensive test scenarios for validating the DSL pipeline end-to-end.

**Prerequisites**: 
- CLI execute command working with database
- Seed data for entity types, roles, document types

---

## Scenario Categories

1. **Individual Onboarding** - Natural persons
2. **Corporate Onboarding** - Companies with UBO structures
3. **Trust Onboarding** - Trust structures with trustees/beneficiaries
4. **Partnership Onboarding** - GP/LP structures
5. **Complex Ownership** - Multi-level ownership chains
6. **Document Workflows** - Document lifecycle operations
7. **Screening Workflows** - KYC/AML screening
8. **Error Cases** - Intentionally invalid DSL for testing validation

---

## Scenario 1: Simple Individual Onboarding

**Description**: Single natural person client with identity documents.

```clojure
(cbu.create 
    :name "John Smith" 
    :client-type "individual" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-proper-person 
    :cbu-id @cbu
    :name "John Smith"
    :first-name "John"
    :last-name "Smith"
    :date-of-birth "1985-03-15"
    :nationality "GB"
    :as @person)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PASSPORT")

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PROOF_OF_ADDRESS")

(screening.pep :entity-id @person)
(screening.sanctions :entity-id @person)
```

**Expected Results**:
- 1 CBU created
- 1 Entity (PROPER_PERSON_NATURAL)
- 2 Documents (PASSPORT, PROOF_OF_ADDRESS)
- 2 Screenings (PEP, SANCTIONS)

---

## Scenario 2: Corporate with Single UBO

**Description**: Private limited company with one majority shareholder.

```clojure
(cbu.create 
    :name "Acme Ltd" 
    :client-type "corporate" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company 
    :cbu-id @cbu
    :name "Acme Ltd"
    :company-number "12345678"
    :jurisdiction "GB"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Jane Doe"
    :first-name "Jane"
    :last-name "Doe"
    :as @ubo)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo
    :target-entity-id @company
    :role "BENEFICIAL_OWNER"
    :ownership-percentage 100.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo
    :target-entity-id @company
    :role "DIRECTOR")

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "CERTIFICATE_OF_INCORPORATION")

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "ARTICLES_OF_ASSOCIATION")

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PASSPORT")

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PROOF_OF_ADDRESS")

(screening.pep :entity-id @ubo)
(screening.sanctions :entity-id @ubo)

(ubo.calculate :cbu-id @cbu :entity-id @company :threshold 25.0)
```

**Expected Results**:
- 1 CBU
- 2 Entities (company + UBO)
- 2 Roles (BENEFICIAL_OWNER, DIRECTOR)
- 4 Documents
- 2 Screenings
- UBO calculation returns Jane Doe at 100%

---

## Scenario 3: Corporate with Multiple UBOs

**Description**: Company with three shareholders, two above UBO threshold.

```clojure
(cbu.create 
    :name "Widget Holdings Ltd" 
    :client-type "corporate" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company 
    :cbu-id @cbu
    :name "Widget Holdings Ltd"
    :company-number "87654321"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Alice Brown"
    :first-name "Alice"
    :last-name "Brown"
    :as @alice)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Bob Green"
    :first-name "Bob"
    :last-name "Green"
    :as @bob)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Charlie White"
    :first-name "Charlie"
    :last-name "White"
    :as @charlie)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @alice
    :target-entity-id @company
    :role "BENEFICIAL_OWNER"
    :ownership-percentage 45.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @bob
    :target-entity-id @company
    :role "BENEFICIAL_OWNER"
    :ownership-percentage 35.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @charlie
    :target-entity-id @company
    :role "BENEFICIAL_OWNER"
    :ownership-percentage 20.0)

(document.catalog :cbu-id @cbu :entity-id @company :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @alice :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @bob :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @charlie :document-type "PASSPORT")

(screening.pep :entity-id @alice)
(screening.pep :entity-id @bob)
(screening.pep :entity-id @charlie)
(screening.sanctions :entity-id @alice)
(screening.sanctions :entity-id @bob)
(screening.sanctions :entity-id @charlie)

(ubo.calculate :cbu-id @cbu :entity-id @company :threshold 25.0)
```

**Expected Results**:
- UBO calculation returns Alice (45%) and Bob (35%)
- Charlie (20%) is NOT returned (below 25% threshold)

---

## Scenario 4: Trust Structure

**Description**: Discretionary trust with trustees and beneficiaries.

```clojure
(cbu.create 
    :name "Smith Family Trust" 
    :client-type "trust" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-trust 
    :cbu-id @cbu
    :name "Smith Family Trust"
    :trust-type "discretionary"
    :as @trust)

(entity.create-proper-person
    :cbu-id @cbu
    :name "John Smith Sr"
    :first-name "John"
    :last-name "Smith"
    :as @settlor)

(entity.create-proper-person
    :cbu-id @cbu
    :name "ABC Trust Company"
    :as @trustee)

(entity.create-proper-person
    :cbu-id @cbu
    :name "John Smith Jr"
    :first-name "John"
    :last-name "Smith"
    :as @beneficiary1)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Jane Smith"
    :first-name "Jane"
    :last-name "Smith"
    :as @beneficiary2)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @settlor
    :target-entity-id @trust
    :role "SETTLOR")

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @trustee
    :target-entity-id @trust
    :role "TRUSTEE")

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @beneficiary1
    :target-entity-id @trust
    :role "BENEFICIARY")

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @beneficiary2
    :target-entity-id @trust
    :role "BENEFICIARY")

(document.catalog :cbu-id @cbu :entity-id @trust :document-type "TRUST_DEED")
(document.catalog :cbu-id @cbu :entity-id @settlor :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @trustee :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @beneficiary1 :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @beneficiary2 :document-type "PASSPORT")

(screening.pep :entity-id @settlor)
(screening.pep :entity-id @trustee)
(screening.pep :entity-id @beneficiary1)
(screening.pep :entity-id @beneficiary2)
```

**Expected Results**:
- 1 Trust entity
- 4 Natural person entities
- 4 Roles (SETTLOR, TRUSTEE, 2x BENEFICIARY)
- 5 Documents (1 TRUST_DEED, 4 PASSPORTs)

---

## Scenario 5: Partnership Structure

**Description**: Limited partnership with general and limited partners.

```clojure
(cbu.create 
    :name "Alpha Capital Partners LP" 
    :client-type "corporate" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-partnership 
    :cbu-id @cbu
    :name "Alpha Capital Partners LP"
    :partnership-type "limited"
    :as @partnership)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Alpha GP Ltd"
    :company-number "GP123456"
    :as @gp)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Fund Manager One"
    :as @gp_ubo)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Pension Fund A"
    :as @lp1)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Insurance Co B"
    :as @lp2)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @gp
    :target-entity-id @partnership
    :role "GENERAL_PARTNER"
    :ownership-percentage 1.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @gp_ubo
    :target-entity-id @gp
    :role "BENEFICIAL_OWNER"
    :ownership-percentage 100.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @lp1
    :target-entity-id @partnership
    :role "LIMITED_PARTNER"
    :ownership-percentage 60.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @lp2
    :target-entity-id @partnership
    :role "LIMITED_PARTNER"
    :ownership-percentage 39.0)

(document.catalog :cbu-id @cbu :entity-id @partnership :document-type "PARTNERSHIP_AGREEMENT")
(document.catalog :cbu-id @cbu :entity-id @gp :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @gp_ubo :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @lp1 :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @lp2 :document-type "CERTIFICATE_OF_INCORPORATION")

(screening.pep :entity-id @gp_ubo)
(screening.sanctions :entity-id @gp_ubo)
```

**Expected Results**:
- 1 Partnership, 3 Companies, 1 Natural Person
- GP owns 1%, LP1 owns 60%, LP2 owns 39%
- GP's UBO (gp_ubo) is the natural person to screen

---

## Scenario 6: Multi-Level Ownership Chain

**Description**: Holding company structure with intermediate holding.

```clojure
(cbu.create 
    :name "Global Holdings Group" 
    :client-type "corporate" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company 
    :cbu-id @cbu
    :name "OpCo Ltd"
    :company-number "OPCO001"
    :as @opco)

(entity.create-limited-company 
    :cbu-id @cbu
    :name "HoldCo Ltd"
    :company-number "HOLDCO01"
    :as @holdco)

(entity.create-limited-company 
    :cbu-id @cbu
    :name "TopCo Ltd"
    :company-number "TOPCO001"
    :as @topco)

(entity.create-proper-person
    :cbu-id @cbu
    :name "Ultimate Owner"
    :first-name "Ultimate"
    :last-name "Owner"
    :as @ultimate_owner)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @holdco
    :target-entity-id @opco
    :role "SHAREHOLDER"
    :ownership-percentage 100.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @topco
    :target-entity-id @holdco
    :role "SHAREHOLDER"
    :ownership-percentage 100.0)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ultimate_owner
    :target-entity-id @topco
    :role "BENEFICIAL_OWNER"
    :ownership-percentage 100.0)

(document.catalog :cbu-id @cbu :entity-id @opco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @holdco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @topco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @ultimate_owner :document-type "PASSPORT")

(screening.pep :entity-id @ultimate_owner)
(screening.sanctions :entity-id @ultimate_owner)

(ubo.calculate :cbu-id @cbu :entity-id @opco :threshold 25.0)
```

**Expected Results**:
- Chain: OpCo ← HoldCo ← TopCo ← Ultimate Owner
- UBO calculation should trace through to Ultimate Owner at 100% effective ownership
- Note: This tests whether UBO calculation follows indirect ownership

---

## Scenario 7: Document Request Workflow

**Description**: Request missing documents from client.

```clojure
(cbu.create 
    :name "Document Test Client" 
    :client-type "individual" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-proper-person 
    :cbu-id @cbu
    :name "Test Person"
    :as @person)

(document.request
    :cbu-id @cbu
    :entity-id @person
    :document-type "PASSPORT"
    :due-date "2025-01-15"
    :priority "high")

(document.request
    :cbu-id @cbu
    :entity-id @person
    :document-type "PROOF_OF_ADDRESS"
    :due-date "2025-01-15"
    :priority "medium")
```

**Expected Results**:
- 2 Document requests created with pending status
- Due dates and priorities recorded

---

## Scenario 8: Full KYC Workflow

**Description**: Complete KYC investigation lifecycle.

```clojure
(cbu.create 
    :name "KYC Test Client" 
    :client-type "individual" 
    :jurisdiction "GB"
    :as @cbu)

(entity.create-proper-person 
    :cbu-id @cbu
    :name "KYC Test Person"
    :as @person)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PASSPORT"
    :as @passport)

(document.extract
    :document-id @passport
    :attributes ["full_name", "date_of_birth", "passport_number", "expiry_date"]
    :use-ocr true)

(screening.pep :entity-id @person)
(screening.sanctions :entity-id @person)
(screening.adverse-media :entity-id @person :lookback-months 24)

(kyc.initiate
    :cbu-id @cbu
    :investigation-type "NEW_CLIENT"
    :as @investigation)
```

**Expected Results**:
- Document cataloged and extracted
- All three screening types run
- KYC investigation initiated

---

## Error Scenarios (Should Fail Validation)

### Error 1: Passport for Company (CSG Violation)

```clojure
(cbu.create :name "Error Test" :client-type "corporate" :jurisdiction "GB" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name "Error Corp" :as @company)
(document.catalog :cbu-id @cbu :entity-id @company :document-type "PASSPORT")
```

**Expected**: CSG error C001 - PASSPORT not applicable to LIMITED_COMPANY

### Error 2: Undefined Symbol Reference

```clojure
(cbu.create :name "Error Test" :as @cbu)
(document.catalog :cbu-id @cbu :entity-id @nonexistent :document-type "PASSPORT")
```

**Expected**: Error - undefined symbol @nonexistent

### Error 3: Unknown Verb

```clojure
(cbu.create :name "Error Test" :as @cbu)
(entity.create-unicorn :cbu-id @cbu :name "Magical")
```

**Expected**: Error - unknown verb entity.create-unicorn

### Error 4: Missing Required Argument

```clojure
(cbu.create :jurisdiction "GB" :as @cbu)
```

**Expected**: Error - cbu.create requires :name

### Error 5: Trust Deed for Company (CSG Violation)

```clojure
(cbu.create :name "Error Test" :client-type "corporate" :jurisdiction "GB" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name "Error Corp" :as @company)
(document.catalog :cbu-id @cbu :entity-id @company :document-type "TRUST_DEED")
```

**Expected**: CSG error - TRUST_DEED not applicable to LIMITED_COMPANY

---

## Test Runner Script

Create file: `rust/tests/dsl_scenarios_test.sh`

```bash
#!/bin/bash
# DSL Scenario Test Runner

set -e

CLI="cargo run --features cli,database --bin dsl_cli --"
DB_URL="${DATABASE_URL:-postgresql://localhost/ob-poc}"

PASS=0
FAIL=0

echo "═══════════════════════════════════════════════════════════"
echo "              DSL Scenario Integration Tests                "
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "Database: $DB_URL"
echo ""

# Helper function for scenarios that should succeed
test_scenario() {
    local name="$1"
    local file="$2"
    
    echo -n "Testing: $name... "
    
    if $CLI execute --db-url "$DB_URL" --file "$file" --format json > /tmp/dsl_result.json 2>&1; then
        if jq -e '.success == true' /tmp/dsl_result.json > /dev/null; then
            echo "PASS"
            PASS=$((PASS + 1))
        else
            echo "FAIL (execution reported failure)"
            cat /tmp/dsl_result.json
            FAIL=$((FAIL + 1))
        fi
    else
        echo "FAIL (command failed)"
        cat /tmp/dsl_result.json
        FAIL=$((FAIL + 1))
    fi
}

# Helper function for scenarios that should fail
test_error_scenario() {
    local name="$1"
    local file="$2"
    local expected_error="$3"
    
    echo -n "Testing: $name (should fail)... "
    
    if $CLI validate --file "$file" --format json > /tmp/dsl_result.json 2>&1; then
        echo "FAIL (expected validation to fail but it passed)"
        FAIL=$((FAIL + 1))
    else
        if grep -q "$expected_error" /tmp/dsl_result.json; then
            echo "PASS (correctly rejected)"
            PASS=$((PASS + 1))
        else
            echo "FAIL (failed but not with expected error)"
            cat /tmp/dsl_result.json
            FAIL=$((FAIL + 1))
        fi
    fi
}

# Run scenarios
# (Assumes scenario files exist in rust/tests/scenarios/)

test_scenario "Individual Onboarding" "rust/tests/scenarios/01_individual.dsl"
test_scenario "Corporate Single UBO" "rust/tests/scenarios/02_corporate_single_ubo.dsl"
test_scenario "Corporate Multiple UBOs" "rust/tests/scenarios/03_corporate_multi_ubo.dsl"
test_scenario "Trust Structure" "rust/tests/scenarios/04_trust.dsl"
test_scenario "Partnership Structure" "rust/tests/scenarios/05_partnership.dsl"
test_scenario "Multi-Level Ownership" "rust/tests/scenarios/06_multi_level.dsl"

# Error scenarios
test_error_scenario "Passport for Company" "rust/tests/scenarios/err_01_passport_company.dsl" "C001"
test_error_scenario "Undefined Symbol" "rust/tests/scenarios/err_02_undefined_symbol.dsl" "undefined"
test_error_scenario "Unknown Verb" "rust/tests/scenarios/err_03_unknown_verb.dsl" "unknown"

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "                       RESULTS                              "
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ $FAIL -gt 0 ]; then
    exit 1
else
    echo "All scenarios passed!"
    exit 0
fi
```

---

## Execution Checklist

### Phase 1: Create Scenario Files
- [ ] Create `rust/tests/scenarios/` directory
- [ ] Create `01_individual.dsl`
- [ ] Create `02_corporate_single_ubo.dsl`
- [ ] Create `03_corporate_multi_ubo.dsl`
- [ ] Create `04_trust.dsl`
- [ ] Create `05_partnership.dsl`
- [ ] Create `06_multi_level.dsl`
- [ ] Create error scenario files

### Phase 2: Seed Database Prerequisites
- [ ] Ensure entity_types has all required types (TRUST, PARTNERSHIP, etc.)
- [ ] Ensure roles table has all required roles (SETTLOR, TRUSTEE, BENEFICIARY, GENERAL_PARTNER, LIMITED_PARTNER, etc.)
- [ ] Ensure document_types has all required types

### Phase 3: Run Scenarios
- [ ] Run validation-only first: `dsl_cli validate --file scenario.dsl`
- [ ] Run dry-run: `dsl_cli execute --dry-run --file scenario.dsl`
- [ ] Run actual execution: `dsl_cli execute --file scenario.dsl`

### Phase 4: Verify Database State
- [ ] Check CBUs created
- [ ] Check entities with correct types
- [ ] Check roles assigned
- [ ] Check documents cataloged
- [ ] Check screenings initiated

---

## Notes

1. **Idempotency**: Running scenarios multiple times will create duplicate data. Consider adding cleanup scripts or using test transactions.

2. **Dependencies**: Some scenarios assume seed data exists (entity types, roles, document types).

3. **UBO Calculation**: The multi-level ownership scenario tests whether UBO calculation can trace through intermediate entities.

4. **Parser Quirk**: Remember that comments cannot be on the first line of the file.
