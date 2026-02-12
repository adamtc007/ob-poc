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

mod access_review_ops;
mod agent_ops;
mod attribute_ops;
pub mod batch_control_ops;
mod billing_ops;
mod board_ops;
mod bods_ops;
mod booking_principal_ops;
mod bpmn_lite_ops;
mod capital_ops;
mod cbu_ops;
mod cbu_role_ops;
mod client_group_ops;
mod control_ops;
mod custody;
mod deal_ops;
mod dilution_ops;
mod docs_bundle_ops;
mod document_ops;
mod economic_exposure_ops;
mod edge_ops;
mod entity_ops;
pub mod entity_query;
mod evidence_ops;
mod gleif_ops;
pub mod helpers;
mod import_run_ops;
mod investor_ops;
mod investor_role_ops;
mod kyc_case_ops;
mod lifecycle_ops;
mod manco_ops;
mod matrix_overlay_ops;
mod observation_ops;
mod onboarding;
mod outreach_ops;
mod ownership_ops;
mod pack_ops;
mod partnership_ops;
mod refdata_loader;
mod regulatory_ops;
mod request_ops;
mod requirement_ops;
mod research_workflow_ops;
mod resource_ops;
pub mod rule_evaluator;
mod runbook_ops;

mod screening_ops;
mod semantic_ops;
mod service_pipeline_ops;
mod session_ops;
mod source_loader_ops;
mod team_ops;
pub mod template_ops;
mod temporal_ops;

mod tollgate_ops;
mod trading_matrix;
mod trading_profile;
mod trading_profile_ca_ops;
mod trust_ops;
mod ubo_analysis;
pub mod ubo_graph_ops;
mod ubo_registry_ops;
mod verify_ops;
mod view_ops;

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

// Re-export DSL types for use by operation implementations
pub use crate::dsl_v2::ast::VerbCall;
pub use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

pub use batch_control_ops::{
    BatchAbortOp, BatchAddProductsOp, BatchContinueOp, BatchControlResult, BatchPauseOp,
    BatchResumeOp, BatchSkipOp, BatchStatusOp,
};
pub use custody::{
    DeriveRequiredCoverageOp, LookupSsiForTradeOp, SetupSsiFromDocumentOp, SubcustodianLookupOp,
    ValidateBookingCoverageOp,
};
pub use entity_query::{EntityQueryOp, EntityQueryResult};
pub use kyc_case_ops::{KycCaseStateOp, WorkstreamStateOp};
pub use lifecycle_ops::{
    LifecycleAnalyzeGapsOp, LifecycleCheckReadinessOp, LifecycleDiscoverOp, LifecycleExecutePlanOp,
    LifecycleGeneratePlanOp, LifecycleProvisionOp,
};
pub use matrix_overlay_ops::{MatrixCompareProductsOp, MatrixEffectiveOp, MatrixUnifiedGapsOp};
pub use onboarding::OnboardingAutoCompleteOp;
pub use refdata_loader::{
    get_refdata_operations, LoadAllRefdataOp, LoadInstrumentClassesOp, LoadMarketsOp,
    LoadSlaTemplatesOp, LoadSubcustodiansOp,
};
pub use request_ops::{
    DocumentRequestOp, DocumentUploadOp, DocumentWaiveOp, RequestCancelOp, RequestCreateOp,
    RequestEscalateOp, RequestExtendOp, RequestFulfillOp, RequestOverdueOp, RequestRemindOp,
    RequestWaiveOp,
};

pub use semantic_ops::{
    SemanticListStagesOp, SemanticMissingEntitiesOp, SemanticNextActionsOp,
    SemanticPromptContextOp, SemanticStagesForProductOp, SemanticStateOp,
};
pub use template_ops::{
    TemplateBatchOp, TemplateBatchResult, TemplateInvokeOp, TemplateInvokeResult,
};
pub use temporal_ops::{
    TemporalCbuRelationshipsAsOfOp, TemporalCbuRolesAsOfOp, TemporalCbuStateAtApprovalOp,
    TemporalCompareOwnershipOp, TemporalEntityHistoryOp, TemporalOwnershipAsOfOp,
    TemporalRelationshipHistoryOp, TemporalUboChainAsOfOp,
};

pub use trading_matrix::{FindImForTradeOp, FindPricingForInstrumentOp, ListOpenSlaBreachesOp};
pub use trading_profile::{
    TradingProfileActivateOp, TradingProfileAddAllowedCurrencyOp, TradingProfileAddBookingRuleOp,
    TradingProfileAddCsaCollateralOp, TradingProfileAddCsaConfigOp, TradingProfileAddImMandateOp,
    TradingProfileAddInstrumentClassOp, TradingProfileAddIsdaConfigOp,
    TradingProfileAddIsdaCoverageOp, TradingProfileAddMarketOp, TradingProfileAddSsiOp,
    TradingProfileApproveOp, TradingProfileArchiveOp, TradingProfileCloneToOp,
    TradingProfileCreateDraftOp, TradingProfileCreateNewVersionOp, TradingProfileDiffOp,
    TradingProfileGetActiveOp, TradingProfileImportOp, TradingProfileLinkCsaSsiOp,
    TradingProfileMaterializeOp, TradingProfileRejectOp, TradingProfileRemoveBookingRuleOp,
    TradingProfileRemoveCsaConfigOp, TradingProfileRemoveImMandateOp,
    TradingProfileRemoveInstrumentClassOp, TradingProfileRemoveIsdaConfigOp,
    TradingProfileRemoveMarketOp, TradingProfileRemoveSsiOp, TradingProfileSetBaseCurrencyOp,
    TradingProfileSubmitOp, TradingProfileUpdateImScopeOp, TradingProfileValidateCoverageOp,
    TradingProfileValidateGoLiveReadyOp,
};
pub use trading_profile_ca_ops::{
    TradingProfileCaAddCutoffOp, TradingProfileCaDisableEventTypesOp,
    TradingProfileCaEnableEventTypesOp, TradingProfileCaGetPolicyOp, TradingProfileCaLinkSsiOp,
    TradingProfileCaRemoveCutoffOp, TradingProfileCaRemoveDefaultOp, TradingProfileCaRemoveSsiOp,
    TradingProfileCaSetDefaultOp, TradingProfileCaSetElectionOp, TradingProfileCaSetNotificationOp,
};
pub use ubo_analysis::{UboCalculateOp, UboListOwnersOp, UboTraceChainsOp};

// Domain-specific operation modules
pub use attribute_ops::{
    AttributeCheckCoverageOp, AttributeListByDocumentOp, AttributeListSinksOp,
    AttributeListSourcesOp, AttributeTraceLineageOp, DocumentCheckExtractionCoverageOp,
    DocumentListAttributesOp,
};
pub use cbu_ops::{CbuAddProductOp, CbuDecideOp, CbuDeleteCascadeOp, CbuShowOp};
pub use cbu_role_ops::{
    CbuRoleAssignControlOp, CbuRoleAssignFundOp, CbuRoleAssignOp, CbuRoleAssignOwnershipOp,
    CbuRoleAssignServiceOp, CbuRoleAssignSignatoryOp, CbuRoleAssignTrustOp, CbuRoleValidateAllOp,
};
pub use document_ops::{DocumentCatalogOp, DocumentExtractOp};

// Task queue document operations (Migration 049)
#[cfg(feature = "database")]
pub use document_ops::{
    DocumentMissingForEntityOp, DocumentRejectOp, DocumentSolicitOp, DocumentSolicitSetOp,
    DocumentUploadVersionOp, DocumentVerifyOp,
};

// Requirement operations (Migration 049)
pub use entity_ops::{EntityGhostOp, EntityIdentifyOp};
pub use observation_ops::{
    DocumentExtractObservationsOp, ObservationFromDocumentOp, ObservationGetCurrentOp,
    ObservationReconcileOp, ObservationVerifyAllegationsOp,
};
pub use pack_ops::{PackAnswerOp, PackAnswerResult, PackSelectOp, PackSelectResult};
#[cfg(feature = "database")]
pub use requirement_ops::{RequirementCreateSetOp, RequirementUnsatisfiedOp};
pub use resource_ops::{
    ResourceActivateOp, ResourceCreateOp, ResourceDecommissionOp, ResourceSetAttrOp,
    ResourceSuspendOp, ResourceValidateAttrsOp,
};
pub use screening_ops::{ScreeningAdverseMediaOp, ScreeningPepOp, ScreeningSanctionsOp};
pub use ubo_graph_ops::{
    UboConvergenceSupersedeOp, UboMarkDeceasedOp, UboTransferControlOp, UboWaiveVerificationOp,
};

// Team operations (only transfer-member needs plugin, others are CRUD)
pub use team_ops::TeamTransferMemberOp;

// Access Review operations (complex multi-step transactional operations only)
pub use access_review_ops::{
    AccessReviewAttestOp, AccessReviewBulkConfirmOp, AccessReviewConfirmCleanOp,
    AccessReviewLaunchOp, AccessReviewPopulateOp, AccessReviewProcessDeadlineOp,
    AccessReviewRevokeOp, AccessReviewSendRemindersOp,
};

// BODS operations (UBO discovery via GLEIF + BODS)
pub use bods_ops::{
    BodsDiscoverUbosOp, BodsFindByLeiOp, BodsGetStatementOp, BodsImportOp, BodsListOwnershipOp,
    BodsSyncFromGleifOp,
};

// View operations (session scope and selection management)
pub use view_ops::{
    ViewBackToOp, ViewBookOp, ViewBreadcrumbsOp, ViewCbuOp, ViewClearOp, ViewEntityForestOp,
    ViewLayoutOp, ViewOpResult, ViewRefineOp, ViewSelectOp, ViewSelectionInfoOp, ViewStatusOp,
    ViewUniverseOp, ViewZoomInOp, ViewZoomOutOp,
};

// KYC Control Enhancement operations (capital, board, trust, partnership, tollgate, control)
pub use board_ops::BoardAnalyzeControlOp;
pub use capital_ops::{
    CapitalBuybackOp, CapitalCancelOp, CapitalCancelSharesOp, CapitalCapTableOp, CapitalHoldersOp,
    CapitalIssueInitialOp, CapitalIssueNewOp, CapitalIssueSharesOp, CapitalOwnershipChainOp,
    CapitalReconcileOp, CapitalShareClassCreateOp, CapitalShareClassGetSupplyOp, CapitalSplitOp,
    CapitalTransferOp,
};
pub use control_ops::{
    ClearBoardControllerOverrideOp, ControlAnalyzeOp, ControlBuildGraphOp, ControlIdentifyUbosOp,
    ControlReconcileOwnershipOp, ControlTraceChainOp, ImportGleifControlOp, ImportPscRegisterOp,
    RecomputeBoardControllerOp, SetBoardControllerOp, ShowBoardControllerOp,
};
pub use dilution_ops::{
    DilutionCreateConvertibleNoteOp, DilutionCreateSafeOp, DilutionExerciseOp, DilutionForfeitOp,
    DilutionGetSummaryOp, DilutionGrantOptionsOp, DilutionIssueWarrantOp, DilutionListOp,
};
pub use ownership_ops::{
    OwnershipAnalyzeGapsOp, OwnershipComputeOp, OwnershipControlPositionsOp,
    OwnershipReconcileFindingsOp, OwnershipReconcileOp, OwnershipSnapshotListOp,
    OwnershipTraceChainOp, OwnershipWhoControlsOp,
};
pub use partnership_ops::{
    PartnershipAnalyzeControlOp, PartnershipContributionOp, PartnershipDistributionOp,
    PartnershipReconcileOp,
};
pub use tollgate_ops::{
    TollgateDecisionReadinessOp, TollgateEvaluateOp, TollgateGetMetricsOp, TollgateOverrideOp,
};
pub use trust_ops::{TrustAnalyzeControlOp, TrustClassifyOp, TrustIdentifyUbosOp};

// Service pipeline operations (intent → discovery → provision → readiness)
pub use service_pipeline_ops::{
    AttributeGapsOp, AttributePopulateOp, AttributeRollupOp, AttributeSetOp, DiscoveryExplainOp,
    DiscoveryRunOp, PipelineFullOp, ProvisioningRunOp, ProvisioningStatusOp, ReadinessComputeOp,
    ReadinessExplainOp, ServiceIntentCreateOp, ServiceIntentListOp, ServiceIntentSupersedeOp,
};

// GLEIF operations (LEI data enrichment)
pub use gleif_ops::{
    GleifEnrichOp, GleifGetChildrenOp, GleifGetManagedFundsOp, GleifGetManagerOp,
    GleifGetMasterFundOp, GleifGetParentOp, GleifGetRecordOp, GleifGetUmbrellaOp,
    GleifImportManagedFundsOp, GleifImportTreeOp, GleifLookupByIsinOp, GleifRefreshOp,
    GleifResolveSuccessorOp, GleifSearchOp, GleifTraceOwnershipOp,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Factory for auto-registration of custom ops via inventory
pub struct CustomOpFactory {
    pub create: fn() -> Arc<dyn CustomOperation>,
}

// Tell inventory to collect these
inventory::collect!(CustomOpFactory);

/// Trait for custom operations that cannot be expressed as data-driven verbs
#[async_trait]
pub trait CustomOperation: Send + Sync {
    /// Domain this operation belongs to
    fn domain(&self) -> &'static str;

    /// Verb name for this operation
    fn verb(&self) -> &'static str;

    /// Why this operation requires custom code (documentation)
    fn rationale(&self) -> &'static str;

    /// Execute the custom operation
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult>;

    /// Execute without database (for testing)
    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult>;

    /// Execute within an existing transaction
    ///
    /// Default implementation logs a warning and falls back to pool-based execution
    /// which does NOT participate in the transaction (auto-commit semantics).
    /// Override this method for true transactional behavior.
    #[cfg(feature = "database")]
    async fn execute_in_tx(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<ExecutionResult> {
        // WARNING: Default implementation does not participate in the caller's transaction.
        // This means if the batch fails, this operation will NOT be rolled back.
        // Implementors should override this method for transactional operations.
        tracing::warn!(
            "CustomOperation {}.{} does not implement execute_in_tx, using pool (non-transactional)",
            self.domain(),
            self.verb()
        );
        Err(anyhow::anyhow!(
            "Operation {}.{} does not support transactional execution. \
             Override execute_in_tx() for transaction support.",
            self.domain(),
            self.verb()
        ))
    }
}

/// Registry for custom operations
pub struct CustomOperationRegistry {
    operations: HashMap<(String, String), Arc<dyn CustomOperation>>,
}

impl CustomOperationRegistry {
    /// Create a new registry with all custom operations.
    ///
    /// All operations are auto-registered via the `#[register_custom_op]` macro
    /// and the `inventory` crate. This eliminates the need for manual registration.
    pub fn new() -> Self {
        let mut registry = Self {
            operations: HashMap::new(),
        };

        // Auto-register all ops annotated with #[register_custom_op]
        // The inventory crate collects these at link time
        for factory in inventory::iter::<CustomOpFactory> {
            let op = (factory.create)();
            registry.register_internal(op);
        }

        // Log the count for sanity checking
        tracing::info!(
            "CustomOperationRegistry initialized with {} ops from inventory",
            registry.operations.len()
        );

        registry
    }

    /// Internal registration with duplicate detection.
    /// Used by inventory auto-registration. Panics on duplicate to catch bugs early.
    fn register_internal(&mut self, op: Arc<dyn CustomOperation>) {
        let key = (op.domain().to_string(), op.verb().to_string());

        if self.operations.contains_key(&key) {
            panic!(
                "Duplicate custom op registration: {}.{} — this is a bug. \
                 Check for both #[register_custom_op] and manual registration.",
                key.0, key.1
            );
        }
        self.operations.insert(key, op);
    }

    /// Register a custom operation (manual registration, allows overwrite for migration)
    pub fn register(&mut self, op: Arc<dyn CustomOperation>) {
        let key = (op.domain().to_string(), op.verb().to_string());

        // P1 FIX: Warn on overwrite during migration period
        if self.operations.contains_key(&key) {
            tracing::warn!(
                "Manual registration overwriting existing op: {}.{} — \
                 this is expected during migration but should be removed after",
                key.0,
                key.1
            );
        }

        // During migration period, manual registration can overwrite inventory registration
        // This allows gradual migration without breaking anything
        self.operations.insert(key, op);
    }

    /// Get a custom operation by domain and verb
    pub fn get(&self, domain: &str, verb: &str) -> Option<Arc<dyn CustomOperation>> {
        let key = (domain.to_string(), verb.to_string());
        self.operations.get(&key).cloned()
    }

    /// Check if an operation exists
    pub fn has(&self, domain: &str, verb: &str) -> bool {
        let key = (domain.to_string(), verb.to_string());
        self.operations.contains_key(&key)
    }

    /// List all registered custom operations (deterministically sorted by domain, verb)
    pub fn list(&self) -> Vec<(&str, &str, &str)> {
        let mut entries: Vec<_> = self
            .operations
            .values()
            .map(|op| (op.domain(), op.verb(), op.rationale()))
            .collect();
        entries.sort_by_key(|(d, v, _)| (*d, *v));
        entries
    }
}

impl Default for CustomOperationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

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
        assert!(registry.has("cbu", "show"));
        assert!(registry.has("cbu", "delete-cascade"));
        // CBU Role operations (Role Taxonomy V2)
        assert!(registry.has("cbu.role", "assign"));
        assert!(registry.has("cbu.role", "assign-ownership"));
        assert!(registry.has("cbu.role", "assign-control"));
        assert!(registry.has("cbu.role", "assign-trust-role"));
        assert!(registry.has("cbu.role", "assign-fund-role"));
        assert!(registry.has("cbu.role", "assign-service-provider"));
        assert!(registry.has("cbu.role", "assign-signatory"));
        assert!(registry.has("cbu.role", "validate"));
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
        // KYC case state operations
        assert!(registry.has("kyc-case", "state"));
        assert!(registry.has("entity-workstream", "state"));
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
        assert!(registry.has("view", "select"));
        assert!(registry.has("view", "layout"));
        assert!(registry.has("view", "status"));
        assert!(registry.has("view", "selection-info"));
        // Zoom navigation (fractal taxonomy navigation)
        assert!(registry.has("view", "zoom-in"));
        assert!(registry.has("view", "zoom-out"));
        assert!(registry.has("view", "back-to"));
        assert!(registry.has("view", "breadcrumbs"));

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
        assert!(registry.has("ownership", "control-positions"));
        assert!(registry.has("ownership", "who-controls"));
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
        // KYC Control Enhancement: Unified control operations
        assert!(registry.has("control", "analyze"));
        assert!(registry.has("control", "build-graph"));
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
        assert!(registry.has("agent", "confirm"));
        assert!(registry.has("agent", "reject"));
        assert!(registry.has("agent", "select"));
        assert!(registry.has("agent", "status"));
        assert!(registry.has("agent", "history"));
        assert!(registry.has("agent", "set-threshold"));
        assert!(registry.has("agent", "set-mode"));
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
        assert!(registry.has("manco", "bridge.manco-roles"));
        assert!(registry.has("manco", "bridge.gleif-fund-managers"));
        assert!(registry.has("manco", "bridge.bods-ownership"));
        assert!(registry.has("manco", "group.derive"));
        assert!(registry.has("manco", "group.cbus"));
        assert!(registry.has("manco", "group.for-cbu"));
        assert!(registry.has("manco", "primary-controller"));
        assert!(registry.has("manco", "control-chain"));
        assert!(registry.has("ownership", "control-links.compute"));
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

        // Runbook ops (staged execution model)
        assert!(registry.has("runbook", "stage"));
        assert!(registry.has("runbook", "pick"));
        assert!(registry.has("runbook", "run"));
        assert!(registry.has("runbook", "show"));
        assert!(registry.has("runbook", "preview"));
        assert!(registry.has("runbook", "remove"));
        assert!(registry.has("runbook", "abort"));
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
