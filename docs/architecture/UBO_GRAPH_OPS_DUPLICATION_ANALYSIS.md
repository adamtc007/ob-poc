# UBO Graph Ops Deep Dive: Duplication Analysis

## Summary

**File:** `ubo_graph_ops.rs`  
**Total Lines:** 3,164  
**Operations:** 16  
**Avg Lines/Op:** 198  

**Finding: ~400-500 lines (15-16%) are extractable duplication.**

The file is legitimately large because UBO convergence is complex. But there ARE repeated patterns worth extracting.

---

## Duplication Inventory

### 1. CBU ID Extraction — 12 occurrences × ~10 lines = **~120 lines**

```rust
// This EXACT pattern appears 12 times:
let cbu_id: Uuid = verb_call
    .arguments
    .iter()
    .find(|a| a.key == "cbu" || a.key == "cbu-id")
    .and_then(|a| {
        if let Some(name) = a.value.as_symbol() {
            ctx.resolve(name)
        } else {
            a.value.as_uuid()
        }
    })
    .ok_or_else(|| anyhow::anyhow!("Missing cbu argument"))?;
```

**Should be:** `helpers::extract_cbu_id(verb_call, ctx)?`

---

### 2. Non-Database Stubs — 16 occurrences × ~8 lines = **~128 lines**

```rust
#[cfg(not(feature = "database"))]
async fn execute(
    &self,
    _verb_call: &VerbCall,
    _ctx: &mut ExecutionContext,
) -> Result<ExecutionResult> {
    Ok(ExecutionResult::Affected(1))
}
```

These vary slightly (some return `Uuid`, some `Affected(1)`, some `Json`).  
Could use a macro: `stub_execute!(Affected(1))` or similar.

---

### 3. Entity Name Lookups — 3 occurrences × ~4 lines = **~12 lines**

```rust
let from_name: Option<String> =
    sqlx::query_scalar(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
        .bind(from_entity_id)
        .fetch_optional(pool)
        .await?;
```

**Should be:** `helpers::get_entity_name(pool, entity_id).await?`

---

### 4. Assertion Log Inserts — 9 occurrences × ~8 lines = **~72 lines**

```rust
sqlx::query(
    r#"INSERT INTO "ob-poc".ubo_assertion_log
       (cbu_id, assertion_type, expected_value, actual_value, passed)
       VALUES ($1, 'converged', true, $2, $3)"#,
)
.bind(cbu_id)
.bind(actual_value)
.bind(passed)
.execute(pool)
.await;
```

**Should be:** 
```rust
helpers::log_assertion(pool, cbu_id, "converged", expected, actual, passed).await?;
```

---

### 5. Argument Missing Errors — 32 occurrences × ~1 line = **~32 lines**

```rust
.ok_or_else(|| anyhow::anyhow!("Missing cbu argument"))?;
.ok_or_else(|| anyhow::anyhow!("Missing reason argument"))?;
.ok_or_else(|| anyhow::anyhow!("Missing edge-id argument"))?;
// ... 29 more
```

This is just the error message; the full extraction pattern is larger.

---

### 6. Similar Argument Extraction Patterns

| Arg Name | Occurrences |
|----------|-------------|
| `cbu-id` | 12 |
| `reason` | 6 |
| `percentage` | 4 |
| `type` | 2 |
| `effective-date` | 2 |
| `edge-id` | 2 |

Each extraction is 4-8 lines. Total: ~50 unique args × ~5 lines avg = **~250 lines of extraction boilerplate**

---

## What's NOT Duplicated (Legitimately Unique)

The actual SQL queries are mostly unique - different tables, different WHERE clauses, different business logic:

| Table | INSERTs | UPDATEs | SELECTs |
|-------|---------|---------|---------|
| `cbu_relationship_verification` | 3 | 8 | 3 |
| `entity_relationships` | 3 | 6 | 2 |
| `ubo_assertion_log` | 9 | 0 | 0 |
| `entities` | 0 | 0 | 7 |
| `kyc_decisions` | 1 | 1 | 1 |
| `proofs` | 1 | 1 | 0 |
| Other | 1 | 3 | 6 |

The 56 SQL statements serve different purposes - this is NOT duplication, this is a complex domain.

---

## Operations Breakdown

| Line Range | Op | Lines | Purpose |
|------------|-----|-------|---------|
| 35-245 | `UboAllegeOp` | 210 | Create alleged edge |
| 246-423 | `UboLinkProofOp` | 177 | Link document proof to edge |
| 424-522 | `UboUpdateAllegationOp` | 98 | Update alleged values |
| 523-626 | `UboRemoveEdgeOp` | 103 | Soft delete edge |
| 910-1127 | `UboVerifyOp` | 217 | Compare observations vs allegations |
| 1128-1268 | `UboStatusOp` | 140 | Get convergence status |
| 1270-1524 | `UboAssertOp` | 254 | Gate assertions for progression |
| 1525-1730 | `UboEvaluateOp` | 205 | Calculate risk/recommendations |
| 1731-1864 | `UboTraverseOp` | 133 | Walk ownership graph |
| 1865-2075 | `KycDecisionOp` | 210 | Record KYC decision |
| 2076-2175 | `UboMarkDirtyOp` | 99 | Invalidate for re-review |
| 2176-2302 | `UboScheduleReviewOp` | 126 | Schedule periodic review |
| 2303-2522 | `UboMarkDeceasedOp` | 219 | Handle death events |
| 2523-2745 | `UboConvergenceSupersedeOp` | 222 | Corporate restructure handling |
| 2746-2971 | `UboTransferControlOp` | 225 | Transfer control relationships |
| 2972-3164 | `UboWaiveVerificationOp` | 192 | Waive verification requirements |

Each operation handles a distinct lifecycle event. This is domain complexity, not bloat.

---

## Recommended Extractions

### helpers.rs additions:

```rust
/// Extract CBU ID from verb call (supports @symbol or UUID)
pub fn extract_cbu_id(verb_call: &VerbCall, ctx: &ExecutionContext) -> Result<Uuid>;

/// Get entity name by ID
pub async fn get_entity_name(pool: &PgPool, entity_id: Uuid) -> Result<Option<String>>;

/// Log assertion to audit trail
pub async fn log_assertion(
    pool: &PgPool,
    cbu_id: Uuid,
    assertion_type: &str,
    expected: serde_json::Value,
    actual: serde_json::Value,
    passed: bool,
) -> Result<()>;

/// Extract optional string argument
pub fn extract_string_opt(verb_call: &VerbCall, arg_name: &str) -> Option<String>;

/// Extract required string argument
pub fn extract_string(verb_call: &VerbCall, arg_name: &str) -> Result<String>;

/// Extract optional decimal argument  
pub fn extract_decimal_opt(verb_call: &VerbCall, arg_name: &str) -> Option<Decimal>;
```

### Potential macro for stubs:

```rust
macro_rules! stub_execute {
    (uuid) => {
        #[cfg(not(feature = "database"))]
        async fn execute(&self, _: &VerbCall, ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
            let id = uuid::Uuid::new_v4();
            ctx.bind("result", id);
            Ok(ExecutionResult::Uuid(id))
        }
    };
    (affected($n:expr)) => {
        #[cfg(not(feature = "database"))]
        async fn execute(&self, _: &VerbCall, _: &mut ExecutionContext) -> Result<ExecutionResult> {
            Ok(ExecutionResult::Affected($n))
        }
    };
}
```

---

## Verdict

| Category | Lines | % of File |
|----------|-------|-----------|
| Extractable duplication | 400-500 | 15% |
| Unique business logic | 2,600-2,700 | 85% |

**The file is large because the domain is complex, not because of sloppiness.**

The 16 operations cover the full UBO convergence lifecycle:
- Graph building (allege, link-proof, update, remove)
- Verification (verify, status)
- Gating (assert)
- Decision (evaluate, decision)
- Maintenance (mark-dirty, schedule-review)
- Lifecycle events (deceased, supersede, transfer, waive)

**Recommendation:**
1. Extract the ~400 lines of helper duplication (Phase 1 work)
2. DO NOT split this file - it's a cohesive domain module
3. The SQL queries are unique and should stay inline (they're domain logic, not infrastructure)

---

## Comparison: If Split

If we split by lifecycle phase:

| File | Ops | Est. Lines |
|------|-----|------------|
| `ubo_graph/building.rs` | 4 | ~590 |
| `ubo_graph/verification.rs` | 2 | ~360 |
| `ubo_graph/assertion.rs` | 1 | ~255 |
| `ubo_graph/evaluation.rs` | 2 | ~340 |
| `ubo_graph/decision.rs` | 3 | ~435 |
| `ubo_graph/removal.rs` | 4 | ~860 |
| `ubo_graph/helpers.rs` | - | ~120 (new) |
| `ubo_graph/mod.rs` | - | ~50 (re-exports) |

After extracting common helpers, each file would be 300-800 lines. More navigable, but requires coordination to maintain.

**Trade-off:** Easier to find things vs. harder to see full lifecycle in one place.
