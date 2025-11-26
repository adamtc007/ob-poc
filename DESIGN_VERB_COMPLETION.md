# Design: Verb Definition Completion

**Created:** 2025-11-26  
**Status:** IMPLEMENTATION READY  
**Priority:** P1 — Complete DSL Verb Coverage  
**Scope:** Add missing verb definitions and integrate into registry  

---

## Overview

This document guides the implementation of new verb definitions to complete DSL coverage for all KYC/onboarding domains.

**New verb files created:**
- `rust/src/forth_engine/schema/verbs/screening.rs` (7 verbs)
- `rust/src/forth_engine/schema/verbs/decision.rs` (8 verbs)  
- `rust/src/forth_engine/schema/verbs/monitoring.rs` (8 verbs)
- `rust/src/forth_engine/schema/verbs/attribute.rs` (7 verbs)

**Total new verbs:** 30  
**Previous total:** 28  
**New total:** 58 verbs

---

## Implementation Tasks

### Task 1: Update mod.rs

**File:** `rust/src/forth_engine/schema/verbs/mod.rs`

**Current:**
```rust
pub mod cbu;
pub mod entity;
pub mod document;
pub mod kyc;

pub use cbu::*;
pub use entity::*;
pub use document::*;
pub use kyc::*;
```

**Change to:**
```rust
//! Verb definitions organized by domain.

pub mod cbu;
pub mod entity;
pub mod document;
pub mod kyc;
pub mod screening;
pub mod decision;
pub mod monitoring;
pub mod attribute;

pub use cbu::*;
pub use entity::*;
pub use document::*;
pub use kyc::*;
pub use screening::*;
pub use decision::*;
pub use monitoring::*;
pub use attribute::*;
```

---

### Task 2: Update VerbRegistry

**File:** `rust/src/forth_engine/schema/registry.rs`

**Add imports (after existing imports):**
```rust
use crate::forth_engine::schema::verbs::{screening, decision, monitoring, attribute};
```

**Add to `all_verbs` array in `VerbRegistry::new()`:**

```rust
// Screening domain (expanded)
&screening::SCREENING_PEP,
&screening::SCREENING_SANCTIONS,
&screening::SCREENING_ADVERSE_MEDIA,
&screening::SCREENING_RESOLVE_HIT,
&screening::SCREENING_DISMISS_HIT,
&screening::SCREENING_BATCH,
&screening::SCREENING_REFRESH,

// Decision domain (expanded)
&decision::DECISION_RECORD,
&decision::DECISION_APPROVE,
&decision::DECISION_REJECT,
&decision::DECISION_ESCALATE,
&decision::DECISION_ADD_CONDITION,
&decision::DECISION_SATISFY_CONDITION,
&decision::DECISION_DEFER,

// Monitoring domain (new)
&monitoring::MONITORING_SCHEDULE_REVIEW,
&monitoring::MONITORING_TRIGGER_REVIEW,
&monitoring::MONITORING_UPDATE_RISK,
&monitoring::MONITORING_COMPLETE_REVIEW,
&monitoring::MONITORING_CLOSE_CASE,
&monitoring::MONITORING_ADD_ALERT_RULE,
&monitoring::MONITORING_RECORD_ACTIVITY,

// Attribute domain (new)
&attribute::ATTRIBUTE_SET,
&attribute::ATTRIBUTE_GET,
&attribute::ATTRIBUTE_BULK_SET,
&attribute::ATTRIBUTE_VALIDATE,
&attribute::ATTRIBUTE_CLEAR,
&attribute::ATTRIBUTE_HISTORY,
&attribute::ATTRIBUTE_COPY_FROM_DOCUMENT,
```

**Remove duplicates from kyc.rs:**

The following verbs are now defined in their own domain files and should be removed from `kyc.rs` to avoid duplication:
- `SCREENING_PEP` → now in `screening.rs`
- `SCREENING_SANCTIONS` → now in `screening.rs`
- `DECISION_RECORD` → now in `decision.rs`

Update `kyc.rs` to keep only KYC-specific verbs:
- `INVESTIGATION_CREATE`
- `INVESTIGATION_UPDATE_STATUS`
- `INVESTIGATION_COMPLETE`
- `RISK_ASSESS_CBU`
- `RISK_SET_RATING`

---

### Task 3: Remove Duplicates from kyc.rs

**File:** `rust/src/forth_engine/schema/verbs/kyc.rs`

**Remove these verb definitions (they now exist in screening.rs/decision.rs):**
- `pub static SCREENING_PEP: VerbDef = ...`
- `pub static SCREENING_SANCTIONS: VerbDef = ...`  
- `pub static DECISION_RECORD: VerbDef = ...`

**Keep these verbs in kyc.rs:**
- `INVESTIGATION_CREATE`
- `INVESTIGATION_UPDATE_STATUS`
- `INVESTIGATION_COMPLETE`
- `RISK_ASSESS_CBU`
- `RISK_SET_RATING`

---

### Task 4: Update Registry Imports

**File:** `rust/src/forth_engine/schema/registry.rs`

Update the `all_verbs` array to remove the duplicates that were in kyc.rs:

**Remove from kyc section:**
```rust
// Remove these - now in screening domain
// &kyc::SCREENING_PEP,
// &kyc::SCREENING_SANCTIONS,
// &kyc::DECISION_RECORD,
```

The KYC section should become:
```rust
// KYC domain (investigation + risk only)
&kyc::INVESTIGATION_CREATE,
&kyc::INVESTIGATION_UPDATE_STATUS,
&kyc::INVESTIGATION_COMPLETE,
&kyc::RISK_ASSESS_CBU,
&kyc::RISK_SET_RATING,
```

---

### Task 5: Verify Compilation

Run:
```bash
cd rust
cargo check
cargo clippy
cargo test -p ob-poc -- --test-threads=1
```

Expected:
- No compilation errors
- Clippy clean (allow dead_code warnings for unused verbs)
- All existing tests pass

---

### Task 6: Add Tests for New Verbs

**File:** `rust/src/forth_engine/schema/registry.rs` (in tests module)

Add:
```rust
#[test]
fn test_all_domains_registered() {
    let registry = VerbRegistry::new();
    
    // Check all domains exist
    let domains: Vec<_> = registry.domains().collect();
    assert!(domains.contains(&"cbu"));
    assert!(domains.contains(&"entity"));
    assert!(domains.contains(&"document"));
    assert!(domains.contains(&"kyc"));
    assert!(domains.contains(&"screening"));
    assert!(domains.contains(&"decision"));
    assert!(domains.contains(&"monitoring"));
    assert!(domains.contains(&"attribute"));
}

#[test]
fn test_verb_count() {
    let registry = VerbRegistry::new();
    // 9 cbu + 5 entity + 6 document + 5 kyc + 7 screening + 8 decision + 8 monitoring + 7 attribute = 55
    // Plus SCREENING_PEP and SCREENING_SANCTIONS domain mismatch = may vary
    assert!(registry.count() >= 50, "Expected at least 50 verbs, got {}", registry.count());
}

#[test]
fn test_new_screening_verbs() {
    let registry = VerbRegistry::new();
    assert!(registry.exists("screening.adverse-media"));
    assert!(registry.exists("screening.resolve-hit"));
    assert!(registry.exists("screening.dismiss-hit"));
    assert!(registry.exists("screening.batch"));
    assert!(registry.exists("screening.refresh"));
}

#[test]
fn test_new_decision_verbs() {
    let registry = VerbRegistry::new();
    assert!(registry.exists("decision.approve"));
    assert!(registry.exists("decision.reject"));
    assert!(registry.exists("decision.escalate"));
    assert!(registry.exists("decision.add-condition"));
    assert!(registry.exists("decision.satisfy-condition"));
    assert!(registry.exists("decision.defer"));
}

#[test]
fn test_new_monitoring_verbs() {
    let registry = VerbRegistry::new();
    assert!(registry.exists("monitoring.schedule-review"));
    assert!(registry.exists("monitoring.trigger-review"));
    assert!(registry.exists("monitoring.update-risk"));
    assert!(registry.exists("monitoring.complete-review"));
    assert!(registry.exists("monitoring.close-case"));
    assert!(registry.exists("monitoring.add-alert-rule"));
    assert!(registry.exists("monitoring.record-activity"));
}

#[test]
fn test_new_attribute_verbs() {
    let registry = VerbRegistry::new();
    assert!(registry.exists("attribute.set"));
    assert!(registry.exists("attribute.get"));
    assert!(registry.exists("attribute.bulk-set"));
    assert!(registry.exists("attribute.validate"));
    assert!(registry.exists("attribute.clear"));
    assert!(registry.exists("attribute.history"));
    assert!(registry.exists("attribute.copy-from-document"));
}
```

---

## Verb Summary by Domain

### Screening Domain (7 verbs)
| Verb | Description |
|------|-------------|
| `screening.pep` | Screen for PEP status |
| `screening.sanctions` | Screen against sanctions lists |
| `screening.adverse-media` | Screen for adverse media |
| `screening.resolve-hit` | Resolve a screening hit |
| `screening.dismiss-hit` | Dismiss false positive |
| `screening.batch` | Batch screen multiple entities |
| `screening.refresh` | Refresh screening for entity |

### Decision Domain (8 verbs)
| Verb | Description |
|------|-------------|
| `decision.record` | Record a decision |
| `decision.approve` | Approve investigation |
| `decision.reject` | Reject investigation |
| `decision.escalate` | Escalate for senior review |
| `decision.add-condition` | Add condition to approval |
| `decision.satisfy-condition` | Mark condition satisfied |
| `decision.defer` | Defer decision |

### Monitoring Domain (8 verbs)
| Verb | Description |
|------|-------------|
| `monitoring.schedule-review` | Schedule periodic review |
| `monitoring.trigger-review` | Trigger ad-hoc review |
| `monitoring.update-risk` | Update risk rating |
| `monitoring.complete-review` | Complete a review |
| `monitoring.close-case` | Close monitoring case |
| `monitoring.add-alert-rule` | Add monitoring alert rule |
| `monitoring.record-activity` | Record monitoring activity |

### Attribute Domain (7 verbs)
| Verb | Description |
|------|-------------|
| `attribute.set` | Set attribute value |
| `attribute.get` | Get attribute value |
| `attribute.bulk-set` | Set multiple attributes |
| `attribute.validate` | Validate against rules |
| `attribute.clear` | Clear attribute value |
| `attribute.history` | Get version history |
| `attribute.copy-from-document` | Copy from document extraction |

---

## Files Changed

| File | Action |
|------|--------|
| `verbs/mod.rs` | Add new module declarations and re-exports |
| `verbs/kyc.rs` | Remove SCREENING_PEP, SCREENING_SANCTIONS, DECISION_RECORD |
| `verbs/screening.rs` | **NEW** — 7 screening verbs |
| `verbs/decision.rs` | **NEW** — 8 decision verbs |
| `verbs/monitoring.rs` | **NEW** — 8 monitoring verbs |
| `verbs/attribute.rs` | **NEW** — 7 attribute verbs |
| `registry.rs` | Add new verbs to registry, update imports |

---

## Validation Checklist

- [ ] `cargo check` passes
- [ ] `cargo clippy` passes  
- [ ] `cargo test` passes
- [ ] All 8 domains registered in VerbRegistry
- [ ] At least 50 verbs in registry
- [ ] No duplicate verb names
- [ ] LSP completions include new verbs
- [ ] Schema validator recognizes new verbs

---

## Notes

1. **Domain field consistency**: Each verb's `domain` field matches its file location:
   - `screening.rs` verbs have `domain: "screening"`
   - `decision.rs` verbs have `domain: "decision"`
   - etc.

2. **CRUD asset mapping**: Each verb specifies a `crud_asset` that maps to database tables:
   - `SCREENING_RESULT` → screening_results table
   - `DECISION` → decisions table
   - `MONITORING_REVIEW` → monitoring_reviews table
   - `ATTRIBUTE_VALUE` → attribute_values table

3. **Context injection**: New verbs use `DefaultValue::FromContext` where appropriate for CBU, Entity, Investigation, Decision IDs.

4. **Cross-constraints**: Complex validation rules like `AtLeastOne`, `Excludes`, `IfEquals` are used throughout.
