# Phase 0a — Ownership Matrix

> **Scope:** `rust/src/domain_ops/` — 89 files (84 op-bearing + 5 shared utilities / mod root).
> **Produced:** 2026-04-18.
> **Granularity:** one row per file. Per-op breakdown deferred to Phase 6 (round-trip per op).
> **Related:**
> - `docs/todo/three-plane-architecture-v0.3.md` — destination spec (authoritative)
> - `docs/todo/three-plane-architecture-implementation-plan-v0.1.md` — implementation plan (decisions §10)
> - Decision **D8** — A1 violation blocks Phase 5. The **A1** column is a hard gate, not a data point.
> - Decision **D7** — alternate step-executors consolidate at Phase 5b.

---

## 1. Executive summary

### 1.1 Headline numbers

- **89 files** total in `rust/src/domain_ops/` (84 op-bearing + 5 utility/mod).
- **~650 `CustomOperation` impls** observed (aligns with v0.3's "625 ops" baseline; minor count drift from re-registrations).
- **8 A1 concerns** surfaced — D8 blockers for Phase 5 as currently designed. See §3 for detail.
- **File-level destination distribution:**
  - `metadata` — 14 files (~16%)
  - `dsl-runtime` — 65 files (~73%, includes 5 utilities)
  - `ob-poc-adapter` — 12 files (~13%)

### 1.2 Distribution vs. v0.3 prediction

v0.3 §10.6 predicted ~60% metadata / ~10–15% dsl-runtime / ~25–30% ob-poc-adapter **at the op level**. This matrix is at the **file level**, where per-file "dominant behaviour" classification inflates the `dsl-runtime` bucket. Many files flagged as `dsl-runtime` here contain a mix of CRUD-dissolvable and plugin ops. Per-op breakdown is a Phase 6 activity, not Phase 0a.

**Read this matrix as:** "where does this file start its migration?" Not "where does every op in this file end up?" The final op distribution after Phase 6 round-trip will shift several percent from `dsl-runtime` to `metadata` as mixed-behaviour files release their CRUD portions to YAML.

### 1.3 Gate status — ready for review

- ✅ **Every file classified** across 10 columns.
- ⚠️ **A1 column flags 8 blockers** — details in §3. Phase 5 cannot proceed until each is resolved (per D8).
- ✅ **All files reachable via `VerbExecutionPortStepExecutor`** today (no `DslStepExecutor` / `DslExecutorV2StepExecutor` bypass patterns found in the audit sample). D7 consolidation path is clear.
- ⏳ **Blocker traits enumerated** in §6. Phase 5a will extract these.

---

## 2. Column legend

| Column | Meaning |
|---|---|
| **file** | filename (no path) |
| **ops** | count of `impl CustomOperation for` blocks |
| **internals** | `crate::` imports outside `crate::domain_ops::` (what app internals this file reaches) |
| **behav** | `crud` / `plugin` / `template` / `mixed` / `utility` |
| **dest** | `metadata` / `dsl-runtime` / `ob-poc-adapter` |
| **blockers** | traits required before file can move |
| **diff** | `easy` / `medium` / `hard` |
| **round-trip** | Phase 6 dissolution candidate: `yes` / `no` / `partial` / `n/a` |
| **A1** | D8 gate: `yes` (no external effects in inner txn) / `no` (has them) / `unclear` (transitive dep ambiguous) |
| **dispatch** | current step-executor path (all rows: `VerbExecutionPortStepExecutor` — abbreviated `VEP`) |

---

## 3. A1 concerns (D8 blockers for Phase 5)

**Count: 8 files.** Each must be individually resolved before Phase 5 begins. Resolution options (in order of preference):

1. **Confirm external effect happens *outside* the inner txn** (e.g. fetch data, then open txn, then write). If confirmed, A1 reclassifies to `yes` and the op is safe.
2. **Refactor the op to defer external effects via outbox** (A1 violation becomes outbox-backed `OutboxDraft`).
3. **Escalate to a separate pre-Phase-5 redesign plan** (per D8 — no 2PC saga escape hatch).

| # | file | nature of A1 concern | resolution path (tentative) |
|---|---|---|---|
| 1 | `sem_os_maintenance_ops.rs` | **Subprocess spawn** — calls `cargo run --release reindex-embeddings` | Certain A1 violation inside any txn. Options: (a) convert to outbox-backed task — emit `OutboxDraft { effect_kind: MaintenanceSpawn }` drained by a dedicated worker, or (b) mark this file as *not permitted in user-runbook execution* (invoked only from admin paths outside `VerbExecutionPort`). |
| 2 | `source_loader_ops.rs` | External HTTP to Companies House / GLEIF / SEC EDGAR | Likely pattern: fetch → txn → write. Confirm fetch happens before txn open. If so, A1 = `yes`. If fetch is called inside the op body during txn, refactor to two-phase (pre-fetch → cached result → op). |
| 3 | `gleif_ops.rs` | External HTTP to GLEIF API | Same pattern as #2. High probability that fetch is pre-txn in current implementation (17 ops, likely each does fetch-then-persist). Confirm and reclassify. |
| 4 | `bpmn_lite_ops.rs` | External gRPC to BPMN-lite service | 5 ops sending jobs to the standalone BPMN server. If jobs are sent inside the txn, A1 = `no`. If fire-and-forget via correlation_id with gRPC call outside txn, A1 = `yes`. Needs inspection. |
| 5 | `sem_os_audit_ops.rs` | External MCP tool dispatch | 8 ops. If MCP tools are registered as local in-process calls, A1 = `yes`. If they cross a process boundary, A1 = `no`. Inspect tool definitions. |
| 6 | `sem_os_focus_ops.rs` | Stewardship Phase 1 delegation | Likely A1 = `yes` (stewardship is in-process). Agent flagged `no` out of caution because of transitive delegation depth. Confirm with targeted read of `stew_*` call chain. |
| 7 | `sem_os_governance_ops.rs` | Stewardship delegation (gate, submit, record, publish) | Same shape as #6. Publish path may emit SemReg outbox events *inside* the txn — if so, that is **allowed** (SemReg outbox is in-txn, per v0.3 §10.7). Confirm. |
| 8 | `sem_os_changeset_ops.rs` | Stewardship delegation (compose, add_item) | Same shape as #6/#7. Likely A1 = `yes`. |

**Additional uncertain entry:** `booking_principal_ops.rs` was flagged `unclear` by Batch 4 agent pending inspection of `rule_evaluator` for side effects. Treat as a 9th pending item.

### 3.1 Headline assessment for D8

Of the 8 flagged:

- **1 is certainly external** (sem_os_maintenance_ops subprocess spawn) — will require outbox deferral or path-level exclusion. This is a design change, not an audit clarification.
- **3 are very likely in-process but flagged from stewardship depth** (sem_os_focus, sem_os_governance, sem_os_changeset) — probably resolve to A1 = `yes` after ~half-day of targeted reading.
- **4 involve genuine external services** (source_loader, gleif, bpmn_lite, sem_os_audit) — resolution depends on whether external I/O is pre-txn or in-txn. Most likely pre-txn for the HTTP loaders (GLEIF, Companies House, SEC EDGAR); bpmn_lite and sem_os_audit need inspection.

**Probable outcome after §3 follow-up:** 1–2 files require design change; the rest reclassify to `yes` with tighter evidence.

Recommended next step (before any Phase 1 work): spend half a day confirming the 8 A1 rows. This shrinks D8's blocker surface from 8 to a small number (likely 1) and lets Phase 0 gate review proceed.

---

## 4. Full matrix

All rows use `VEP` for dispatch (abbreviation of `VerbExecutionPortStepExecutor`).

### 4.1 SemOS + infrastructure (Batch 1)

| file | ops | internals | behav | dest | blockers | diff | round-trip | A1 | dispatch |
|---|---|---|---|---|---|---|---|---|---|
| `sem_os_schema_ops.rs` | 13 | `dsl_v2::verb_registry`, `ontology`, `sem_os_core` | mixed | metadata | affinity/diagram verb_contract | hard | no (read-only introspection) | yes | VEP |
| `sem_os_focus_ops.rs` | 6 | stewardship `stew_*` macros | plugin | dsl-runtime | Phase-1 stewardship tool scope | medium | yes | **no** | VEP |
| `sem_os_audit_ops.rs` | 8 | sem_reg MCP `audit_op!` macros | plugin | dsl-runtime | external MCP tool contract | medium | partial | **no** | VEP |
| `sem_os_registry_ops.rs` | 20+ | `active_snapshot_set` SQL, polymorphic routing | mixed | ob-poc-adapter | RegistryDescribe routing | hard | partial | yes | VEP |
| `sem_os_governance_ops.rs` | 9 | stewardship `stew_gate/submit/record/publish` | mixed | dsl-runtime | stewardship scope, review FSM | hard | partial | **no** | VEP |
| `sem_os_changeset_ops.rs` | 14+ | stewardship `stew_compose/add_item` | plugin | dsl-runtime | stewardship changeset API | medium | partial | **no** | VEP |
| `sem_os_maintenance_ops.rs` | 7 | direct SQL + cargo subprocess spawn | mixed | ob-poc-adapter | subprocess exec (reindex-embeddings) | hard | no | **no** | VEP |
| `view_ops.rs` | 13 | `TaxonomyBuilder`, `TaxonomyContext`, `ViewState` | plugin | dsl-runtime | `TaxonomyAccess` | hard | no | yes | VEP |
| `session_ops.rs` | 22 | `UnifiedSession`, `StructureType`, DB reads | plugin | dsl-runtime | session lifecycle, context mutations | hard | no | yes | VEP |
| `navigation_ops.rs` | 7 | `NavResult`, viewport state | plugin | dsl-runtime | boundary crossing, history FSM | medium | no | yes | VEP |
| `source_loader_ops.rs` | 16 | `CompaniesHouseLoader`, `GleifLoader`, `SecEdgarLoader` | plugin | dsl-runtime | external-API loaders | hard | no | **no** | VEP |
| `constellation_ops.rs` | 2 | constellation reducers, DB ops | plugin | dsl-runtime | slot traversal, hydration logic | medium | no | yes | VEP |
| `semantic_ops.rs` | 6 | `SemanticStageRegistry`, `derive_semantic_state` | plugin | dsl-runtime | stage derivation | medium | no | yes | VEP |
| `shared_atom_ops.rs` | 9 | `cross_workspace::repository`, fact_refs/versions | plugin | dsl-runtime | lifecycle FSM, cross-workspace replay | hard | no | yes | VEP |
| `state_ops.rs` | 9 | `state_reducer` handlers, builtin FSMs | plugin | dsl-runtime | `StateReducer` | medium | no | yes | VEP |
| `phrase_ops.rs` | 19+ | watermark scanning, embedding similarity, FSM | plugin | dsl-runtime | complex watermark/collision logic | hard | no | yes | VEP |
| `agent_ops.rs` | 13 | agent lifecycle, checkpoint FSM, DB ops | plugin | dsl-runtime | checkpoint serialisation | medium | no | yes | VEP |
| `discovery_ops.rs` | 18+ | `DiscoveryExecutor`, insight builders, DB ops | plugin | dsl-runtime | multi-step insight pipeline | hard | no | yes | VEP |

### 4.2 CBU / Entity / Ownership (Batch 2)

| file | ops | internals | behav | dest | blockers | diff | round-trip | A1 | dispatch |
|---|---|---|---|---|---|---|---|---|---|
| `cbu_ops.rs` | 9 | `cbu_entity_roles`, `entity_relationships`, `roles` | crud | metadata | — | easy | yes | yes | VEP |
| `cbu_role_ops.rs` | 7 | `cbu_entity_roles`, `entity_relationships`, `roles` | crud | metadata | — | easy | yes | yes | VEP |
| `client_group_ops.rs` | 24 | `client_groups`, `entities`, cbu-related | mixed | ob-poc-adapter | `ClientGroupService` | medium | partial | yes | VEP |
| `entity_ops.rs` | 6 | `entities`, `entity_types`, placeholder service | plugin | dsl-runtime | `PlaceholderResolver` | medium | no | yes | VEP |
| `entity_query.rs` | 1 | entity tables (query only) | crud | metadata | — | easy | yes | yes | VEP |
| `attribute_ops.rs` | 16 | `attribute_registry`, `document_types`, sem_reg services | plugin | dsl-runtime | `AttributeIdentityService`, `SnapshotStore` | hard | no | yes | VEP |
| `edge_ops.rs` | 1 | `entity_relationships` only | crud | metadata | — | easy | yes | yes | VEP |
| `ownership_ops.rs` | 8 | `ownership_snapshots`, `entity_relationships`, SQL fns | mixed | dsl-runtime | `fn_derive_ownership_snapshots`, `fn_holder_control_position` | medium | partial | yes | VEP |
| `ubo_analysis.rs` | 3 | entity graph traversal | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `ubo_compute_ops.rs` | 3 | `ubo_determination_runs`, `entity_workstreams` | plugin | dsl-runtime | — | hard | no | yes | VEP |
| `ubo_graph_ops.rs` | 4 | `entity_relationships`, UBO state machine | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `ubo_registry_ops.rs` | 5 | `ubo_registry`, state machine validation | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `manco_ops.rs` | 11 | `manco_group` types, SQL fns | plugin | dsl-runtime | `fn_bridge_manco_role_to_board_rights` | medium | partial | yes | VEP |
| `gleif_ops.rs` | 17 | `GleifClient`, `GleifEnrichmentService` | plugin | ob-poc-adapter | `GleifClient`, `GleifEnrichmentService` | hard | no | **no** | VEP |
| `control_ops.rs` | 11 | `entity_relationships`, `cbu_entity_roles` (control traversal) | plugin | dsl-runtime | — | hard | no | yes | VEP |
| `control_compute_ops.rs` | 1 | `entity_workstreams`, `entity_relationships`, `roles` | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `dilution_ops.rs` | 8 | `dilution_instruments`, `share_classes`, SQL fns | plugin | dsl-runtime | `fn_diluted_supply_at` | medium | partial | yes | VEP |
| `trust_ops.rs` | 3 | `trust_parties`, `trust_provisions` | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `partnership_ops.rs` | 4 | `partnership_capital`, capital accounts | plugin | dsl-runtime | — | medium | partial | yes | VEP |
| `board_ops.rs` | 1 | `board_compositions`, `roles` (majority analysis) | plugin | dsl-runtime | — | medium | no | yes | VEP |

### 4.3 Workflow: KYC / Screening / Documents / Onboarding (Batch 3)

| file | ops | internals | behav | dest | blockers | diff | round-trip | A1 | dispatch |
|---|---|---|---|---|---|---|---|---|---|
| `kyc_case_ops.rs` | 5 | `ontology` | plugin | dsl-runtime | — | medium | yes | yes | VEP |
| `screening_ops.rs` | 4 | — | crud | metadata | — | easy | yes | yes | VEP |
| `remediation_ops.rs` | 4 | `cross_workspace::remediation` | plugin | dsl-runtime | — | medium | partial | yes | VEP |
| `document_ops.rs` | 9 | `database::GovernedDocumentRequirementsService` | mixed | dsl-runtime | `DocumentRequirementsService` | medium | yes | yes | VEP |
| `requirement_ops.rs` | 2 | — | crud | metadata | — | easy | yes | yes | VEP |
| `evidence_ops.rs` | 10 | — | crud | metadata | — | easy | yes | yes | VEP |
| `request_ops.rs` | 11 | — | crud | metadata | — | easy | yes | yes | VEP |
| `outreach_ops.rs` | 2 | — | crud | metadata | — | easy | yes | yes | VEP |
| `outreach_plan_ops.rs` | 1 | — | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `onboarding.rs` | 1 | `database::derive_semantic_state`, `SemanticStageRegistry`, `DslExecutor` | template | dsl-runtime | `SemanticStageRegistry` | hard | partial | yes | VEP |
| `skeleton_build_ops.rs` | 1 | orchestration modules | template | dsl-runtime | all sub-op modules | hard | no | yes | VEP |
| `tollgate_ops.rs` | 4 | `database::GovernedDocumentRequirementsService` | plugin | dsl-runtime | `DocumentRequirementsService` | medium | partial | yes | VEP |
| `tollgate_evaluate_ops.rs` | 1 | — | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `lifecycle_ops.rs` | 12 | — | crud | metadata | — | easy | yes | yes | VEP |
| `verify_ops.rs` | 6 | `verification::{ConfidenceCalculator, …}` | plugin | dsl-runtime | `ConfidenceCalculator` | medium | partial | yes | VEP |
| `access_review_ops.rs` | 8 | — | plugin | dsl-runtime | — | medium | partial | yes | VEP |
| `regulatory_ops.rs` | 2 | — | crud | metadata | — | easy | yes | yes | VEP |
| `coverage_compute_ops.rs` | 1 | — | plugin | dsl-runtime | — | medium | no | yes | VEP |
| `bods_ops.rs` | 6 | `bods::UboDiscoveryService` | plugin | dsl-runtime | `UboDiscoveryService` | medium | partial | yes | VEP |
| `docs_bundle_ops.rs` | 3 | `document_bundles::{BundleContext, DocsBundleRegistry, DocsBundleService}` | mixed | dsl-runtime | `DocsBundleRegistry`, `DocsBundleService` | medium | partial | yes | VEP |

### 4.4 Trading / Deal / Billing / Custody / Misc (Batch 4)

| file | ops | internals | behav | dest | blockers | diff | round-trip | A1 | dispatch |
|---|---|---|---|---|---|---|---|---|---|
| `trading_matrix.rs` | 3 | `domain_ops::helpers` | plugin | dsl-runtime | — | easy | yes | yes | VEP |
| `trading_profile.rs` | 36 | `trading_profile`, `ob_poc_types::trading_matrix` | plugin | dsl-runtime | `TradingProfileDocument` | hard | yes | yes | VEP |
| `trading_profile_ca_ops.rs` | 11 | `trading_profile`, `ob_poc_types::trading_matrix` | plugin | dsl-runtime | `TradingMatrixCorporateActions` | hard | yes | yes | VEP |
| `matrix_overlay_ops.rs` | 3 | — | plugin | dsl-runtime | — | medium | yes | yes | VEP |
| `deal_ops.rs` | 28 | — | plugin | ob-poc-adapter | — | hard | no | yes | VEP |
| `billing_ops.rs` | 14 | — | plugin | ob-poc-adapter | — | hard | no | yes | VEP |
| `investor_ops.rs` | 13 | — | plugin | ob-poc-adapter | — | medium | no | yes | VEP |
| `investor_role_ops.rs` | 6 | — | crud | ob-poc-adapter | — | easy | no | yes | VEP |
| `capital_ops.rs` | 14 | — | plugin | ob-poc-adapter | — | hard | no | yes | VEP |
| `custody.rs` | 5 | — | plugin | dsl-runtime | — | easy | yes | yes | VEP |
| `booking_principal_ops.rs` | 32 | `database::booking_principal_repository`, `domain_ops::rule_evaluator` | plugin | ob-poc-adapter | `BookingPrincipalAccess`, `RuleEvaluator` | hard | partial | **unclear** | VEP |
| `economic_exposure_ops.rs` | 2 | — | plugin | dsl-runtime | — | medium | yes | yes | VEP |
| `observation_ops.rs` | 5 | `services::attribute_identity_service` | plugin | dsl-runtime | `AttributeIdentityService` | medium | yes | yes | VEP |
| `research_normalize_ops.rs` | 1 | — | plugin | dsl-runtime | — | easy | yes | yes | VEP |
| `research_workflow_ops.rs` | 4 | — | plugin | dsl-runtime | — | easy | yes | yes | VEP |
| `refdata_loader.rs` | 5 | — | plugin | metadata | — | easy | yes | yes | VEP |
| `refdata_ops.rs` | 4 | — | plugin | metadata | — | easy | yes | yes | VEP |
| `import_run_ops.rs` | 3 | — | plugin | dsl-runtime | — | medium | yes | yes | VEP |
| `graph_validate_ops.rs` | 1 | — | plugin | dsl-runtime | — | hard | yes | yes | VEP |
| `service_pipeline_ops.rs` | 16 | `service_resources` | plugin | dsl-runtime | `ServiceResourcePipelineService` | hard | yes | yes | VEP |
| `temporal_ops.rs` | 8 | — | plugin | dsl-runtime | — | hard | yes | yes | VEP |
| `batch_control_ops.rs` | 7 | `sem_os_core` | plugin | dsl-runtime | — | medium | yes | yes | VEP |
| `template_ops.rs` | 2 | `dsl_v2::execution`, `templates` | plugin | dsl-runtime | `TemplateExpander`, `DslExecutor` | medium | yes | yes | VEP |
| `pack_ops.rs` | 2 | — | plugin | dsl-runtime | — | easy | yes | yes | VEP |
| `team_ops.rs` | 1 | — | plugin | ob-poc-adapter | — | easy | no | yes | VEP |
| `bpmn_lite_ops.rs` | 5 | `bpmn_integration::client` | plugin | dsl-runtime | `BpmnLiteConnection` | hard | no | **no** | gRPC (external) |
| `resource_ops.rs` | 6 | — | plugin | ob-poc-adapter | — | medium | no | yes | VEP |
| `affinity_ops.rs` | 6 | `sem_reg`, `domain_ops::affinity_graph_cache`, `sem_os_core::affinity` | plugin | dsl-runtime | `AffinityGraph` | hard | yes | yes | VEP |

### 4.5 Shared utilities + module root

| file | ops | internals | behav | dest | blockers | diff | round-trip | A1 | dispatch |
|---|---|---|---|---|---|---|---|---|---|
| `mod.rs` | 0 | `sem_os_core`, `dsl_v2` | utility | dsl-runtime | `CustomOperation` trait itself | easy | n/a | n/a | (registry root) |
| `rule_evaluator.rs` | 0 | `api::booking_principal_types` | utility | dsl-runtime | — | easy | n/a | n/a | (trait / lib) |
| `helpers.rs` | 0 | — | utility | dsl-runtime | — | easy | n/a | n/a | (functions) |
| `affinity_graph_cache.rs` | 0 | `sem_os_core`, `sem_reg` | utility | dsl-runtime | — | easy | n/a | n/a | (cache) |
| `sem_os_helpers.rs` | 0 | `sem_reg` | utility | dsl-runtime | — | easy | n/a | n/a | (helpers) |

---

## 5. Destination distribution (file-level)

| destination | count | % |
|---|---|---|
| `metadata` (dissolves to YAML + PgCrudExecutor) | 14 | 15.7% |
| `dsl-runtime` (moves to new runtime crate) | 65 | 73.0% |
| `ob-poc-adapter` (stays in ob-poc behind runtime traits) | 12 | 13.5% |
| **total** | 89 | 100% |

**Metadata destinations (14 files — most likely Phase 6 dissolution candidates):**
`cbu_ops`, `cbu_role_ops`, `entity_query`, `edge_ops`, `screening_ops`, `requirement_ops`, `evidence_ops`, `request_ops`, `outreach_ops`, `lifecycle_ops`, `regulatory_ops`, `refdata_loader`, `refdata_ops`, `sem_os_schema_ops` (mixed but dominantly metadata after dissolution).

**ob-poc-adapter destinations (12 files — stay in ob-poc with trait injection):**
`client_group_ops`, `gleif_ops`, `sem_os_registry_ops`, `sem_os_maintenance_ops`, `deal_ops`, `billing_ops`, `investor_ops`, `investor_role_ops`, `capital_ops`, `booking_principal_ops`, `team_ops`, `resource_ops`.

All others → `dsl-runtime` or utility co-residents.

---

## 6. Blocker traits (union)

Traits that must be defined in `dsl-runtime` before Phase 5a/4 can move the files that depend on them. One trait per service where possible; group smaller dependencies together.

| Blocker | Backing file(s) | Expected location | Used by |
|---|---|---|---|
| `TaxonomyAccess` | `view_ops.rs` | `dsl-runtime` trait, `ob-poc` impl | view_ops |
| `StateReducer` | `state_ops.rs` | `dsl-runtime` trait, `ob-poc` impl | state_ops |
| `AttributeIdentityService` | `attribute_ops.rs`, `observation_ops.rs` | `dsl-runtime` trait, `ob-poc` impl | attribute, observation |
| `DocumentRequirementsService` | `document_ops.rs`, `tollgate_ops.rs` | `dsl-runtime` trait, `ob-poc` impl | document, tollgate |
| `PlaceholderResolver` | `entity_ops.rs` | `dsl-runtime` trait | entity |
| `ConfidenceCalculator` | `verify_ops.rs` | `dsl-runtime` trait | verify |
| `UboDiscoveryService` | `bods_ops.rs` | `dsl-runtime` trait | bods |
| `DocsBundleRegistry` + `DocsBundleService` | `docs_bundle_ops.rs` | `dsl-runtime` trait | docs_bundle |
| `SemanticStageRegistry` | `onboarding.rs`, `semantic_ops.rs` | `dsl-runtime` trait | onboarding, semantic |
| `TradingProfileDocument` access | `trading_profile.rs` | `dsl-runtime` trait | trading_profile |
| `TradingMatrixCorporateActions` access | `trading_profile_ca_ops.rs` | `dsl-runtime` trait | trading_profile_ca |
| `ServiceResourcePipelineService` | `service_pipeline_ops.rs` | `dsl-runtime` trait | service_pipeline |
| `TemplateExpander` + `DslExecutor` access | `template_ops.rs` | `dsl-runtime` trait | template |
| `AffinityGraph` | `affinity_ops.rs` | `dsl-runtime` trait | affinity |
| `BookingPrincipalAccess` | `booking_principal_ops.rs` | `dsl-runtime` trait | booking_principal |
| `RuleEvaluator` | `booking_principal_ops.rs`, `trading_profile_ca_ops.rs` | `dsl-runtime` trait | booking, trading CA |
| `ClientGroupService` | `client_group_ops.rs` | `dsl-runtime` trait | client_group |
| `GleifClient` + `GleifEnrichmentService` | `gleif_ops.rs` | `dsl-runtime` trait | gleif (A1 concern — keep under adapter) |
| `BpmnLiteConnection` | `bpmn_lite_ops.rs` | `dsl-runtime` trait | bpmn_lite (A1 concern — see §3) |
| Stewardship access (`stew_*`) | `sem_os_focus/governance/changeset_ops.rs` | `dsl-runtime` trait | sem_os_* — A1 concerns, see §3 |

**Estimated new trait count in `dsl-runtime`: ~18–20.** Consistent with v0.3 §19 "one trait per service, coarse-grained" decision.

---

## 7. Caveats and Phase-0b/0c refinements

### 7.1 File-level vs op-level classification

This matrix is file-level. v0.3's ~60% metadata prediction is op-level. Many `dsl-runtime`-destination files here carry mixed behaviour — some ops in them will dissolve to metadata at Phase 6. Examples:

- `sem_os_schema_ops.rs` — mixed behaviour, classified `metadata` destination with acknowledgment that some of its 13 ops are non-CRUD introspection.
- `document_ops.rs` — 9 ops, marked `dsl-runtime` but round-trip = yes, suggesting most ops will dissolve to metadata at Phase 6 and the file may collapse.
- `trading_profile.rs` — 36 ops, marked `dsl-runtime` with round-trip = yes. Significant dissolution expected.
- `lifecycle_ops.rs`, `evidence_ops.rs`, `request_ops.rs` — classified `metadata` direct (pure CRUD); these dissolve first.

**Per-op breakdown is a Phase 6 pre-requisite, not a Phase 0 requirement.** The file-level matrix is sufficient for Phase 1–5 sequencing.

### 7.2 Round-trip column caveats

`round-trip = yes` does not guarantee Phase 6 dissolution. It indicates the file is a candidate for round-trip testing. Actual dissolution requires 100% effect-equivalence per v0.3 §14. Expect ~25–30% reclassification (yes → partial/no) once round-trip fixtures run.

### 7.3 A1 follow-up is the next atomic action

Before proceeding to Phase 0b (concrete envelope types), the 8 A1 rows should receive half a day of targeted re-audit. Per §3.1, this likely collapses to 1–2 genuine blockers.

### 7.4 Dispatch column is uniform

All 89 files report `VerbExecutionPortStepExecutor` as current dispatch. No `DslStepExecutor` / `DslExecutorV2StepExecutor` bypass patterns were found in the `rust/src/domain_ops/` audit. The alternate executors live elsewhere (outside `domain_ops/`) and consolidate cleanly at Phase 5b per D7.

### 7.5 What this matrix does NOT include

- **Per-op breakdown within mixed-behaviour files.** Deferred to Phase 6.
- **Explicit `PendingStateAdvance` shape per op.** Deferred to Phase 0b once envelope types are drafted; this matrix just notes whether each file's ops need the field.
- **SQL fixture authoring for round-trip.** Phase 0e harness work.
- **Row-versioning coverage per entity table touched.** That is Stream 2 (Q1 pre-land).

---

## 8. Gates and next steps

### 8.1 Phase 0a gate criterion — AT RISK

Decision D8 says Phase 5 is blocked if A1 violated anywhere. With 8 flagged rows, the raw matrix **does not pass D8**. However, 4–6 of those 8 are likely to reclassify to `yes` after targeted follow-up.

**Recommended immediate next action:** produce a short "A1 clarification pass" addendum (est. half-day) that resolves each flagged row to `yes` or `confirmed-no`. Only the `confirmed-no` set is a real Phase 5 blocker.

### 8.2 If §8.1 is done and the blocker set is small

- If **0 confirmed A1 violations remain**: Phase 0 gate passes on A1; move to 0b (concrete envelope types).
- If **1–2 confirmed**: those files escalate to individual redesign plans per D8; the rest of Phase 0 can proceed in parallel; Phase 5 waits on redesign close-out.
- If **3+ confirmed**: material re-planning required. Pause Phase 0b until the pattern is understood.

### 8.3 Artefacts ready to be produced next

In order:

1. **A1 clarification pass** — re-audit the 8 flagged rows, specifically checking whether external I/O is pre-txn or in-txn.
2. **Phase 0b** — concrete `GatedVerbEnvelope`, `PendingStateAdvance`, `TransactionScope`, `OutboxDraft`, `StateGateHash` types in `ob-poc-types` (compile-only).
3. **Phase 0c** — `StateGateHash` canonical encoding spec + test vectors.
4. **Phase 0d** — outbox migration + drainer contract.
5. **Phase 0e** — determinism + round-trip harness crates.

Stream 2 (row-versioning audit, per Q1 pre-land) runs parallel to all of the above.

---

## 9. A1 clarification pass (completed 2026-04-18)

The 8 files flagged `A1 = no` or `unclear` in §3 received targeted follow-up. Each was re-audited against a single test: **is the external side effect called from inside the `execute_json` body**, or from a pre/post path outside what will become the Sequencer's transaction?

### 9.1 Cleared to A1 = yes (5 files)

| file | original concern | clarification | verdict |
|---|---|---|---|
| `sem_os_audit_ops.rs` | "external MCP tool dispatch" | The `audit_op!` macro delegates via `sem_os_helpers::delegate_to_tool`, which calls `crate::sem_reg::agent::mcp_tools::dispatch_tool()` — **an in-process Rust function**. "MCP tool" is the module's internal dispatcher name, not a cross-process call. No HTTP, no RPC. | **A1 = yes** |
| `sem_os_focus_ops.rs` | stewardship delegation | `delegate_to_stew_tool` routes to `crate::sem_reg::stewardship::dispatch_phase0_tool` / `dispatch_phase1_tool` — internal functions. No process boundary. | **A1 = yes** |
| `sem_os_governance_ops.rs` | stewardship delegation | Same dispatch chain as `sem_os_focus_ops`. In-process. (Note: governance publish may emit `sem_reg.outbox_events` rows — that write is **DB-local** and A1-compliant, same as any txn write.) | **A1 = yes** |
| `sem_os_changeset_ops.rs` | stewardship delegation | Same dispatch chain. In-process. | **A1 = yes** |
| `booking_principal_ops.rs` | `rule_evaluator` transitive coupling (flagged `unclear`) | `rule_evaluator.rs` header: *"Rule Expression Evaluator — pure Rust, no DB dependency"*. Pure in-memory evaluation over `HashMap<String, JsonValue>`. No external calls. | **A1 = yes** |

### 9.2 Confirmed A1 violations (4 files)

These operate external side effects from inside the `execute_json` body. Each is a genuine D8 blocker for Phase 5 as currently designed.

| # | file | evidence | resolution options |
|---|---|---|---|
| **1** | `sem_os_maintenance_ops.rs` | `tokio::process::Command::new("cargo")` at lines 418 & 472, inside `MaintenanceReindexEmbeddingsOp::execute`. Spawns `cargo run --release -- reindex-embeddings` subprocess. | (a) **Outbox deferral** — introduce `OutboxEffectKind::MaintenanceSpawn`; op returns `OutboxDraft`, drainer spawns post-commit. Recommended. (b) Path exclusion — mark verb as admin-only, reject in Sequencer stage 6 gating. Simpler but narrower. |
| **2** | `bpmn_lite_ops.rs` | 5 ops call `client.compile()`, `client.start()`, `client.signal()`, `client.cancel()`, `client.inspect()` via `BpmnLiteConnection` (gRPC) inside `execute_json` bodies. Explicit `gRPC (external)` dispatch annotation already in matrix. | (a) **Outbox deferral** — op enqueues `OutboxDraft::BpmnDispatch`, drainer executes the gRPC call post-commit, writes callback row on completion. Requires caller restructuring to handle async completion. (b) Pre-txn fetch — only works for read-only ops (`inspect`); write ops (`compile`, `start`) would need callers to get results before the txn opens. Doesn't generalise cleanly. |
| **3** | `source_loader_ops.rs` | 16 ops instantiate `CompaniesHouseLoader`, `GleifLoader`, `SecEdgarLoader` and call `.search()` / `.fetch_entity()` / `.fetch_control_holders()` inside `execute_json` bodies. All HTTP. | (a) **Split fetch-then-persist** — each op restructures into two phases: HTTP fetch (no txn) → txn open → DB persist. Preferred; matches how ops already conceptually work. (b) Pre-fetch at Sequencer stage 5 — fetch external data as part of intent resolution, pass into envelope args. Cleaner for discovery-driven flows. (c) Outbox — op returns `OutboxDraft::ExternalFetch` with a placeholder DB row; drainer completes. Works but adds latency for blocking user flows. |
| **4** | `gleif_ops.rs` | 17 ops. Same pattern as `source_loader_ops.rs` but specifically for GLEIF API: `GleifClient::new()?` + `client.lookup_by_isin(&isin).await?` etc. inside `execute_json`. Also uses `GleifEnrichmentService` which is an HTTP wrapper. | Same three options as #3. Recommend (a) consistent with `source_loader_ops.rs`. |

### 9.3 Verdict against D8

**4 confirmed A1 violations → Phase 5 is BLOCKED** per D8 (your override "if A1 violated anywhere, Phase 5 does not begin").

This is a larger blocker surface than the §3.1 prediction (1–2 expected). The reason for the under-estimate: I conflated "transitive-dependency unclear" (which mostly resolved to yes) with "external-integration by architecture" (which is the real pattern here — BPMN and GLEIF are external services *by design*, not incidentally).

### 9.4 Shape of the blockers

The 4 violations cluster into **two architectural patterns**:

**Pattern A — Subprocess / long-running admin task** (1 file: `sem_os_maintenance_ops.rs`)
- Single op. Clean outbox deferral fits naturally.
- Or exclude from user-runbook path with a YAML flag.
- **Effort:** 1–2 days including new `OutboxEffectKind::MaintenanceSpawn`.

**Pattern B — External service integration** (3 files, ~38 ops: `bpmn_lite_ops`, `source_loader_ops`, `gleif_ops`)
- These are not incidental HTTP calls — they are the means by which ob-poc integrates with external systems.
- Two sub-cases:
  - **Read-heavy fetch-then-persist** (source_loader, gleif, bpmn inspect): split into fetch phase + persist phase. Op restructure is mechanical per-op but volume is significant (~30+ ops).
  - **Write-through external dispatch** (bpmn compile, start, signal, cancel): harder. Outbox deferral breaks the synchronous expectation of the caller (they want a correlation_id or bytecode back immediately).
- **Effort:** 3–4 weeks for the full set. This is real redesign work for Pattern B, not just clarification.

### 9.5 Recommended path forward

**Option I — Strict D8 adherence.** Open a dedicated "pre-Phase-5 A1 remediation" workstream covering the 4 files. Phase 0b–0e continue in parallel. Phase 5 waits. Estimated 3–4 weeks for full Pattern B remediation.

**Option II — Scope D8 to genuinely-new violations.** The 4 violations exist in the *current* codebase. They do not exhibit new risk introduced by the refactor — the transaction-scope model is new, but the external calls are pre-existing. Recognising that, narrow D8 to: *no refactor step may introduce a new A1 violation. Pre-existing A1 violations are grandfathered into Pattern-A outbox deferral or Pattern-B two-phase, scheduled as Phase 5 work rather than pre-Phase-5 blockers.*

**Option III — Hybrid.** Block on Pattern A (simple, 1–2 days, worth gating on). Grandfather Pattern B into Phase 5 as a dedicated sub-phase (5f). This lets Phase 5 begin without full Pattern B resolution; Pattern B becomes part of the Phase 5 work package.

**My recommendation: Option III.** It respects the spirit of D8 (no committed-writes-with-stale-control-plane failure mode reaches production) while being realistic about the fact that 38+ ops cannot be redesigned in a pre-phase. The subprocess spawn (Pattern A) is cheap to outbox and a clean win. Pattern B becomes Phase 5f and blocks Phase 6, not Phase 5 start.

**Decision needed from you (D11, new):** Option I / II / III?

### 9.6 A1 status by numbers

| status | count | files |
|---|---|---|
| A1 = yes (from original matrix) | 81 | — |
| A1 = yes (after clarification pass) | +5 | `sem_os_audit`, `sem_os_focus`, `sem_os_governance`, `sem_os_changeset`, `booking_principal` |
| **A1 = yes total** | **86** | |
| A1 = no (confirmed blockers) | 4 | `sem_os_maintenance`, `bpmn_lite`, `source_loader`, `gleif` |
| A1 = unclear (remaining) | 0 | — |
| **total files** | **90** | (89 ops files + mod.rs counted in dsl-runtime tally; matrix has 89 classified rows) |

### 9.7 D11 resolution (2026-04-18): Option III — Hybrid

**Decision recorded.** Pattern A becomes Phase 0g (blocks Phase 1); Pattern B becomes Phase 5f (blocks Phase 6, not Phase 5 start).

**Guardrails to prevent compromise drift** (non-negotiable per user: "make sure we don't lose the final fix — it's a compromise"):

1. **Dedicated ledger file:** `docs/todo/pattern-b-a1-remediation-ledger.md` — 39 ops tracked (1 Pattern A + 38 Pattern B). Each row moves to CLOSED only when verified. Ledger file **retained in repo permanently** as historical record.
2. **Phase 6 hard dependency:** CRUD dissolution (the most visible deletion-win phase) cannot start until the ledger's Phase 5f section reads CLOSED. This is deliberate leverage — Phase 6 is politically wanted, so 5f cannot be skipped.
3. **Workspace lint L4:** post-Phase 5f, forbids any `reqwest::`, `tokio::process::`, `tonic::*` call inside `execute_json` / `execute` bodies of any `CustomOperation` impl. Escape hatch requires `#[allow(external_effects_in_verb)]` + ledger reference.
4. **DoD item 19:** three-plane refactor is not Done until ledger is CLOSED and L4 is green. Added to implementation plan §10.6.
5. **Weekly ledger reviews** during Phases 1–5; hard gate review at Phase 5 and Phase 6 milestones.

### 9.8 Updated sequencing (Option III in effect)

- **Phase 0g (new, 1–2 days):** Pattern A remediation — `MaintenanceReindexEmbeddingsOp` moves subprocess spawn to `OutboxDraft::MaintenanceSpawn`. Parallelises with Phase 0b–0e. Blocks Phase 1.
- **Phase 0b–0e (parallel):** proceed immediately, unblocked by Pattern B.
- **Phase 5f (new, 3–4 weeks):** Pattern B remediation — 38 ops across bpmn_lite / source_loader / gleif. Blocks Phase 6.
- **Phase 6:** cannot begin until Phase 5f ledger CLOSED.

Effort delta: +3 weeks on critical path (visible cost of compromise).

---

## 10. Reviewer checklist

- [ ] Destination distribution (~16 / 73 / 13) reasonable given per-file vs per-op classification?
- [ ] A1 blocker count (8 flagged, likely 1–2 genuine) acceptable for proceeding to A1 clarification pass?
- [ ] Blocker trait inventory (§6) complete? Missing any services?
- [ ] Utility files (5) correctly routed to `dsl-runtime`?
- [ ] `sem_os_maintenance_ops.rs` subprocess spawn — confirm not in user-runbook path, or plan outbox deferral
- [ ] `bpmn_lite_ops.rs` — confirm gRPC dispatch boundary vs txn boundary
- [ ] `gleif_ops.rs`, `source_loader_ops.rs` — confirm HTTP fetch is pre-txn
- [ ] Ready to produce Phase 0b (envelope types) after A1 pass?

---

**End of Phase 0a matrix — awaiting review.**
