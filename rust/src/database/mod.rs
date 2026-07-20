//! Database connection and management module
//!
//! This module provides database connection management, connection pooling,
//! and configuration for the DSL architecture.
//!
//! ## Architecture
//! Database operations flow through dsl_v2::DslExecutor which generates SQL
//! from verb definitions. Domain services provide specialized operations.

use std::time::Duration;

pub mod attribute_values_service;
pub mod bods_service;
// ob-poc-domain split v1 Slice A1 (2026-05-14): bods_types now lives in
// `ob-poc-bods`. The compat re-export below keeps `super::bods_types::*`
// (bods_service) and `crate::database::bods_types::*` (downstream
// consumers) working unchanged.
pub use ob_poc_bods as bods_types;
pub mod cbu_entity_roles_service;
pub mod cbu_service;
pub mod context_discovery_service;
pub mod crud_service;
// Phase 4 Slice B — document_policy_service + governed_document_requirements_service
// relocated to `dsl-runtime::document_requirements::{policy, governed}`.
pub mod document_service;
pub mod dsl_repository;
pub mod entity_service;
pub mod execution_audit;
pub mod expansion_audit;
pub mod semantic_state_service;
// Fuzzy search is now handled by EntityGateway gRPC service.
// See rust/crates/entity-gateway/ for the central lookup service.
pub mod deal_repository;
pub mod generation_log_repository;
pub mod graph_repository;
pub mod locks;
pub mod policy_version_binding_service;
pub mod product_service;
pub mod resource_instance_service;
pub mod service_resource_service;
pub mod service_service;
pub mod session_repository;
pub mod verb_service;
// Phase 4.2b (2026-05-13): now lives in ob-poc-domain (slice 2q → 4.2b).
// ob-poc-domain split v1 Slice C2 (2026-05-14): view_config_service now
// lives in `ob-poc-taxonomy` (paired with taxonomy::rules which imports it).
pub use ob_poc_taxonomy::view_config_service;
pub mod view_state_audit;
pub mod viewport_service;
pub mod visualization_repository;

// Legacy modules not yet integrated - kept for reference but not compiled
// pub mod attribute_repository;
// pub mod document_type_repository;
// pub mod taxonomy_repository;

// Re-export for convenience
pub(crate) use attribute_values_service::{AttributeValueRow, AttributeValuesService};
pub(crate) use bods_service::BodsService;
pub use cbu_entity_roles_service::{RoleRow};
pub(crate) use cbu_entity_roles_service::{CbuEntityRoleExpanded, CbuEntityRolesService};
pub use cbu_service::{CbuRow, CbuService};
pub(crate) use cbu_service::{NewCbuFields};
pub(crate) use crud_service::{AssetType, CrudOperation, CrudService, OperationType};
pub(crate) use document_service::{
    DocumentCatalogEntry, DocumentService, DocumentType, NewDocumentFields,
};
pub(crate) use dsl_repository::{DslRepository, DslSaveResult};
pub use entity_service::{EntityRow, EntityService};
pub(crate) use entity_service::{CbuEntityRoleRow, LimitedCompanyRow, NewEntityFields, NewLimitedCompanyFields, NewPartnershipFields, NewProperPersonFields, NewTrustFields, PartnershipRow, TrustRow};
pub use ob_poc_bods::{
    BodsEntityType, BodsInterestType, EntityIdentifier, EntityWithLei, GleifHierarchyEntry,
    GleifRelationship, NewEntityIdentifier, NewGleifRelationship, NewPersonPepStatus,
    PersonPepStatus, UboInterest,
};

pub(crate) use generation_log_repository::{
    CompileResult, CorrectionPair, GenerationAttempt, GenerationLogRepository, GenerationLogRow,
    GenerationStatsSummary, LintResult, ParseResult, PromptStats, TrainingPair,
};
pub(crate) use product_service::{ProductRow, ProductService};
pub(crate) use resource_instance_service::{
    NewResourceInstance, ResourceInstanceAttributeRow, ResourceInstanceRow,
    ResourceInstanceService, ServiceDeliveryRow, SetInstanceAttribute,
};
pub(crate) use service_resource_service::{ServiceResourceRow, ServiceResourceService};
pub(crate) use service_service::{ServiceRow, ServiceService};
pub(crate) use visualization_repository::{
    CbuBasicView, CbuDocumentView, CbuEntityView, CbuRoleView, CbuScreeningView, CbuSummaryView,
    CbuView, ControlRelationshipView, DocumentAttributeView, DocumentTypeView, EntityAttributeView,
    EntityBasicView, EntityCbuView, EntityRoleView, EntityScreeningView, EntityTypeView,
    EntityView, EntityWithRoleView, HoldingView, LayoutOverrideView, OfficerView, RoleView,
    ServiceDeliveryView, ShareClassView, VisualizationRepository,
};

pub(crate) use session_repository::{
    detect_domain, extract_domains, CbuDslState, DslSnapshot, EntityCreated, PersistedSession,
    SessionEventType, SessionRepository, SessionStatus,
};

pub(crate) use graph_repository::{DerivedBook, GraphRepository, PgGraphRepository};

pub use locks::{acquire_locks, advisory_xact_lock, lock_key, try_advisory_xact_lock};
pub(crate) use locks::{lock_key_from_struct, LockAcquisitionResult, LockError};

pub(crate) use policy_version_binding_service::{
    NewPolicyVersionBinding, PolicyVersionBindingRow, PolicyVersionBindingService,
};

pub(crate) use execution_audit::{
    ExecutionAuditRepository, ExecutionByVerbHash, ExecutionVerbAudit, VerbConfigAtExecution,
};

pub(crate) use expansion_audit::{ExpansionAuditRepository, ExpansionReportRow};

pub(crate) use context_discovery_service::{
    CbuContextRow, ContextDiscoveryService, DiscoveredContext, LinkedContextRow,
};

pub(crate) use view_state_audit::{
    RecordViewStateChange, SessionViewHistoryEntry, ViewStateAuditRepository, ViewStateChange,
};

pub use verb_service::{VerbService};
pub(crate) use verb_service::{SemanticMatch, UserLearnedExactMatch, VerbDescription};

pub use view_config_service::{
    EdgeTypeConfig, LayoutCacheEntry, LayoutConfigEntry, NodeLayoutOverride, NodeTypeConfig,
    ViewConfigService, ViewModeConfig,
};

pub(crate) use viewport_service::{
    CbuCategoryCounts, CbuEntityMember, CbuViewportContainer, ConfidenceZone, EntityRelationship,
    EntityViewportDetail, InstrumentMatrixSummary, InstrumentTypeNode, ViewportService,
};

pub use semantic_state_service::derive_semantic_state;

pub(crate) use deal_repository::DealRepository;

/// Database configuration
#[derive(Debug, Clone)]
pub(crate) struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string()),
            max_connections: std::env::var("DATABASE_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10 minutes
            max_lifetime: Some(Duration::from_secs(1800)), // 30 minutes
        }
    }
}
