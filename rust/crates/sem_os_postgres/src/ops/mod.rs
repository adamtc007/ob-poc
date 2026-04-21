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

pub mod affinity;
pub mod attribute;
pub mod audit;
pub mod board;
pub mod changeset;
pub mod constellation;
pub mod docs_bundle;
pub mod focus;
pub mod governance;
pub mod import_run;
pub mod maintenance;
pub mod matrix_overlay;
pub mod nav;
pub mod outreach;
pub mod pack_answer;
pub mod pack_select;
pub mod phrase;
pub mod registry;
pub mod registry_ops;
pub mod regulatory;
pub mod remediation;
pub mod requirement;
pub mod research_normalize;
pub mod schema;
pub mod screening;
pub mod semantic;
pub mod service_pipeline;
pub mod shared_atom;
pub mod session;
pub mod state;
pub mod stewardship_helper;
pub mod team;
pub mod ubo_registry;
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
