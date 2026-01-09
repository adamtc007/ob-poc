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
mod attribute_ops;
pub mod batch_control_ops;
mod board_ops;
mod bods_ops;
mod capital_ops;
mod cbu_ops;
mod cbu_role_ops;
mod control_ops;
mod custody;
mod document_ops;
mod entity_ops;
pub mod entity_query;
mod gleif_ops;
pub mod helpers;
mod investor_ops;
mod kyc_case_ops;
mod lifecycle_ops;
mod matrix_overlay_ops;
mod observation_ops;
mod onboarding;
mod partnership_ops;
mod refdata_loader;
mod regulatory_ops;
mod request_ops;
mod resource_ops;
mod rfi;
mod screening_ops;
mod semantic_ops;
mod session_ops;
mod team_ops;
pub mod template_ops;
mod temporal_ops;
mod threshold;
mod tollgate_ops;
mod trading_matrix;
mod trading_profile;
mod trust_ops;
mod ubo_analysis;
pub mod ubo_graph_ops;
mod verify_ops;
mod view_ops;
mod viewport_ops;

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
    TradingProfileActivateOp,
    TradingProfileAddAllowedCurrencyOp,
    TradingProfileAddBookingRuleOp,
    TradingProfileAddCsaCollateralOp,
    TradingProfileAddCsaConfigOp,
    TradingProfileAddImMandateOp,
    TradingProfileAddInstrumentClassOp,
    TradingProfileAddIsdaConfigOp,
    TradingProfileAddIsdaCoverageOp,
    TradingProfileAddMarketOp,
    TradingProfileAddSsiOp,
    TradingProfileApproveOp,
    TradingProfileArchiveOp,
    TradingProfileCloneToOp,
    TradingProfileCreateDraftOp,
    // Versioned lifecycle: create new version from ACTIVE
    TradingProfileCreateNewVersionOp,
    TradingProfileDiffOp,
    TradingProfileGetActiveOp,
    TradingProfileImportOp,
    TradingProfileLinkCsaSsiOp,
    // Versioned lifecycle: mark as ops-validated (DRAFT → VALIDATED)
    TradingProfileMarkValidatedOp,
    TradingProfileMaterializeOp,
    TradingProfileRejectOp,
    TradingProfileRemoveBookingRuleOp,
    TradingProfileRemoveImMandateOp,
    TradingProfileRemoveInstrumentClassOp,
    TradingProfileRemoveMarketOp,
    TradingProfileRemoveSsiOp,
    TradingProfileSetBaseCurrencyOp,
    // Phase 6: Lifecycle operations
    TradingProfileSubmitOp,
    TradingProfileSyncFromOperationalOp,
    TradingProfileUpdateImScopeOp,
    TradingProfileValidateCoverageOp,
    TradingProfileValidateGoLiveReadyOp,
    TradingProfileValidateOp,
};
pub use ubo_analysis::{
    UboCalculateOp, UboCheckCompletenessOp, UboCompareSnapshotOp, UboDiscoverOwnerOp,
    UboInferChainOp, UboListOwnersOp, UboSnapshotCbuOp, UboSupersedeOp, UboTraceChainsOp,
};

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
pub use entity_ops::{EntityCreateOp, EntityGhostOp, EntityIdentifyOp, EntityRenameOp};
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

// BODS operations (UBO discovery via GLEIF + BODS)
pub use bods_ops::{
    BodsDiscoverUbosOp, BodsFindByLeiOp, BodsGetStatementOp, BodsImportOp, BodsListOwnershipOp,
    BodsSyncFromGleifOp,
};

// View operations (session scope and selection management)
pub use view_ops::{
    ViewBackToOp,
    ViewBlackHolesOp,
    ViewBookOp,
    ViewBreadcrumbsOp,
    ViewCbuOp,
    ViewClearOp,
    ViewContextOp,
    ViewDetailOp,
    // Esper navigation operations (Phase 1: Blade Runner-inspired navigation)
    ViewDrillOp,
    ViewEntityForestOp,
    ViewIlluminateOp,
    ViewLayoutOp,
    ViewOpResult,
    ViewPeelOp,
    ViewRedFlagOp,
    ViewRefineOp,
    ViewSelectOp,
    ViewSelectionInfoOp,
    ViewShadowOp,
    ViewStatusOp,
    ViewSurfaceOp,
    ViewTraceOp,
    ViewUniverseOp,
    ViewXrayOp,
    ViewZoomInOp,
    ViewZoomOutOp,
};

// Viewport operations (viewport state management for Decker/Esper navigation)
pub use viewport_ops::{
    ViewportAscendOp, ViewportCameraOp, ViewportClearOp, ViewportDescendOp, ViewportEnhanceOp,
    ViewportFilterOp, ViewportFocusOp, ViewportTrackOp, ViewportViewTypeOp,
};

// KYC Control Enhancement operations (capital, board, trust, partnership, tollgate, control)
pub use board_ops::BoardAnalyzeControlOp;
pub use capital_ops::{
    CapitalCancelSharesOp, CapitalIssueSharesOp, CapitalOwnershipChainOp, CapitalReconcileOp,
    CapitalTransferOp,
};
pub use control_ops::{
    ControlAnalyzeOp, ControlBuildGraphOp, ControlIdentifyUbosOp, ControlReconcileOwnershipOp,
    ControlTraceChainOp,
};
pub use partnership_ops::{
    PartnershipAnalyzeControlOp, PartnershipContributionOp, PartnershipDistributionOp,
    PartnershipReconcileOp,
};
pub use tollgate_ops::{
    TollgateDecisionReadinessOp, TollgateEvaluateOp, TollgateGetMetricsOp, TollgateOverrideOp,
};
pub use trust_ops::{TrustAnalyzeControlOp, TrustClassifyOp, TrustIdentifyUbosOp};

// GLEIF operations (LEI data enrichment)
pub use gleif_ops::{
    GleifEnrichOp, GleifGetChildrenOp, GleifGetManagedFundsOp, GleifGetManagerOp,
    GleifGetMasterFundOp, GleifGetParentOp, GleifGetRecordOp, GleifGetUmbrellaOp,
    GleifImportManagedFundsOp, GleifImportTreeOp, GleifLookupByIsinOp, GleifRefreshOp,
    GleifResolveSuccessorOp, GleifSearchOp, GleifTraceOwnershipOp,
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
        registry.register(Arc::new(EntityGhostOp));
        registry.register(Arc::new(EntityIdentifyOp));
        registry.register(Arc::new(EntityRenameOp));
        registry.register(Arc::new(DocumentCatalogOp));
        registry.register(Arc::new(DocumentExtractOp));

        // Attribute operations (document-attribute catalogue management)
        registry.register(Arc::new(AttributeListSourcesOp));
        registry.register(Arc::new(AttributeListSinksOp));
        registry.register(Arc::new(AttributeTraceLineageOp));
        registry.register(Arc::new(AttributeListByDocumentOp));
        registry.register(Arc::new(AttributeCheckCoverageOp));
        registry.register(Arc::new(DocumentListAttributesOp));
        registry.register(Arc::new(DocumentCheckExtractionCoverageOp));

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
        // Document construction operations (Phase 1)
        registry.register(Arc::new(TradingProfileCreateDraftOp));
        registry.register(Arc::new(TradingProfileAddInstrumentClassOp));
        registry.register(Arc::new(TradingProfileRemoveInstrumentClassOp));
        registry.register(Arc::new(TradingProfileAddMarketOp));
        registry.register(Arc::new(TradingProfileRemoveMarketOp));
        registry.register(Arc::new(TradingProfileAddSsiOp));
        registry.register(Arc::new(TradingProfileRemoveSsiOp));
        registry.register(Arc::new(TradingProfileAddBookingRuleOp));
        registry.register(Arc::new(TradingProfileRemoveBookingRuleOp));
        // ISDA/CSA construction operations (Phase 2)
        registry.register(Arc::new(TradingProfileAddIsdaConfigOp));
        registry.register(Arc::new(TradingProfileAddIsdaCoverageOp));
        registry.register(Arc::new(TradingProfileAddCsaConfigOp));
        registry.register(Arc::new(TradingProfileAddCsaCollateralOp));
        registry.register(Arc::new(TradingProfileLinkCsaSsiOp));
        // IM mandate and settlement config operations (Phase 3)
        registry.register(Arc::new(TradingProfileAddImMandateOp));
        registry.register(Arc::new(TradingProfileUpdateImScopeOp));
        registry.register(Arc::new(TradingProfileRemoveImMandateOp));
        registry.register(Arc::new(TradingProfileSetBaseCurrencyOp));
        registry.register(Arc::new(TradingProfileAddAllowedCurrencyOp));
        // Sync operations (Phase 4)
        registry.register(Arc::new(TradingProfileDiffOp));
        registry.register(Arc::new(TradingProfileSyncFromOperationalOp));
        // Validation operations (Phase 5)
        registry.register(Arc::new(TradingProfileValidateCoverageOp));
        registry.register(Arc::new(TradingProfileValidateGoLiveReadyOp));

        // Lifecycle operations (Phase 6)
        registry.register(Arc::new(TradingProfileSubmitOp));
        registry.register(Arc::new(TradingProfileApproveOp));
        registry.register(Arc::new(TradingProfileRejectOp));
        registry.register(Arc::new(TradingProfileArchiveOp));

        // Versioned document lifecycle operations (Phase 7)
        registry.register(Arc::new(TradingProfileCreateNewVersionOp));
        registry.register(Arc::new(TradingProfileMarkValidatedOp));

        // Clone operation
        registry.register(Arc::new(TradingProfileCloneToOp));

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

        // GLEIF operations (LEI data enrichment from GLEIF API)
        registry.register(Arc::new(GleifEnrichOp));
        registry.register(Arc::new(GleifSearchOp));
        registry.register(Arc::new(GleifImportTreeOp));
        registry.register(Arc::new(GleifImportManagedFundsOp));
        registry.register(Arc::new(GleifRefreshOp));
        registry.register(Arc::new(GleifGetRecordOp));
        registry.register(Arc::new(GleifGetParentOp));
        registry.register(Arc::new(GleifGetChildrenOp));
        registry.register(Arc::new(GleifTraceOwnershipOp));
        registry.register(Arc::new(GleifGetManagedFundsOp));
        registry.register(Arc::new(GleifResolveSuccessorOp));
        // Lean GLEIF fund structure relationship verbs
        registry.register(Arc::new(GleifGetUmbrellaOp));
        registry.register(Arc::new(GleifGetManagerOp));
        registry.register(Arc::new(GleifGetMasterFundOp));
        registry.register(Arc::new(GleifLookupByIsinOp));

        // BODS operations (UBO discovery via GLEIF + BODS)
        registry.register(Arc::new(BodsDiscoverUbosOp));
        registry.register(Arc::new(BodsImportOp));
        registry.register(Arc::new(BodsGetStatementOp));
        registry.register(Arc::new(BodsFindByLeiOp));
        registry.register(Arc::new(BodsListOwnershipOp));
        registry.register(Arc::new(BodsSyncFromGleifOp));

        // View operations (session scope and selection management)
        registry.register(Arc::new(ViewUniverseOp));
        registry.register(Arc::new(ViewBookOp));
        registry.register(Arc::new(ViewCbuOp));
        registry.register(Arc::new(ViewEntityForestOp));
        registry.register(Arc::new(ViewRefineOp));
        registry.register(Arc::new(ViewClearOp));
        registry.register(Arc::new(ViewSelectOp));
        registry.register(Arc::new(ViewLayoutOp));
        registry.register(Arc::new(ViewStatusOp));
        registry.register(Arc::new(ViewSelectionInfoOp));
        // Zoom navigation (fractal taxonomy navigation)
        registry.register(Arc::new(ViewZoomInOp));
        registry.register(Arc::new(ViewZoomOutOp));
        registry.register(Arc::new(ViewBackToOp));
        registry.register(Arc::new(ViewBreadcrumbsOp));

        // Esper navigation operations (Phase 1: Blade Runner-inspired navigation)
        registry.register(Arc::new(ViewDrillOp));
        registry.register(Arc::new(ViewSurfaceOp));
        registry.register(Arc::new(ViewTraceOp));
        registry.register(Arc::new(ViewXrayOp));
        registry.register(Arc::new(ViewPeelOp));
        registry.register(Arc::new(ViewIlluminateOp));
        registry.register(Arc::new(ViewShadowOp));
        registry.register(Arc::new(ViewRedFlagOp));
        registry.register(Arc::new(ViewBlackHolesOp));
        registry.register(Arc::new(ViewDetailOp));
        registry.register(Arc::new(ViewContextOp));

        // Viewport operations (viewport state management for Decker/Esper navigation)
        registry.register(Arc::new(ViewportFocusOp));
        registry.register(Arc::new(ViewportEnhanceOp));
        registry.register(Arc::new(ViewportAscendOp));
        registry.register(Arc::new(ViewportDescendOp));
        registry.register(Arc::new(ViewportCameraOp));
        registry.register(Arc::new(ViewportFilterOp));
        registry.register(Arc::new(ViewportTrackOp));
        registry.register(Arc::new(ViewportClearOp));
        registry.register(Arc::new(ViewportViewTypeOp));

        // KYC Control Enhancement operations (capital, board, trust, partnership, tollgate, control)
        // Capital operations (share class transfers, reconciliation, ownership chains)
        registry.register(Arc::new(CapitalTransferOp));
        registry.register(Arc::new(CapitalReconcileOp));
        registry.register(Arc::new(CapitalOwnershipChainOp));
        registry.register(Arc::new(CapitalIssueSharesOp));
        registry.register(Arc::new(CapitalCancelSharesOp));

        // Board operations (board composition control analysis)
        registry.register(Arc::new(BoardAnalyzeControlOp));

        // Trust operations (trust control analysis, UBO identification)
        registry.register(Arc::new(TrustAnalyzeControlOp));
        registry.register(Arc::new(TrustIdentifyUbosOp));
        registry.register(Arc::new(TrustClassifyOp));

        // Partnership operations (capital contributions, distributions, GP/LP control)
        registry.register(Arc::new(PartnershipContributionOp));
        registry.register(Arc::new(PartnershipDistributionOp));
        registry.register(Arc::new(PartnershipReconcileOp));
        registry.register(Arc::new(PartnershipAnalyzeControlOp));

        // Tollgate operations (decision gates with metrics and overrides)
        registry.register(Arc::new(TollgateEvaluateOp));
        registry.register(Arc::new(TollgateGetMetricsOp));
        registry.register(Arc::new(TollgateOverrideOp));
        registry.register(Arc::new(TollgateDecisionReadinessOp));

        // Unified control operations (cross-vector control analysis)
        registry.register(Arc::new(ControlAnalyzeOp));
        registry.register(Arc::new(ControlBuildGraphOp));
        registry.register(Arc::new(ControlIdentifyUbosOp));
        registry.register(Arc::new(ControlTraceChainOp));
        registry.register(Arc::new(ControlReconcileOwnershipOp));

        // Investor lifecycle operations (TA KYC-as-a-Service)
        investor_ops::register_investor_ops(&mut registry);

        // Session scope management operations
        session_ops::register_session_ops(&mut registry);

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
        // Entity ghost lifecycle operations
        assert!(registry.has("entity", "ghost"));
        assert!(registry.has("entity", "identify"));
        assert!(registry.has("entity", "rename"));
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
        // Esper navigation operations (Phase 1: Blade Runner-inspired navigation)
        assert!(registry.has("view", "drill"));
        assert!(registry.has("view", "surface"));
        assert!(registry.has("view", "trace"));
        assert!(registry.has("view", "xray"));
        assert!(registry.has("view", "peel"));
        assert!(registry.has("view", "illuminate"));
        assert!(registry.has("view", "shadow"));
        assert!(registry.has("view", "red-flag"));
        assert!(registry.has("view", "black-holes"));
        assert!(registry.has("view", "detail"));
        assert!(registry.has("view", "context"));
        // Viewport operations (Decker/Esper viewport state management)
        assert!(registry.has("viewport", "focus"));
        assert!(registry.has("viewport", "enhance"));
        assert!(registry.has("viewport", "ascend"));
        assert!(registry.has("viewport", "descend"));
        assert!(registry.has("viewport", "camera"));
        assert!(registry.has("viewport", "filter"));
        assert!(registry.has("viewport", "track"));
        assert!(registry.has("viewport", "clear"));
        assert!(registry.has("viewport", "view-type"));
        // KYC Control Enhancement: Capital operations
        assert!(registry.has("capital", "transfer"));
        assert!(registry.has("capital", "reconcile"));
        assert!(registry.has("capital", "get-ownership-chain"));
        assert!(registry.has("capital", "issue-shares"));
        assert!(registry.has("capital", "cancel-shares"));
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
        assert!(registry.has("trading-profile", "mark-validated"));
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
}
