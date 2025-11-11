//! Models module for DSL domain architecture
//!
//! This module contains all the data structures and types used to represent
//! DSL domains, versions, AST storage, and execution tracking in the database.

pub mod business_request_models;
pub mod document_models;
pub mod domain_models;

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
    AiExtractionRequest, AiExtractionResult, ApiResponse, AssetSuitabilityCheck,
    AttributeValidationError, BulkDocumentImport, BulkImportResult, DocumentAttributeStatistics,
    DocumentCatalog, DocumentCatalogWithAttributes, DocumentDslIntegration, DocumentIssuer,
    DocumentMappingSummary, DocumentRelationship, DocumentSearchRequest, DocumentSearchResponse,
    DocumentStateSnapshot, DocumentType, DocumentTypeStatistics, DocumentUsage,
    DocumentValidationResult, InvestmentMandateExtraction, InvestmentMandateValidation,
    IsoAssetType, NewDocumentCatalog, NewDocumentIssuer, NewDocumentRelationship, NewDocumentType,
    NewDocumentUsage, NewIsoAssetType, UpdateDocumentCatalog, UpdateDocumentType,
};
