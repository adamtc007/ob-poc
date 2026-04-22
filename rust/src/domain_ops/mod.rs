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

// Re-export DSL types for use by operation implementations
pub use crate::dsl_v2::ast::VerbCall;
pub use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

// Phase 5c — entity_query relocated. Types accessed via dsl_runtime::domain_ops::entity_query.
// Phase 5a composite-blocker #5 — kyc_case_ops relocated. Types accessed via
// dsl_runtime::domain_ops::kyc_case_ops; no external ob-poc consumer imports them.
// Phase 5c — lifecycle_ops relocated to `dsl-runtime::domain_ops::lifecycle_ops`.
// Registration flows through inventory automatically; external ob-poc code doesn't
// import these types directly.
pub use onboarding::OnboardingAutoComplete;
// Phase 5c — refdata_loader relocated. Types accessed via dsl_runtime::domain_ops::refdata_loader.
// Phase 5c — refdata_ops relocated. Types accessed via dsl_runtime::domain_ops::refdata_ops.
pub use request_ops::{
    DocumentRequest, DocumentUpload, DocumentWaive, RequestCancel, RequestCreate,
    RequestEscalate, RequestExtend, RequestFulfill, RequestOverdue, RequestRemind,
    RequestWaive,
};

// Phase 5a — semantic_ops relocated to dsl-runtime. Inventory registration
// automatic; external ob-poc code does not import these types directly.
pub use template_ops::{
    TemplateBatch, TemplateBatchResult, TemplateInvoke, TemplateInvokeResult,
};

pub use trading_profile::{
    TradingProfileActivate, TradingProfileAddAllowedCurrency, TradingProfileAddBookingRule,
    TradingProfileAddComponent, TradingProfileAddCsaCollateral, TradingProfileAddCsaConfig,
    TradingProfileAddImMandate, TradingProfileAddInstrumentClass, TradingProfileAddIsdaConfig,
    TradingProfileAddIsdaCoverage, TradingProfileAddMarket, TradingProfileAddSsi,
    TradingProfileApprove, TradingProfileArchive, TradingProfileCloneTo,
    TradingProfileCreateDraft, TradingProfileCreateNewVersion, TradingProfileDiff,
    TradingProfileGetActive, TradingProfileImportVerb, TradingProfileLinkCsaSsi,
    TradingProfileMaterialize, TradingProfileReject, TradingProfileRemoveBookingRule,
    TradingProfileRemoveComponent, TradingProfileRemoveCsaConfig, TradingProfileRemoveImMandate,
    TradingProfileRemoveInstrumentClass, TradingProfileRemoveIsdaConfig, TradingProfileRemoveMarket,
    TradingProfileRemoveSsi, TradingProfileSetBaseCurrency, TradingProfileSubmit,
    TradingProfileUpdateImScope, TradingProfileValidateCoverage, TradingProfileValidateGoLiveReady,
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
    GleifEnrich, GleifGetChildren, GleifGetManagedFunds, GleifGetManager, GleifGetMasterFund,
    GleifGetParent, GleifGetRecord, GleifGetUmbrella, GleifImportManagedFunds,
    GleifImportToClientGroup, GleifImportTree, GleifLookup, GleifLookupByIsin, GleifRefresh,
    GleifResolveSuccessor, GleifSearch, GleifTraceOwnership,
};

// =============================================================================
// YAML ↔ OP SANITY CHECK
// =============================================================================
//
// Post Phase 5c-migrate slice #80 every plugin verb lives in the
// `SemOsVerbOpRegistry` — either inside `sem_os_postgres::ops::*` (registered
// through `sem_os_postgres::ops::build_registry()`) or inside
// `rust/src/domain_ops/` for the Pattern B ops that reach into ob-poc
// internals (registered through [`extend_registry`] below).
//
// The strict coverage check (`verify_plugin_verb_coverage` / `_strict`)
// that used to live here has been removed alongside the legacy
// `CustomOperationRegistry`. Equivalent lint coverage is now the
// responsibility of `cargo x verbs lint` in `rust/xtask/src/verbs.rs`
// which builds its handler list directly from the SemOS registry.

/// Register ob-poc's Pattern B plugin ops into the SemOS registry.
///
/// Phase 5c-migrate Phase B Pattern B slice (#72+) ports ops that reach
/// into ob-poc internals from `CustomOperation` + `inventory` to
/// `SemOsVerbOp`, but keeps the op bodies inside `rust/src/domain_ops/`
/// because the internals (`crate::database::*`, `crate::ontology::*`,
/// `crate::dsl_v2::*`, `crate::sem_os_runtime::*`) can't be inverted
/// behind a service trait without a disproportionate refactor. This
/// function is called from `ob-poc-web::main` right after
/// `sem_os_postgres::ops::build_registry()` to merge the Pattern B
/// ops into the single canonical registry.
pub fn extend_registry(registry: &mut sem_os_postgres::ops::SemOsVerbOpRegistry) {
    use std::sync::Arc;

    // Phase B Pattern B slice #72: onboarding.auto-complete (bridges to
    // crate::database::derive_semantic_state + crate::ontology::SemanticStageRegistry
    // + crate::dsl_v2::executor::DslExecutor).
    registry.register(Arc::new(onboarding::OnboardingAutoComplete));

    // Phase B Pattern B slice #73: bpmn.* gRPC pass-through verbs
    // (bridges to crate::bpmn_integration::client).
    registry.register(Arc::new(bpmn_lite_ops::BpmnCompile));
    registry.register(Arc::new(bpmn_lite_ops::BpmnStart));
    registry.register(Arc::new(bpmn_lite_ops::BpmnSignal));
    registry.register(Arc::new(bpmn_lite_ops::BpmnCancel));
    registry.register(Arc::new(bpmn_lite_ops::BpmnInspect));

    // Phase B Pattern B slice #74: template.* (template invocation +
    // batch execution; bridges to crate::templates::TemplateExpander,
    // crate::dsl_v2::{parser, execution_plan, execution::DslExecutor,
    // batch_executor::BatchExecutor, runtime_registry}).
    registry.register(Arc::new(template_ops::TemplateInvoke));
    registry.register(Arc::new(template_ops::TemplateBatch));

    // Phase B Pattern B slice #75: research.{sources,companies-house,
    // sec-edgar}.* (15 verbs — external source loaders bridging to
    // crate::research::sources::* + crate::research::companies_house::*
    // + crate::research::sec_edgar::*).
    registry.register(Arc::new(source_loader_ops::SourcesList));
    registry.register(Arc::new(source_loader_ops::SourcesInfo));
    registry.register(Arc::new(source_loader_ops::SourcesSearch));
    registry.register(Arc::new(source_loader_ops::SourcesFetch));
    registry.register(Arc::new(source_loader_ops::SourcesFindForJurisdiction));
    registry.register(Arc::new(source_loader_ops::CompaniesHouseSearch));
    registry.register(Arc::new(source_loader_ops::CompaniesHouseFetchCompany));
    registry.register(Arc::new(source_loader_ops::CompaniesHouseFetchPsc));
    registry.register(Arc::new(source_loader_ops::CompaniesHouseFetchOfficers));
    registry.register(Arc::new(source_loader_ops::CompaniesHouseImportCompany));
    registry.register(Arc::new(source_loader_ops::SecEdgarSearch));
    registry.register(Arc::new(source_loader_ops::SecEdgarFetchCompany));
    registry.register(Arc::new(source_loader_ops::SecEdgarFetchBeneficialOwners));
    registry.register(Arc::new(source_loader_ops::SecEdgarFetchFilings));
    registry.register(Arc::new(source_loader_ops::SecEdgarImportCompany));

    // Phase B Pattern B slice #76: request.* + document.* (11 verbs —
    // outstanding requests + blocker auto-unblock + bpmn-lite signal
    // routing; bridges to crate::bpmn_integration).
    registry.register(Arc::new(request_ops::RequestCreate));
    registry.register(Arc::new(request_ops::RequestOverdue));
    registry.register(Arc::new(request_ops::RequestFulfill));
    registry.register(Arc::new(request_ops::RequestCancel));
    registry.register(Arc::new(request_ops::RequestExtend));
    registry.register(Arc::new(request_ops::RequestRemind));
    registry.register(Arc::new(request_ops::RequestEscalate));
    registry.register(Arc::new(request_ops::RequestWaive));
    registry.register(Arc::new(request_ops::DocumentRequest));
    registry.register(Arc::new(request_ops::DocumentUpload));
    registry.register(Arc::new(request_ops::DocumentWaive));

    // Phase B Pattern B slice #77: gleif.* (17 verbs — LEI lookup,
    // hierarchy import, ownership trace, successor resolution,
    // client-group import; bridges to crate::gleif::{client, service}
    // and crate::dsl_v2::{execution::DslExecutor, executor::ExecutionContext}
    // for recursive idempotent entity/CBU/role creation).
    registry.register(Arc::new(gleif_ops::GleifEnrich));
    registry.register(Arc::new(gleif_ops::GleifSearch));
    registry.register(Arc::new(gleif_ops::GleifImportTree));
    registry.register(Arc::new(gleif_ops::GleifRefresh));
    registry.register(Arc::new(gleif_ops::GleifGetRecord));
    registry.register(Arc::new(gleif_ops::GleifGetParent));
    registry.register(Arc::new(gleif_ops::GleifImportManagedFunds));
    registry.register(Arc::new(gleif_ops::GleifGetChildren));
    registry.register(Arc::new(gleif_ops::GleifTraceOwnership));
    registry.register(Arc::new(gleif_ops::GleifGetManagedFunds));
    registry.register(Arc::new(gleif_ops::GleifResolveSuccessor));
    registry.register(Arc::new(gleif_ops::GleifGetUmbrella));
    registry.register(Arc::new(gleif_ops::GleifGetManager));
    registry.register(Arc::new(gleif_ops::GleifGetMasterFund));
    registry.register(Arc::new(gleif_ops::GleifLookupByIsin));
    registry.register(Arc::new(gleif_ops::GleifImportToClientGroup));
    registry.register(Arc::new(gleif_ops::GleifLookup));

    // Phase B Pattern B slice #78: booking_principal_ops (32 verbs —
    // legal-entity / rule-field / booking-location / booking-principal
    // / client-principal-relationship / service-availability / ruleset
    // / rule / contract-pack). Bridges to
    // crate::database::booking_principal_repository +
    // crate::domain_ops::rule_evaluator; ob-poc API DTOs in
    // crate::api::booking_principal_types stay in ob-poc.
    registry.register(Arc::new(booking_principal_ops::LegalEntityCreate));
    registry.register(Arc::new(booking_principal_ops::LegalEntityUpdate));
    registry.register(Arc::new(booking_principal_ops::LegalEntityList));
    registry.register(Arc::new(booking_principal_ops::RuleFieldRegister));
    registry.register(Arc::new(booking_principal_ops::RuleFieldList));
    registry.register(Arc::new(booking_principal_ops::BookingLocationCreate));
    registry.register(Arc::new(booking_principal_ops::BookingLocationUpdate));
    registry.register(Arc::new(booking_principal_ops::BookingLocationList));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalCreate));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalUpdate));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalRetire));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalEvaluate));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalSelect));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalExplain));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalCoverageMatrix));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalGapReport));
    registry.register(Arc::new(booking_principal_ops::BookingPrincipalImpactAnalysis));
    registry.register(Arc::new(booking_principal_ops::ClientPrincipalRelationshipRecord));
    registry.register(Arc::new(booking_principal_ops::ClientPrincipalRelationshipTerminate));
    registry.register(Arc::new(booking_principal_ops::ClientPrincipalRelationshipList));
    registry.register(Arc::new(booking_principal_ops::ClientPrincipalRelationshipCrossSellCheck));
    registry.register(Arc::new(booking_principal_ops::ServiceAvailabilitySet));
    registry.register(Arc::new(booking_principal_ops::ServiceAvailabilityList));
    registry.register(Arc::new(booking_principal_ops::ServiceAvailabilityListByPrincipal));
    registry.register(Arc::new(booking_principal_ops::RulesetCreate));
    registry.register(Arc::new(booking_principal_ops::RulesetPublish));
    registry.register(Arc::new(booking_principal_ops::RulesetRetire));
    registry.register(Arc::new(booking_principal_ops::RuleAdd));
    registry.register(Arc::new(booking_principal_ops::RuleUpdate));
    registry.register(Arc::new(booking_principal_ops::RuleDisable));
    registry.register(Arc::new(booking_principal_ops::ContractPackCreate));
    registry.register(Arc::new(booking_principal_ops::ContractPackAddTemplate));

    // Phase B Pattern B slice #79: trading-profile.* (36 verbs — full
    // draft→submit→approve→activate→materialize→archive lifecycle,
    // component CRUD dispatchers, ISDA/CSA/SSI/IM config, validation).
    // Bridges to crate::trading_profile::{ast_db, document_ops}.
    registry.register(Arc::new(trading_profile::TradingProfileImportVerb));
    registry.register(Arc::new(trading_profile::TradingProfileGetActive));
    registry.register(Arc::new(trading_profile::TradingProfileActivate));
    registry.register(Arc::new(trading_profile::TradingProfileMaterialize));
    registry.register(Arc::new(trading_profile::TradingProfileCreateDraft));
    registry.register(Arc::new(trading_profile::TradingProfileAddComponent));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveComponent));
    registry.register(Arc::new(trading_profile::TradingProfileAddInstrumentClass));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveInstrumentClass));
    registry.register(Arc::new(trading_profile::TradingProfileAddMarket));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveMarket));
    registry.register(Arc::new(trading_profile::TradingProfileAddSsi));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveSsi));
    registry.register(Arc::new(trading_profile::TradingProfileAddBookingRule));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveBookingRule));
    registry.register(Arc::new(trading_profile::TradingProfileAddIsdaConfig));
    registry.register(Arc::new(trading_profile::TradingProfileAddIsdaCoverage));
    registry.register(Arc::new(trading_profile::TradingProfileAddCsaConfig));
    registry.register(Arc::new(trading_profile::TradingProfileAddCsaCollateral));
    registry.register(Arc::new(trading_profile::TradingProfileLinkCsaSsi));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveIsdaConfig));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveCsaConfig));
    registry.register(Arc::new(trading_profile::TradingProfileAddImMandate));
    registry.register(Arc::new(trading_profile::TradingProfileUpdateImScope));
    registry.register(Arc::new(trading_profile::TradingProfileRemoveImMandate));
    registry.register(Arc::new(trading_profile::TradingProfileSetBaseCurrency));
    registry.register(Arc::new(trading_profile::TradingProfileAddAllowedCurrency));
    registry.register(Arc::new(trading_profile::TradingProfileDiff));
    registry.register(Arc::new(trading_profile::TradingProfileValidateCoverage));
    registry.register(Arc::new(trading_profile::TradingProfileValidateGoLiveReady));
    registry.register(Arc::new(trading_profile::TradingProfileSubmit));
    registry.register(Arc::new(trading_profile::TradingProfileApprove));
    registry.register(Arc::new(trading_profile::TradingProfileReject));
    registry.register(Arc::new(trading_profile::TradingProfileArchive));
    registry.register(Arc::new(trading_profile::TradingProfileCloneTo));
    registry.register(Arc::new(trading_profile::TradingProfileCreateNewVersion));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_verb_coverage() {
        // Every YAML plugin verb must have a handler registered in the
        // canonical `SemOsVerbOpRegistry` (either from
        // `sem_os_postgres::ops::build_registry()` or appended by
        // [`extend_registry`] for Pattern B ops that stay in `ob-poc`).
        use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};
        use std::collections::HashSet;

        let mut sem_os_registry = sem_os_postgres::ops::build_registry();
        extend_registry(&mut sem_os_registry);
        let sem_os_fqns: HashSet<String> = sem_os_registry.manifest().into_iter().collect();

        let mut missing: Vec<String> = Vec::new();
        let runtime_reg = runtime_registry();
        for verb in runtime_reg.all_verbs() {
            if let RuntimeBehavior::Plugin(_handler) = &verb.behavior {
                let fqn = format!("{}.{}", verb.domain, verb.verb);
                if !sem_os_fqns.contains(&fqn) {
                    missing.push(fqn);
                }
            }
        }
        missing.sort();

        assert!(
            missing.is_empty(),
            "YAML plugin verbs missing a SemOsVerbOp registration: {:?}",
            missing
        );
    }

    #[test]
    fn test_extend_registry_adds_pattern_b_ops() {
        // Smoke test — `extend_registry()` should register at least the
        // Pattern B ops listed in the Phase 5c-migrate slice comments.
        let mut registry = sem_os_postgres::ops::SemOsVerbOpRegistry::empty();
        extend_registry(&mut registry);
        assert!(
            registry.has("onboarding.auto-complete"),
            "extend_registry should register onboarding.auto-complete"
        );
        assert!(
            registry.has("template.invoke"),
            "extend_registry should register template.invoke"
        );
        assert!(
            registry.has("gleif.enrich"),
            "extend_registry should register gleif.enrich"
        );
        // Reasonable lower bound for Pattern B ops (7 domains × handful per).
        assert!(
            registry.len() >= 100,
            "extend_registry should add at least 100 Pattern B ops, got {}",
            registry.len()
        );
    }
}

