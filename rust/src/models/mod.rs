//! Models module for DSL domain architecture
//!
//! This module contains all the data structures and types used to represent
//! DSL domains, versions, AST storage, and execution tracking in the database.

pub mod business_request_models;
pub mod document_models;
pub mod domain_models;
pub mod entity_models;

// Re-export commonly used types for convenience
pub use domain_models::{
    CompilationStatus, DomainStatistics, DslDomain, DslExecutionLog, DslExecutionSummary,
    DslLatestVersion, DslVersion, ExecutionPhase, ExecutionStatus, NewDslVersion, NewParsedAst,
    ParsedAst, VersionHistoryEntry,
};

pub use business_request_models::{
    ActiveBusinessRequestView, BusinessRequestSummary, DslBusinessRequest, DslRequestType,
    DslRequestWorkflowState, NewDslBusinessRequest, NewDslRequestWorkflowState, PriorityLevel,
    RequestStatus, RequestWorkflowHistory, UpdateDslBusinessRequest,
};

pub use document_models::{
    AiExtractionRequest, AiExtractionResult, ApiResponse, AttributeValue, BulkDocumentImport,
    BulkImportResult, ConfidenceWarning, DocumentCatalog, DocumentCatalogWithMetadata,
    DocumentDetails, DocumentDslContext, DocumentDslOperation, DocumentMetadata,
    DocumentMetadataBatch, DocumentOperationType, DocumentRelationship, DocumentRelationshipType,
    DocumentSearchRequest, DocumentSearchResponse, DocumentStatistics, DocumentSummary,
    DocumentUsage, DocumentUsageContext, DocumentValidationResult, ExtractionStatus,
    NewDocumentCatalog, NewDocumentMetadata, NewDocumentRelationship, NewDocumentUsage,
    UpdateDocumentCatalog, ValidationError,
};

pub use entity_models::{
    AgenticEntityCreateRequest, AgenticEntityCrudResponse, AgenticEntityDeleteRequest,
    AgenticEntityReadRequest, AgenticEntityUpdateRequest, CbuEntityRole, CrudOperation,
    CrudOperationType, DslExample, Entity, EntityAssetType, EntityCrudRule, EntityDetails,
    EntityType, EntityWithDetails, EntityWithType, ExecutionStatus as EntityExecutionStatus,
    LimitedCompany, Partnership, PartnershipControlMechanism,
    PartnershipControlMechanismWithEntity, PartnershipInterest, PartnershipInterestWithEntity,
    PartnershipWithRelationships, ProperPerson, RagEmbedding, Trust, TrustBeneficiaryClass,
    TrustParty, TrustPartyWithEntity, TrustProtectorPower, TrustProtectorPowerWithParty,
    TrustWithRelationships,
};
