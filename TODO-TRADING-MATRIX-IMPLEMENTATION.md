# TODO: Trading Matrix Storage Architecture Implementation
## Implementation Plan for Claude Code

**Created:** December 22, 2025  
**Scope:** Traded Instruments Matrix, SLA Framework, Service Resource Traceability  
**Estimated Effort:** 2-3 weeks  
**Reference Docs:** 
- `docs/TRADING_MATRIX_STORAGE_ARCHITECTURE.md`
- `docs/GAP_ANALYSIS_TRADED_INSTRUMENTS.md`

---

## Pre-Implementation Checklist

Before starting, verify:
- [ ] Database connection available (`rust/.env` configured)
- [ ] Current schema exported (`pg_dump` for rollback)
- [ ] All tests passing (`cargo test`)
- [ ] No uncommitted changes in working directory

---

## Phase 1: Schema Migration (Day 1)

### 1.1 Run Core Migration
**File:** `rust/migrations/202412_trading_matrix_storage.sql`

```bash
# From project root
psql -d ob_poc -f rust/migrations/202412_trading_matrix_storage.sql
```

- [ ] Execute migration script
- [ ] Verify all tables created:
  ```sql
  SELECT table_schema, table_name 
  FROM information_schema.tables 
  WHERE table_name IN (
      'trading_profile_documents', 'cbu_im_assignments', 'cbu_pricing_config',
      'cbu_cash_sweep_config', 'resource_profile_sources', 'sla_metric_types',
      'sla_templates', 'cbu_sla_commitments', 'sla_measurements', 'sla_breaches'
  );
  ```
- [ ] Verify new document types inserted (15 types)
- [ ] Verify new service resource types inserted (14 types)
- [ ] Verify SLA metric types inserted (12 metrics)
- [ ] Verify SLA templates inserted (9 templates)
- [ ] Verify instrument classes added (STIF, MMF, REPO)

### 1.2 Update SQLx Offline Cache
```bash
cd rust
cargo sqlx prepare --workspace
```

- [ ] Regenerate `.sqlx` cache files
- [ ] Commit updated `.sqlx` directory

### 1.3 Export Updated Schema
```bash
pg_dump -s ob_poc > schema_docker.sql
```

- [ ] Update `schema_docker.sql` for Docker builds
- [ ] Update `schema_export.sql` if maintained separately

---

## Phase 2: Entity Taxonomy Updates (Day 1-2)

### 2.1 Add New Entity Types
**File:** `rust/config/ontology/entity_taxonomy.yaml`

Add the following entity definitions after existing entities:

- [ ] Add `im_assignment` entity type:
```yaml
  im_assignment:
    description: "Investment Manager assignment to CBU"
    category: custody
    db:
      schema: custody
      table: cbu_im_assignments
      pk: assignment_id
    search_keys:
      - columns: [cbu_id, manager_lei]
        unique: false
    lifecycle:
      status_column: status
      states: [ACTIVE, SUSPENDED, TERMINATED]
      initial_state: ACTIVE
    implicit_create:
      allowed: true
      canonical_verb: investment-manager.assign
      required_args: [cbu-id, manager-lei, priority, instruction-method]
```

- [ ] Add `pricing_config` entity type
- [ ] Add `cash_sweep_config` entity type
- [ ] Add `sla_commitment` entity type
- [ ] Add `sla_measurement` entity type
- [ ] Add `sla_breach` entity type

### 2.2 Add FK Relationships
**File:** `rust/config/ontology/entity_taxonomy.yaml` (relationships section)

- [ ] Add `cbu` → `im_assignment` relationship
- [ ] Add `cbu` → `pricing_config` relationship
- [ ] Add `cbu` → `cash_sweep_config` relationship
- [ ] Add `cbu` → `sla_commitment` relationship
- [ ] Add `trading_profile` → `im_assignment` relationship
- [ ] Add `trading_profile` → `sla_commitment` relationship
- [ ] Add `sla_commitment` → `sla_measurement` relationship
- [ ] Add `sla_measurement` → `sla_breach` relationship

---

## Phase 3: Verb Registration (Day 2)

### 3.1 Add New Verb Files to Loader
**File:** `rust/src/dsl_v2/verb_registry.rs` (or equivalent loader)

New verb files to register:
- [ ] `rust/config/verbs/investment-manager.yaml`
- [ ] `rust/config/verbs/sla.yaml`
- [ ] `rust/config/verbs/pricing-config.yaml`
- [ ] `rust/config/verbs/cash-sweep.yaml`

### 3.2 Verify Verb Loading
```bash
cd rust
cargo test verb_loading
```

- [ ] All new verbs parse without errors
- [ ] Verb count increased (check logs)
- [ ] No duplicate verb names

### 3.3 Test Basic CRUD Verbs
Create test DSL file `rust/examples/trading_matrix_test.dsl`:

```clojure
; Test IM assignment
(investment-manager.assign 
  :cbu-id "test-cbu" 
  :manager-lei "549300TEST000001" 
  :priority 10 
  :instruction-method SWIFT)

; Test pricing config
(pricing-config.set 
  :cbu-id "test-cbu" 
  :instrument-class "EQUITY" 
  :priority 1 
  :source BLOOMBERG)

; Test cash sweep
(cash-sweep.configure 
  :cbu-id "test-cbu" 
  :currency "USD" 
  :threshold-amount 100000 
  :vehicle-type STIF 
  :sweep-time "16:00" 
  :sweep-timezone "America/New_York")

; Test SLA commitment
(sla.commit 
  :cbu-id "test-cbu" 
  :template-code "CUSTODY_SETTLE_DVP")
```

- [ ] Basic CRUD verbs execute without errors
- [ ] Records created in database
- [ ] Symbol capture working (`→ @assignment-id`)

---

## Phase 4: Plugin Handlers (Day 3-5)

### 4.1 Investment Manager Handlers
**File:** `rust/src/plugins/investment_manager.rs` (new file)

- [ ] Create module file
- [ ] Implement `find_im_for_trade` handler:
  - Input: cbu_id, market, instrument_class, currency, isda_asset_class
  - Logic: Query `cbu_im_assignments` with scope matching
  - Return: Matching assignment with instruction method
- [ ] Register handler in plugin registry

```rust
// Signature
pub async fn find_im_for_trade(
    pool: &PgPool,
    args: &HashMap<String, Value>,
) -> Result<ExecutionResult, DslError>
```

### 4.2 Pricing Config Handlers
**File:** `rust/src/plugins/pricing_config.rs` (new file)

- [ ] Create module file
- [ ] Implement `find_pricing_for_instrument` handler:
  - Input: cbu_id, instrument_class, market, currency
  - Logic: Query `cbu_pricing_config` with priority ordering
  - Return: Best matching pricing source config
- [ ] Register handler in plugin registry

### 4.3 SLA Handlers
**File:** `rust/src/plugins/sla.rs` (new file)

- [ ] Create module file
- [ ] Implement `list_open_sla_breaches` handler:
  - Input: cbu_id, optional severity filter
  - Logic: Join commitments → measurements → breaches
  - Return: Open breaches with commitment details
- [ ] Implement `auto_detect_breach` helper (for measurement recording):
  - Compare measured value to target
  - Auto-create breach record if threshold crossed
- [ ] Register handlers in plugin registry

### 4.4 Update Plugin Registry
**File:** `rust/src/plugins/mod.rs`

- [ ] Add `mod investment_manager;`
- [ ] Add `mod pricing_config;`
- [ ] Add `mod sla;`
- [ ] Register all new handlers in `get_plugin_handler()` match

---

## Phase 5: Trading Profile Materialization (Day 5-7)

### 5.1 Extend Materialize Handler
**File:** `rust/src/plugins/trading_profile.rs`

Current `materialize_trading_profile` handler needs extension to:

- [ ] **Parse `investment_managers` section** → Insert into `cbu_im_assignments`
  ```rust
  for im in profile.investment_managers {
      // Insert assignment
      // Track profile_id linkage
  }
  ```

- [ ] **Parse `pricing_matrix` section** → Insert into `cbu_pricing_config`
  ```rust
  for pricing in profile.pricing_matrix {
      // Insert config
      // Link to profile
  }
  ```

- [ ] **Parse `cash_sweep_config` section** (NEW) → Insert into `cbu_cash_sweep_config`
  - Add new section to trading profile schema
  - Parse and insert

- [ ] **Parse `sla_commitments` section** (NEW) → Insert into `cbu_sla_commitments`
  - Add new section to trading profile schema
  - Parse and insert

- [ ] **Create `resource_profile_sources` links** for all materialized records
  ```rust
  // After inserting IM assignment
  insert_resource_profile_source(
      instance_id,
      profile_id,
      "investment_managers",
      format!("$.investment_managers[{}]", idx)
  );
  ```

### 5.2 Update Trading Profile Schema
**File:** `rust/config/seed/trading_profiles/allianzgi_complete.yaml`

Add new sections:

- [ ] Add `cash_sweep_config` section:
```yaml
cash_sweep_config:
  enabled: true
  sweeps:
    - currency: EUR
      threshold_amount: 50000
      vehicle_type: STIF
      vehicle_id: "BNYINSTCASH001"
      sweep_time: "17:00"
      sweep_timezone: Europe/Luxembourg
    - currency: USD
      threshold_amount: 100000
      vehicle_type: STIF
      sweep_time: "16:00"
      sweep_timezone: America/New_York
```

- [ ] Add `sla_commitments` section:
```yaml
sla_commitments:
  - template_code: CUSTODY_SETTLE_DVP
    scope_instrument_classes: [EQUITY, GOVT_BOND, CORP_BOND]
  - template_code: FA_NAV_DELIVERY
  - template_code: CSA_MARGIN_CALL
    bound_to: isda_agreements  # Links to ISDA section
```

### 5.3 Update Profile Validation
**File:** `rust/src/plugins/trading_profile.rs` (validate handler)

- [ ] Add validation for `cash_sweep_config` section
- [ ] Add validation for `sla_commitments` section
- [ ] Validate template_code references exist
- [ ] Validate cross-references (e.g., `bound_to: isda_agreements`)

### 5.4 Implement Resource Provisioning from Profile
**File:** `rust/src/plugins/trading_profile.rs`

Add new verb handler `provision_profile_resources`:

- [ ] Parse `investment_managers` → Provision connectivity resources
  - SWIFT → Provision SWIFT_GATEWAY
  - CTM → Provision CTM_CONNECTION
  - FIX → Provision FIX_SESSION
  - API → Provision API_ENDPOINT
- [ ] Parse `pricing_matrix` → Provision pricing resources
  - BLOOMBERG → Provision BLOOMBERG_TERMINAL
  - MARKIT → Provision MARKIT_PRICING
- [ ] Parse `cash_sweep_config` → Provision cash management resources
  - STIF → Provision STIF_ACCOUNT + CASH_SWEEP_ENGINE
- [ ] Create `resource_profile_sources` links for all provisioned resources
- [ ] Return provisioning report with resource URLs

---

## Phase 6: Extend Trading Profile Verbs (Day 7-8)

### 6.1 Add New Verbs to trading-profile.yaml
**File:** `rust/config/verbs/trading-profile.yaml`

- [ ] Add `link-document` verb (already in architecture doc)
- [ ] Add `generate-from-document` verb
- [ ] Add `provision-resources` verb
- [ ] Add `check-sla-coverage` verb:
```yaml
check-sla-coverage:
  description: Check SLA coverage gaps for profile
  behavior: plugin
  handler: check_profile_sla_coverage
  args:
    - name: profile-id
      type: uuid
      required: true
  returns:
    type: record
    description: Coverage report with gaps
```

### 6.2 Implement Document Extraction Handler
**File:** `rust/src/plugins/trading_profile.rs`

`generate_profile_from_document` handler:
- [ ] Accept document_id and section
- [ ] Call LLM extraction (if INVESTMENT_MANDATE doc type)
- [ ] Generate YAML/JSON for specified section
- [ ] Merge into existing profile (APPEND/REPLACE/MERGE modes)
- [ ] Return generated content for review

---

## Phase 7: Database Functions (Day 8)

### 7.1 SSI Lookup Enhancement
**File:** New migration or append to existing

- [ ] Update `custody.find_ssi_for_trade` to consider IM assignments:
```sql
-- Consider IM instruction method when selecting SSI
CREATE OR REPLACE FUNCTION custody.find_ssi_for_trade_v2(...)
```

### 7.2 SLA Measurement Auto-Status
**File:** New migration

- [ ] Create trigger to auto-set measurement status:
```sql
CREATE OR REPLACE FUNCTION "ob-poc".sla_measurement_status_trigger()
RETURNS TRIGGER AS $$
BEGIN
    -- Compare NEW.measured_value to commitment target
    -- Set NEW.status accordingly
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

### 7.3 SLA Breach Auto-Creation
**File:** New migration

- [ ] Create trigger to auto-create breach on BREACH status:
```sql
CREATE OR REPLACE FUNCTION "ob-poc".sla_breach_auto_create_trigger()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = 'BREACH' THEN
        INSERT INTO "ob-poc".sla_breaches (...)
        VALUES (...);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

---

## Phase 8: Integration Testing (Day 9-10)

### 8.1 Create End-to-End Test Scenario
**File:** `rust/examples/trading_matrix_e2e.dsl`

```clojure
; === PHASE 1: Create CBU ===
(cbu.create :name "Test Multi-Asset Fund" :jurisdiction "LU" :cbu-category FUND_MANDATE)
→ @test-fund

; === PHASE 2: Import Trading Profile ===
(trading-profile.import 
  :cbu-id @test-fund 
  :file-path "config/seed/trading_profiles/allianzgi_complete.yaml"
  :status DRAFT)
→ @profile

; === PHASE 3: Validate Profile ===
(trading-profile.validate :profile-id @profile)

; === PHASE 4: Activate Profile ===
(trading-profile.activate :profile-id @profile)

; === PHASE 5: Materialize to Operational Tables ===
(trading-profile.materialize :profile-id @profile)

; === PHASE 6: Provision Resources ===
(trading-profile.provision-resources :profile-id @profile)

; === PHASE 7: Verify IM Assignments ===
(investment-manager.list :cbu-id @test-fund)

; === PHASE 8: Test IM Lookup ===
(investment-manager.find-for-trade 
  :cbu-id @test-fund 
  :market "XETR" 
  :instrument-class "EQUITY")

; === PHASE 9: Verify Pricing Config ===
(pricing-config.list :cbu-id @test-fund)

; === PHASE 10: Verify SLA Commitments ===
(sla.list-commitments :cbu-id @test-fund)

; === PHASE 11: Record SLA Measurement ===
(sla.record-measurement 
  :commitment-id @settle-sla 
  :period-start "2025-01-01" 
  :period-end "2025-01-31"
  :measured-value 99.7
  :status MET)
```

- [ ] Execute full scenario without errors
- [ ] Verify all tables populated correctly
- [ ] Verify resource provisioning completed
- [ ] Verify SLA bindings created

### 8.2 Create Unit Tests
**File:** `rust/tests/trading_matrix_tests.rs`

- [ ] Test IM scope matching logic
- [ ] Test pricing config priority resolution
- [ ] Test SLA measurement status calculation
- [ ] Test materialization idempotency
- [ ] Test profile diff detection

### 8.3 Create Traceability Tests
**File:** `rust/tests/traceability_tests.rs`

- [ ] Test: Resource → Profile source link exists
- [ ] Test: SLA → Profile binding exists
- [ ] Test: IM Assignment → Connectivity resource link exists
- [ ] Test: Pricing Config → Pricing resource link exists

---

## Phase 9: Agent Integration (Day 10-11)

### 9.1 Update Agent System Prompt
**File:** `rust/src/api/agent_service.rs` (or agent prompt config)

Add examples for:
- [ ] Trading matrix construction conversation
- [ ] IM assignment via chat
- [ ] SLA commitment creation
- [ ] Pricing source configuration

### 9.2 Create Agent Training Examples
**File:** `rust/config/agent_examples/trading_matrix.yaml` (new)

```yaml
examples:
  - user: "Set up investment managers for our new fund"
    intent: "configure_investment_managers"
    dsl: |
      (investment-manager.assign :cbu-id @current-cbu ...)
      
  - user: "What's our settlement SLA?"
    intent: "query_sla"
    dsl: |
      (sla.list-commitments :cbu-id @current-cbu)
      
  - user: "IM1 trades European equities via CTM, IM2 handles derivatives via API"
    intent: "multi_im_setup"
    dsl: |
      (investment-manager.assign :cbu-id @current-cbu :manager-lei "..." 
        :scope-markets ["XETR" "XLON"] :instruction-method CTM)
      (investment-manager.assign :cbu-id @current-cbu :manager-lei "..."
        :scope-instrument-classes ["OTC_DERIVATIVE"] :instruction-method API)
```

- [ ] Add 10+ training examples
- [ ] Cover common scenarios
- [ ] Include error cases

### 9.3 Test Agent Conversations
- [ ] Test: "Set up trading profile for new fund"
- [ ] Test: "Add investment manager with European equity scope"
- [ ] Test: "Configure Bloomberg for equity pricing"
- [ ] Test: "Set up EUR cash sweep to STIF"
- [ ] Test: "What SLAs apply to our derivatives trading?"

---

## Phase 10: Documentation & Cleanup (Day 11-12)

### 10.1 Update API Documentation
- [ ] Document new endpoints (if REST API exposed)
- [ ] Document new DSL verbs in `docs/DSL_REFERENCE.md`
- [ ] Add examples to `docs/examples/`

### 10.2 Update CLAUDE.md
**File:** `CLAUDE.md`

- [ ] Add trading matrix domain overview
- [ ] Document new verb domains
- [ ] Add common DSL patterns

### 10.3 Update Schema Reference
**File:** `SCHEMA_REFERENCE.md`

- [ ] Add new tables to schema documentation
- [ ] Document relationships
- [ ] Add ER diagram updates

### 10.4 Code Cleanup
- [ ] Remove any TODO comments
- [ ] Run `cargo clippy` and fix warnings
- [ ] Run `cargo fmt`
- [ ] Update `Cargo.toml` if new dependencies added

---

## Verification Checklist

### Schema Verification
```sql
-- Count new tables
SELECT COUNT(*) FROM information_schema.tables 
WHERE table_name IN ('cbu_im_assignments', 'cbu_pricing_config', 
                     'cbu_cash_sweep_config', 'sla_metric_types',
                     'sla_templates', 'cbu_sla_commitments',
                     'sla_measurements', 'sla_breaches',
                     'trading_profile_documents', 'resource_profile_sources');
-- Expected: 10

-- Count new resource types
SELECT COUNT(*) FROM "ob-poc".service_resource_types 
WHERE resource_code IN ('SWIFT_GATEWAY', 'CTM_CONNECTION', 'ALERT_CONNECTION',
                        'FIX_SESSION', 'API_ENDPOINT', 'BLOOMBERG_TERMINAL',
                        'BLOOMBERG_BVAL', 'REFINITIV_FEED', 'MARKIT_PRICING',
                        'ICE_PRICING', 'CASH_SWEEP_ENGINE', 'STIF_ACCOUNT',
                        'SETTLEMENT_INSTRUCTION_ENGINE', 'CSD_GATEWAY');
-- Expected: 14
```

### Verb Verification
```bash
# Count verbs per domain
grep -r "^      [a-z]" rust/config/verbs/investment-manager.yaml | wc -l
grep -r "^      [a-z]" rust/config/verbs/sla.yaml | wc -l
grep -r "^      [a-z]" rust/config/verbs/pricing-config.yaml | wc -l
grep -r "^      [a-z]" rust/config/verbs/cash-sweep.yaml | wc -l
```

### Test Verification
```bash
cd rust
cargo test trading_matrix
cargo test sla
cargo test investment_manager
cargo test pricing_config
```

---

## Rollback Plan

If issues encountered:

1. **Schema Rollback:**
   ```sql
   -- Drop new tables in reverse order
   DROP TABLE IF EXISTS "ob-poc".sla_breaches CASCADE;
   DROP TABLE IF EXISTS "ob-poc".sla_measurements CASCADE;
   DROP TABLE IF EXISTS "ob-poc".cbu_sla_commitments CASCADE;
   DROP TABLE IF EXISTS "ob-poc".sla_templates CASCADE;
   DROP TABLE IF EXISTS "ob-poc".sla_metric_types CASCADE;
   DROP TABLE IF EXISTS "ob-poc".resource_profile_sources CASCADE;
   DROP TABLE IF EXISTS custody.cbu_cash_sweep_config CASCADE;
   DROP TABLE IF EXISTS custody.cbu_pricing_config CASCADE;
   DROP TABLE IF EXISTS custody.cbu_im_assignments CASCADE;
   DROP TABLE IF EXISTS "ob-poc".trading_profile_documents CASCADE;
   
   -- Remove new columns from cbu_trading_profiles
   ALTER TABLE "ob-poc".cbu_trading_profiles 
     DROP COLUMN IF EXISTS source_document_id,
     DROP COLUMN IF EXISTS materialization_status,
     DROP COLUMN IF EXISTS materialized_at,
     DROP COLUMN IF EXISTS materialization_hash,
     DROP COLUMN IF EXISTS sla_profile_id;
   ```

2. **Restore from backup:**
   ```bash
   psql -d ob_poc < backup_pre_trading_matrix.sql
   ```

3. **Revert verb files:**
   ```bash
   git checkout rust/config/verbs/investment-manager.yaml
   git checkout rust/config/verbs/sla.yaml
   git checkout rust/config/verbs/pricing-config.yaml
   git checkout rust/config/verbs/cash-sweep.yaml
   ```

---

## Success Criteria

Implementation is complete when:

1. ✅ All 10 new tables exist and are populated with seed data
2. ✅ All 4 new verb domains registered and functional
3. ✅ Trading profile materialize populates IM assignments, pricing, sweeps
4. ✅ Resource provisioning creates connectivity and pricing resources
5. ✅ SLA commitments can be bound to profiles, services, resources, ISDA
6. ✅ Full traceability: Resource → Profile → Source Document
7. ✅ Agent can construct trading matrix via conversation
8. ✅ All tests passing
9. ✅ Documentation updated

---

## Notes for Claude Code

- **Start with Phase 1** - schema must exist before anything else
- **Test incrementally** - run tests after each phase
- **Use existing patterns** - look at `cbu-custody.yaml` for verb structure examples
- **Check `rust/src/plugins/` for handler patterns**
- **The `trading-profile.materialize` handler is the most complex** - study existing code first
- **SLA auto-breach creation can be deferred** to Phase 2 if time constrained
