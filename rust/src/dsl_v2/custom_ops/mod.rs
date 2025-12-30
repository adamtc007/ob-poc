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
pub mod batch_control_ops;
mod cbu_ops;
mod cbu_role_ops;
mod custody;
mod document_ops;
mod entity_ops;
pub mod entity_query;
pub mod helpers;
mod kyc_case_ops;
mod lifecycle_ops;
mod matrix_overlay_ops;
mod observation_ops;
mod onboarding;
mod refdata_loader;
mod regulatory_ops;
mod request_ops;
mod resource_ops;
mod rfi;
mod screening_ops;
mod semantic_ops;
mod team_ops;
pub mod template_ops;
mod temporal_ops;
mod threshold;
mod trading_matrix;
mod trading_profile;
mod ubo_analysis;
pub mod ubo_graph_ops;
mod verify_ops;

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use super::ast::VerbCall;
use super::executor::{ExecutionContext, ExecutionResult};

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
pub use onboarding::{
    OnboardingAutoCompleteOp, OnboardingEnsureOp, OnboardingExecuteOp, OnboardingGetUrlsOp,
    OnboardingPlanOp, OnboardingShowPlanOp, OnboardingStatusOp,
};
pub use refdata_loader::{
    get_refdata_operations, LoadAllRefdataOp, LoadInstrumentClassesOp, LoadMarketsOp,
    LoadSlaTemplatesOp, LoadSubcustodiansOp,
};
pub use request_ops::{
    DocumentRequestOp, DocumentUploadOp, DocumentWaiveOp, RequestCancelOp, RequestCreateOp,
    RequestEscalateOp, RequestExtendOp, RequestFulfillOp, RequestOverdueOp, RequestRemindOp,
    RequestWaiveOp,
};
pub use rfi::{RfiCheckCompletionOp, RfiGenerateOp, RfiListByCaseOp};
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
pub use threshold::{ThresholdCheckEntityOp, ThresholdDeriveOp, ThresholdEvaluateOp};
pub use trading_matrix::{FindImForTradeOp, FindPricingForInstrumentOp, ListOpenSlaBreachesOp};
pub use trading_profile::{
    TradingProfileActivateOp, TradingProfileGetActiveOp, TradingProfileImportOp,
    TradingProfileMaterializeOp, TradingProfileValidateOp,
};
pub use ubo_analysis::{
    UboCalculateOp, UboCheckCompletenessOp, UboCompareSnapshotOp, UboDiscoverOwnerOp,
    UboInferChainOp, UboListOwnersOp, UboSnapshotCbuOp, UboSupersedeOp, UboTraceChainsOp,
};

// Domain-specific operation modules
pub use cbu_ops::{CbuAddProductOp, CbuDecideOp, CbuDeleteCascadeOp, CbuShowOp};
pub use cbu_role_ops::{
    CbuRoleAssignControlOp, CbuRoleAssignFundOp, CbuRoleAssignOp, CbuRoleAssignOwnershipOp,
    CbuRoleAssignServiceOp, CbuRoleAssignSignatoryOp, CbuRoleAssignTrustOp, CbuRoleValidateAllOp,
};
pub use document_ops::{DocumentCatalogOp, DocumentExtractOp};
pub use entity_ops::EntityCreateOp;
pub use observation_ops::{
    DocumentExtractObservationsOp, ObservationFromDocumentOp, ObservationGetCurrentOp,
    ObservationReconcileOp, ObservationVerifyAllegationsOp,
};
pub use resource_ops::{
    ResourceActivateOp, ResourceCreateOp, ResourceDecommissionOp, ResourceSetAttrOp,
    ResourceSuspendOp, ResourceValidateAttrsOp,
};
pub use screening_ops::{ScreeningAdverseMediaOp, ScreeningPepOp, ScreeningSanctionsOp};
pub use ubo_graph_ops::{
    // Phase 6: Decision & review
    KycDecisionOp,
    // Phase 2: Graph building
    UboAllegeOp,
    // Phase 4: Assertions
    UboAssertOp,
    UboConvergenceSupersedeOp,
    // Phase 5: Evaluation
    UboEvaluateOp,
    UboLinkProofOp,
    // Phase 7: Removal operations
    UboMarkDeceasedOp,
    UboMarkDirtyOp,
    UboRemoveEdgeOp,
    UboScheduleReviewOp,
    UboStatusOp,
    UboTransferControlOp,
    UboTraverseOp,
    UboUpdateAllegationOp,
    // Phase 3: Verification & convergence
    UboVerifyOp,
    UboWaiveVerificationOp,
};

// Team operations (only transfer-member needs plugin, others are CRUD)
pub use team_ops::TeamTransferMemberOp;

// Access Review operations (complex multi-step transactional operations only)
pub use access_review_ops::{
    AccessReviewAttestOp, AccessReviewBulkConfirmOp, AccessReviewConfirmCleanOp,
    AccessReviewLaunchOp, AccessReviewPopulateOp, AccessReviewProcessDeadlineOp,
    AccessReviewRevokeOp, AccessReviewSendRemindersOp,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

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
}

/// Registry for custom operations
pub struct CustomOperationRegistry {
    operations: HashMap<(String, String), Arc<dyn CustomOperation>>,
}

impl CustomOperationRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            operations: HashMap::new(),
        };

        // Register built-in custom operations
        registry.register(Arc::new(EntityCreateOp));
        registry.register(Arc::new(DocumentCatalogOp));
        registry.register(Arc::new(DocumentExtractOp));
        registry.register(Arc::new(UboCalculateOp));
        registry.register(Arc::new(ScreeningPepOp));
        registry.register(Arc::new(ScreeningSanctionsOp));
        registry.register(Arc::new(ScreeningAdverseMediaOp));

        // Resource instance operations
        registry.register(Arc::new(ResourceCreateOp));
        registry.register(Arc::new(ResourceSetAttrOp));
        registry.register(Arc::new(ResourceActivateOp));
        registry.register(Arc::new(ResourceSuspendOp));
        registry.register(Arc::new(ResourceDecommissionOp));
        registry.register(Arc::new(ResourceValidateAttrsOp));

        // CBU operations
        registry.register(Arc::new(CbuAddProductOp));
        registry.register(Arc::new(CbuShowOp));
        registry.register(Arc::new(CbuDecideOp));
        registry.register(Arc::new(CbuDeleteCascadeOp));

        // CBU Role operations (Role Taxonomy V2)
        registry.register(Arc::new(CbuRoleAssignOp));
        registry.register(Arc::new(CbuRoleAssignOwnershipOp));
        registry.register(Arc::new(CbuRoleAssignControlOp));
        registry.register(Arc::new(CbuRoleAssignTrustOp));
        registry.register(Arc::new(CbuRoleAssignFundOp));
        registry.register(Arc::new(CbuRoleAssignServiceOp));
        registry.register(Arc::new(CbuRoleAssignSignatoryOp));
        registry.register(Arc::new(CbuRoleValidateAllOp));

        // NOTE: delivery.record, delivery.complete, delivery.fail are now CRUD verbs
        // defined in config/verbs/delivery.yaml - no plugin needed

        // Custody operations
        registry.register(Arc::new(SubcustodianLookupOp));
        registry.register(Arc::new(LookupSsiForTradeOp));
        registry.register(Arc::new(ValidateBookingCoverageOp));
        registry.register(Arc::new(DeriveRequiredCoverageOp));
        registry.register(Arc::new(SetupSsiFromDocumentOp));

        // Observation operations
        registry.register(Arc::new(ObservationFromDocumentOp));
        registry.register(Arc::new(ObservationGetCurrentOp));
        registry.register(Arc::new(ObservationReconcileOp));
        registry.register(Arc::new(ObservationVerifyAllegationsOp));

        // Document extraction to observations
        registry.register(Arc::new(DocumentExtractObservationsOp));

        // Threshold operations (Phase 2)
        registry.register(Arc::new(ThresholdDeriveOp));
        registry.register(Arc::new(ThresholdEvaluateOp));
        registry.register(Arc::new(ThresholdCheckEntityOp));

        // Semantic stage operations (onboarding journey progress tracking)
        registry.register(Arc::new(SemanticStateOp));
        registry.register(Arc::new(SemanticListStagesOp));
        registry.register(Arc::new(SemanticStagesForProductOp));
        registry.register(Arc::new(SemanticNextActionsOp));
        registry.register(Arc::new(SemanticMissingEntitiesOp));
        registry.register(Arc::new(SemanticPromptContextOp));

        // RFI operations (Phase 3) - works with existing kyc.doc_requests
        registry.register(Arc::new(RfiGenerateOp));
        registry.register(Arc::new(RfiCheckCompletionOp));
        registry.register(Arc::new(RfiListByCaseOp));

        // UBO Analysis operations (Phase 4)
        registry.register(Arc::new(UboDiscoverOwnerOp));
        registry.register(Arc::new(UboInferChainOp));
        registry.register(Arc::new(UboTraceChainsOp));
        registry.register(Arc::new(UboListOwnersOp));
        registry.register(Arc::new(UboCheckCompletenessOp));
        registry.register(Arc::new(UboSupersedeOp));
        registry.register(Arc::new(UboSnapshotCbuOp));
        registry.register(Arc::new(UboCompareSnapshotOp));

        // UBO Graph/Convergence operations (KYC convergence model)
        // Phase 2: Graph building
        registry.register(Arc::new(UboAllegeOp));
        registry.register(Arc::new(UboLinkProofOp));
        registry.register(Arc::new(UboUpdateAllegationOp));
        registry.register(Arc::new(UboRemoveEdgeOp));
        // Phase 3: Verification & convergence
        registry.register(Arc::new(UboVerifyOp));
        registry.register(Arc::new(UboStatusOp));
        // Phase 4: Assertions
        registry.register(Arc::new(UboAssertOp));
        // Phase 5: Evaluation
        registry.register(Arc::new(UboEvaluateOp));
        registry.register(Arc::new(UboTraverseOp));
        // Phase 6: Decision & review
        registry.register(Arc::new(KycDecisionOp));
        registry.register(Arc::new(UboMarkDirtyOp));
        registry.register(Arc::new(UboScheduleReviewOp));
        // Phase 7: Removal operations
        registry.register(Arc::new(UboMarkDeceasedOp));
        registry.register(Arc::new(UboConvergenceSupersedeOp));
        registry.register(Arc::new(UboTransferControlOp));
        registry.register(Arc::new(UboWaiveVerificationOp));

        // Onboarding operations (Terraform-like resource provisioning with dependencies)
        registry.register(Arc::new(OnboardingPlanOp));
        registry.register(Arc::new(OnboardingShowPlanOp));
        registry.register(Arc::new(OnboardingExecuteOp));
        registry.register(Arc::new(OnboardingStatusOp));
        registry.register(Arc::new(OnboardingGetUrlsOp));
        registry.register(Arc::new(OnboardingEnsureOp));
        registry.register(Arc::new(OnboardingAutoCompleteOp));

        // Trading Profile operations
        registry.register(Arc::new(TradingProfileImportOp));
        registry.register(Arc::new(TradingProfileGetActiveOp));
        registry.register(Arc::new(TradingProfileActivateOp));
        registry.register(Arc::new(TradingProfileMaterializeOp));
        registry.register(Arc::new(TradingProfileValidateOp));

        // Entity query for batch template execution
        registry.register(Arc::new(EntityQueryOp));

        // Template operations
        registry.register(Arc::new(TemplateInvokeOp));
        registry.register(Arc::new(TemplateBatchOp));

        // Batch control operations (pause/resume/status)
        registry.register(Arc::new(BatchPauseOp));
        registry.register(Arc::new(BatchResumeOp));
        registry.register(Arc::new(BatchContinueOp));
        registry.register(Arc::new(BatchSkipOp));
        registry.register(Arc::new(BatchAbortOp));
        registry.register(Arc::new(BatchStatusOp));
        registry.register(Arc::new(BatchAddProductsOp));

        // Verification operations (adversarial agent model)
        registry.register(Arc::new(verify_ops::VerifyDetectPatternsOp));
        registry.register(Arc::new(verify_ops::VerifyDetectEvasionOp));
        registry.register(Arc::new(verify_ops::VerifyCalculateConfidenceOp));
        registry.register(Arc::new(verify_ops::VerifyGetStatusOp));
        registry.register(Arc::new(verify_ops::VerifyAgainstRegistryOp));
        registry.register(Arc::new(verify_ops::VerifyAssertOp));

        // Trading Matrix operations (IM assignment, pricing config, SLA)
        registry.register(Arc::new(FindImForTradeOp));
        registry.register(Arc::new(FindPricingForInstrumentOp));
        registry.register(Arc::new(ListOpenSlaBreachesOp));

        // Lifecycle operations (Instrument → Lifecycle → Resource taxonomy)
        registry.register(Arc::new(LifecycleProvisionOp));
        registry.register(Arc::new(LifecycleAnalyzeGapsOp));
        registry.register(Arc::new(LifecycleCheckReadinessOp));
        registry.register(Arc::new(LifecycleDiscoverOp));
        registry.register(Arc::new(LifecycleGeneratePlanOp));
        registry.register(Arc::new(LifecycleExecutePlanOp));

        // Matrix-Overlay operations (Trading Matrix ↔ Product linkage)
        registry.register(Arc::new(MatrixEffectiveOp));
        registry.register(Arc::new(MatrixUnifiedGapsOp));
        registry.register(Arc::new(MatrixCompareProductsOp));

        // Regulatory operations (multi-regulator support)
        registry.register(Arc::new(regulatory_ops::RegistrationVerifyOp));
        registry.register(Arc::new(regulatory_ops::RegulatoryStatusCheckOp));

        // Reference Data bulk loading operations
        for op in get_refdata_operations() {
            registry.register(Arc::from(op));
        }

        // Outstanding Request operations (async fire-and-forget pattern)
        registry.register(Arc::new(RequestCreateOp));
        registry.register(Arc::new(RequestOverdueOp));
        registry.register(Arc::new(RequestFulfillOp));
        registry.register(Arc::new(RequestCancelOp));
        registry.register(Arc::new(RequestExtendOp));
        registry.register(Arc::new(RequestRemindOp));
        registry.register(Arc::new(RequestEscalateOp));
        registry.register(Arc::new(RequestWaiveOp));

        // Document request operations (integrate with outstanding requests)
        registry.register(Arc::new(DocumentRequestOp));
        registry.register(Arc::new(DocumentUploadOp));
        registry.register(Arc::new(DocumentWaiveOp));

        // KYC case state operations (domain-embedded requests)
        registry.register(Arc::new(KycCaseStateOp));
        registry.register(Arc::new(WorkstreamStateOp));

        // Team operations (only transfer-member needs plugin, others are CRUD)
        registry.register(Arc::new(TeamTransferMemberOp));

        // Access Review operations (complex multi-step transactional operations)
        registry.register(Arc::new(AccessReviewPopulateOp));
        registry.register(Arc::new(AccessReviewLaunchOp));
        registry.register(Arc::new(AccessReviewRevokeOp));
        registry.register(Arc::new(AccessReviewBulkConfirmOp));
        registry.register(Arc::new(AccessReviewConfirmCleanOp));
        registry.register(Arc::new(AccessReviewAttestOp));
        registry.register(Arc::new(AccessReviewProcessDeadlineOp));
        registry.register(Arc::new(AccessReviewSendRemindersOp));

        // Temporal operations (point-in-time queries for regulatory lookback)
        registry.register(Arc::new(TemporalOwnershipAsOfOp));
        registry.register(Arc::new(TemporalUboChainAsOfOp));
        registry.register(Arc::new(TemporalCbuRelationshipsAsOfOp));
        registry.register(Arc::new(TemporalCbuRolesAsOfOp));
        registry.register(Arc::new(TemporalCbuStateAtApprovalOp));
        registry.register(Arc::new(TemporalRelationshipHistoryOp));
        registry.register(Arc::new(TemporalEntityHistoryOp));
        registry.register(Arc::new(TemporalCompareOwnershipOp));

        registry
    }

    /// Register a custom operation
    pub fn register(&mut self, op: Arc<dyn CustomOperation>) {
        let key = (op.domain().to_string(), op.verb().to_string());
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

    /// List all registered custom operations
    pub fn list(&self) -> Vec<(&str, &str, &str)> {
        self.operations
            .values()
            .map(|op| (op.domain(), op.verb(), op.rationale()))
            .collect()
    }
}

impl Default for CustomOperationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = CustomOperationRegistry::new();
        assert!(registry.has("document", "catalog"));
        assert!(registry.has("document", "extract"));
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
    }

    #[test]
    fn test_registry_list() {
        let registry = CustomOperationRegistry::new();
        let ops = registry.list();
        // Count updated after entity relationship consolidation
        // Verify we have a reasonable number of operations registered
        assert!(
            ops.len() >= 60,
            "Expected at least 60 operations, got {}",
            ops.len()
        );
    }
}
