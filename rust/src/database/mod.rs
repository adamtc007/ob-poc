//! Database connection and management module
//!
//! This module provides database connection management, connection pooling,
//! and configuration for the DSL architecture.
//!
//! ## Architecture
//! Database operations flow through dsl_v2::DslExecutor which generates SQL
//! from verb definitions. Domain services provide specialized operations.

// ob-poc-domain split v1 Slice A1 (2026-05-14): bods_types now lives in
// `ob-poc-bods`. The compat re-export below keeps `super::bods_types::*`
// (bods_service) and `crate::database::bods_types::*` (downstream
// consumers) working unchanged.
pub use ob_poc_bods as bods_types;
pub mod cbu_service;
pub mod context_discovery_service;
// Phase 4 Slice B — document_policy_service + governed_document_requirements_service
// relocated to `dsl-runtime::document_requirements::{policy, governed}`.
pub mod dsl_repository;
pub mod entity_service;
pub mod expansion_audit;
pub mod semantic_state_service;
// Fuzzy search is now handled by EntityGateway gRPC service.
// See rust/crates/entity-gateway/ for the central lookup service.
pub mod deal_repository;
pub mod generation_log_repository;
pub mod graph_repository;
pub mod locks;
pub mod session_repository;
pub mod verb_service;
// Phase 4.2b (2026-05-13): now lives in ob-poc-domain (slice 2q → 4.2b).
// ob-poc-domain split v1 Slice C2 (2026-05-14): view_config_service now
// lives in `ob-poc-taxonomy` (paired with taxonomy::rules which imports it).
pub use ob_poc_taxonomy::view_config_service;
pub mod visualization_repository;

// Legacy modules not yet integrated - kept for reference but not compiled
// pub mod attribute_repository;
// pub mod document_type_repository;
// pub mod taxonomy_repository;

// Re-export for convenience
pub use cbu_service::{CbuRow, CbuService};
pub(crate) use cbu_service::{NewCbuFields};
pub(crate) use dsl_repository::{DslRepository, DslSaveResult};
pub use entity_service::{EntityRow, EntityService};
pub(crate) use entity_service::{CbuEntityRoleRow, LimitedCompanyRow, NewEntityFields, NewLimitedCompanyFields, NewPartnershipFields, NewProperPersonFields, NewTrustFields, PartnershipRow, TrustRow};
pub use ob_poc_bods::{
    BodsEntityType, BodsInterestType, EntityIdentifier, EntityWithLei, GleifHierarchyEntry,
    GleifRelationship, NewEntityIdentifier, NewGleifRelationship, NewPersonPepStatus,
    PersonPepStatus, UboInterest,
};

pub(crate) use generation_log_repository::{
    CompileResult, GenerationAttempt, GenerationLogRepository, LintResult, ParseResult,
};
pub(crate) use visualization_repository::{
    CbuBasicView, CbuDocumentView, CbuEntityView, CbuRoleView, CbuScreeningView, CbuSummaryView,
    DocumentTypeView, EntityAttributeView,
    EntityBasicView, EntityCbuView, EntityRoleView, EntityScreeningView, EntityTypeView,
    LayoutOverrideView, RoleView, VisualizationRepository,
};

pub(crate) use session_repository::{
    detect_domain, extract_domains, DslSnapshot, EntityCreated, PersistedSession,
    SessionEventType, SessionStatus,
};
// `CbuDslState`/`SessionRepository` are `pub` (not `pub(crate)`): the
// `dsl_cli` binary (`src/bin/dsl_cli.rs`) is a separate compilation unit
// and needs to name them across the crate boundary.
pub use session_repository::{CbuDslState, SessionRepository};

pub(crate) use graph_repository::{GraphRepository, PgGraphRepository};

pub use locks::{acquire_locks, advisory_xact_lock, lock_key, try_advisory_xact_lock};
pub(crate) use locks::{lock_key_from_struct, LockAcquisitionResult, LockError};

pub(crate) use expansion_audit::ExpansionAuditRepository;

pub(crate) use context_discovery_service::{
    CbuContextRow, ContextDiscoveryService, DiscoveredContext, LinkedContextRow,
};

pub use verb_service::{VerbService};
pub(crate) use verb_service::{SemanticMatch, UserLearnedExactMatch, VerbDescription};

pub use view_config_service::{
    EdgeTypeConfig, LayoutCacheEntry, LayoutConfigEntry, NodeLayoutOverride, NodeTypeConfig,
    ViewConfigService, ViewModeConfig,
};

pub use semantic_state_service::derive_semantic_state;

pub(crate) use deal_repository::DealRepository;
