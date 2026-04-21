//! Semantic OS verb ops — the post-5c-migrate home for plugin verb
//! implementations.
//!
//! # Why here
//!
//! The op trait needs access to `TransactionScope`, `VerbExecutionContext`,
//! and `VerbExecutionOutcome` (all in `dsl-runtime`) plus `Principal` /
//! `SemOsError` (in `sem_os_core`). `sem_os_postgres` is the only crate
//! upstream of the composition plane that sees all four — it already
//! depends on `dsl-runtime`, which transitively depends on `sem_os_core`.
//!
//! # Rinse-and-repeat pattern
//!
//! Phase A of the relocation (this file + the registry + the dispatcher
//! branch in `ObPocVerbExecutor`) is pure plumbing. Phase B migrates op
//! bodies one domain at a time, YAML-first: read `config/verbs/<domain>.yaml`,
//! write a fresh [`SemOsVerbOp`] impl in this module tree, register it at
//! startup, delete the corresponding legacy `CustomOperation` impl from
//! `dsl-runtime::domain_ops`. The legacy fallback in
//! `dispatch_plugin_via_execute_json` absorbs every unmigrated verb until
//! the migration closes out; the final cleanup slice strips the fallback,
//! the `CustomOperation` trait, the `inventory` registry, and every file
//! under `rust/crates/dsl-runtime/src/domain_ops/` and
//! `rust/src/domain_ops/`.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

pub mod access_review;
pub mod affinity;
pub mod attribute;
pub mod audit;
pub mod board;
pub mod bods;
pub mod cbu_role;
pub mod changeset;
pub mod constellation;
pub mod control;
pub mod control_compute;
pub mod coverage_compute;
pub mod custody;
pub mod docs_bundle;
pub mod economic_exposure;
pub mod edge;
pub mod entity;
pub mod evidence;
pub mod focus;
pub mod governance;
pub mod graph_validate;
pub mod import_run;
pub mod investor;
pub mod investor_role;
pub mod lifecycle;
pub mod maintenance;
pub mod manco;
pub mod matrix_overlay;
pub mod nav;
pub mod observation;
pub mod outreach;
pub mod outreach_plan;
pub mod ownership;
pub mod pack_answer;
pub mod partnership;
pub mod pack_select;
pub mod phrase;
pub mod refdata;
pub mod refdata_loader;
pub mod registry;
pub mod registry_ops;
pub mod regulatory;
pub mod remediation;
pub mod requirement;
pub mod research_normalize;
pub mod research_workflow;
pub mod schema;
pub mod screening;
pub mod semantic;
pub mod service_pipeline;
pub mod service_resource;
pub mod shared_atom;
pub mod session;
pub mod state;
pub mod stewardship_helper;
pub mod team;
pub mod temporal;
pub mod tollgate;
pub mod tollgate_evaluate;
pub mod trading_matrix;
pub mod trading_profile_ca;
pub mod trust;
pub mod ubo_analysis;
pub mod ubo_compute;
pub mod ubo_graph;
pub mod ubo_registry;
pub mod verify;
pub mod view;

pub use registry::SemOsVerbOpRegistry;

/// Build the canonical [`SemOsVerbOpRegistry`] with every op currently
/// registered in this module tree. Called from `ob-poc-web::main` at
/// startup AND from `ob-poc` coverage tests, so the FQN set stays in
/// sync automatically — any op added here becomes covered without
/// touching the tests.
pub fn build_registry() -> SemOsVerbOpRegistry {
    use std::sync::Arc;

    let mut registry = SemOsVerbOpRegistry::empty();

    // Phase B slice #1: pack domain.
    registry.register(Arc::new(pack_select::PackSelect));
    registry.register(Arc::new(pack_answer::PackAnswer));

    // Phase B slice #2: nav domain.
    registry.register(Arc::new(nav::Drill));
    registry.register(Arc::new(nav::ZoomOut));
    registry.register(Arc::new(nav::Select));
    registry.register(Arc::new(nav::SetClusterType));
    registry.register(Arc::new(nav::SetLens));
    registry.register(Arc::new(nav::HistoryBack));
    registry.register(Arc::new(nav::HistoryForward));

    // Phase B slice #3: constellation domain (service-dispatch Category D).
    registry.register(Arc::new(constellation::Hydrate));
    registry.register(Arc::new(constellation::Summary));

    // Phase B slice #4: phrase domain (service-dispatch via PhraseService).
    registry.register(Arc::new(phrase::ObserveMisses));
    registry.register(Arc::new(phrase::CoverageReport));
    registry.register(Arc::new(phrase::CheckCollisions));
    registry.register(Arc::new(phrase::Propose));
    registry.register(Arc::new(phrase::BatchPropose));
    registry.register(Arc::new(phrase::ReviewProposals));
    registry.register(Arc::new(phrase::Approve));
    registry.register(Arc::new(phrase::Reject));
    registry.register(Arc::new(phrase::Defer));

    // Phase B slice #5: view/session/attribute/service_pipeline
    // (service-dispatch batch).
    registry.register(Arc::new(view::Universe));
    registry.register(Arc::new(view::Book));
    registry.register(Arc::new(view::Cbu));
    registry.register(Arc::new(view::EntityForest));
    registry.register(Arc::new(view::Refine));
    registry.register(Arc::new(view::ClearRefinements));
    registry.register(Arc::new(view::Clear));
    registry.register(Arc::new(view::SetSelection));
    registry.register(Arc::new(view::SetLayout));
    registry.register(Arc::new(view::ReadStatus));
    registry.register(Arc::new(view::ReadSelectionInfo));
    registry.register(Arc::new(view::ZoomIn));
    registry.register(Arc::new(view::ZoomOut));
    registry.register(Arc::new(view::NavigateBackTo));
    registry.register(Arc::new(view::ReadBreadcrumbs));

    registry.register(Arc::new(session::Start));
    registry.register(Arc::new(session::LoadUniverse));
    registry.register(Arc::new(session::LoadGalaxy));
    registry.register(Arc::new(session::LoadCluster));
    registry.register(Arc::new(session::LoadSystem));
    registry.register(Arc::new(session::UnloadSystem));
    registry.register(Arc::new(session::FilterJurisdiction));
    registry.register(Arc::new(session::Clear));
    registry.register(Arc::new(session::Undo));
    registry.register(Arc::new(session::Redo));
    registry.register(Arc::new(session::Info));
    registry.register(Arc::new(session::List));
    registry.register(Arc::new(session::SetClient));
    registry.register(Arc::new(session::SetPersona));
    registry.register(Arc::new(session::SetStructure));
    registry.register(Arc::new(session::SetCase));
    registry.register(Arc::new(session::SetMandate));
    registry.register(Arc::new(session::LoadDeal));
    registry.register(Arc::new(session::UnloadDeal));

    registry.register(Arc::new(attribute::AttributeListSources));
    registry.register(Arc::new(attribute::AttributeListSinks));
    registry.register(Arc::new(attribute::AttributeTraceLineage));
    registry.register(Arc::new(attribute::AttributeListByDocument));
    registry.register(Arc::new(attribute::AttributeCheckCoverage));
    registry.register(Arc::new(attribute::DocumentListAttributes));
    registry.register(Arc::new(attribute::DocumentCheckExtractionCoverage));
    registry.register(Arc::new(attribute::AttributeDefineGoverned));
    registry.register(Arc::new(attribute::AttributeDefineInternal));
    registry.register(Arc::new(attribute::AttributeUpdateInternal));
    registry.register(Arc::new(attribute::AttributeDefineDerived));
    registry.register(Arc::new(attribute::AttributeSetEvidenceGrade));
    registry.register(Arc::new(attribute::AttributeDeprecate));
    registry.register(Arc::new(attribute::AttributeInspect));
    registry.register(Arc::new(attribute::DerivationRecomputeStale));
    registry.register(Arc::new(attribute::AttributeBridgeToSemos));

    registry.register(Arc::new(service_pipeline::ServiceIntentCreate));
    registry.register(Arc::new(service_pipeline::ServiceIntentList));
    registry.register(Arc::new(service_pipeline::ServiceIntentSupersede));
    registry.register(Arc::new(service_pipeline::DiscoveryRun));
    registry.register(Arc::new(service_pipeline::DiscoveryExplain));
    registry.register(Arc::new(service_pipeline::AttributeRollup));
    registry.register(Arc::new(service_pipeline::AttributePopulate));
    registry.register(Arc::new(service_pipeline::AttributeGaps));
    registry.register(Arc::new(service_pipeline::AttributeSet));
    registry.register(Arc::new(service_pipeline::ProvisioningRun));
    registry.register(Arc::new(service_pipeline::ProvisioningStatus));
    registry.register(Arc::new(service_pipeline::ReadinessCompute));
    registry.register(Arc::new(service_pipeline::ReadinessExplain));
    registry.register(Arc::new(service_pipeline::PipelineFull));
    registry.register(Arc::new(service_pipeline::ServiceResourceCheckAttributeGaps));
    registry.register(Arc::new(service_pipeline::ServiceResourceSyncDefinitions));

    // Phase B slice #6: sem_os_* stewardship-dispatch family
    // (focus / governance / changeset / audit / registry_ops).

    // focus (6)
    registry.register(Arc::new(focus::Get));
    registry.register(Arc::new(focus::Set));
    registry.register(Arc::new(focus::Render));
    registry.register(Arc::new(focus::Viewport));
    registry.register(Arc::new(focus::Diff));
    registry.register(Arc::new(focus::CaptureManifest));

    // governance (9)
    registry.register(Arc::new(governance::GatePrecheck));
    registry.register(Arc::new(governance::SubmitForReview));
    registry.register(Arc::new(governance::RecordReview));
    registry.register(Arc::new(governance::Validate));
    registry.register(Arc::new(governance::DryRun));
    registry.register(Arc::new(governance::PlanPublish));
    registry.register(Arc::new(governance::Publish));
    registry.register(Arc::new(governance::PublishBatch));
    registry.register(Arc::new(governance::Rollback));

    // changeset (14)
    registry.register(Arc::new(changeset::Compose));
    registry.register(Arc::new(changeset::AddItem));
    registry.register(Arc::new(changeset::RemoveItem));
    registry.register(Arc::new(changeset::RefineItem));
    registry.register(Arc::new(changeset::Suggest));
    registry.register(Arc::new(changeset::ApplyTemplate));
    registry.register(Arc::new(changeset::AttachBasis));
    registry.register(Arc::new(changeset::ValidateEdit));
    registry.register(Arc::new(changeset::CrossReference));
    registry.register(Arc::new(changeset::ImpactAnalysis));
    registry.register(Arc::new(changeset::ResolveConflict));
    registry.register(Arc::new(changeset::Get));
    registry.register(Arc::new(changeset::Diff));
    registry.register(Arc::new(changeset::List));

    // audit (8)
    registry.register(Arc::new(audit::CreatePlan));
    registry.register(Arc::new(audit::AddPlanStep));
    registry.register(Arc::new(audit::ValidatePlan));
    registry.register(Arc::new(audit::ExecutePlanStep));
    registry.register(Arc::new(audit::RecordDecision));
    registry.register(Arc::new(audit::RecordEscalation));
    registry.register(Arc::new(audit::RecordDisambiguation));
    registry.register(Arc::new(audit::RecordObservation));

    // registry (20 = 17 macros + 2 polymorphic + 1 direct SQL)
    registry.register(Arc::new(registry_ops::Search));
    registry.register(Arc::new(registry_ops::ResolveContext));
    registry.register(Arc::new(registry_ops::VerbSurface));
    registry.register(Arc::new(registry_ops::AttributeProducers));
    registry.register(Arc::new(registry_ops::Lineage));
    registry.register(Arc::new(registry_ops::RegulationTrace));
    registry.register(Arc::new(registry_ops::TaxonomyTree));
    registry.register(Arc::new(registry_ops::TaxonomyMembers));
    registry.register(Arc::new(registry_ops::Classify));
    registry.register(Arc::new(registry_ops::DescribeView));
    registry.register(Arc::new(registry_ops::ApplyView));
    registry.register(Arc::new(registry_ops::DescribePolicy));
    registry.register(Arc::new(registry_ops::CoverageReport));
    registry.register(Arc::new(registry_ops::EvidenceFreshness));
    registry.register(Arc::new(registry_ops::EvidenceGaps));
    registry.register(Arc::new(registry_ops::SnapshotHistory));
    registry.register(Arc::new(registry_ops::SnapshotDiff));
    registry.register(Arc::new(registry_ops::DescribeObject));
    registry.register(Arc::new(registry_ops::ListObjects));
    registry.register(Arc::new(registry_ops::ActiveManifest));

    // Phase B slice #7: sem_os_maintenance domain (direct-sqlx Category B).
    registry.register(Arc::new(maintenance::HealthPending));
    registry.register(Arc::new(maintenance::HealthStaleDryruns));
    registry.register(Arc::new(maintenance::Cleanup));
    registry.register(Arc::new(maintenance::BootstrapSeeds));
    registry.register(Arc::new(maintenance::DrainOutbox));
    registry.register(Arc::new(maintenance::ReindexEmbeddings));
    registry.register(Arc::new(maintenance::ValidateSchemaSync));

    // Phase B slice #8: semantic domain (service-dispatch via SemanticStateService).
    registry.register(Arc::new(semantic::GetState));
    registry.register(Arc::new(semantic::ListStages));
    registry.register(Arc::new(semantic::StagesForProduct));
    registry.register(Arc::new(semantic::MissingEntities));
    registry.register(Arc::new(semantic::NextActions));
    registry.register(Arc::new(semantic::PromptContext));

    // Phase B slice #9: affinity navigation (registry.*-for-* verbs).
    registry.register(Arc::new(affinity::VerbsForTable));
    registry.register(Arc::new(affinity::VerbsForAttribute));
    registry.register(Arc::new(affinity::DataForVerb));
    registry.register(Arc::new(affinity::AdjacentVerbs));
    registry.register(Arc::new(affinity::GovernanceGaps));
    registry.register(Arc::new(affinity::DiscoverDsl));

    // Phase B slice #10: team domain (direct-sqlx, multi-step txn).
    registry.register(Arc::new(team::TransferMember));

    // Phase B slice #11: requirement domain (direct-sqlx batch).
    registry.register(Arc::new(requirement::CreateSet));
    registry.register(Arc::new(requirement::ListOutstanding));

    // Phase B slice #12: research-generic normalize (direct-sqlx + sha2/hex).
    registry.register(Arc::new(research_normalize::Normalize));

    // Phase B slice #13: docs-bundle domain.
    registry.register(Arc::new(docs_bundle::Apply));
    registry.register(Arc::new(docs_bundle::ListApplied));
    registry.register(Arc::new(docs_bundle::ListAvailable));

    // Phase B slice #14: remediation domain (cross-workspace helpers).
    registry.register(Arc::new(remediation::ListOpen));
    registry.register(Arc::new(remediation::Defer));
    registry.register(Arc::new(remediation::RevokeDeferral));
    registry.register(Arc::new(remediation::ConfirmExternalCorrection));

    // Phase B slice #16: research.outreach domain (direct-sqlx,
    // 2 plugin verbs — record-response + list-overdue).
    registry.register(Arc::new(outreach::RecordResponse));
    registry.register(Arc::new(outreach::ListOverdue));

    // Phase B slice #17: board domain (direct-sqlx, 1 plugin verb —
    // analyze-control).
    registry.register(Arc::new(board::AnalyzeControl));

    // Phase B slice #18: regulatory domain (ex-`sqlx::query!` macro,
    // rewritten as runtime `sqlx::query_as` — 2 plugin verbs).
    registry.register(Arc::new(regulatory::RegistrationVerify));
    registry.register(Arc::new(regulatory::StatusCheck));

    // Phase B slice #19: screening domain (4 plugin verbs — PEP,
    // sanctions, adverse-media stub, bulk-refresh). Shared
    // enqueue_workstream_screening helper.
    registry.register(Arc::new(screening::Pep));
    registry.register(Arc::new(screening::Sanctions));
    registry.register(Arc::new(screening::AdverseMedia));
    registry.register(Arc::new(screening::BulkRefresh));

    // Phase B slice #20: matrix-overlay domain (3 plugin verbs —
    // effective-matrix, unified-gaps, compare-products).
    registry.register(Arc::new(matrix_overlay::EffectiveMatrix));
    registry.register(Arc::new(matrix_overlay::UnifiedGaps));
    registry.register(Arc::new(matrix_overlay::CompareProducts));

    // Phase B slice #21: state domain (8 plugin verbs — state
    // reducer derive/diagnose/override lifecycle). Transitional
    // `scope.pool()` pattern until state_reducer handlers take
    // `&mut dyn TransactionScope`.
    registry.register(Arc::new(state::Derive));
    registry.register(Arc::new(state::Diagnose));
    registry.register(Arc::new(state::DeriveAll));
    registry.register(Arc::new(state::BlockedWhy));
    registry.register(Arc::new(state::CheckConsistency));
    registry.register(Arc::new(state::Override));
    registry.register(Arc::new(state::RevokeOverride));
    registry.register(Arc::new(state::ListOverrides));

    // Phase B slice #22: shared-atom domain (8 plugin verbs — atom
    // registry + lifecycle FSM + cross-workspace replay).
    registry.register(Arc::new(shared_atom::Register));
    registry.register(Arc::new(shared_atom::Activate));
    registry.register(Arc::new(shared_atom::Deprecate));
    registry.register(Arc::new(shared_atom::Retire));
    registry.register(Arc::new(shared_atom::List));
    registry.register(Arc::new(shared_atom::ListConsumers));
    registry.register(Arc::new(shared_atom::ReplayConstellation));
    registry.register(Arc::new(shared_atom::AcknowledgeSharedUpdate));

    // Phase B slice #23: research.import-run domain (3 plugin verbs —
    // begin/complete/supersede, supersession cascade).
    registry.register(Arc::new(import_run::Begin));
    registry.register(Arc::new(import_run::Complete));
    registry.register(Arc::new(import_run::Supersede));

    // Phase B slice #24: ubo.registry domain (5 plugin verbs — state
    // machine lifecycle promote/advance/waive/reject/expire).
    registry.register(Arc::new(ubo_registry::Promote));
    registry.register(Arc::new(ubo_registry::Advance));
    registry.register(Arc::new(ubo_registry::Waive));
    registry.register(Arc::new(ubo_registry::Reject));
    registry.register(Arc::new(ubo_registry::Expire));

    // Phase B slice #25: entity domain (6 plugin verbs — ghost/
    // identify person lifecycle + placeholder resolution).
    registry.register(Arc::new(entity::Ghost));
    registry.register(Arc::new(entity::Identify));
    registry.register(Arc::new(entity::EnsureOrPlaceholder));
    registry.register(Arc::new(entity::ResolvePlaceholder));
    registry.register(Arc::new(entity::ListPlaceholders));
    registry.register(Arc::new(entity::PlaceholderSummary));

    // Phase B slice #26: trading-matrix domain (3 plugin verbs —
    // IM find-for-trade, pricing-config find-for-instrument, SLA
    // list-open-breaches). NUMERIC cols cast to ::text to avoid
    // `rust_decimal` dep.
    registry.register(Arc::new(trading_matrix::FindImForTrade));
    registry.register(Arc::new(trading_matrix::FindPricingForInstrument));
    registry.register(Arc::new(trading_matrix::ListOpenSlaBreaches));

    // Phase B slice #27: service-resource domain (6 plugin verbs —
    // provision/set-attr/activate/suspend/decommission/validate-attrs).
    registry.register(Arc::new(service_resource::Provision));
    registry.register(Arc::new(service_resource::SetAttr));
    registry.register(Arc::new(service_resource::Activate));
    registry.register(Arc::new(service_resource::Suspend));
    registry.register(Arc::new(service_resource::Decommission));
    registry.register(Arc::new(service_resource::ValidateAttrs));

    // Phase B slice #28: investor-role domain (6 plugin verbs — set,
    // read-as-of, 4 convenience mark-as-* variants that all delegate
    // to the `upsert_role_profile` stored proc).
    registry.register(Arc::new(investor_role::Set));
    registry.register(Arc::new(investor_role::ReadAsOf));
    registry.register(Arc::new(investor_role::MarkAsNominee));
    registry.register(Arc::new(investor_role::MarkAsFof));
    registry.register(Arc::new(investor_role::MarkAsMasterPool));
    registry.register(Arc::new(investor_role::MarkAsEndInvestor));

    // Phase B slice #29: economic-exposure domain (2 plugin verbs —
    // compute look-through + summary). Unblocked by rust_decimal dep
    // addition to sem_os_postgres.
    registry.register(Arc::new(economic_exposure::Compute));
    registry.register(Arc::new(economic_exposure::Summary));

    // Phase B slice #30: edge domain (1 plugin verb — `upsert` with
    // end-and-insert semantics on entity_relationships).
    registry.register(Arc::new(edge::Upsert));

    // Phase B slice #31: manco + ownership governance controller
    // (9 plugin verbs — 3 bridges, 4 group queries, control-links
    // compute, pipeline refresh).
    registry.register(Arc::new(manco::BridgeMancoRoles));
    registry.register(Arc::new(manco::BridgeGleifFundManagers));
    registry.register(Arc::new(manco::BridgeBodsOwnership));
    registry.register(Arc::new(manco::GroupDerive));
    registry.register(Arc::new(manco::GroupCbus));
    registry.register(Arc::new(manco::GroupForCbu));
    registry.register(Arc::new(manco::PrimaryController));
    registry.register(Arc::new(manco::ControlChain));
    registry.register(Arc::new(manco::BookSummary));
    registry.register(Arc::new(manco::ComputeControlLinks));
    registry.register(Arc::new(manco::Refresh));

    // Phase B slice #32: temporal queries (8 plugin verbs — regulatory
    // lookback "what did X look like on date Y?").
    registry.register(Arc::new(temporal::OwnershipAsOf));
    registry.register(Arc::new(temporal::UboChainAsOf));
    registry.register(Arc::new(temporal::CbuRelationshipsAsOf));
    registry.register(Arc::new(temporal::CbuRolesAsOf));
    registry.register(Arc::new(temporal::CbuStateAtApproval));
    registry.register(Arc::new(temporal::RelationshipHistory));
    registry.register(Arc::new(temporal::EntityHistory));
    registry.register(Arc::new(temporal::CompareOwnership));

    // Phase B slice #33: control domain (11 plugin verbs — graph-level
    // analysis + board-controller lifecycle + import stubs).
    registry.register(Arc::new(control::ControlAnalyze));
    registry.register(Arc::new(control::ControlBuildGraph));
    registry.register(Arc::new(control::ControlIdentifyUbos));
    registry.register(Arc::new(control::ControlTraceChain));
    registry.register(Arc::new(control::ControlReconcileOwnership));
    registry.register(Arc::new(control::ShowBoardController));
    registry.register(Arc::new(control::RecomputeBoardController));
    registry.register(Arc::new(control::SetBoardController));
    registry.register(Arc::new(control::ClearBoardControllerOverride));
    registry.register(Arc::new(control::ImportPscRegister));
    registry.register(Arc::new(control::ImportGleifControl));

    // Phase B slice #34: control.compute-controllers (remaining
    // legacy control op — KYC case controller aggregation).
    registry.register(Arc::new(control_compute::ComputeControllers));

    // Phase B slice #35: coverage.compute (per-prong UBO evidence
    // coverage + gap ID generation + tollgate-blocking annotation).
    registry.register(Arc::new(coverage_compute::Compute));

    // Phase B slice #36: observation + document.extract-to-observations
    // (5 plugin verbs — attribute observations / reconciliation /
    // allegation verification / document extraction).
    registry.register(Arc::new(observation::RecordFromDocument));
    registry.register(Arc::new(observation::GetCurrent));
    registry.register(Arc::new(observation::Reconcile));
    registry.register(Arc::new(observation::VerifyAllegations));
    registry.register(Arc::new(observation::ExtractToObservations));

    // Phase B slice #37: bods (6 plugin verbs — BODS statement
    // queries + UBO discovery service).
    registry.register(Arc::new(bods::DiscoverUbos));
    registry.register(Arc::new(bods::Import));
    registry.register(Arc::new(bods::GetStatement));
    registry.register(Arc::new(bods::FindByLei));
    registry.register(Arc::new(bods::ListOwnership));
    registry.register(Arc::new(bods::SyncFromGleif));

    // Phase B slice #38: tollgate decision engine (4 plugin verbs —
    // evaluate/get-metrics/override/get-decision-readiness).
    registry.register(Arc::new(tollgate::Evaluate));
    registry.register(Arc::new(tollgate::GetMetrics));
    registry.register(Arc::new(tollgate::Override));
    registry.register(Arc::new(tollgate::GetDecisionReadiness));

    // Phase B slice #39: ubo.calculate / trace-chains / list-owners
    // (3 plugin verbs — recursive ownership chain + temporal owners).
    registry.register(Arc::new(ubo_analysis::Calculate));
    registry.register(Arc::new(ubo_analysis::TraceChains));
    registry.register(Arc::new(ubo_analysis::ListOwners));

    // Phase B slice #40: evidence state machine (5 canonical verbs +
    // 5 compatibility aliases).
    registry.register(Arc::new(evidence::Require));
    registry.register(Arc::new(evidence::Link));
    registry.register(Arc::new(evidence::Verify));
    registry.register(Arc::new(evidence::Reject));
    registry.register(Arc::new(evidence::Waive));
    registry.register(Arc::new(evidence::CreateRequirement));
    registry.register(Arc::new(evidence::AttachDocument));
    registry.register(Arc::new(evidence::MarkVerified));
    registry.register(Arc::new(evidence::MarkRejected));
    registry.register(Arc::new(evidence::MarkWaived));

    // Phase B slice #41: research.outreach.plan-generate (gap→doc
    // mapping + prioritised per-entity bundling).
    registry.register(Arc::new(outreach_plan::PlanGenerate));

    // Phase B slice #42: trust.analyze-control / identify-ubos / classify.
    registry.register(Arc::new(trust::AnalyzeControl));
    registry.register(Arc::new(trust::IdentifyUbos));
    registry.register(Arc::new(trust::Classify));

    // Phase B slice #43: partnership capital + control analysis.
    registry.register(Arc::new(partnership::RecordContribution));
    registry.register(Arc::new(partnership::RecordDistribution));
    registry.register(Arc::new(partnership::Reconcile));
    registry.register(Arc::new(partnership::AnalyzeControl));

    // Phase B slice #44: access-review automation (8 plugin verbs).
    registry.register(Arc::new(access_review::PopulateCampaign));
    registry.register(Arc::new(access_review::LaunchCampaign));
    registry.register(Arc::new(access_review::RevokeAccess));
    registry.register(Arc::new(access_review::BulkConfirm));
    registry.register(Arc::new(access_review::ConfirmAllClean));
    registry.register(Arc::new(access_review::Attest));
    registry.register(Arc::new(access_review::ProcessDeadline));
    registry.register(Arc::new(access_review::SendReminders));

    // Phase B slice #45: refdata ensure/read/list/deactivate — unified
    // access across 9 reference-data tables via compile-time dispatch.
    registry.register(Arc::new(refdata::Ensure));
    registry.register(Arc::new(refdata::Read));
    registry.register(Arc::new(refdata::List));
    registry.register(Arc::new(refdata::Deactivate));

    // Phase B slice #46: investor lifecycle (13 plugin verbs — full
    // ENQUIRY→OFFBOARDED state machine + suspend/reinstate +
    // count-by-state).
    registry.register(Arc::new(investor::RequestDocuments));
    registry.register(Arc::new(investor::StartKyc));
    registry.register(Arc::new(investor::ApproveKyc));
    registry.register(Arc::new(investor::RejectKyc));
    registry.register(Arc::new(investor::MarkEligible));
    registry.register(Arc::new(investor::RecordSubscription));
    registry.register(Arc::new(investor::Activate));
    registry.register(Arc::new(investor::StartRedemption));
    registry.register(Arc::new(investor::CompleteRedemption));
    registry.register(Arc::new(investor::Offboard));
    registry.register(Arc::new(investor::Suspend));
    registry.register(Arc::new(investor::Reinstate));
    registry.register(Arc::new(investor::CountByState));

    // Phase B slice #47: verify.* adversarial-agent (6 plugin verbs —
    // pattern/evasion detection, confidence, registry verification,
    // gate assertion).
    registry.register(Arc::new(verify::DetectPatterns));
    registry.register(Arc::new(verify::DetectEvasion));
    registry.register(Arc::new(verify::CalculateConfidence));
    registry.register(Arc::new(verify::GetStatus));
    registry.register(Arc::new(verify::VerifyAgainstRegistry));
    registry.register(Arc::new(verify::Assert));

    // Phase B slice #48: ubo.compute-chains / snapshot.capture /
    // snapshot.diff — in-memory ownership graph + JSONB snapshot
    // persistence with SHA-256 code hash.
    registry.register(Arc::new(ubo_compute::ComputeChains));
    registry.register(Arc::new(ubo_compute::SnapshotCapture));
    registry.register(Arc::new(ubo_compute::SnapshotDiff));

    // Phase B slice #49: ubo graph-lifecycle (mark-deceased,
    // convergence-supersede, transfer-control, waive-verification).
    registry.register(Arc::new(ubo_graph::MarkDeceased));
    registry.register(Arc::new(ubo_graph::ConvergenceSupersede));
    registry.register(Arc::new(ubo_graph::TransferControl));
    registry.register(Arc::new(ubo_graph::WaiveVerification));

    // Phase B slice #50: custody (5 plugin verbs across
    // `subcustodian` + `cbu-custody` domains).
    registry.register(Arc::new(custody::SubcustodianLookup));
    registry.register(Arc::new(custody::LookupSsiForTrade));
    registry.register(Arc::new(custody::ValidateBookingCoverage));
    registry.register(Arc::new(custody::DeriveRequiredCoverage));
    registry.register(Arc::new(custody::SetupSsi));

    // Phase B slice #15: schema domain (structure semantics + stewardship
    // dispatch + AffinityGraph-backed diagram generation).
    registry.register(Arc::new(schema::SchemaDomainDescribe));
    registry.register(Arc::new(schema::SchemaEntityDescribe));
    registry.register(Arc::new(schema::SchemaEntityListFields));
    registry.register(Arc::new(schema::SchemaEntityListRelationships));
    registry.register(Arc::new(schema::SchemaEntityListVerbs));
    registry.register(Arc::new(schema::SchemaIntrospect));
    registry.register(Arc::new(schema::SchemaExtractAttributes));
    registry.register(Arc::new(schema::SchemaExtractVerbs));
    registry.register(Arc::new(schema::SchemaExtractEntities));
    registry.register(Arc::new(schema::SchemaCrossReference));
    registry.register(Arc::new(schema::SchemaGenerateErd));
    registry.register(Arc::new(schema::SchemaGenerateVerbFlow));
    registry.register(Arc::new(schema::SchemaGenerateDiscoveryMap));

    // Phase B slice #51: tollgate.check-gate — decision gate evaluation
    // (SKELETON_READY / EVIDENCE_COMPLETE / REVIEW_COMPLETE).
    registry.register(Arc::new(tollgate_evaluate::CheckGate));

    // Phase B slice #58: research.workflow.* (4 plugin verbs —
    // confirm-decision, reject-decision, audit-trail (multi-table
    // aggregate), supersession-trail (full supersession chain)).
    registry.register(Arc::new(research_workflow::ConfirmDecision));
    registry.register(Arc::new(research_workflow::RejectDecision));
    registry.register(Arc::new(research_workflow::AuditTrail));
    registry.register(Arc::new(research_workflow::SupersessionTrail));

    // Phase B slice #57: lifecycle.* (12 plugin verbs — 6 canonical
    // + 6 `service-resource.*-lifecycle` compat aliases). Shared
    // `do_*` helper functions keep the compat aliases single-line.
    registry.register(Arc::new(lifecycle::Provision));
    registry.register(Arc::new(lifecycle::AnalyzeGaps));
    registry.register(Arc::new(lifecycle::CheckReadiness));
    registry.register(Arc::new(lifecycle::Discover));
    registry.register(Arc::new(lifecycle::GeneratePlan));
    registry.register(Arc::new(lifecycle::ExecutePlan));
    registry.register(Arc::new(lifecycle::ServiceProvisionLifecycle));
    registry.register(Arc::new(lifecycle::ServiceAnalyzeLifecycleGaps));
    registry.register(Arc::new(lifecycle::ServiceCheckLifecycleReadiness));
    registry.register(Arc::new(lifecycle::ServiceDiscoverLifecycles));
    registry.register(Arc::new(lifecycle::ServiceGenerateLifecyclePlan));
    registry.register(Arc::new(lifecycle::ServiceExecuteLifecyclePlan));

    // Phase B slice #56: refdata loader (5 plugin verbs — bulk YAML→DB
    // for markets / instrument_classes / subcustodian_network /
    // sla_templates + load-all orchestrator).
    registry.register(Arc::new(refdata_loader::LoadMarkets));
    registry.register(Arc::new(refdata_loader::LoadInstrumentClasses));
    registry.register(Arc::new(refdata_loader::LoadSubcustodians));
    registry.register(Arc::new(refdata_loader::LoadSlaTemplates));
    registry.register(Arc::new(refdata_loader::LoadAll));

    // Phase B slice #55: ownership.* (8 plugin verbs — snapshot derive/list,
    // control positions, controller finder, reconciliation run/findings,
    // gap analysis, recursive chain trace).
    registry.register(Arc::new(ownership::Compute));
    registry.register(Arc::new(ownership::SnapshotList));
    registry.register(Arc::new(ownership::ListControlPositions));
    registry.register(Arc::new(ownership::FindController));
    registry.register(Arc::new(ownership::Reconcile));
    registry.register(Arc::new(ownership::ReconcileFindings));
    registry.register(Arc::new(ownership::AnalyzeGaps));
    registry.register(Arc::new(ownership::TraceChain));

    // Phase B slice #54: graph.validate (1 plugin verb — cycle detection +
    // terminus integrity + per-target supply + source conflict +
    // orphan-entity + anomaly persistence).
    registry.register(Arc::new(graph_validate::Validate));

    // Phase B slice #53: cbu-specialist-roles (7 plugin verbs —
    // dual-write into cbu_entity_roles + entity_relationships edge).
    registry.register(Arc::new(cbu_role::AssignOwnership));
    registry.register(Arc::new(cbu_role::AssignControl));
    registry.register(Arc::new(cbu_role::AssignTrustRole));
    registry.register(Arc::new(cbu_role::AssignFundRole));
    registry.register(Arc::new(cbu_role::AssignServiceProvider));
    registry.register(Arc::new(cbu_role::AssignSignatory));
    registry.register(Arc::new(cbu_role::ValidateRoles));

    // Phase B slice #52: trading-profile.ca.* (11 plugin verbs — matrix
    // JSONB mutations via TradingProfileDocument service trait).
    registry.register(Arc::new(trading_profile_ca::EnableEventTypes));
    registry.register(Arc::new(trading_profile_ca::DisableEventTypes));
    registry.register(Arc::new(trading_profile_ca::SetNotificationPolicy));
    registry.register(Arc::new(trading_profile_ca::SetElectionPolicy));
    registry.register(Arc::new(trading_profile_ca::SetDefaultOption));
    registry.register(Arc::new(trading_profile_ca::RemoveDefaultOption));
    registry.register(Arc::new(trading_profile_ca::AddCutoffRule));
    registry.register(Arc::new(trading_profile_ca::RemoveCutoffRule));
    registry.register(Arc::new(trading_profile_ca::LinkProceedsSsi));
    registry.register(Arc::new(trading_profile_ca::RemoveProceedsSsi));
    registry.register(Arc::new(trading_profile_ca::GetPolicy));

    registry
}

/// Plugin verb operation executed under a Sequencer-owned transaction scope.
///
/// Implementations live in domain submodules (`sem_os_postgres::ops::<domain>`)
/// and are registered at startup by `ob-poc-web::main` via
/// [`SemOsVerbOpRegistry`]. The dispatcher opens a `PgTransactionScope`,
/// invokes [`Self::execute`], then commits on `Ok` / rolls back on `Err`.
///
/// # Contract authority
///
/// Args + returns are defined by the verb's YAML contract
/// (`config/verbs/<domain>.yaml` — ingested into SemOS as `VerbContractBody`
/// snapshots). Op bodies must honour that contract rather than transliterate
/// whatever the legacy `CustomOperation` impl did: read the YAML first,
/// re-implement against it.
#[async_trait]
pub trait SemOsVerbOp: Send + Sync {
    /// Fully-qualified verb name (e.g. `"entity.ghost"`).
    fn fqn(&self) -> &str;

    /// Execute the op.
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome>;
}
