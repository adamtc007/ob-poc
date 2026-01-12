# TODO: ISDA DSL Consolidation into Trading Matrix

> **Status:** READY FOR EXECUTION
> **Priority:** HIGH  
> **Risk Level:** LOW (removing redundant paths, matrix already has coverage)
> **Constraint:** Do NOT break materialize. Verify with `cargo build` after each step.

---

## Context

The `isda.*` domain has direct operational write verbs that bypass the canonical trading-profile matrix. This creates:
- Two paths to create ISDA data (violation of single source of truth)
- Potential for ops data to diverge from matrix
- Ghost verbs that agents might call incorrectly

**Current state:**
```
isda.create          → writes custody.isda_agreements directly      ❌ BYPASS
isda.add-coverage    → writes custody.isda_product_coverage directly ❌ BYPASS  
isda.add-csa         → writes custody.csa_agreements directly        ❌ BYPASS
isda.remove-coverage → deletes directly                              ❌ BYPASS
isda.remove-csa      → deletes directly                              ❌ BYPASS
isda.list            → reads (OK, but needs tier fix)                ⚠️ KEEP

trading-profile.add-isda-config      → writes matrix JSONB          ✅ CORRECT
trading-profile.add-isda-coverage    → writes matrix JSONB          ✅ CORRECT
trading-profile.add-csa-config       → writes matrix JSONB          ✅ CORRECT
trading-profile.materialize          → projects to operational      ✅ CORRECT
```

**Target state:**
- DELETE all `isda.*` write verbs
- KEEP `isda.list` as read-only diagnostics (relabeled)
- ADD missing `trading-profile.remove-isda-config` and `trading-profile.remove-csa-config`
- Verify materialize handles deletes on re-projection

---

## Execution Steps

### Phase 1: Add Missing Matrix Remove Verbs

**File:** `config/verbs/trading-profile.yaml`

**Step 1.1:** Add `remove-isda-config` after `add-csa-config` section (~line 760):

```yaml
      remove-isda-config:
        description: Remove ISDA agreement from trading profile
        behavior: plugin
        handler: TradingProfileRemoveIsdaConfigOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: isda
          tags: [authoring, isda, otc]
        args:
          - name: profile-id
            type: uuid
            required: true
            description: "Trading profile ID"
          - name: counterparty-ref
            type: string
            required: true
            description: "Counterparty name or entity ID to remove"
        returns:
          type: affected
```

**Step 1.2:** Add `remove-csa-config` after the above:

```yaml
      remove-csa-config:
        description: Remove CSA from ISDA agreement in trading profile
        behavior: plugin
        handler: TradingProfileRemoveCsaConfigOp
        metadata:
          tier: intent
          source_of_truth: matrix
          scope: cbu
          noun: csa
          tags: [authoring, csa, otc, collateral]
        args:
          - name: profile-id
            type: uuid
            required: true
            description: "Trading profile ID"
          - name: counterparty-ref
            type: string
            required: true
            description: "Counterparty name or entity ID"
          - name: csa-type
            type: string
            required: false
            valid_values: [VM, VM_IM, IM]
            description: "CSA type to remove (if multiple exist)"
        returns:
          type: affected
```

**Verify:** `cargo build --features database`

---

### Phase 2: Implement Remove Handlers

**File:** `src/dsl_v2/custom_ops/trading_profile.rs`

**Step 2.1:** Add struct definitions (near other TradingProfile ops, ~line 50):

```rust
pub struct TradingProfileRemoveIsdaConfigOp;
pub struct TradingProfileRemoveCsaConfigOp;
```

**Step 2.2:** Implement `TradingProfileRemoveIsdaConfigOp`:

```rust
impl CustomOperation for TradingProfileRemoveIsdaConfigOp {
    fn domain(&self) -> &'static str {
        "trading-profile"
    }

    fn verb(&self) -> &'static str {
        "remove-isda-config"
    }

    fn rationale(&self) -> &'static str {
        "Removes an ISDA agreement from the trading profile JSONB document"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let profile_id: Uuid = verb_call.get_uuid_arg("profile-id", ctx)?;
        let counterparty_ref: String = verb_call.get_string_arg("counterparty-ref")?;

        // Load current document
        let mut document = load_trading_profile_document(pool, profile_id).await?;

        // Find and remove the ISDA
        let original_len = document.isda_agreements.len();
        document.isda_agreements.retain(|isda| {
            isda.counterparty_name != counterparty_ref 
                && isda.counterparty_entity_id.map(|id| id.to_string()) != Some(counterparty_ref.clone())
        });

        if document.isda_agreements.len() == original_len {
            return Err(anyhow!("ISDA with counterparty '{}' not found in profile", counterparty_ref));
        }

        // Save updated document
        save_trading_profile_document(pool, profile_id, &document).await?;

        Ok(ExecutionResult::Affected(1))
    }
}
```

**Step 2.3:** Implement `TradingProfileRemoveCsaConfigOp` (similar pattern, removes from nested CSA array within ISDA).

**Step 2.4:** Register in `CustomOperationRegistry::new()`:

```rust
registry.register(Arc::new(TradingProfileRemoveIsdaConfigOp));
registry.register(Arc::new(TradingProfileRemoveCsaConfigOp));
```

**Verify:** `cargo build --features database`

---

### Phase 3: Delete Redundant ISDA Write Verbs

**File:** `config/verbs/custody/isda.yaml`

**Step 3.1:** DELETE these verb definitions entirely:
- `create` (lines ~5-50)
- `add-coverage` (lines ~51-95)
- `add-csa` (lines ~96-155)
- `remove-coverage` (lines ~156-175)
- `remove-csa` (lines ~176-195)

**Step 3.2:** KEEP and UPDATE `list` verb - change metadata:

```yaml
domains:
  isda:
    description: ISDA and CSA agreement diagnostics (read-only - use trading-profile for authoring)
    verbs:
      list:
        description: List ISDA agreements for CBU (read from operational tables)
        metadata:
          tier: diagnostics
          source_of_truth: operational  # Reading projected data
          scope: cbu
          noun: isda
          tags: [read, operational-state]
        behavior: crud
        crud:
          operation: list_by_fk
          table: isda_agreements
          schema: custody
          fk_col: cbu_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: counterparty
            type: uuid
            required: false
            maps_to: counterparty_entity_id
        returns:
          type: record_set

      list-coverage:
        description: List product coverage for an ISDA agreement
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: isda
          tags: [read, operational-state]
        behavior: crud
        crud:
          operation: list_by_fk
          table: isda_product_coverage
          schema: custody
          fk_col: isda_id
        args:
          - name: isda-id
            type: uuid
            required: true
            maps_to: isda_id
        returns:
          type: record_set

      list-csa:
        description: List CSA agreements for an ISDA
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: csa
          tags: [read, operational-state]
        behavior: crud
        crud:
          operation: list_by_fk
          table: csa_agreements
          schema: custody
          fk_col: isda_id
        args:
          - name: isda-id
            type: uuid
            required: true
            maps_to: isda_id
        returns:
          type: record_set
```

**Verify:** `cargo build --features database`

---

### Phase 4: FIX Materialize Orphan Cleanup (REQUIRED)

**File:** `src/dsl_v2/custom_ops/trading_profile.rs`

**CONFIRMED GAP:** `materialize_isda_agreements` does NOT delete orphaned records. The `force` flag only controls UPSERT behavior, not orphan cleanup.

**Step 4.1:** Add orphan cleanup at START of `materialize_isda_agreements` function (around line 892):

```rust
#[cfg(feature = "database")]
async fn materialize_isda_agreements(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pool: &PgPool,
    cbu_id: Uuid,
    isda_agreements: &[IsdaAgreementConfig],
    ssi_name_to_id: &HashMap<String, Uuid>,
) -> Result<i32> {
    let mut created = 0;

    // =========================================================================
    // ORPHAN CLEANUP: Delete ISDAs that are no longer in the matrix
    // =========================================================================
    
    // Build set of counterparty keys from incoming matrix
    let mut incoming_keys: Vec<(Uuid, chrono::NaiveDate)> = Vec::new();
    for isda in isda_agreements {
        if let Ok(counterparty_id) = resolve_entity_ref(pool, &isda.counterparty).await {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&isda.agreement_date, "%Y-%m-%d") {
                incoming_keys.push((counterparty_id, date));
            }
        }
    }
    
    // Get existing ISDA IDs for this CBU
    let existing: Vec<(Uuid, Uuid, chrono::NaiveDate)> = sqlx::query_as(
        r#"SELECT isda_id, counterparty_entity_id, agreement_date 
           FROM custody.isda_agreements WHERE cbu_id = $1"#
    )
    .bind(cbu_id)
    .fetch_all(&mut **tx)
    .await?;
    
    // Find orphans (exist in DB but not in incoming matrix)
    for (isda_id, counterparty_id, agreement_date) in existing {
        let key = (counterparty_id, agreement_date);
        if !incoming_keys.contains(&key) {
            tracing::info!(
                isda_id = %isda_id,
                counterparty = %counterparty_id,
                "materialize_isda_agreements: deleting orphaned ISDA"
            );
            
            // Delete CSAs first (FK constraint)
            sqlx::query("DELETE FROM custody.csa_agreements WHERE isda_id = $1")
                .bind(isda_id)
                .execute(&mut **tx)
                .await?;
            
            // Delete product coverage (FK constraint)
            sqlx::query("DELETE FROM custody.isda_product_coverage WHERE isda_id = $1")
                .bind(isda_id)
                .execute(&mut **tx)
                .await?;
            
            // Delete ISDA
            sqlx::query("DELETE FROM custody.isda_agreements WHERE isda_id = $1")
                .bind(isda_id)
                .execute(&mut **tx)
                .await?;
        }
    }
    
    // =========================================================================
    // Now proceed with UPSERT logic (existing code)
    // =========================================================================
    
    for isda in isda_agreements {
        // ... existing code continues ...
```

**Step 4.2:** Also fix CSA duplicate issue. The current code uses `ON CONFLICT (csa_id)` but generates a NEW csa_id each time, so it never matches. Change the conflict key:

Find this line (~line 998):
```rust
sqlx::query(
    r#"INSERT INTO custody.csa_agreements
       ...
       ON CONFLICT (csa_id) DO UPDATE SET
```

Change to:
```rust
sqlx::query(
    r#"INSERT INTO custody.csa_agreements
       ...
       ON CONFLICT (isda_id, csa_type) DO UPDATE SET
```

**CONFIRMED MISSING:** The unique constraint does NOT exist. Create migration:

**File:** `migrations/YYYYMMDD_csa_unique_constraint.sql` (use today's date)

```sql
-- =============================================================================
-- CSA UNIQUE CONSTRAINT FIX
-- =============================================================================
-- Problem: materialize_isda_agreements uses ON CONFLICT (csa_id) but generates
-- a NEW csa_id each time, so duplicates accumulate. Need (isda_id, csa_type) key.
-- =============================================================================

BEGIN;

-- First, clean up any existing duplicates (keep newest by created_at)
DELETE FROM custody.csa_agreements a
USING custody.csa_agreements b
WHERE a.isda_id = b.isda_id 
  AND a.csa_type = b.csa_type
  AND a.created_at < b.created_at;

-- Add unique constraint
ALTER TABLE custody.csa_agreements 
ADD CONSTRAINT csa_agreements_isda_id_csa_type_key 
UNIQUE (isda_id, csa_type);

COMMIT;
```

Run: `psql $DATABASE_URL -f migrations/YYYYMMDD_csa_unique_constraint.sql`

**Step 4.3:** Same pattern for product coverage orphan cleanup (delete coverage entries for ISDAs we're about to update, then re-insert):

After the orphan cleanup block but before the main loop, add:
```rust
// Clean existing product coverage for ISDAs we're about to update
// (they'll be re-inserted from the matrix)
for isda in isda_agreements {
    if let Ok(counterparty_id) = resolve_entity_ref(pool, &isda.counterparty).await {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&isda.agreement_date, "%Y-%m-%d") {
            sqlx::query(
                r#"DELETE FROM custody.isda_product_coverage 
                   WHERE isda_id IN (
                       SELECT isda_id FROM custody.isda_agreements 
                       WHERE cbu_id = $1 AND counterparty_entity_id = $2 AND agreement_date = $3
                   )"#
            )
            .bind(cbu_id)
            .bind(counterparty_id)
            .bind(date)
            .execute(&mut **tx)
            .await?;
        }
    }
}
```

**Verify:** 
```bash
cargo build --features database
cargo test --features database materialize
```

---

### Phase 5: Update Verb Inventory

**Step 5.1:** Run verb inventory regeneration:
```bash
cargo x verbs inventory --update-claude-md
```

**Step 5.2:** Run verb reconciliation check:
```bash
cargo x verbs check
cargo x verbs lint
```

---

### Phase 6: Final Verification

```bash
# Full build
cargo build --features database

# Run DSL tests
cargo test --features database dsl

# Verify verb count
cargo x verbs compile -v | grep -E "(isda|csa|trading-profile)"
```

---

## Expected Outcome

| Before | After |
|--------|-------|
| `isda.create` | ❌ DELETED |
| `isda.add-coverage` | ❌ DELETED |
| `isda.add-csa` | ❌ DELETED |
| `isda.remove-coverage` | ❌ DELETED |
| `isda.remove-csa` | ❌ DELETED |
| `isda.list` | ✅ KEPT (diagnostics) |
| — | ✅ `isda.list-coverage` (NEW diagnostics) |
| — | ✅ `isda.list-csa` (NEW diagnostics) |
| `trading-profile.add-isda-config` | ✅ KEPT |
| `trading-profile.add-isda-coverage` | ✅ KEPT |
| `trading-profile.add-csa-config` | ✅ KEPT |
| — | ✅ `trading-profile.remove-isda-config` (NEW) |
| — | ✅ `trading-profile.remove-csa-config` (NEW) |

**Single source of truth:** All ISDA/CSA authoring via `trading-profile.*` → `materialize` → operational tables.

---

## Abort Conditions

**STOP and ask human if:**
- `materialize_isda_agreements` doesn't handle deletes (requires schema thought)
- Any test referencing `isda.create` etc. fails (need to update test fixtures)
- More than 10 files need changes (scope creep)


---

## Phase 7: DAG/Compiler Matrix Dependency Rules

> **Context:** The current DAG (`crates/dsl-core/src/dag.rs`) handles coarse-grained dependencies
> (CBU → Doc → Materialize) but doesn't validate the INTERNAL reference integrity within
> the trading matrix JSONB document.

### 7.1 Internal Reference Dependencies

The trading matrix has complex internal cross-references that must be validated:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     TRADING MATRIX INTERNAL DEPENDENCIES                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Universe (instrument_classes, markets)                                     │
│       │                                                                     │
│       ▼                                                                     │
│  Standing Instructions (SSIs)                                               │
│       │                                                                     │
│       ├────────────► Booking Rules (ssi_ref → SSI.name)                     │
│       │                                                                     │
│       ├────────────► CSA Agreements (collateral_ssi_ref → SSI.name)         │
│       │                                                                     │
│       └────────────► Corporate Actions (proceeds_ssi → SSI.name)            │
│                                                                             │
│  ISDA Agreements (counterparty → Entity)                                    │
│       │                                                                     │
│       ├────────────► Product Coverage (asset_class → InstrumentClass)       │
│       │                                                                     │
│       └────────────► CSA Agreements (csa_type, thresholds)                  │
│                                                                             │
│  Investment Manager Mandates (manager → Entity)                             │
│       │                                                                     │
│       └────────────► Scope (markets, instrument_classes)                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Validation Requirements

**File:** `src/dsl_v2/custom_ops/trading_profile.rs` (in validate_go_live_ready)

Add these validation checks to `validate_go_live_ready` handler:

```rust
/// Validate internal reference integrity within trading matrix
fn validate_matrix_references(doc: &TradingMatrixDocument) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    
    // 1. Collect all SSI names defined in standing_instructions
    let ssi_names: HashSet<String> = doc.standing_instructions
        .iter()
        .flat_map(|cat| cat.ssis.iter())
        .map(|ssi| ssi.name.clone())
        .collect();
    
    // 2. Validate booking rules reference existing SSIs
    for rule in &doc.booking_rules {
        if !ssi_names.contains(&rule.ssi_reference) {
            errors.push(ValidationError {
                path: format!("booking_rules.{}.ssi_reference", rule.rule_id),
                message: format!("SSI '{}' not found in standing_instructions", rule.ssi_reference),
                severity: Severity::Error,
            });
        }
    }
    
    // 3. Validate CSA collateral_ssi_ref references existing SSIs
    for isda in &doc.isda_agreements {
        if let Some(ref csa) = isda.csa {
            if let Some(ref ssi_ref) = csa.collateral_ssi_ref {
                if !ssi_names.contains(ssi_ref) {
                    errors.push(ValidationError {
                        path: format!("isda_agreements.{}.csa.collateral_ssi_ref", isda.counterparty_name),
                        message: format!("Collateral SSI '{}' not found in standing_instructions", ssi_ref),
                        severity: Severity::Error,
                    });
                }
            }
        }
    }
    
    // 4. Validate CA proceeds SSI references
    if let Some(ref ca) = doc.corporate_actions {
        for proceeds in &ca.proceeds_ssi_mapping {
            if !ssi_names.contains(&proceeds.ssi_ref) {
                errors.push(ValidationError {
                    path: format!("corporate_actions.proceeds_ssi_mapping.{}", proceeds.proceeds_type),
                    message: format!("Proceeds SSI '{}' not found", proceeds.ssi_ref),
                    severity: Severity::Error,
                });
            }
        }
    }
    
    // 5. Validate IM mandate scope references valid markets/instrument classes
    let valid_markets: HashSet<String> = doc.universe
        .iter()
        .flat_map(|ic| ic.markets.iter())
        .map(|m| m.mic.clone())
        .collect();
    
    for mandate in &doc.investment_managers {
        for mic in &mandate.scope_mics {
            if !valid_markets.contains(mic) {
                errors.push(ValidationError {
                    path: format!("investment_managers.{}.scope_mics", mandate.manager_ref),
                    message: format!("Market '{}' not in trading universe", mic),
                    severity: Severity::Warning, // Warning not error - scope can be broader
                });
            }
        }
    }
    
    errors
}
```

### 7.3 Materialize Dependency Ordering

**File:** `src/dsl_v2/custom_ops/trading_profile.rs`

The `materialize` function already handles ordering implicitly (SSIs before booking rules).
Document this explicitly:

```rust
/// Materialize execution order (enforced by function call sequence):
///
/// 1. SSIs (no deps within matrix)
/// 2. Universe entries (no deps within matrix)  
/// 3. Booking rules (depend on SSIs by name)
/// 4. ISDA agreements (depend on counterparty entities - external)
/// 5. CSA agreements (depend on ISDAs and SSIs)
/// 6. Corporate actions (depend on SSIs for proceeds)
/// 7. Investment manager assignments (depend on IM entities - external)
/// 8. SLA commitments (depend on services/resources)
///
/// Note: External entity dependencies (counterparties, IMs) are resolved at
/// runtime via EntityGateway lookup, not DAG edges.
```

### 7.4 Compiler Op Extensions (OPTIONAL - Future Enhancement)

If full DAG-level validation is needed, add these Op types to `crates/dsl-core/src/ops.rs`:

```rust
/// Trading profile section operations (for future DAG validation)
Op::AddMatrixSSI {
    profile: DocKey,
    ssi_name: String,
    source_stmt: usize,
}

Op::AddMatrixBookingRule {
    profile: DocKey,
    ssi_ref: String,  // Creates dependency on SSI
    source_stmt: usize,
}

Op::AddMatrixIsda {
    profile: DocKey,
    counterparty: EntityKey,  // Creates dependency on entity
    source_stmt: usize,
}
```

**Current recommendation:** Skip this for now. The validation in `validate_go_live_ready` 
is sufficient. Full DAG-level Op types would be overkill unless we need parallel 
execution of matrix section writes.

---

## Phase 8: Matrix Test Harness

> **Purpose:** Comprehensive test coverage for matrix CRUD operations, idempotency,
> and correct projection to operational tables.

### 8.1 Test File Structure

Create: `rust/tests/trading_matrix_integration_tests.rs`

```rust
//! Trading Matrix Integration Tests
//!
//! Tests the full lifecycle:
//! 1. Matrix creation via trading-profile verbs
//! 2. Internal reference validation
//! 3. Materialize projection to operational tables
//! 4. Idempotency (re-materialize produces same state)
//! 5. Orphan cleanup (removed items deleted from ops tables)

use ob_poc::dsl_v2::executor::Executor;
use ob_poc::dsl_v2::registry::CustomOperationRegistry;
use sqlx::PgPool;
use uuid::Uuid;

mod common;
use common::setup_test_db;

/// Helper to execute DSL and return profile ID
async fn execute_dsl(pool: &PgPool, dsl: &str) -> anyhow::Result<Uuid> {
    let registry = CustomOperationRegistry::new();
    let mut executor = Executor::new(pool.clone(), registry);
    executor.execute_dsl(dsl).await
}

/// Helper to count rows in a table for a CBU
async fn count_rows(pool: &PgPool, table: &str, cbu_id: Uuid) -> i64 {
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM {} WHERE cbu_id = $1", table
    ))
    .bind(cbu_id)
    .fetch_one(pool)
    .await
    .unwrap()
}
```

### 8.2 Test Cases

#### Test 1: Basic Matrix Creation

```rust
#[sqlx::test]
async fn test_matrix_creation_and_materialize(pool: PgPool) {
    // 1. Create CBU
    let cbu_id = execute_dsl(&pool, r#"
        (cbu.ensure :name "Test Fund" :jurisdiction "LU" :as @fund)
    "#).await.unwrap();
    
    // 2. Create trading profile with SSI and booking rule
    execute_dsl(&pool, r#"
        (trading-profile.create-draft :cbu-id @fund :as @profile)
        (trading-profile.add-standing-instruction 
            :profile-id @profile
            :ssi-type "SECURITIES"
            :ssi-name "US_EQUITY"
            :safekeeping-account "123456"
            :safekeeping-bic "IRVTUS3N"
            :cash-currency "USD")
        (trading-profile.add-market
            :profile-id @profile
            :instrument-class "EQUITY"
            :mic "XNYS")
        (trading-profile.add-booking-rule
            :profile-id @profile
            :rule-name "US Equities"
            :priority 10
            :ssi-ref "US_EQUITY"
            :match-mic "XNYS")
        (trading-profile.activate :profile-id @profile)
        (trading-profile.materialize :profile-id @profile)
    "#).await.unwrap();
    
    // 3. Verify operational tables populated
    assert_eq!(count_rows(&pool, "custody.cbu_ssi", cbu_id).await, 1);
    assert_eq!(count_rows(&pool, "custody.cbu_instrument_universe", cbu_id).await, 1);
    assert_eq!(count_rows(&pool, "custody.ssi_booking_rules", cbu_id).await, 1);
}
```

#### Test 2: Idempotency - Re-materialize Same State

```rust
#[sqlx::test]
async fn test_materialize_idempotency(pool: PgPool) {
    // Setup: Create profile with data
    execute_dsl(&pool, SETUP_DSL).await.unwrap();
    
    // First materialize
    execute_dsl(&pool, r#"(trading-profile.materialize :profile-id @profile)"#).await.unwrap();
    let count_1 = count_rows(&pool, "custody.cbu_ssi", cbu_id).await;
    
    // Second materialize (should be idempotent)
    execute_dsl(&pool, r#"(trading-profile.materialize :profile-id @profile)"#).await.unwrap();
    let count_2 = count_rows(&pool, "custody.cbu_ssi", cbu_id).await;
    
    assert_eq!(count_1, count_2, "Materialize should be idempotent");
    
    // Verify no duplicate SSIs
    let duplicates: i64 = sqlx::query_scalar(r#"
        SELECT COUNT(*) FROM (
            SELECT ssi_name, COUNT(*) as cnt 
            FROM custody.cbu_ssi 
            WHERE cbu_id = $1 
            GROUP BY ssi_name 
            HAVING COUNT(*) > 1
        ) dups
    "#)
    .bind(cbu_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    
    assert_eq!(duplicates, 0, "No duplicate SSIs should exist");
}
```

#### Test 3: Orphan Cleanup - Removed Items Deleted

```rust
#[sqlx::test]
async fn test_orphan_cleanup_on_rematerialize(pool: PgPool) {
    // 1. Create profile with 3 SSIs
    execute_dsl(&pool, r#"
        (trading-profile.create-draft :cbu-id @fund :as @profile)
        (trading-profile.add-standing-instruction :profile-id @profile :ssi-name "SSI_A" ...)
        (trading-profile.add-standing-instruction :profile-id @profile :ssi-name "SSI_B" ...)
        (trading-profile.add-standing-instruction :profile-id @profile :ssi-name "SSI_C" ...)
        (trading-profile.activate :profile-id @profile)
        (trading-profile.materialize :profile-id @profile)
    "#).await.unwrap();
    
    assert_eq!(count_rows(&pool, "custody.cbu_ssi", cbu_id).await, 3);
    
    // 2. Create new version, remove SSI_B
    execute_dsl(&pool, r#"
        (trading-profile.create-new-version :cbu-id @fund :as @profile_v2)
        (trading-profile.remove-standing-instruction :profile-id @profile_v2 :ssi-name "SSI_B")
        (trading-profile.activate :profile-id @profile_v2)
        (trading-profile.materialize :profile-id @profile_v2)
    "#).await.unwrap();
    
    // 3. Verify SSI_B removed from operational tables
    assert_eq!(count_rows(&pool, "custody.cbu_ssi", cbu_id).await, 2);
    
    let ssi_b_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM custody.cbu_ssi WHERE cbu_id = $1 AND ssi_name = 'SSI_B')"
    )
    .bind(cbu_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    
    assert!(!ssi_b_exists, "SSI_B should be deleted after removal from matrix");
}
```

#### Test 4: ISDA Orphan Cleanup

```rust
#[sqlx::test]
async fn test_isda_orphan_cleanup(pool: PgPool) {
    // 1. Create profile with 2 ISDA agreements
    execute_dsl(&pool, r#"
        (trading-profile.add-isda-config :profile-id @profile :counterparty-name "Goldman Sachs" ...)
        (trading-profile.add-isda-config :profile-id @profile :counterparty-name "JPMorgan" ...)
        (trading-profile.materialize :profile-id @profile)
    "#).await.unwrap();
    
    assert_eq!(count_rows(&pool, "custody.isda_agreements", cbu_id).await, 2);
    
    // 2. Remove Goldman ISDA
    execute_dsl(&pool, r#"
        (trading-profile.create-new-version :cbu-id @fund :as @profile_v2)
        (trading-profile.remove-isda-config :profile-id @profile_v2 :counterparty-ref "Goldman Sachs")
        (trading-profile.activate :profile-id @profile_v2)
        (trading-profile.materialize :profile-id @profile_v2)
    "#).await.unwrap();
    
    // 3. Verify Goldman ISDA and its CSA/coverage removed
    assert_eq!(count_rows(&pool, "custody.isda_agreements", cbu_id).await, 1);
    
    let goldman_exists: bool = sqlx::query_scalar(r#"
        SELECT EXISTS(
            SELECT 1 FROM custody.isda_agreements ia
            JOIN "ob-poc".entities e ON ia.counterparty_entity_id = e.entity_id
            WHERE ia.cbu_id = $1 AND e.name ILIKE '%goldman%'
        )
    "#)
    .bind(cbu_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    
    assert!(!goldman_exists, "Goldman ISDA should be deleted");
}
```

#### Test 5: Internal Reference Validation

```rust
#[sqlx::test]
async fn test_invalid_ssi_reference_rejected(pool: PgPool) {
    // Try to add booking rule referencing non-existent SSI
    let result = execute_dsl(&pool, r#"
        (trading-profile.create-draft :cbu-id @fund :as @profile)
        (trading-profile.add-booking-rule
            :profile-id @profile
            :rule-name "Bad Rule"
            :priority 10
            :ssi-ref "NONEXISTENT_SSI"  ;; This SSI doesn't exist
            :match-mic "XNYS")
        (trading-profile.validate-go-live-ready :profile-id @profile)
    "#).await;
    
    // Should fail validation
    assert!(result.is_err() || result.unwrap().contains("not found"));
}
```

#### Test 6: CSA Duplicate Prevention

```rust
#[sqlx::test]
async fn test_csa_no_duplicates(pool: PgPool) {
    // Materialize same profile multiple times
    for _ in 0..3 {
        execute_dsl(&pool, r#"
            (trading-profile.materialize :profile-id @profile :force true)
        "#).await.unwrap();
    }
    
    // Count CSAs - should be exactly 1 per ISDA
    let csa_count: i64 = sqlx::query_scalar(r#"
        SELECT COUNT(*) FROM custody.csa_agreements ca
        JOIN custody.isda_agreements ia ON ca.isda_id = ia.isda_id
        WHERE ia.cbu_id = $1
    "#)
    .bind(cbu_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    
    // Should match expected count, not 3x
    assert_eq!(csa_count, expected_csa_count, "CSA duplicates detected");
}
```

### 8.3 DSL Test Scenario File

Create: `rust/tests/scenarios/trading_matrix_crud_test.dsl`

```clojure
;; ==============================================================================
;; TRADING MATRIX CRUD TEST SCENARIO
;; ==============================================================================
;; Tests the matrix-centric authoring workflow:
;; 1. Build matrix incrementally via trading-profile verbs
;; 2. Validate internal references
;; 3. Materialize to operational tables
;; 4. Modify matrix, re-materialize, verify sync
;; ==============================================================================

;; --- Setup: Reference Data ---
(instrument-class.ensure :code "EQUITY" :name "Equity" :as @ic-equity)
(market.ensure :mic "XNYS" :name "NYSE" :country-code "US" :as @mkt-xnys)
(market.ensure :mic "XLON" :name "LSE" :country-code "GB" :as @mkt-xlon)

;; --- Create CBU ---
(cbu.ensure :name "Matrix Test Fund" :jurisdiction "LU" :as @fund)

;; --- Phase 1: Create Draft Trading Profile ---
(trading-profile.create-draft :cbu-id @fund :as @profile)

;; --- Phase 2: Build Universe ---
(trading-profile.add-instrument-class :profile-id @profile :class-code "EQUITY" :is-traded true)
(trading-profile.add-market :profile-id @profile :instrument-class "EQUITY" :mic "XNYS")
(trading-profile.add-market :profile-id @profile :instrument-class "EQUITY" :mic "XLON")

;; --- Phase 3: Add SSIs ---
(trading-profile.add-standing-instruction
  :profile-id @profile
  :ssi-type "SECURITIES"
  :ssi-name "US_SSI"
  :safekeeping-account "US-123"
  :safekeeping-bic "IRVTUS3N"
  :cash-currency "USD")

(trading-profile.add-standing-instruction
  :profile-id @profile
  :ssi-type "SECURITIES"
  :ssi-name "UK_SSI"
  :safekeeping-account "UK-456"
  :safekeeping-bic "MIDLGB22"
  :cash-currency "GBP")

;; --- Phase 4: Add Booking Rules (reference SSIs) ---
(trading-profile.add-booking-rule
  :profile-id @profile
  :rule-name "US Equities"
  :priority 10
  :ssi-ref "US_SSI"
  :match-mic "XNYS")

(trading-profile.add-booking-rule
  :profile-id @profile
  :rule-name "UK Equities"
  :priority 10
  :ssi-ref "UK_SSI"
  :match-mic "XLON")

;; --- Phase 5: Validate and Activate ---
(trading-profile.validate-go-live-ready :profile-id @profile :strictness "STANDARD")
(trading-profile.activate :profile-id @profile)

;; --- Phase 6: Materialize to Operational Tables ---
(trading-profile.materialize :profile-id @profile)

;; --- Verify: Query Operational State ---
(cbu-custody.list-universe :cbu-id @fund)
(cbu-custody.list-ssis :cbu-id @fund)
(cbu-custody.list-booking-rules :cbu-id @fund)

;; --- Phase 7: Modify Matrix (New Version) ---
(trading-profile.create-new-version :cbu-id @fund :as @profile_v2)

;; Add new market
(trading-profile.add-market :profile-id @profile_v2 :instrument-class "EQUITY" :mic "XETR")

;; Remove UK SSI (and its booking rule will fail validation)
;; First remove the booking rule
(trading-profile.remove-booking-rule :profile-id @profile_v2 :ssi-ref "UK_SSI" :rule-id "UK Equities")

;; Then remove the SSI
(trading-profile.remove-standing-instruction :profile-id @profile_v2 :ssi-name "UK_SSI")

;; --- Phase 8: Re-validate, Activate, Materialize ---
(trading-profile.validate-go-live-ready :profile-id @profile_v2)
(trading-profile.activate :profile-id @profile_v2)
(trading-profile.materialize :profile-id @profile_v2)

;; --- Verify: UK SSI should be GONE from operational tables ---
(cbu-custody.list-ssis :cbu-id @fund)
;; Expected: Only US_SSI remains

;; ==============================================================================
;; END TEST SCENARIO
;; ==============================================================================
```

### 8.4 Run Tests

```bash
# Unit tests
cargo test --features database trading_matrix

# Integration tests with real DB
DATABASE_URL=postgres://... cargo test --features database -- --test-threads=1

# DSL scenario test
cargo run --features database -- exec tests/scenarios/trading_matrix_crud_test.dsl
```

---

## Summary Checklist

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `remove-isda-config`, `remove-csa-config` to YAML | ⬜ TODO |
| 2 | Implement remove handlers in Rust | ⬜ TODO |
| 3 | Delete redundant `isda.*` write verbs | ⬜ TODO |
| 4 | Fix materialize orphan cleanup + CSA constraint | ⬜ TODO |
| 5 | Update verb inventory | ⬜ TODO |
| 6 | Final verification | ⬜ TODO |
| 7 | DAG/Compiler matrix dependency rules | ⬜ TODO |
| 8 | Matrix test harness | ⬜ TODO |

---

## Execution Notes for Claude

1. **Start with Phase 4** (materialize fixes) - this is the critical path
2. **Phase 1-3** can be done after Phase 4 is stable
3. **Phase 7** is mostly documentation + adding validation to existing handler
4. **Phase 8** should be done LAST as it validates all other phases

**Verify after each phase:**
```bash
cargo build --features database
cargo test --features database
```
