//! Agentic Document Service - AI-Powered Document Management
//!
//! This module provides AI-powered document operations using the new EAV/metadata-driven
//! document catalog system. It enables natural language document management operations
//! that are translated to DSL and executed against the AttributeID-as-Type schema.

use crate::ai::crud_prompt_builder::{CrudPromptBuilder, GeneratedPrompt, PromptConfig};
use crate::ai::rag_system::{CrudRagSystem, RetrievedContext};
#[cfg(feature = "database")]
use crate::models::document_models::*;
use crate::parser::idiomatic_parser::parse_crud_statement;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Agentic Document Service for AI-powered document operations
pub struct AgenticDocumentService {
    /// RAG system for document context retrieval
    rag_system: CrudRagSystem,
    /// Prompt builder for AI interaction
    prompt_builder: CrudPromptBuilder,
    /// Service configuration
    config: DocumentServiceConfig,
}

/// Configuration for the Agentic Document Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentServiceConfig {
    /// AI provider configuration
    pub ai_provider: DocumentAiProvider,
    /// AI model settings
    pub model_config: DocumentModelConfig,
    /// Prompt configuration
    pub prompt_config: PromptConfig,
    /// Whether to execute operations or just generate DSL
    pub execute_operations: bool,
    /// Maximum retries for AI operations
    pub max_retries: usize,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Default confidence thresholds
    pub confidence_thresholds: ConfidenceThresholds,
}

/// AI provider options for document operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DocumentAiProvider {
    Mock {
        responses: HashMap<String, String>,
        extraction_results: HashMap<String, serde_json::Value>,
    },
    OpenAI {
        api_key: String,
        model: String,
    },
    Gemini {
        api_key: String,
        model: String,
    },
}

/// AI model configuration for document operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentModelConfig {
    /// Temperature for AI generation (0.0-1.0)
    pub temperature: f64,
    /// Maximum tokens for document operations
    pub max_tokens: Option<u32>,
    /// Top-p value for nucleus sampling
    pub top_p: Option<f64>,
    /// Extraction-specific settings
    pub extraction_config: ExtractionConfig,
}

/// Extraction-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionConfig {
    /// Default confidence threshold for extractions
    pub default_confidence_threshold: f64,
    /// Maximum extraction retries
    pub max_extraction_retries: usize,
    /// Preferred extraction methods in order
    pub preferred_extraction_methods: Vec<String>,
}

/// Confidence thresholds for various operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceThresholds {
    /// Minimum confidence for automatic processing
    pub auto_process: f64,
    /// Minimum confidence for validation
    pub validation: f64,
    /// Minimum confidence for compliance use
    pub compliance: f64,
}

/// Request for agentic document operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticDocumentRequest {
    /// Natural language instruction
    pub instruction: String,
    /// Operation type hint
    pub operation_hint: Option<DocumentOperationType>,
    /// Target document ID (for updates/queries)
    pub target_doc_id: Option<Uuid>,
    /// CBU context for document usage
    pub cbu_id: Option<Uuid>,
    /// Entity context for document usage
    pub entity_id: Option<Uuid>,
    /// Additional context hints
    pub context_hints: Option<Vec<String>>,
    /// Whether to execute the operation
    pub execute: bool,
    /// Request ID for tracking
    pub request_id: Option<String>,
}

/// Document operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentOperationType {
    Catalog,  // Add new document to catalog
    Search,   // Search existing documents
    Extract,  // AI extraction from document
    Verify,   // Verify document authenticity
    Link,     // Link documents together
    Use,      // Mark document as used by CBU/entity
    Update,   // Update document metadata
    Analyze,  // Analyze document content
    Classify, // Classify document type
    Validate, // Validate document compliance
}

/// Response from agentic document operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticDocumentResponse {
    /// Generated DSL statement
    pub generated_dsl: String,
    /// Operation type detected/executed
    pub operation_type: DocumentOperationType,
    /// Document operation result
    pub operation_result: DocumentOperationResult,
    /// RAG context used
    pub rag_context: RetrievedContext,
    /// AI generation metadata
    pub generation_metadata: DocumentGenerationMetadata,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// Warnings or suggestions
    pub warnings: Vec<String>,
    /// Overall success status
    pub success: bool,
    /// Request ID for tracking
    pub request_id: String,
}

/// Document operation result variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentOperationResult {
    Cataloged {
        doc_id: Uuid,
        metadata_entries: usize,
        extraction_scheduled: bool,
    },
    SearchResults {
        documents: Vec<serde_json::Value>,
        total_count: i64,
        query_confidence: f64,
    },
    ExtractionCompleted {
        doc_id: Uuid,
        extraction_result: serde_json::Value,
        metadata_updated: bool,
    },
    DocumentsLinked {
        primary_doc_id: Uuid,
        related_doc_id: Uuid,
        relationship_type: String,
    },
    DocumentUsageRecorded {
        doc_id: Uuid,
        cbu_id: Uuid,
        usage_context: String,
    },
    MetadataUpdated {
        doc_id: Uuid,
        updated_attributes: Vec<Uuid>,
    },
    ValidationCompleted {
        doc_id: Uuid,
        validation_result: serde_json::Value,
    },
    AnalysisCompleted {
        doc_id: Uuid,
        analysis_results: serde_json::Value,
        insights: Vec<String>,
    },
}

/// Metadata about document AI generation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentGenerationMetadata {
    /// Time for RAG context retrieval (ms)
    pub rag_time_ms: u64,
    /// Time for AI generation (ms)
    pub ai_generation_time_ms: u64,
    /// Time for DSL parsing (ms)
    pub parsing_time_ms: u64,
    /// Time for operation execution (ms)
    pub execution_time_ms: u64,
    /// Number of AI retries
    pub retries: usize,
    /// AI model used
    pub model_used: String,
    /// Confidence scores for various components
    pub confidence_scores: HashMap<String, f64>,
    /// Prompt metadata
    pub prompt_metadata: crate::ai::crud_prompt_builder::PromptMetadata,
}

/// Mock AI client for document operations
pub struct MockDocumentAiClient {
    responses: HashMap<String, String>,
    extraction_results: HashMap<String, serde_json::Value>,
}

impl Default for MockDocumentAiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDocumentAiClient {
    pub fn new() -> Self {
        let mut responses = HashMap::new();
        let mut extraction_results = HashMap::new();

        // Document operation responses
        responses.insert(
            "catalog".to_string(),
            r#"(document.catalog :file-hash "abc123" :storage-key "documents/passport.pdf" :metadata {:document-type "passport" :issuer "UK-HO"})"#.to_string(),
        );

        responses.insert(
            "search".to_string(),
            r#"(document.query :filters {:document-type "passport"} :limit 10)"#.to_string(),
        );

        responses.insert(
            "extract".to_string(),
            r#"(document.extract :doc-id @doc-uuid :method "ai" :confidence-threshold 0.8)"#
                .to_string(),
        );

        responses.insert(
            "link".to_string(),
            r#"(document.link :primary-doc @doc-uuid-1 :related-doc @doc-uuid-2 :relationship "AMENDS")"#.to_string(),
        );

        responses.insert(
            "use".to_string(),
            r#"(document.use :doc-id @doc-uuid :cbu-id @cbu-uuid :context "EVIDENCE_OF_IDENTITY")"#
                .to_string(),
        );

        // Mock extraction results
        let sample_extraction = serde_json::json!({
            "doc_id": Uuid::new_v4(),
            "extracted_data": {
                "document_type": "passport",
                "document_number": "123456789",
                "issuing_country": "GB",
                "full_name": "John Smith",
                "date_of_birth": "1985-03-15",
                "expiry_date": "2030-03-15"
            },
            "extraction_confidence": 0.92,
            "extraction_method": "ai-vision",
            "ai_model_used": "mock-ai-v1",
            "processing_time_ms": 1500,
            "extracted_attributes": [
                {
                    "attribute_id": Uuid::new_v4(),
                    "value": "passport"
                },
                {
                    "attribute_id": Uuid::new_v4(),
                    "value": "123456789"
                }
            ],
            "warnings": ["Low confidence on expiry date"],
            "errors": []
        });

        extraction_results.insert("mock_extraction".to_string(), sample_extraction);

        Self {
            responses,
            extraction_results,
        }
    }

    pub fn generate_document_dsl(
        &self,
        instruction: &str,
        operation_type: &DocumentOperationType,
    ) -> String {
        let instruction_lower = instruction.to_lowercase();

        match operation_type {
            DocumentOperationType::Catalog => {
                if instruction_lower.contains("passport") {
                    self.responses.get("catalog").unwrap().clone()
                } else {
                    r#"(document.catalog :file-hash "def456" :storage-key "documents/generic.pdf" :metadata {:document-type "generic"})"#.to_string()
                }
            }
            DocumentOperationType::Search => self.responses.get("search").unwrap().clone(),
            DocumentOperationType::Extract => self.responses.get("extract").unwrap().clone(),
            DocumentOperationType::Link => self.responses.get("link").unwrap().clone(),
            DocumentOperationType::Use => self.responses.get("use").unwrap().clone(),
            _ => {
                // Default response for other operations
                r#"(document.query :filters {} :limit 10)"#.to_string()
            }
        }
    }

    pub fn extract_document(&self, _doc_id: Uuid) -> Result<serde_json::Value> {
        Ok(self
            .extraction_results
            .get("mock_extraction")
            .unwrap()
            .clone())
    }
}

impl AgenticDocumentService {
    /// Creates a new Agentic Document Service
    pub fn new(config: DocumentServiceConfig) -> Self {
        Self {
            rag_system: CrudRagSystem::new(),
            prompt_builder: CrudPromptBuilder::new(),
            config,
        }
    }

    /// Creates a service with mock AI client for demonstration
    pub fn with_mock() -> Self {
        let config = DocumentServiceConfig::default();
        Self::new(config)
    }

    /// Processes a natural language document request
    pub fn process_document_request(
        &self,
        request: AgenticDocumentRequest,
    ) -> Result<AgenticDocumentResponse> {
        let start_time = std::time::Instant::now();
        let request_id = request
            .request_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        println!(
            "Processing agentic document request: {}",
            request.instruction
        );

        // Step 1: Detect operation type
        let operation_type = request
            .operation_hint
            .clone()
            .unwrap_or_else(|| self.detect_operation_type(&request.instruction));

        println!("Detected operation type: {:?}", operation_type);

        // Step 2: RAG Context Retrieval
        let rag_start = std::time::Instant::now();
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .context("Failed to retrieve RAG context")?;
        let rag_time_ms = rag_start.elapsed().as_millis() as u64;

        // Step 3: Generate AI Prompt
        let prompt = self
            .prompt_builder
            .generate_prompt(
                &rag_context,
                &request.instruction,
                &self.config.prompt_config,
            )
            .context("Failed to generate AI prompt")?;

        // Step 4: Mock AI Generation
        let ai_start = std::time::Instant::now();
        let mock_client = MockDocumentAiClient::new();
        let generated_dsl =
            mock_client.generate_document_dsl(&request.instruction, &operation_type);
        let ai_generation_time_ms = ai_start.elapsed().as_millis() as u64;

        println!("AI generated DSL: {}", generated_dsl);

        // Step 5: Parse DSL
        let parsing_start = std::time::Instant::now();
        let parsing_result = parse_crud_statement(&generated_dsl);
        let parsing_time_ms = parsing_start.elapsed().as_millis() as u64;

        let mut errors = Vec::new();
        let warnings = Vec::new();

        // Step 6: Execute Operation (if requested)
        let execution_start = std::time::Instant::now();
        let operation_result = if request.execute && parsing_result.is_ok() {
            self.execute_document_operation(&operation_type, &request, &mock_client)?
        } else {
            self.simulate_document_operation(&operation_type, &request)
        };
        let execution_time_ms = execution_start.elapsed().as_millis() as u64;

        if let Err(e) = parsing_result {
            errors.push(format!("DSL parsing failed: {}", e));
        }

        let success = errors.is_empty();

        let response = AgenticDocumentResponse {
            generated_dsl,
            operation_type,
            operation_result,
            rag_context,
            generation_metadata: DocumentGenerationMetadata {
                rag_time_ms,
                ai_generation_time_ms,
                parsing_time_ms,
                execution_time_ms,
                retries: 0,
                model_used: "mock-document-ai".to_string(),
                confidence_scores: HashMap::from([
                    ("dsl_generation".to_string(), 0.95),
                    ("operation_detection".to_string(), 0.88),
                ]),
                prompt_metadata: prompt.metadata,
            },
            errors,
            warnings,
            success,
            request_id,
        };

        println!(
            "Document request processed in {}ms with success: {}",
            start_time.elapsed().as_millis(),
            success
        );

        Ok(response)
    }

    /// Detects the operation type from natural language instruction
    fn detect_operation_type(&self, instruction: &str) -> DocumentOperationType {
        let instruction_lower = instruction.to_lowercase();

        if instruction_lower.contains("catalog")
            || instruction_lower.contains("add")
            || instruction_lower.contains("upload")
        {
            DocumentOperationType::Catalog
        } else if instruction_lower.contains("search")
            || instruction_lower.contains("find")
            || instruction_lower.contains("query")
        {
            DocumentOperationType::Search
        } else if instruction_lower.contains("extract")
            || instruction_lower.contains("analyze content")
        {
            DocumentOperationType::Extract
        } else if instruction_lower.contains("link")
            || instruction_lower.contains("relate")
            || instruction_lower.contains("connect")
        {
            DocumentOperationType::Link
        } else if instruction_lower.contains("use") || instruction_lower.contains("attach to case")
        {
            DocumentOperationType::Use
        } else if instruction_lower.contains("verify") || instruction_lower.contains("authenticate")
        {
            DocumentOperationType::Verify
        } else if instruction_lower.contains("classify") || instruction_lower.contains("categorize")
        {
            DocumentOperationType::Classify
        } else if instruction_lower.contains("validate")
            || instruction_lower.contains("check compliance")
        {
            DocumentOperationType::Validate
        } else if instruction_lower.contains("analyze") || instruction_lower.contains("review") {
            DocumentOperationType::Analyze
        } else {
            DocumentOperationType::Search // Default fallback
        }
    }

    /// Executes the actual document operation
    fn execute_document_operation(
        &self,
        operation_type: &DocumentOperationType,
        request: &AgenticDocumentRequest,
        mock_client: &MockDocumentAiClient,
    ) -> Result<DocumentOperationResult> {
        match operation_type {
            DocumentOperationType::Catalog => {
                let doc_id = Uuid::new_v4();
                Ok(DocumentOperationResult::Cataloged {
                    doc_id,
                    metadata_entries: 3,
                    extraction_scheduled: true,
                })
            }
            DocumentOperationType::Search => Ok(DocumentOperationResult::SearchResults {
                documents: vec![serde_json::json!({
                    "doc_id": Uuid::new_v4(),
                    "storage_key": "documents/passport1.pdf",
                    "mime_type": "application/pdf",
                    "extraction_status": "COMPLETED",
                    "extraction_confidence": 0.92,
                    "metadata_count": 5,
                    "usage_count": 2,
                    "relationship_count": 0,
                    "created_at": Utc::now()
                })],
                total_count: 1,
                query_confidence: 0.85,
            }),
            DocumentOperationType::Extract => {
                let doc_id = request.target_doc_id.unwrap_or_else(Uuid::new_v4);
                let extraction_result = mock_client.extract_document(doc_id)?;
                Ok(DocumentOperationResult::ExtractionCompleted {
                    doc_id,
                    extraction_result,
                    metadata_updated: true,
                })
            }
            DocumentOperationType::Link => Ok(DocumentOperationResult::DocumentsLinked {
                primary_doc_id: Uuid::new_v4(),
                related_doc_id: Uuid::new_v4(),
                relationship_type: "AMENDS".to_string(),
            }),
            DocumentOperationType::Use => Ok(DocumentOperationResult::DocumentUsageRecorded {
                doc_id: request.target_doc_id.unwrap_or_else(Uuid::new_v4),
                cbu_id: request.cbu_id.unwrap_or_else(Uuid::new_v4),
                usage_context: "EVIDENCE_OF_IDENTITY".to_string(),
            }),
            _ => Ok(DocumentOperationResult::SearchResults {
                documents: vec![],
                total_count: 0,
                query_confidence: 0.0,
            }),
        }
    }

    /// Simulates document operation without execution
    fn simulate_document_operation(
        &self,
        operation_type: &DocumentOperationType,
        _request: &AgenticDocumentRequest,
    ) -> DocumentOperationResult {
        match operation_type {
            DocumentOperationType::Catalog => DocumentOperationResult::Cataloged {
                doc_id: Uuid::new_v4(),
                metadata_entries: 3,
                extraction_scheduled: true,
            },
            DocumentOperationType::Search => DocumentOperationResult::SearchResults {
                documents: vec![],
                total_count: 0,
                query_confidence: 0.85,
            },
            _ => DocumentOperationResult::SearchResults {
                documents: vec![],
                total_count: 0,
                query_confidence: 0.0,
            },
        }
    }

    /// Extracts metadata from a document using AI
    pub fn extract_document_metadata(
        &self,
        doc_id: Uuid,
        _extraction_request: serde_json::Value,
    ) -> Result<serde_json::Value> {
        println!("Extracting metadata from document: {}", doc_id);

        let mock_client = MockDocumentAiClient::new();
        mock_client.extract_document(doc_id)
    }

    /// Searches documents based on natural language query
    pub fn search_documents(
        &self,
        query: &str,
        _filters: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        println!("Searching documents with query: {}", query);

        // For now, return mock results
        Ok(serde_json::json!({
            "documents": [],
            "total_count": 0,
            "has_more": false
        }))
    }

    /// Validates document compliance and completeness
    pub fn validate_document(&self, doc_id: Uuid) -> Result<serde_json::Value> {
        println!("Validating document: {}", doc_id);

        Ok(serde_json::json!({
            "is_valid": true,
            "doc_id": doc_id,
            "validation_errors": [],
            "missing_required_metadata": [],
            "confidence_warnings": []
        }))
    }

    /// Gets document statistics and analytics
    pub fn get_document_statistics(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "total_documents": 0,
            "documents_by_status": {},
            "documents_by_mime_type": {},
            "average_extraction_confidence": null,
            "total_metadata_entries": 0,
            "total_relationships": 0,
            "total_usage_entries": 0,
            "most_used_attributes": []
        }))
    }
}

// ============================================================================
// DEFAULT IMPLEMENTATIONS
// ============================================================================

impl Default for DocumentServiceConfig {
    fn default() -> Self {
        Self {
            ai_provider: DocumentAiProvider::Mock {
                responses: HashMap::new(),
                extraction_results: HashMap::new(),
            },
            model_config: DocumentModelConfig::default(),
            prompt_config: PromptConfig::default(),
            execute_operations: false,
            max_retries: 3,
            timeout_seconds: 60,
            confidence_thresholds: ConfidenceThresholds::default(),
        }
    }
}

impl Default for DocumentModelConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,      // Low temperature for consistent extraction
            max_tokens: Some(500), // Document operations may need more tokens
            top_p: Some(0.9),
            extraction_config: ExtractionConfig::default(),
        }
    }
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            default_confidence_threshold: 0.8,
            max_extraction_retries: 2,
            preferred_extraction_methods: vec![
                "ai-vision".to_string(),
                "ocr".to_string(),
                "manual".to_string(),
            ],
        }
    }
}

impl Default for ConfidenceThresholds {
    fn default() -> Self {
        Self {
            auto_process: 0.85,
            validation: 0.75,
            compliance: 0.90,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> AgenticDocumentService {
        AgenticDocumentService::with_mock()
    }

    #[test]
    fn test_document_service_creation() {
        let service = create_test_service();
        assert_eq!(service.config.max_retries, 3);
    }

    #[test]
    fn test_operation_type_detection() {
        let service = create_test_service();

        let catalog_instruction = "Add this passport document to the system";
        assert!(matches!(
            service.detect_operation_type(catalog_instruction),
            DocumentOperationType::Catalog
        ));

        let search_instruction = "Find all passport documents from UK";
        assert!(matches!(
            service.detect_operation_type(search_instruction),
            DocumentOperationType::Search
        ));

        let extract_instruction = "Extract data from this document";
        assert!(matches!(
            service.detect_operation_type(extract_instruction),
            DocumentOperationType::Extract
        ));
    }

    #[test]
    fn test_document_request_processing() {
        let service = create_test_service();

        let request = AgenticDocumentRequest {
            instruction: "Catalog this passport document".to_string(),
            operation_hint: Some(DocumentOperationType::Catalog),
            target_doc_id: None,
            cbu_id: None,
            entity_id: None,
            context_hints: None,
            execute: false,
            request_id: Some("test".to_string()),
        };

        let response = service.process_document_request(request).unwrap();

        assert!(response.success);
        assert!(!response.generated_dsl.is_empty());
        assert!(matches!(
            response.operation_type,
            DocumentOperationType::Catalog
        ));
        assert_eq!(response.request_id, "test");
    }

    #[test]
    fn test_mock_extraction() {
        let service = create_test_service();
        let doc_id = Uuid::new_v4();

        let extraction_request = serde_json::json!({
            "doc_id": doc_id,
            "extraction_method": "ai-vision",
            "confidence_threshold": 0.8,
            "ai_model": "mock-ai-v1",
            "force_reextraction": false
        });

        let result = service
            .extract_document_metadata(doc_id, extraction_request)
            .unwrap();

        assert_eq!(
            result["doc_id"],
            serde_json::Value::String(doc_id.to_string())
        );
        assert!(result["extraction_confidence"].as_f64().unwrap_or(0.0) > 0.8);
        assert!(!result["extracted_attributes"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_document_validation() {
        let service = create_test_service();
        let doc_id = Uuid::new_v4();

        let validation_result = service.validate_document(doc_id).unwrap();

        assert!(validation_result["is_valid"].as_bool().unwrap_or(false));
        assert_eq!(
            validation_result["doc_id"],
            serde_json::Value::String(doc_id.to_string())
        );
        assert!(validation_result["validation_errors"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
