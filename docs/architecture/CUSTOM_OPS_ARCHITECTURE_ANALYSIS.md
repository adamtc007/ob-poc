# Custom Operations Architecture Analysis

## Executive Summary

**Recommendation: DO NOT split into separate crates yet, but DO refactor internally.**

The 22K lines across 27 files in `custom_ops` are complex but **not tightly coupled**. However, they suffer from:
1. Code duplication (helper functions reinvented in multiple files)
2. Inconsistent patterns (some files use `super::*`, others explicit imports)
3. Missing shared abstractions
4. Large monolithic files that could be split by lifecycle phase

Splitting into crates would create more problems than it solves. Internal restructuring is the better path.

---

## Current State

### Size Distribution

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          LINES OF CODE BY FILE                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ ubo_graph_ops.rs      ████████████████████████████████████████   3,163      │
│ request_ops.rs        ████████████████████████████               1,610      │
│ onboarding.rs         ██████████████████████████                 1,533      │
│ cbu_ops.rs            ████████████████████████                   1,383      │
│ cbu_role_ops.rs       ██████████████████████                     1,302      │
│ ubo_analysis.rs       ███████████████████████                    1,140      │
│ trading_profile.rs    █████████████████████                      1,120      │
│ refdata_loader.rs     █████████████████████                      1,082      │
│ verify_ops.rs         █████████████████                            954      │
│ lifecycle_ops.rs      ████████████████                             939      │
│ custody.rs            ████████████████                             908      │
│ other (16 files)      ████████████████                           6,594      │
│ ─────────────────────────────────────────────────────────────────────────── │
│ TOTAL                                                           22,228      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Operations Count by File

| File | Op Count | Avg Lines/Op | Domain |
|------|----------|--------------|--------|
| ubo_graph_ops.rs | 16 | 198 | KYC Convergence |
| request_ops.rs | 11 | 146 | Outstanding Requests |
| ubo_analysis.rs | 8 | 143 | UBO Chain Discovery |
| cbu_role_ops.rs | 8 | 163 | Entity Relationships |
| onboarding.rs | 7 | 219 | Resource Provisioning |
| refdata_loader.rs | 5 | 216 | Reference Data |
| trading_profile.rs | 5 | 224 | Trading Mandates |
| cbu_ops.rs | 4 | 346 | CBU CRUD |
| lifecycle_ops.rs | 6 | 157 | Lifecycle Management |
| verify_ops.rs | 6 | 159 | Adversarial Verification |

---

## Coupling Analysis

### 1. Database Table Dependencies

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TABLE OWNERSHIP BY OPERATION FILE                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│ ENTITY CORE (entities, entity_*)                                            │
│ ├── entity_ops.rs          PRIMARY                                          │
│ ├── entity_query.rs        READ                                             │
│ ├── ubo_graph_ops.rs       READ/WRITE (entity_relationships)                │
│ ├── cbu_ops.rs             READ/WRITE (entity_*)                            │
│ ├── verify_ops.rs          READ                                             │
│ └── observation_ops.rs     READ                                             │
│                                                                             │
│ CBU CORE (cbus, cbu_entity_roles)                                           │
│ ├── cbu_ops.rs             PRIMARY                                          │
│ ├── cbu_role_ops.rs        WRITE (cbu_entity_roles)                         │
│ ├── kyc_case_ops.rs        READ/WRITE                                       │
│ ├── ubo_graph_ops.rs       READ                                             │
│ └── threshold.rs           READ                                             │
│                                                                             │
│ UBO/CONVERGENCE (entity_relationships, cbu_relationship_verification)      │
│ ├── ubo_graph_ops.rs       PRIMARY                                          │
│ ├── ubo_analysis.rs        READ (uses DB functions)                         │
│ ├── cbu_role_ops.rs        WRITE (entity_relationships)                     │
│ └── observation_ops.rs     READ (client_allegations)                        │
│                                                                             │
│ DOCUMENTS (document_catalog, document_types)                                │
│ ├── document_ops.rs        PRIMARY                                          │
│ ├── request_ops.rs         WRITE (uploads)                                  │
│ ├── custody.rs             READ                                             │
│ └── ubo_analysis.rs        READ                                             │
│                                                                             │
│ RESOURCES (cbu_resource_instances, service_resource_*)                      │
│ ├── resource_ops.rs        PRIMARY                                          │
│ └── onboarding.rs          WRITE (provisioning)                             │
│                                                                             │
│ TRADING (cbu_trading_profiles, trading_matrix tables)                       │
│ ├── trading_profile.rs     PRIMARY (profiles)                               │
│ └── trading_matrix.rs      PRIMARY (IM, pricing, SLA)                       │
│                                                                             │
│ LIFECYCLE (instrument_lifecycles, lifecycle_*)                              │
│ ├── lifecycle_ops.rs       PRIMARY                                          │
│ └── matrix_overlay_ops.rs  READ                                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2. External Crate Dependencies

Only 3 files have dependencies outside `dsl_v2`:

| File | External Dependency | Nature |
|------|---------------------|--------|
| semantic_ops.rs | `crate::database::derive_semantic_state` | DB function wrapper |
| semantic_ops.rs | `crate::ontology::SemanticStageRegistry` | Stage config |
| template_ops.rs | `crate::templates::*` | Template expansion |
| trading_profile.rs | `crate::trading_profile::*` | Profile management |

**All others are self-contained** - they only depend on:
- `dsl_v2::ast::VerbCall`
- `dsl_v2::executor::{ExecutionContext, ExecutionResult}`
- `super::CustomOperation`
- `sqlx::PgPool`
- Standard crates: `anyhow`, `async_trait`, `serde`, `uuid`, `chrono`

### 3. Cross-File Dependencies Within custom_ops

**None.** Each file only imports from `super::` (the trait definition in mod.rs).

This is good - they're loosely coupled at the module level.

---

## The Minefield

### Issue 1: Duplicated Helper Functions

The same extraction helpers are reimplemented in multiple files:

```
extract_uuid_arg()     →  kyc_case_ops.rs, ubo_graph_ops.rs
extract_entity_ref()   →  ubo_graph_ops.rs
resolve_cbu_id()       →  onboarding.rs, request_ops.rs
extract_uuid()         →  multiple files
extract_string_opt()   →  multiple files
```

**Risk:** Changes to argument extraction logic require updating multiple files.

### Issue 2: ubo_graph_ops.rs is 3,163 Lines

This single file contains 16 operations spanning the entire UBO convergence lifecycle:
- Graph Building (4 ops)
- Verification (2 ops)
- Assertions (1 op)
- Evaluation (2 ops)
- Decision & Review (3 ops)
- Removal (4 ops)

**Risk:** Any refactoring of UBO convergence touches a massive file.

### Issue 3: Inconsistent Pattern Implementation

Some operations follow a clean pattern:
```rust
impl CustomOperation for SomeOp {
    fn domain() -> &'static str { "..." }
    fn verb() -> &'static str { "..." }
    fn rationale() -> &'static str { "..." }
    async fn execute(&self, ...) -> Result<ExecutionResult>
}
```

Others have:
- Private helper functions scattered through the impl
- Inline SQL strings vs extracted queries
- Different approaches to UUID resolution

### Issue 4: Test Isolation

The `#[cfg(not(feature = "database"))]` pattern for non-DB testing is inconsistent.
Some files have it, others don't.

---

## Recommendations

### DO NOT: Split into Separate Crates

**Why not?**
1. All ops share the same trait (`CustomOperation`)
2. All ops need the same dependencies (`sqlx`, `anyhow`, etc.)
3. Zero cross-file coupling means nothing gained from crate boundaries
4. Crate splits add compile-time overhead and cargo complexity
5. The linker already handles dead code elimination

### DO: Internal Restructuring

#### Phase 1: Extract Shared Helpers (~1 day)

Create `helpers.rs` with common extraction logic:

```rust
// src/dsl_v2/custom_ops/helpers.rs

pub fn extract_uuid_arg(verb_call: &VerbCall, arg_name: &str, ctx: &ExecutionContext) -> Result<Uuid>;
pub fn extract_string_opt(verb_call: &VerbCall, arg_name: &str) -> Option<String>;
pub fn extract_decimal_opt(verb_call: &VerbCall, arg_name: &str) -> Option<Decimal>;
pub async fn resolve_entity_ref(verb_call: &VerbCall, arg_name: &str, ctx: &ExecutionContext, pool: &PgPool) -> Result<Uuid>;
pub async fn resolve_cbu_id(verb_call: &VerbCall, ctx: &ExecutionContext, pool: &PgPool) -> Result<Uuid>;
```

**Impact:** Removes ~500 lines of duplication.

#### Phase 2: Split ubo_graph_ops.rs (~2 days)

Split the 3,163-line file by lifecycle phase:

```
custom_ops/
├── ubo_graph/
│   ├── mod.rs              # re-exports
│   ├── building.rs         # UboAllegeOp, UboLinkProofOp, UboUpdateAllegationOp, UboRemoveEdgeOp
│   ├── verification.rs     # UboVerifyOp, UboStatusOp
│   ├── assertion.rs        # UboAssertOp
│   ├── evaluation.rs       # UboEvaluateOp, UboTraverseOp
│   ├── decision.rs         # KycDecisionOp, UboMarkDirtyOp, UboScheduleReviewOp
│   └── removal.rs          # UboMarkDeceasedOp, UboConvergenceSupersedeOp, ...
```

**Impact:** ~500 lines per file, single responsibility.

#### Phase 3: Standardize Patterns (~1 day)

1. Consistent SQL query extraction (move complex SQL to `const` strings or separate queries module)
2. Consistent test patterns (`#[cfg(not(feature = "database"))]` everywhere or nowhere)
3. Consistent error handling (use `anyhow::Context` consistently)

#### Phase 4: Consider Feature Flags (optional)

If certain domain ops are truly optional:

```toml
[features]
default = ["trading", "ubo", "custody"]
trading = []      # trading_profile.rs, trading_matrix.rs
ubo = []          # ubo_*.rs
custody = []      # custody.rs
screening = []    # screening_ops.rs (external API)
```

**Impact:** Faster compile times for focused development.

---

## Crate Split Viability Assessment

IF you were to split into crates (not recommended), here's how it would look:

| Potential Crate | Files | Lines | External Deps | Viability |
|-----------------|-------|-------|---------------|-----------|
| `ob-ops-core` | helpers, entity_ops, document_ops | ~800 | None | ✓ Clean |
| `ob-ops-cbu` | cbu_ops, cbu_role_ops | ~2,700 | None | ✓ Clean |
| `ob-ops-ubo` | ubo_*.rs | ~4,300 | None | ✓ But large |
| `ob-ops-trading` | trading_*.rs | ~1,600 | crate::trading_profile | ⚠ External dep |
| `ob-ops-onboarding` | onboarding.rs, resource_ops | ~2,200 | None | ✓ Clean |
| `ob-ops-request` | request_ops, rfi, kyc_case_ops | ~2,400 | None | ✓ Clean |
| `ob-ops-lifecycle` | lifecycle_ops, matrix_overlay | ~1,400 | None | ✓ Clean |
| `ob-ops-verify` | verify_ops, threshold, observation | ~2,200 | None | ✓ Clean |
| `ob-ops-refdata` | refdata_loader | ~1,100 | None | ✓ Clean |
| `ob-ops-screening` | screening_ops | ~300 | External APIs | ⚠ Thin |
| `ob-ops-semantic` | semantic_ops | ~500 | crate::ontology | ⚠ External dep |

**Total: 11 crates** - This is excessive and adds maintenance burden.

---

## Action Plan

### Immediate (Do This Week)

1. **Extract helpers.rs** - Consolidate argument extraction
2. **Add module-level documentation** - Each file should document its domain

### Short-term (Next Sprint)

3. **Split ubo_graph_ops.rs** - By lifecycle phase
4. **Standardize patterns** - Consistent SQL, errors, tests

### Long-term (If Needed)

5. **Feature flags** - Only if compile times become painful
6. **Crate splits** - Only if team scales and ownership boundaries emerge

---

## Appendix: Full Operation Inventory

### UBO Domain (24 ops)

| File | Operations |
|------|------------|
| ubo_graph_ops.rs | UboAllegeOp, UboLinkProofOp, UboUpdateAllegationOp, UboRemoveEdgeOp, UboVerifyOp, UboStatusOp, UboAssertOp, UboEvaluateOp, UboTraverseOp, KycDecisionOp, UboMarkDirtyOp, UboScheduleReviewOp, UboMarkDeceasedOp, UboConvergenceSupersedeOp, UboTransferControlOp, UboWaiveVerificationOp |
| ubo_analysis.rs | UboCalculateOp, UboDiscoverOwnerOp, UboTraceChainsOp, UboInferChainOp, UboCheckCompletenessOp, UboSupersedeOp, UboSnapshotCbuOp, UboCompareSnapshotOp |

### CBU Domain (12 ops)

| File | Operations |
|------|------------|
| cbu_ops.rs | CbuAddProductOp, CbuShowOp, CbuDecideOp, CbuDeleteCascadeOp |
| cbu_role_ops.rs | CbuRoleAssignOp, CbuRoleAssignOwnershipOp, CbuRoleAssignControlOp, CbuRoleAssignTrustOp, CbuRoleAssignFundOp, CbuRoleAssignServiceOp, CbuRoleAssignSignatoryOp, CbuRoleValidateAllOp |

### Request Domain (11 ops)

| File | Operations |
|------|------------|
| request_ops.rs | RequestCreateOp, RequestOverdueOp, RequestFulfillOp, RequestCancelOp, RequestExtendOp, RequestRemindOp, RequestEscalateOp, RequestWaiveOp, DocumentRequestOp, DocumentUploadOp, DocumentWaiveOp |

### Onboarding Domain (7 ops)

| File | Operations |
|------|------------|
| onboarding.rs | OnboardingPlanOp, OnboardingShowPlanOp, OnboardingExecuteOp, OnboardingStatusOp, OnboardingGetUrlsOp, OnboardingEnsureOp, OnboardingAutoCompleteOp |

### Trading Domain (8 ops)

| File | Operations |
|------|------------|
| trading_profile.rs | TradingProfileImportOp, TradingProfileGetActiveOp, TradingProfileActivateOp, TradingProfileMaterializeOp, TradingProfileValidateOp |
| trading_matrix.rs | FindImForTradeOp, FindPricingForInstrumentOp, ListOpenSlaBreachesOp |

### Verification Domain (9 ops)

| File | Operations |
|------|------------|
| verify_ops.rs | VerifyDetectPatternsOp, VerifyDetectEvasionOp, VerifyCalculateConfidenceOp, VerifyGetStatusOp, VerifyAgainstRegistryOp, VerifyAssertOp |
| threshold.rs | ThresholdDeriveOp, ThresholdEvaluateOp, ThresholdCheckEntityOp |

### Other Domains

| Domain | File | Op Count |
|--------|------|----------|
| Lifecycle | lifecycle_ops.rs | 6 |
| Resources | resource_ops.rs | 6 |
| Refdata | refdata_loader.rs | 5 |
| Observation | observation_ops.rs | 4 |
| Custody | custody.rs | 5 |
| Screening | screening_ops.rs | 3 |
| RFI | rfi.rs | 3 |
| Semantic | semantic_ops.rs | 6 |
| Matrix Overlay | matrix_overlay_ops.rs | 3 |
| Documents | document_ops.rs | 2 |
| Entity | entity_ops.rs | 1 |
| Entity Query | entity_query.rs | 1 |
| Template | template_ops.rs | 2 |
| Batch Control | batch_control_ops.rs | 7 |
| KYC Case | kyc_case_ops.rs | 2 |
| Regulatory | regulatory_ops.rs | 2 |

**TOTAL: ~100 operations** across 27 files.

---

## Conclusion

The custom_ops module is complex but well-structured at the macro level. The individual files are loosely coupled (good), but suffer from internal duplication (fixable). The right path is internal refactoring, not crate explosion.

**Bottom line:** Clean up the internals, split the giant files, extract shared helpers. Don't create new crates unless team/ownership boundaries demand it.
