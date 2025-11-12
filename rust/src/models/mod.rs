//! Models module for DSL domain architecture
//!
//! This module contains all the data structures and types used to represent
//! DSL domains, versions, AST storage, and execution tracking in the database.

pub(crate) mod business_request_models;
pub mod dictionary_models;
pub(crate) mod document_models;
pub(crate) mod domain_models;
pub(crate) mod entity_models;

// Re-export commonly used types for convenience
pub(crate) use domain_models::{
    CompilationStatus, DomainStatistics, DslDomain, DslExecutionLog, DslExecutionSummary,
    DslLatestVersion, DslVersion, ExecutionPhase, ExecutionStatus, NewDslVersion, NewParsedAst,
    ParsedAst, VersionHistoryEntry,
};

pub(crate) use business_request_models::{
    ActiveBusinessRequestView, BusinessRequestSummary, DslBusinessRequest, DslRequestType,
    DslRequestWorkflowState, NewDslBusinessRequest, NewDslRequestWorkflowState, PriorityLevel,
    RequestStatus, RequestWorkflowHistory, UpdateDslBusinessRequest,
};

pub(crate) use document_models::{
    AiExtractionRequest, AiExtractionResult, ApiResponse, AttributeValue, BulkDocumentImport,
    BulkImportResult, ConfidenceWarning, DocumentCatalog, DocumentCatalogWithMetadata,
    DocumentDetails, DocumentDslContext, DocumentDslOperation, DocumentMetadata,
    DocumentMetadataBatch, DocumentRelationship, DocumentRelationshipType, DocumentSearchRequest,
    DocumentSearchResponse, DocumentStatistics, DocumentSummary, DocumentUsage,
    DocumentUsageContext, DocumentValidationResult, ExtractionStatus, NewDocumentCatalog,
    NewDocumentMetadata, NewDocumentRelationship, NewDocumentUsage, UpdateDocumentCatalog,
    ValidationError,
};

pub use dictionary_models::{
    AgenticAttributeCreateRequest, AgenticAttributeCrudResponse, AgenticAttributeDeleteRequest,
    AgenticAttributeDiscoverRequest, AgenticAttributeReadRequest, AgenticAttributeSearchRequest,
    AgenticAttributeUpdateRequest, AgenticAttributeValidateRequest, AttributeAssetType,
    AttributeBatchItemResult, AttributeBatchRequest, AttributeBatchResult,
    AttributeDiscoveryRequest, AttributeOperationType, AttributeSearchCriteria,
    AttributeValidationRequest, AttributeValidationResult, DictionaryAttribute,
    DictionaryAttributeWithMetadata, DictionaryExecutionStatus, DictionaryHealthCheck,
    DictionaryStatistics, DiscoveredAttribute, NewDictionaryAttribute, UpdateDictionaryAttribute,
};

pub(crate) use entity_models::{
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
