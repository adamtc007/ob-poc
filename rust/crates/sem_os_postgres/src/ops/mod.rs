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

pub mod attribute;
pub mod constellation;
pub mod nav;
pub mod pack_answer;
pub mod pack_select;
pub mod phrase;
pub mod registry;
pub mod service_pipeline;
pub mod session;
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
