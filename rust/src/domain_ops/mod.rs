//! Custom Operations (Tier 2)
//!
//! This module contains operations that cannot be expressed as data-driven
//! verb definitions. Each custom operation must have a clear rationale for
//! why it requires custom code.
//!
//! ## When to use Custom Operations
//!
//! - External API calls (screening services, AI extraction)
//! - Complex business logic (UBO calculation, graph traversal)
//! - Operations requiring multiple database transactions
//! - Operations with side effects (file I/O, notifications)
//!
//! ## Guidelines
//!
//! 1. Exhaust all options for data-driven verbs first
//! 2. Document WHY this operation requires custom code
//! 3. Keep operations focused and single-purpose
//! 4. Ensure operations are testable in isolation

// Phase 5a composite-blocker #24 — affinity_graph_cache relocated to
// `dsl-runtime::domain_ops::affinity_graph_cache`. Inlined the
// `PgSnapshotRow` Postgres-row decode + conversion to
// `sem_os_core::types::SnapshotRow` to drop the `crate::sem_reg::types`
// dep (61 LOC → ~140 LOC after inlining; trade-off accepted vs
// relocating the whole sem_reg::types module).
// Phase 5a composite-blocker #24 — affinity_ops relocated to
// `dsl-runtime::domain_ops::affinity_ops` consuming the new
// `dyn SemOsContextResolver` trait via the ServiceRegistry.
// `ObPocSemOsContextResolver` in ob-poc bridges to
// `crate::sem_reg::agent::mcp_tools::build_sem_os_service(pool).resolve_context(...)`.
// Internal `ActorContext`/`Classification` references switched from
// `crate::sem_reg::*` to `sem_os_core::abac::*` (already JSON-compatible
// per the original `to_sem_os_actor` round-trip helper, now eliminated).
// affinity_graph_cache also relocated alongside (slice #24 step B).
// Phase 5a — agent_ops relocated to `dsl-runtime::domain_ops::agent_ops`
// consuming `dyn McpToolRegistry` via the ServiceRegistry.
// Phase 5a composite-blocker #30 — attribute_ops relocated to
// `dsl-runtime::domain_ops::attribute_ops` via YAML-first re-implementation
// (`config/verbs/attribute.yaml` + observation/derivation.yaml). 16 verbs
// across `attribute.*`, `document.*`, and `derivation.*` dispatch through
// the new `dyn AttributeService` trait (single-method bridge at
// `crate::services::attribute_service_impl::ObPocAttributeService`) which
// keeps `crate::sem_reg::*` and `crate::services::attribute_identity_service`
// in ob-poc. The bridge returns `AttributeDispatchOutcome { outcome,
// bindings }` so the wrapper can apply `@attribute` bindings from the
// 3 `define*` verbs via `ctx.bind`.
// Phase 5a composite-blocker #19 — billing_ops relocated to
// `dsl-runtime::domain_ops::billing_ops`. Pure clean lift — matrix
// tag "ob-poc-adapter destination" was wrong (same as slices #16-#18).
// All 14 ops are direct sqlx against the billing tables. Strip via
// Python script: 14 legacy execute blocks deleted, all cfg gates
// stripped. Registration flows through inventory.
// Phase 5e — board_ops relocated to `dsl-runtime::domain_ops::board_ops`
// Phase 4 Slice B Group 3 — bods_ops relocated to `dsl-runtime::domain_ops::bods_ops`
// alongside the `bods/` module it consumes.
mod booking_principal_ops;
mod bpmn_lite_ops;
// Phase 5a composite-blocker #20 — capital_ops relocated to
// `dsl-runtime::domain_ops::capital_ops`. Pure clean lift — matrix tag
// "ob-poc-adapter destination" was wrong (4th of the 6 to be re-classified).
// 14 ops, all direct sqlx against capital structure tables. Stripped via
// the same Python script as slice #19 billing_ops. Registration flows
// through inventory.
// Phase 5d — cbu_ops relocated to `dsl-runtime::domain_ops::cbu_ops`
// Phase 5d — cbu_role_ops relocated to `dsl-runtime::domain_ops::cbu_role_ops`
// Phase 5a composite-blocker #22 — client_group_ops relocated to
// `dsl-runtime::domain_ops::client_group_ops`. Pure clean lift —
// matrix tag wrong (6th in this run after #16-#21). 24 ops, all
// direct sqlx against client-group tables. Stripped via the same
// Python script as slice #19 billing_ops.
// Phase 5a composite-blocker #9 — constellation_ops relocated to
// `dsl-runtime::domain_ops::constellation_ops` consuming the new
// `dyn ConstellationRuntime` trait via the ServiceRegistry.
// `ObPocConstellationRuntime` in ob-poc bridges to
// `crate::sem_os_runtime::constellation_runtime::handle_constellation_{hydrate,summary}`,
// projecting each result through `serde_json::to_value` so the
// internal `HydratedConstellation` / `ConstellationSummary` types
// stay in ob-poc. Registration flows through inventory; external
// ob-poc code does not import these types directly.
// Phase 5e — control_compute_ops relocated to `dsl-runtime::domain_ops::control_compute_ops`
// Phase 5e — control_ops relocated to `dsl-runtime::domain_ops::control_ops`
// Phase 5a composite-blocker #21 — deal_ops relocated to
// `dsl-runtime::domain_ops::deal_ops`. Pure clean lift — matrix tag
// "ob-poc-adapter destination" was wrong (5th in this run).
// 28 ops, all direct sqlx against deal lifecycle tables. Stripped via
// the same Python script as slice #19 billing_ops.
// Phase 5e — dilution_ops relocated to `dsl-runtime::domain_ops::dilution_ops`
// Phase 5a composite-blocker #23 — discovery_ops relocated to
// `dsl-runtime::domain_ops::discovery_ops` along with its two helper
// modules: `entity_kind` (81 LOC, zero deps) and `stategraph/`
// (514 LOC, dsl_core only). 4 ob-poc consumers updated from
// `crate::entity_kind` → `dsl_runtime::entity_kind`.
// Inside discovery_ops: `dispatch_tool` calls routed via
// `StewardshipDispatch` (slice #7 cascade trick); `gateway_addr()`
// inlined to direct `std::env::var` lookup; legacy `sem_reg_tool`
// helper deleted (unused after legacy execute paths removed).
// Phase 4 Slice B Group 4 — docs_bundle_ops relocated to `dsl-runtime::domain_ops::docs_bundle_ops`
// alongside the `document_bundles/` module it consumes.
// Phase 4 Slice B Group 6 — document_ops relocated to `dsl-runtime::domain_ops::document_ops`
// alongside the `document_requirements/` module it consumes.
// Phase 5c — edge_ops relocated to `dsl-runtime::domain_ops::edge_ops`
// Phase 5c — entity_query relocated to `dsl-runtime::domain_ops::entity_query`
// Phase 5c — evidence_ops relocated to `dsl-runtime::domain_ops::evidence_ops`
mod gleif_ops;
pub mod helpers;
// Phase 5a composite-blocker #14 — investor_ops relocated to
// `dsl-runtime::domain_ops::investor_ops`. Pure clean lift — no new
// service trait needed (same pattern as slice #12 sibling
// `investor_role_ops`). The 2 ob-poc deps were `crate::dsl_v2::*`
// legacy-path artifacts; after stripping the 13 legacy
// `execute(&VerbCall, ...)` blocks and 4 file-local `get_*` helpers,
// every op is direct sqlx against the `"ob-poc".investors` (and
// `"ob-poc".holdings`) tables. Registration flows through inventory.
// Phase 5a composite-blocker #12 — investor_role_ops relocated to
// `dsl-runtime::domain_ops::investor_role_ops`. Pure clean lift — no
// new service trait needed. After stripping the legacy
// `execute(&VerbCall, ...)` blocks (used 6 file-local helpers
// `get_required_uuid` / `get_optional_*`) and dropping the
// `#[cfg(feature = "database")]` gates, every op is a direct sqlx
// call to the in-DB `"ob-poc".upsert_role_profile` function.
// Registration flows through inventory; external ob-poc code does not
// import these types directly.
// Phase 5a composite-blocker #5 — kyc_case_ops relocated to
// `dsl-runtime::domain_ops::kyc_case_ops` consuming the new
// `dyn LifecycleCatalog` service. `ObPocLifecycleCatalog` in ob-poc
// bridges to the taxonomy-loaded `OntologyService` singleton.
// Registration flows through inventory automatically; external ob-poc
// code doesn't import these types directly.
// Phase 5c — lifecycle_ops relocated to `dsl-runtime::domain_ops::lifecycle_ops`
// Phase 5a composite-blocker #6 — manco_ops relocated to
// `dsl-runtime::domain_ops::manco_ops`. Self-contained (no service-trait
// dependency); all 11 ops call DB-level `fn_*` PL/pgSQL functions via
// sqlx. Registration flows through inventory automatically; external
// ob-poc code doesn't import these types directly.
// Phase 5a composite-blocker #10 — navigation_ops relocated to
// `dsl-runtime::domain_ops::navigation_ops`. Pure clean lift — no new
// service trait needed. After stripping the legacy
// `execute(&VerbCall, ...)` blocks (used `find_arg(verb_call, ...)`)
// and the supporting helper, every op is pure JSON-construction
// (no DB calls, no service calls). The Sequencer reads
// `NavResult` records via message-prefix matching against
// `ReplResponseV2.message` (`apply_nav_result_if_present`), so the
// `NavResult` struct lives wherever the verbs do — no cross-plane
// type sharing required. Registration flows through inventory;
// external ob-poc code does not import these types directly.
// Phase 5a composite-blocker #8 — observation_ops relocated to
// `dsl-runtime::domain_ops::observation_ops` consuming the new
// `dyn AttributeIdentityService` trait via the ServiceRegistry.
// `ObPocAttributeIdentityService` in ob-poc bridges to the in-crate
// `services::attribute_identity_service::AttributeIdentityService`
// (multi-namespace UNION query over dictionary, registry, SemOS defs).
// Registration flows through inventory; external ob-poc code does not
// import these types directly.
mod onboarding;
// Phase 5c — outreach_ops relocated to `dsl-runtime::domain_ops::outreach_ops`
// Phase 5e — ownership_ops relocated to `dsl-runtime::domain_ops::ownership_ops`
// Phase 5e — partnership_ops relocated to `dsl-runtime::domain_ops::partnership_ops`
// Phase 5a composite-blocker #29 — phrase_ops relocated to
// `dsl-runtime::domain_ops::phrase_ops` via YAML-first re-implementation
// (`config/verbs/phrase.yaml`). 9 `phrase.*` verbs dispatch through the
// new `dyn PhraseService` trait (single-method bridge at
// `crate::services::phrase_service_impl::ObPocPhraseService`) which
// keeps `crate::sem_reg::store::SnapshotStore` + `crate::sem_reg::types::*`
// + `crate::sem_reg::ids::object_id_for` and the embedding-similarity
// SQL on `verb_pattern_embeddings` / `phrase_bank` / `session_traces`
// in ob-poc. The dispatch signature carries `&Principal` for snapshot
// audit fields (mirroring `StewardshipDispatch::dispatch`).
// Phase 5c — refdata_loader relocated to `dsl-runtime::domain_ops::refdata_loader`
// Phase 5c — refdata_ops relocated to `dsl-runtime::domain_ops::refdata_ops`
// Phase 5d — regulatory_ops relocated to `dsl-runtime::domain_ops::regulatory_ops`
// Phase 5a composite-blocker #2 — remediation_ops relocated to `dsl-runtime::domain_ops::remediation_ops`
// alongside the `cross_workspace/` module it consumes (relocated together).
mod request_ops;
// Phase 5c — requirement_ops relocated to `dsl-runtime::domain_ops::requirement_ops`
// Phase 5a composite-blocker #4 — research_workflow_ops relocated to
// `dsl-runtime::domain_ops::research_workflow_ops`. Self-contained (no
// service-trait dependency); the 4 ops use only json_* helpers and
// direct sqlx. Registration flows through inventory automatically;
// external ob-poc code doesn't import these types directly.
// Phase 5a composite-blocker #13 — resource_ops relocated to
// `dsl-runtime::domain_ops::resource_ops`. Reuses the existing
// `dyn AttributeIdentityService` trait from slice #8 (already
// registered via `ObPocAttributeIdentityService`); only one of the
// six ops (`set-attr`) needs `resolve_runtime_uuid`. After stripping
// the legacy `execute(&VerbCall, ...)` blocks (deps `crate::dsl_v2::*`)
// and refactoring `resource_set_attr_impl` to take a pre-resolved
// `attribute_id: Uuid` (caller fetches the trait), the ob-poc surface
// reduces to zero. Registration flows through inventory.
pub mod rule_evaluator;
// Phase 5d — screening_ops relocated to `dsl-runtime::domain_ops::screening_ops`
// Phase 5a composite-blocker #7 — sem_os_audit_ops relocated to
// `dsl-runtime::domain_ops::sem_os_audit_ops`. Clean lift on the existing
// `dyn StewardshipDispatch` trait — `ObPocStewardshipDispatch` already
// cascades non-`stew_` tool names through `sem_reg::agent::mcp_tools::dispatch_tool`,
// so the 8 audit ops (`sem_reg_create_plan`, `sem_reg_record_decision`, …)
// resolve transparently without a new service trait. Registration flows
// through inventory; external ob-poc code does not import these types directly.
// Phase 5a — sem_os_{focus,governance,changeset}_ops relocated to
// `dsl-runtime::domain_ops::*` consuming `dyn StewardshipDispatch` via
// the ServiceRegistry. `ObPocStewardshipDispatch` in ob-poc bridges
// to `sem_reg::stewardship::dispatch_phase{0,1}_tool` + general MCP.
pub(crate) mod sem_os_helpers;
// Phase 5a composite-blocker #17 — sem_os_maintenance_ops relocated to
// `dsl-runtime::domain_ops::sem_os_maintenance_ops`. Pure clean lift —
// the spec's "ob-poc-adapter destination" matrix tag turned out wrong
// (same as team_ops slice #16). 7 ops (health-pending, health-stale-
// dryruns, cleanup, bootstrap-seeds, drain-outbox, reindex-embeddings,
// validate-schema-sync) — all direct sqlx against `sem_reg.changesets`,
// `sem_reg.snapshots`, `sem_reg_authoring.change_sets_archive`,
// `public.outbox`. No new service trait.
// Phase 5a composite-blocker #18 — sem_os_registry_ops relocated to
// `dsl-runtime::domain_ops::sem_os_registry_ops` via the existing
// `dyn StewardshipDispatch` trait (slice #7 cascade trick).
// `ObPocStewardshipDispatch` already cascades non-`stew_` tool names
// through the general SemReg dispatcher, so the 20 `sem_reg_*` tools
// route transparently. 1 op (`RegistryActiveManifestOp`) does direct
// sqlx for `active-manifest` (no matching MCP tool); the other 19
// delegate via the `registry_op!` macro.
// Phase 5a composite-blocker #25 — sem_os_schema_ops relocated to
// `dsl-runtime::domain_ops::sem_os_schema_ops` via YAML-first
// re-implementation against `config/verbs/sem-reg/schema.yaml`.
// Three dispatch routes: 5 structure-semantics verbs use the new
// `dyn SchemaIntrospectionAccess` trait (bridge reads ontology +
// verb_registry + sem_reg snapshots); 5 introspect/extract verbs
// route via existing `StewardshipDispatch` cascade to the
// `db_introspect` MCP tool (slice #7 trick); 3 diagram verbs use
// direct sqlx + sem_os_core::diagram + relocated affinity_graph_cache.
// Phase 5a — semantic_ops relocated to `dsl-runtime::domain_ops::semantic_ops`,
// consuming `dyn SemanticStateService` via the ServiceRegistry.
// Phase 5a composite-blocker #28 — service_pipeline_ops relocated to
// `dsl-runtime::domain_ops::service_pipeline_ops` via YAML-first
// re-implementation (`config/verbs/service.yaml`,
// `service-pipeline.yaml`, `service-resource.yaml`,
// `service-availability.yaml`). 16 verbs across 7 domains
// (`service-intent.*`, `discovery.*`, `attributes.*`, `provisioning.*`,
// `readiness.*`, `pipeline.full`, `service-resource.*`) dispatch
// through the new `dyn ServicePipelineService` trait — one method
// `dispatch_service_pipeline_verb(pool, domain, verb, args)` returning
// `VerbExecutionOutcome` directly to preserve the four return-type
// shapes (Uuid / Record / RecordSet / Affected) per YAML
// `returns.type`. Bridge at
// `crate::services::service_pipeline_service_impl::ObPocServicePipelineService`
// keeps `crate::service_resources::*` (engines, orchestrators, SRDEF
// registry loader) in ob-poc.
// Phase 5a composite-blocker #27 — session_ops relocated to
// `dsl-runtime::domain_ops::session_ops` via YAML-first re-implementation
// against `config/verbs/session.yaml`. All 19 `session.*` verbs dispatch
// through the new `dyn SessionService` trait (single-method bridge at
// `crate::services::session_service_impl::ObPocSessionService`) which
// wraps `crate::session::UnifiedSession` — a 10934 LOC multi-consumer
// mega-module that stays in ob-poc. Pending session state crosses turns
// through `ctx.extensions["_pending_session"]` (mirrors the legacy
// `ext_set_pending_session` helper).
// Phase 5a composite-blocker #3 — shared_atom_ops relocated to
// `dsl-runtime::domain_ops::shared_atom_ops`, consuming the already-relocated
// `dsl_runtime::cross_workspace::{repository, fact_refs, fact_versions,
// replay, types}` module. Registration flows through inventory automatically;
// external ob-poc code doesn't import these types directly.
// Phase 5a composite-blocker #15 — skeleton_build_ops relocated to
// `dsl-runtime::domain_ops::skeleton_build_ops`. Pure clean lift —
// no new service trait needed (the spec's projected
// `SkeletonBuildOrchestrator` trait turned out unnecessary). The
// file is self-contained: 1 op (`SkeletonBuildOp`) + 5 long
// `pub async fn run_*` orchestrator helpers (graph_validate,
// ubo_compute, coverage_compute, outreach_plan, tollgate_evaluate)
// called only from within the file. The `pub use run_*` re-exports
// in this mod.rs had zero external consumers; dropped along with
// the file relocation. All ob-poc-side imports were
// `crate::dsl_v2::*` legacy-path artifacts. Registration flows
// through inventory.
// Phase 4 Slice B Group 9 — state_ops relocated to `dsl-runtime::domain_ops::state_ops`
// alongside the `state_reducer/` module it consumes.
mod source_loader_ops;
// Phase 5a composite-blocker #16 — team_ops relocated to
// `dsl-runtime::domain_ops::team_ops`. Pure clean lift — the spec's
// "ob-poc-adapter destination" matrix tag turned out wrong (file is
// self-contained, only legacy-path artifacts as deps). 1 op
// (`TeamTransferMemberOp`), 171 LOC. Registration flows through
// inventory; `pub use team_ops::TeamTransferMemberOp` re-export had
// zero external consumers and was dropped.
pub mod template_ops;

// Phase 4 Slice B Group 6 — tollgate_ops relocated to `dsl-runtime::domain_ops::tollgate_ops`
// alongside the `document_requirements/` module it consumes.
mod trading_profile;
// Phase 5a composite-blocker #11 — trading_profile_ca_ops relocated to
// `dsl-runtime::domain_ops::trading_profile_ca_ops` consuming the new
// `dyn TradingProfileDocument` trait via the ServiceRegistry.
// `ObPocTradingProfileDocument` in ob-poc bridges to
// `crate::trading_profile::ast_db::{load_document, save_document}`.
// `TradingMatrixDocument` already lives in `ob_poc_types::trading_matrix`
// (boundary crate), so the trait can use it directly without
// types-extraction. Registration flows through inventory; external
// ob-poc code does not import these types directly.
// Phase 5e — trust_ops relocated to `dsl-runtime::domain_ops::trust_ops`
// Phase 5e — ubo_analysis relocated to `dsl-runtime::domain_ops::ubo_analysis`
// Phase 5e — ubo_compute_ops relocated to `dsl-runtime::domain_ops::ubo_compute_ops`
// Phase 5e — ubo_graph_ops relocated to `dsl-runtime::domain_ops::ubo_graph_ops`
// Phase 5e — ubo_registry_ops relocated to `dsl-runtime::domain_ops::ubo_registry_ops`
// Phase 4 Slice B Group 2 — verify_ops relocated to `dsl-runtime::domain_ops::verify_ops`
// alongside the `verification/` module it consumes.
// Phase 5a composite-blocker #26 — view_ops relocated to
// `dsl-runtime::domain_ops::view_ops` via YAML-first re-implementation
// (`config/verbs/view.yaml`, 15 verbs). Single-method `dyn ViewService`
// trait dispatches all 15 verbs through the bridge in
// `crate::services::view_service_impl`, which keeps the heavy
// `crate::session::ViewState` + `crate::taxonomy::*` modules in
// ob-poc (both are 5000+ LOC multi-consumer mega-modules).

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

// Re-export DSL types for use by operation implementations
pub use crate::dsl_v2::ast::VerbCall;
pub use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

// Phase 5c — entity_query relocated. Types accessed via dsl_runtime::domain_ops::entity_query.
// Phase 5a composite-blocker #5 — kyc_case_ops relocated. Types accessed via
// dsl_runtime::domain_ops::kyc_case_ops; no external ob-poc consumer imports them.
// Phase 5c — lifecycle_ops relocated to `dsl-runtime::domain_ops::lifecycle_ops`.
// Registration flows through inventory automatically; external ob-poc code doesn't
// import these types directly.
pub use onboarding::OnboardingAutoCompleteOp;
// Phase 5c — refdata_loader relocated. Types accessed via dsl_runtime::domain_ops::refdata_loader.
// Phase 5c — refdata_ops relocated. Types accessed via dsl_runtime::domain_ops::refdata_ops.
pub use request_ops::{
    DocumentRequestOp, DocumentUploadOp, DocumentWaiveOp, RequestCancelOp, RequestCreateOp,
    RequestEscalateOp, RequestExtendOp, RequestFulfillOp, RequestOverdueOp, RequestRemindOp,
    RequestWaiveOp,
};

// Phase 5a — semantic_ops relocated to dsl-runtime. Inventory registration
// automatic; external ob-poc code does not import these types directly.
pub use template_ops::{
    TemplateBatchOp, TemplateBatchResult, TemplateInvokeOp, TemplateInvokeResult,
};

pub use trading_profile::{
    TradingProfileActivateOp, TradingProfileAddAllowedCurrencyOp, TradingProfileAddBookingRuleOp,
    TradingProfileAddComponentOp, TradingProfileAddCsaCollateralOp, TradingProfileAddCsaConfigOp,
    TradingProfileAddImMandateOp, TradingProfileAddInstrumentClassOp,
    TradingProfileAddIsdaConfigOp, TradingProfileAddIsdaCoverageOp, TradingProfileAddMarketOp,
    TradingProfileAddSsiOp, TradingProfileApproveOp, TradingProfileArchiveOp,
    TradingProfileCloneToOp, TradingProfileCreateDraftOp, TradingProfileCreateNewVersionOp,
    TradingProfileDiffOp, TradingProfileGetActiveOp, TradingProfileImportOp,
    TradingProfileLinkCsaSsiOp, TradingProfileMaterializeOp, TradingProfileRejectOp,
    TradingProfileRemoveBookingRuleOp, TradingProfileRemoveComponentOp,
    TradingProfileRemoveCsaConfigOp, TradingProfileRemoveImMandateOp,
    TradingProfileRemoveInstrumentClassOp, TradingProfileRemoveIsdaConfigOp,
    TradingProfileRemoveMarketOp, TradingProfileRemoveSsiOp, TradingProfileSetBaseCurrencyOp,
    TradingProfileSubmitOp, TradingProfileUpdateImScopeOp, TradingProfileValidateCoverageOp,
    TradingProfileValidateGoLiveReadyOp,
};
// Phase 5a composite-blocker #11 — trading_profile_ca_ops re-exports removed; see relocation comment above.
// Phase 5e — ubo_analysis relocated. Types accessed via dsl_runtime::domain_ops::ubo_analysis.
// Phase 5e — ubo_compute_ops relocated. Types accessed via dsl_runtime::domain_ops::ubo_compute_ops.

// Phase 5a composite-blocker #30 — attribute_ops re-exports removed; see relocation comment above.
// Phase 5d — cbu_ops relocated. Types accessed via dsl_runtime::domain_ops::cbu_ops.
// Phase 5d — cbu_role_ops relocated. Types accessed via dsl_runtime::domain_ops::cbu_role_ops.
// Phase 4 Slice B Group 6 — document_ops relocated to `dsl-runtime::domain_ops::document_ops`.
// Registration flows through inventory automatically; external ob-poc code doesn't
// import these types directly.

// Requirement operations (Migration 049)
// Phase 4 Slice B Group 1 — entity_ops relocated to `dsl-runtime::domain_ops::entity_ops`
// alongside the `placeholder/` module it consumes. Registration flows through inventory
// automatically; external ob-poc code doesn't import these types directly.
// Phase 5a composite-blocker #8 — observation_ops re-exports removed; see relocation comment above.
// Phase 4 Slice A — pack_ops moved to `dsl-runtime::domain_ops::pack_ops`.
// Registration flows through `#[register_custom_op]` + inventory automatically;
// downstream ob-poc code does not import these types directly, so no shim
// re-export is needed.
// Phase 5c — requirement_ops relocated. Types accessed via dsl_runtime::domain_ops::requirement_ops.
// Phase 5a composite-blocker #13 — resource_ops re-exports removed; see relocation comment above.
// Phase 5d — screening_ops relocated. Types accessed via dsl_runtime::domain_ops::screening_ops.
// Phase 5e — ubo_graph_ops relocated. Types accessed via dsl_runtime::domain_ops::ubo_graph_ops.

// Team operations (only transfer-member needs plugin, others are CRUD)
// Phase 5a composite-blocker #16 — team_ops re-export removed; see relocation comment above.

// Access Review operations (complex multi-step transactional operations only)

// BODS operations relocated to dsl-runtime::domain_ops::bods_ops in Phase 4 Slice B Group 3.
// Phase 5a composite-blocker #23 — discovery_ops re-exports removed; see relocation comment above.

// Phase 5a composite-blocker #26 — view_ops re-exports removed; see relocation comment above.

// KYC Control Enhancement operations (capital, board, trust, partnership, tollgate, control)
// Phase 5e — board_ops relocated. Types accessed via dsl_runtime::domain_ops::board_ops.
// Phase 5a composite-blocker #20 — capital_ops re-exports removed; see relocation comment above.
// Phase 5e — control_compute_ops relocated. Types accessed via dsl_runtime::domain_ops::control_compute_ops.
// Phase 5e — control_ops relocated. Types accessed via dsl_runtime::domain_ops::control_ops.
// Phase 5e — dilution_ops relocated. Types accessed via dsl_runtime::domain_ops::dilution_ops.
// Phase 5e — ownership_ops relocated. Types accessed via dsl_runtime::domain_ops::ownership_ops.
// Phase 5e — partnership_ops relocated. Types accessed via dsl_runtime::domain_ops::partnership_ops.
// Phase 4 Slice B Group 6 — tollgate_ops relocated to `dsl-runtime::domain_ops::tollgate_ops`.
// Registration flows through inventory automatically; external ob-poc code doesn't
// import these types directly.
// Phase 5e — trust_ops relocated. Types accessed via dsl_runtime::domain_ops::trust_ops.

// Phase 5a composite-blocker #28 — service_pipeline_ops re-exports removed; see relocation comment above.

// GLEIF operations (LEI data enrichment)
pub use gleif_ops::{
    GleifEnrichOp, GleifGetChildrenOp, GleifGetManagedFundsOp, GleifGetManagerOp,
    GleifGetMasterFundOp, GleifGetParentOp, GleifGetRecordOp, GleifGetUmbrellaOp,
    GleifImportManagedFundsOp, GleifImportTreeOp, GleifLookupByIsinOp, GleifRefreshOp,
    GleifResolveSuccessorOp, GleifSearchOp, GleifTraceOwnershipOp,
};

// Re-export the CustomOperation trait + registry from `dsl-runtime`.
// Slice G of Phase 2.5 moved these out of `ob-poc` into the data-plane crate;
// ob-poc code paths continue to use `crate::domain_ops::CustomOperation`
// through this shim so no downstream import sweeps are needed beyond the
// op files themselves.
pub use dsl_runtime::{CustomOpFactory, CustomOperation, CustomOperationRegistry};

// =============================================================================
// YAML ↔ OP SANITY CHECK
// =============================================================================

/// Result of verifying plugin verb coverage
#[derive(Debug, Default)]
pub struct PluginVerbCoverageResult {
    /// YAML plugin verbs that have a registered CustomOp
    pub covered: Vec<(String, String)>,
    /// YAML plugin verbs missing a registered CustomOp (FATAL)
    pub yaml_missing_op: Vec<(String, String)>,
    /// CustomOps without a corresponding YAML plugin verb (WARNING)
    pub op_missing_yaml: Vec<(String, String)>,
}

impl PluginVerbCoverageResult {
    /// Check if the verification passed (no missing ops for YAML verbs)
    pub fn is_ok(&self) -> bool {
        self.yaml_missing_op.is_empty()
    }

    /// Format a summary message
    pub fn summary(&self) -> String {
        format!(
            "Plugin coverage: {} covered, {} YAML verbs missing ops, {} ops missing YAML definitions",
            self.covered.len(),
            self.yaml_missing_op.len(),
            self.op_missing_yaml.len()
        )
    }
}

/// Verify all YAML plugin verbs have corresponding registered CustomOps.
///
/// This function ensures the system is consistent:
/// - Every YAML verb with `behavior: plugin` MUST have a registered CustomOp
/// - CustomOps without YAML definitions are warned about (orphaned ops)
///
/// # Arguments
/// * `custom_ops` - The CustomOperationRegistry to verify against
///
/// # Returns
/// A `PluginVerbCoverageResult` with coverage details
///
/// # Usage
/// Call this at startup after both registries are initialized:
/// ```ignore
/// let registry = CustomOperationRegistry::new();
/// let result = verify_plugin_verb_coverage(&registry);
/// if !result.is_ok() {
///     panic!("Plugin verb coverage check failed: {:?}", result.yaml_missing_op);
/// }
/// ```
pub fn verify_plugin_verb_coverage(
    custom_ops: &CustomOperationRegistry,
) -> PluginVerbCoverageResult {
    use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};
    use std::collections::HashSet;

    let mut result = PluginVerbCoverageResult::default();

    // Track which ops are referenced by YAML
    let mut referenced_ops: HashSet<(String, String)> = HashSet::new();

    // Check all YAML verbs with behavior: plugin
    let runtime_reg = runtime_registry();
    for verb in runtime_reg.all_verbs() {
        if let RuntimeBehavior::Plugin(_handler) = &verb.behavior {
            let key = (verb.domain.clone(), verb.verb.clone());

            if custom_ops.has(&verb.domain, &verb.verb) {
                result.covered.push(key.clone());
                referenced_ops.insert(key);
            } else {
                result.yaml_missing_op.push(key);
            }
        }
    }

    // Find ops that aren't referenced by any YAML plugin verb
    for (domain, verb, _rationale) in custom_ops.list() {
        let key = (domain.to_string(), verb.to_string());
        if !referenced_ops.contains(&key) {
            result.op_missing_yaml.push(key);
        }
    }

    // Sort results for deterministic output
    result.covered.sort();
    result.yaml_missing_op.sort();
    result.op_missing_yaml.sort();

    result
}

/// Verify plugin verb coverage and panic on fatal mismatches.
///
/// This is the strict version that should be called at startup.
/// - Panics if any YAML plugin verb is missing a registered CustomOp
/// - Logs warnings for orphaned CustomOps (ops without YAML definitions)
///
/// # Panics
/// Panics if any YAML plugin verb has no corresponding registered CustomOp.
pub fn verify_plugin_verb_coverage_strict(custom_ops: &CustomOperationRegistry) {
    let result = verify_plugin_verb_coverage(custom_ops);

    // Log warnings for orphaned ops (not fatal, but should be cleaned up)
    for (domain, verb) in &result.op_missing_yaml {
        tracing::warn!(
            "CustomOp {}.{} registered but no YAML plugin verb defined — \
             consider adding YAML definition or removing the op",
            domain,
            verb
        );
    }

    // Panic on missing ops (fatal - YAML promises behavior we can't deliver)
    if !result.yaml_missing_op.is_empty() {
        let missing: Vec<String> = result
            .yaml_missing_op
            .iter()
            .map(|(d, v)| format!("{}.{}", d, v))
            .collect();

        panic!(
            "YAML plugin verb(s) missing registered CustomOp: [{}]. \
             Either add #[register_custom_op] to the op struct or fix the YAML behavior.",
            missing.join(", ")
        );
    }

    tracing::info!("{}", result.summary());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = CustomOperationRegistry::new();
        // Entity ghost lifecycle operations
        assert!(registry.has("entity", "ghost"));
        assert!(registry.has("entity", "identify"));
        assert!(registry.has("document", "catalog"));
        assert!(registry.has("document", "extract"));
        // Attribute operations (document-attribute catalogue)
        assert!(registry.has("attribute", "list-sources"));
        assert!(registry.has("attribute", "list-sinks"));
        assert!(registry.has("attribute", "trace-lineage"));
        assert!(registry.has("attribute", "list-by-document"));
        assert!(registry.has("attribute", "check-coverage"));
        assert!(registry.has("document", "list-attributes"));
        assert!(registry.has("document", "check-extraction-coverage"));
        assert!(registry.has("ubo", "calculate"));
        assert!(registry.has("screening", "pep"));
        assert!(registry.has("screening", "sanctions"));
        // Service resource instance operations
        assert!(registry.has("service-resource", "provision"));
        assert!(registry.has("service-resource", "set-attr"));
        assert!(registry.has("service-resource", "activate"));
        assert!(registry.has("service-resource", "suspend"));
        assert!(registry.has("service-resource", "decommission"));
        assert!(registry.has("service-resource", "validate-attrs"));
        // Delivery operations are now CRUD-based (delivery.yaml)
        // Custody operations
        assert!(registry.has("subcustodian", "lookup"));
        assert!(registry.has("cbu-custody", "lookup-ssi"));
        assert!(registry.has("cbu-custody", "validate-booking-coverage"));
        assert!(registry.has("cbu-custody", "derive-required-coverage"));
        // CBU operations
        assert!(registry.has("cbu", "add-product"));
        assert!(registry.has("cbu", "inspect"));
        assert!(registry.has("cbu", "delete-cascade"));
        // CBU Role operations (Role Taxonomy V2)
        assert!(registry.has("cbu", "assign-ownership"));
        assert!(registry.has("cbu", "assign-control"));
        assert!(registry.has("cbu", "assign-trust-role"));
        assert!(registry.has("cbu", "assign-fund-role"));
        assert!(registry.has("cbu", "assign-service-provider"));
        assert!(registry.has("cbu", "assign-signatory"));
        assert!(registry.has("cbu", "validate-roles"));
        // Trading Matrix operations
        assert!(registry.has("investment-manager", "find-for-trade"));
        assert!(registry.has("pricing-config", "find-for-instrument"));
        assert!(registry.has("sla", "list-open-breaches"));
        // Lifecycle operations
        assert!(registry.has("lifecycle", "provision"));
        assert!(registry.has("lifecycle", "analyze-gaps"));
        assert!(registry.has("lifecycle", "check-readiness"));
        assert!(registry.has("lifecycle", "discover"));
        assert!(registry.has("lifecycle", "generate-plan"));
        assert!(registry.has("lifecycle", "execute-plan"));
        // Matrix-Overlay operations
        assert!(registry.has("matrix-overlay", "effective-matrix"));
        assert!(registry.has("matrix-overlay", "unified-gaps"));
        assert!(registry.has("matrix-overlay", "compare-products"));
        // Regulatory operations
        assert!(registry.has("regulatory.registration", "verify"));
        assert!(registry.has("regulatory.status", "check"));
        // Outstanding Request operations
        assert!(registry.has("request", "create"));
        assert!(registry.has("request", "overdue"));
        assert!(registry.has("request", "fulfill"));
        assert!(registry.has("request", "cancel"));
        assert!(registry.has("request", "extend"));
        assert!(registry.has("request", "remind"));
        assert!(registry.has("request", "escalate"));
        assert!(registry.has("request", "waive"));
        // Document request operations
        assert!(registry.has("document", "request"));
        assert!(registry.has("document", "upload"));
        assert!(registry.has("document", "waive-request"));
        // KYC case operations
        assert!(registry.has("kyc-case", "create"));
        assert!(registry.has("kyc-case", "close"));
        assert!(registry.has("kyc-case", "summarize"));
        assert!(registry.has("entity-workstream", "state"));
        // UBO chain computation (Phase 2.3)
        assert!(registry.has("ubo", "compute-chains"));
        // UBO removal operations (Phase 7)
        assert!(registry.has("ubo", "mark-deceased"));
        assert!(registry.has("ubo", "convergence-supersede"));
        assert!(registry.has("ubo", "transfer-control"));
        assert!(registry.has("ubo", "waive-verification"));
        // Team operations (only transfer-member is a plugin, rest are CRUD)
        assert!(registry.has("team", "transfer-member"));
        // Access Review operations (complex multi-step transactional operations)
        assert!(registry.has("access-review", "populate-campaign"));
        assert!(registry.has("access-review", "launch-campaign"));
        assert!(registry.has("access-review", "revoke-access"));
        assert!(registry.has("access-review", "bulk-confirm"));
        assert!(registry.has("access-review", "confirm-all-clean"));
        assert!(registry.has("access-review", "attest"));
        assert!(registry.has("access-review", "process-deadline"));
        assert!(registry.has("access-review", "send-reminders"));
        // Temporal operations (point-in-time queries)
        assert!(registry.has("temporal", "ownership-as-of"));
        assert!(registry.has("temporal", "ubo-chain-as-of"));
        assert!(registry.has("temporal", "cbu-relationships-as-of"));
        assert!(registry.has("temporal", "cbu-roles-as-of"));
        assert!(registry.has("temporal", "cbu-state-at-approval"));
        assert!(registry.has("temporal", "relationship-history"));
        assert!(registry.has("temporal", "entity-history"));
        assert!(registry.has("temporal", "compare-ownership"));
        // GLEIF operations (LEI data enrichment)
        assert!(registry.has("gleif", "enrich"));
        assert!(registry.has("gleif", "search"));
        assert!(registry.has("gleif", "import-tree"));
        assert!(registry.has("gleif", "refresh"));
        assert!(registry.has("gleif", "get-record"));
        assert!(registry.has("gleif", "get-parent"));
        assert!(registry.has("gleif", "get-children"));
        assert!(registry.has("gleif", "trace-ownership"));
        assert!(registry.has("gleif", "get-managed-funds"));
        assert!(registry.has("gleif", "resolve-successor"));
        // BODS operations (UBO discovery)
        assert!(registry.has("bods", "discover-ubos"));
        assert!(registry.has("bods", "import"));
        assert!(registry.has("bods", "get-statement"));
        assert!(registry.has("bods", "find-by-lei"));
        assert!(registry.has("bods", "list-ownership"));
        assert!(registry.has("bods", "sync-from-gleif"));
        // View operations (session scope and selection management)
        assert!(registry.has("view", "universe"));
        assert!(registry.has("view", "book"));
        assert!(registry.has("view", "cbu"));
        assert!(registry.has("view", "entity-forest"));
        assert!(registry.has("view", "refine"));
        assert!(registry.has("view", "clear"));
        assert!(registry.has("view", "set-selection"));
        assert!(registry.has("view", "set-layout"));
        assert!(registry.has("view", "read-status"));
        assert!(registry.has("view", "read-selection-info"));
        // Zoom navigation (fractal taxonomy navigation)
        assert!(registry.has("view", "zoom-in"));
        assert!(registry.has("view", "zoom-out"));
        assert!(registry.has("view", "navigate-back-to"));
        assert!(registry.has("view", "read-breadcrumbs"));

        // KYC Control Enhancement: Capital operations
        assert!(registry.has("capital", "transfer"));
        assert!(registry.has("capital", "reconcile"));
        assert!(registry.has("capital", "get-ownership-chain"));
        assert!(registry.has("capital", "issue-shares"));
        assert!(registry.has("capital", "cancel-shares"));
        // Capital Structure & Ownership Model operations (Migration 013)
        assert!(registry.has("capital", "share-class.create"));
        assert!(registry.has("capital", "share-class.get-supply"));
        assert!(registry.has("capital", "issue.initial"));
        assert!(registry.has("capital", "issue.new"));
        assert!(registry.has("capital", "split"));
        assert!(registry.has("capital", "buyback"));
        assert!(registry.has("capital", "cancel"));
        assert!(registry.has("capital", "cap-table"));
        assert!(registry.has("capital", "holders"));
        // Dilution instrument operations
        assert!(registry.has("capital", "dilution.grant-options"));
        assert!(registry.has("capital", "dilution.issue-warrant"));
        assert!(registry.has("capital", "dilution.create-safe"));
        assert!(registry.has("capital", "dilution.create-convertible-note"));
        assert!(registry.has("capital", "dilution.exercise"));
        assert!(registry.has("capital", "dilution.forfeit"));
        assert!(registry.has("capital", "dilution.list"));
        assert!(registry.has("capital", "dilution.get-summary"));
        // Ownership operations
        assert!(registry.has("ownership", "compute"));
        assert!(registry.has("ownership", "snapshot.list"));
        assert!(registry.has("ownership", "list-control-positions"));
        assert!(registry.has("ownership", "find-controller"));
        assert!(registry.has("ownership", "reconcile"));
        assert!(registry.has("ownership", "reconcile.findings"));
        assert!(registry.has("ownership", "analyze-gaps"));
        assert!(registry.has("ownership", "trace-chain"));
        // KYC Control Enhancement: Board operations
        assert!(registry.has("board", "analyze-control"));
        // KYC Control Enhancement: Trust operations
        assert!(registry.has("trust", "analyze-control"));
        assert!(registry.has("trust", "identify-ubos"));
        assert!(registry.has("trust", "classify"));
        // KYC Control Enhancement: Partnership operations
        assert!(registry.has("partnership", "record-contribution"));
        assert!(registry.has("partnership", "record-distribution"));
        assert!(registry.has("partnership", "reconcile"));
        assert!(registry.has("partnership", "analyze-control"));
        // KYC Control Enhancement: Tollgate operations
        assert!(registry.has("tollgate", "evaluate"));
        assert!(registry.has("tollgate", "get-metrics"));
        assert!(registry.has("tollgate", "override"));
        assert!(registry.has("tollgate", "get-decision-readiness"));
        // Coverage computation (KYC prong analysis)
        assert!(registry.has("coverage", "compute"));
        // KYC Control Enhancement: Unified control operations
        assert!(registry.has("control", "analyze"));
        assert!(registry.has("control", "build-graph"));
        assert!(registry.has("control", "compute-controllers"));
        assert!(registry.has("control", "identify-ubos"));
        assert!(registry.has("control", "trace-chain"));
        assert!(registry.has("control", "reconcile-ownership"));
        // Trading Profile document construction operations (Phase 1)
        assert!(registry.has("trading-profile", "create-draft"));
        assert!(registry.has("trading-profile", "add-instrument-class"));
        assert!(registry.has("trading-profile", "remove-instrument-class"));
        assert!(registry.has("trading-profile", "add-market"));
        assert!(registry.has("trading-profile", "remove-market"));
        assert!(registry.has("trading-profile", "add-standing-instruction"));
        assert!(registry.has("trading-profile", "remove-standing-instruction"));
        assert!(registry.has("trading-profile", "add-booking-rule"));
        assert!(registry.has("trading-profile", "remove-booking-rule"));
        // Versioned document lifecycle operations (Phase 7)
        assert!(registry.has("trading-profile", "create-new-version"));
        // Investor lifecycle operations (TA KYC-as-a-Service)
        assert!(registry.has("investor", "request-documents"));
        assert!(registry.has("investor", "start-kyc"));
        assert!(registry.has("investor", "approve-kyc"));
        assert!(registry.has("investor", "reject-kyc"));
        assert!(registry.has("investor", "mark-eligible"));
        assert!(registry.has("investor", "record-subscription"));
        assert!(registry.has("investor", "activate"));
        assert!(registry.has("investor", "start-redemption"));
        assert!(registry.has("investor", "complete-redemption"));
        assert!(registry.has("investor", "offboard"));
        assert!(registry.has("investor", "suspend"));
        assert!(registry.has("investor", "reinstate"));
        assert!(registry.has("investor", "count-by-state"));
        // Agent control operations (agent mode lifecycle, checkpoints)
        assert!(registry.has("agent", "start"));
        assert!(registry.has("agent", "pause"));
        assert!(registry.has("agent", "resume"));
        assert!(registry.has("agent", "stop"));
        assert!(registry.has("agent", "confirm-decision"));
        assert!(registry.has("agent", "reject-decision"));
        assert!(registry.has("agent", "select-decision-option"));
        assert!(registry.has("agent", "read-status"));
        assert!(registry.has("agent", "read-history"));
        assert!(registry.has("agent", "set-selection-threshold"));
        assert!(registry.has("agent", "set-execution-mode"));
        // Research source loader operations
        assert!(registry.has("research.sources", "list"));
        assert!(registry.has("research.sources", "info"));
        assert!(registry.has("research.sources", "search"));
        assert!(registry.has("research.sources", "fetch"));
        assert!(registry.has("research.sources", "find-for-jurisdiction"));
        // Companies House operations
        assert!(registry.has("research.companies-house", "search"));
        assert!(registry.has("research.companies-house", "fetch-company"));
        assert!(registry.has("research.companies-house", "fetch-psc"));
        assert!(registry.has("research.companies-house", "fetch-officers"));
        assert!(registry.has("research.companies-house", "import-company"));
        // SEC EDGAR operations
        assert!(registry.has("research.sec-edgar", "search"));
        assert!(registry.has("research.sec-edgar", "fetch-company"));
        assert!(registry.has("research.sec-edgar", "fetch-beneficial-owners"));
        assert!(registry.has("research.sec-edgar", "fetch-filings"));
        assert!(registry.has("research.sec-edgar", "import-company"));
        // ManCo / Governance Controller operations
        assert!(registry.has("manco", "group.derive"));
        assert!(registry.has("manco", "group.cbus"));
        assert!(registry.has("manco", "group.for-cbu"));
        assert!(registry.has("manco", "primary-controller"));
        assert!(registry.has("manco", "control-chain"));
        // Ownership domain operations (bridges + control links + refresh)
        assert!(registry.has("ownership", "bridge.manco-roles"));
        assert!(registry.has("ownership", "bridge.gleif-fund-managers"));
        assert!(registry.has("ownership", "bridge.bods-ownership"));
        assert!(registry.has("ownership", "control-links.compute"));
        assert!(registry.has("ownership", "refresh"));
    }

    #[test]
    fn test_registry_list() {
        let registry = CustomOperationRegistry::new();
        let ops = registry.list();
        // Count updated after KYC Control Enhancement operations added
        // Verify we have a reasonable number of operations registered
        assert!(
            ops.len() >= 80,
            "Expected at least 80 operations, got {}",
            ops.len()
        );
    }

    #[test]
    fn test_plugin_verb_coverage() {
        // This test verifies that all YAML plugin verbs have registered CustomOps
        let registry = CustomOperationRegistry::new();
        let result = super::verify_plugin_verb_coverage(&registry);

        // Print diagnostic info for debugging
        if !result.yaml_missing_op.is_empty() {
            eprintln!("YAML plugin verbs missing CustomOp:");
            for (domain, verb) in &result.yaml_missing_op {
                eprintln!("  - {}.{}", domain, verb);
            }
        }

        if !result.op_missing_yaml.is_empty() {
            eprintln!("CustomOps without YAML plugin definition:");
            for (domain, verb) in &result.op_missing_yaml {
                eprintln!("  - {}.{}", domain, verb);
            }
        }

        eprintln!("{}", result.summary());

        // The strict assertion: all YAML plugin verbs must have ops
        assert!(
            result.is_ok(),
            "YAML plugin verbs missing CustomOps: {:?}",
            result.yaml_missing_op
        );
    }

    #[test]
    fn test_registry_list_is_sorted() {
        // Verify deterministic ordering (sorted by domain, verb)
        let registry = CustomOperationRegistry::new();
        let list = registry.list();

        for i in 1..list.len() {
            let prev = (list[i - 1].0, list[i - 1].1);
            let curr = (list[i].0, list[i].1);
            assert!(
                prev <= curr,
                "Registry list not sorted: ({}, {}) > ({}, {})",
                prev.0,
                prev.1,
                curr.0,
                curr.1
            );
        }
    }

    #[test]
    fn test_registry_has_inventory_ops() {
        // Verify ops registered via #[register_custom_op] macro are present
        let registry = CustomOperationRegistry::new();

        // Document ops (use macro)
        assert!(registry.has("document", "solicit"));
        assert!(registry.has("document", "verify"));
        assert!(registry.has("document", "reject"));

        // Investor role ops (convenience verbs added in PR1)
        assert!(registry.has("investor-role", "mark-as-nominee"));
        assert!(registry.has("investor-role", "mark-as-fof"));
        assert!(registry.has("investor-role", "mark-as-master-pool"));
        assert!(registry.has("investor-role", "mark-as-end-investor"));

        // Manco ops
        assert!(registry.has("manco", "book.summary"));

        // Pack operations (Journey Pack lifecycle)
        assert!(registry.has("pack", "select"));
        assert!(registry.has("pack", "answer"));
        // Booking principal operations
        assert!(registry.has("legal-entity", "create"));
        assert!(registry.has("legal-entity", "update"));
        assert!(registry.has("legal-entity", "list"));
        assert!(registry.has("booking-location", "create"));
        assert!(registry.has("booking-location", "update"));
        assert!(registry.has("booking-location", "list"));
        assert!(registry.has("booking-principal", "create"));
        assert!(registry.has("booking-principal", "update"));
        assert!(registry.has("booking-principal", "retire"));
        assert!(registry.has("booking-principal", "evaluate"));
        assert!(registry.has("booking-principal", "select"));
        assert!(registry.has("booking-principal", "explain"));
        assert!(registry.has("booking-principal", "coverage-matrix"));
        assert!(registry.has("booking-principal", "gap-report"));
        assert!(registry.has("booking-principal", "impact-analysis"));
        assert!(registry.has("client-principal-relationship", "record"));
        assert!(registry.has("client-principal-relationship", "terminate"));
        assert!(registry.has("client-principal-relationship", "list"));
        assert!(registry.has("client-principal-relationship", "cross-sell-check"));
        assert!(registry.has("service-availability", "set"));
        assert!(registry.has("service-availability", "list"));
        assert!(registry.has("service-availability", "list-by-principal"));
        assert!(registry.has("ruleset", "create"));
        assert!(registry.has("ruleset", "publish"));
        assert!(registry.has("ruleset", "retire"));
        assert!(registry.has("rule", "add"));
        assert!(registry.has("rule", "update"));
        assert!(registry.has("rule", "disable"));
        assert!(registry.has("contract-pack", "create"));
        assert!(registry.has("contract-pack", "add-template"));
    }
}
