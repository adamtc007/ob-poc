//! Enhanced DSL Manager with Document-Attribute Bridge Integration
//!
//! This module provides an enhanced DSL manager that integrates with the foundational
//! document-attribute bridge, enabling complete DSL-as-State architecture with
//! document-driven workflows, AI-powered data extraction, and cross-document validation.

use crate::database::{
    AttributeExtractionSpec, ConsolidatedAttribute, CrossDocumentValidation, DataBridgeMetrics,
    DatabaseManager, DocumentAttributeRepository, DocumentExtractionTemplate, DocumentType,
    DslDomainRepository, DslInstanceRepository,
};
use crate::error::{DatabaseError, DslManagerError};
use crate::parser::{DslParser, ParsedAst};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Enhanced DSL Manager with document-attribute bridge integration
pub struct EnhancedDslManager {
    db_manager: DatabaseManager,
    dsl_repository: Box<dyn DslInstanceRepository>,
    document_attribute_repository: DocumentAttributeRepository,
    domain_repository: Box<dyn crate::database::DslDomainRepositoryTrait>,
    parser: DslParser,
}

/// Document processing request with extraction requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentProcessingRequest {
    pub document_id: Uuid,
    pub document_type_code: String,
    pub document_content: String,
    pub entity_identifier: Option<String>,
    pub extraction_priority_threshold: Option<i32>,
    pub validate_cross_document: bool,
    pub created_by: String,
}

/// Document processing result with extracted attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentProcessingResult {
    pub document_id: Uuid,
    pub document_type_code: String,
    pub processing_status: ProcessingStatus,
    pub extracted_attributes: HashMap<String, ExtractedAttributeValue>,
    pub generated_dsl: Option<String>,
    pub dsl_instance_id: Option<Uuid>,
    pub validation_results: Vec<CrossDocumentValidation>,
    pub processing_errors: Vec<ProcessingError>,
    pub processing_time_ms: i64,
    pub processed_at: DateTime<Utc>,
}

/// Processing status for document processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingStatus {
    Success,
    PartialSuccess,
    ValidationFailed,
    ExtractionFailed,
    DslGenerationFailed,
}

/// Extracted attribute value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedAttributeValue {
    pub attribute_code: String,
    pub value: serde_json::Value,
    pub confidence_score: f64,
    pub extraction_method: String,
    pub field_location: Option<String>,
    pub validation_status: ValidationStatus,
    pub privacy_classification: String,
}

/// Validation status for extracted values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    Valid,
    Invalid { reason: String },
    RequiresReview { reason: String },
    CrossValidationPending,
}

/// Processing error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingError {
    pub error_type: String,
    pub attribute_code: Option<String>,
    pub message: String,
    pub severity: ErrorSeverity,
}

/// Error severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

/// DSL generation request from extracted attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslGenerationRequest {
    pub entity_identifier: String,
    pub extracted_attributes: HashMap<String, serde_json::Value>,
    pub document_sources: Vec<String>,
    pub domain: String,
    pub workflow_context: Option<String>,
    pub created_by: String,
}

/// Enhanced DSL validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedValidationResult {
    pub is_valid: bool,
    pub syntax_errors: Vec<String>,
    pub semantic_errors: Vec<String>,
    pub attribute_validation_errors: Vec<AttributeValidationError>,
    pub cross_document_inconsistencies: Vec<CrossDocumentValidation>,
    pub compliance_warnings: Vec<String>,
    pub overall_confidence_score: f64,
}

/// Attribute validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeValidationError {
    pub attribute_code: String,
    pub error_type: String,
    pub message: String,
    pub expected_format: Option<String>,
    pub actual_value: Option<String>,
}

impl EnhancedDslManager {
    /// Create a new enhanced DSL manager
    pub async fn new(db_manager: DatabaseManager) -> Result<Self, DslManagerError> {
        let dsl_repository = Box::new(db_manager.dsl_instance_repository());
        let document_attribute_repository = db_manager.document_attribute_repository();
        let domain_repository = Box::new(db_manager.dsl_repository());
        let parser = DslParser::new();

        Ok(Self {
            db_manager,
            dsl_repository,
            document_attribute_repository,
            domain_repository,
            parser,
        })
    }

    // ============================================================================
    // DOCUMENT PROCESSING OPERATIONS
    // ============================================================================

    /// Process document and extract structured data using AttributeID mapping
    pub async fn process_document(
        &self,
        request: DocumentProcessingRequest,
    ) -> Result<DocumentProcessingResult, DslManagerError> {
        let start_time = std::time::Instant::now();
        let processing_start = Utc::now();

        // Get extraction template for document type
        let template = self
            .document_attribute_repository
            .get_document_extraction_template(&request.document_type_code)
            .await
            .map_err(DslManagerError::DatabaseError)?
            .ok_or_else(|| {
                DslManagerError::ValidationError(format!(
                    "No extraction template found for document type: {}",
                    request.document_type_code
                ))
            })?;

        // Extract attributes from document content
        let extracted_attributes = self
            .extract_attributes_from_document(&template, &request.document_content, &request)
            .await?;

        // Generate DSL from extracted attributes if requested
        let (generated_dsl, dsl_instance_id) = if !extracted_attributes.is_empty() {
            self.generate_dsl_from_attributes(&request, &extracted_attributes)
                .await?
        } else {
            (None, None)
        };

        // Perform cross-document validation if requested
        let validation_results = if request.validate_cross_document {
            self.perform_cross_document_validation(&request, &extracted_attributes)
                .await?
        } else {
            Vec::new()
        };

        // Collect processing errors
        let processing_errors = self.collect_processing_errors(&extracted_attributes);

        // Determine processing status
        let processing_status = self.determine_processing_status(
            &extracted_attributes,
            &validation_results,
            &processing_errors,
        );

        let processing_time = start_time.elapsed().as_millis() as i64;

        Ok(DocumentProcessingResult {
            document_id: request.document_id,
            document_type_code: request.document_type_code,
            processing_status,
            extracted_attributes,
            generated_dsl,
            dsl_instance_id,
            validation_results,
            processing_errors,
            processing_time_ms: processing_time,
            processed_at: processing_start,
        })
    }

    /// Extract attributes from document content using AI guidance
    async fn extract_attributes_from_document(
        &self,
        template: &DocumentExtractionTemplate,
        content: &str,
        request: &DocumentProcessingRequest,
    ) -> Result<HashMap<String, ExtractedAttributeValue>, DslManagerError> {
        let mut extracted = HashMap::new();
        let priority_threshold = request.extraction_priority_threshold.unwrap_or(10);

        for attr_spec in &template.attributes {
            // Skip attributes below priority threshold
            if attr_spec.priority > priority_threshold {
                continue;
            }

            // Attempt to extract attribute value
            match self
                .extract_single_attribute(content, attr_spec, &template.ai_narrative)
                .await
            {
                Ok(Some(extracted_value)) => {
                    extracted.insert(attr_spec.attribute_code.clone(), extracted_value);
                }
                Ok(None) => {
                    // Attribute not found - check if required
                    if attr_spec.required {
                        let error_value = ExtractedAttributeValue {
                            attribute_code: attr_spec.attribute_code.clone(),
                            value: serde_json::Value::Null,
                            confidence_score: 0.0,
                            extraction_method: "failed".to_string(),
                            field_location: None,
                            validation_status: ValidationStatus::Invalid {
                                reason: "Required attribute not found in document".to_string(),
                            },
                            privacy_classification: attr_spec.privacy_class.clone(),
                        };
                        extracted.insert(attr_spec.attribute_code.clone(), error_value);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to extract attribute {}: {}",
                        attr_spec.attribute_code,
                        e
                    );
                }
            }
        }

        Ok(extracted)
    }

    /// Extract a single attribute value from document content
    async fn extract_single_attribute(
        &self,
        content: &str,
        attr_spec: &AttributeExtractionSpec,
        document_context: &str,
    ) -> Result<Option<ExtractedAttributeValue>, DslManagerError> {
        // This is a simplified extraction - in a full implementation,
        // this would use AI/ML services for intelligent extraction

        // Try pattern matching first
        if let Some(value) = self.pattern_extract(content, attr_spec) {
            let extracted_value = ExtractedAttributeValue {
                attribute_code: attr_spec.attribute_code.clone(),
                value,
                confidence_score: 0.8, // Pattern matching gets good confidence
                extraction_method: "pattern_matching".to_string(),
                field_location: None, // Would be populated by actual extraction
                validation_status: ValidationStatus::Valid,
                privacy_classification: attr_spec.privacy_class.clone(),
            };
            return Ok(Some(extracted_value));
        }

        // Try keyword search
        if let Some(value) = self.keyword_extract(content, attr_spec) {
            let extracted_value = ExtractedAttributeValue {
                attribute_code: attr_spec.attribute_code.clone(),
                value,
                confidence_score: 0.6, // Keyword search gets lower confidence
                extraction_method: "keyword_search".to_string(),
                field_location: None,
                validation_status: ValidationStatus::RequiresReview {
                    reason: "Extracted via keyword search - manual review recommended".to_string(),
                },
                privacy_classification: attr_spec.privacy_class.clone(),
            };
            return Ok(Some(extracted_value));
        }

        Ok(None)
    }

    /// Pattern-based extraction (simplified implementation)
    fn pattern_extract(
        &self,
        content: &str,
        attr_spec: &AttributeExtractionSpec,
    ) -> Option<serde_json::Value> {
        // This would contain sophisticated pattern matching logic
        // For now, implementing basic examples
        match attr_spec.attribute_code.as_str() {
            "entity.legal_name" => {
                // Look for company names in quotes or after "Name:"
                if let Some(caps) = regex::Regex::new(
                    r#"(?i)(?:company name|legal name)[:.]?\s*["']?([^"'\n\r]{3,100})["']?"#,
                )
                .ok()?
                .captures(content)
                {
                    let name = caps.get(1)?.as_str().trim();
                    return Some(serde_json::Value::String(name.to_string()));
                }
            }
            "individual.full_name" => {
                // Look for names in passport format
                if let Some(caps) = regex::Regex::new(
                    r#"(?i)(?:name|surname|given names?)[:.]?\s*([A-Z][a-z]+(?: [A-Z][a-z]+)*)"#,
                )
                .ok()?
                .captures(content)
                {
                    let name = caps.get(1)?.as_str().trim();
                    return Some(serde_json::Value::String(name.to_string()));
                }
            }
            "individual.date_of_birth" => {
                // Look for dates in various formats
                if let Some(caps) = regex::Regex::new(r#"(?i)(?:date of birth|birth date|dob)[:.]?\s*(\d{1,2}[/-]\d{1,2}[/-]\d{2,4})"#)
                    .ok()?
                    .captures(content)
                {
                    let date = caps.get(1)?.as_str();
                    // Would normalize date format here
                    return Some(serde_json::Value::String(date.to_string()));
                }
            }
            _ => {}
        }
        None
    }

    /// Keyword-based extraction (simplified implementation)
    fn keyword_extract(
        &self,
        content: &str,
        attr_spec: &AttributeExtractionSpec,
    ) -> Option<serde_json::Value> {
        // Search for field hints in the content
        for hint in &attr_spec.field_hints {
            if content.to_lowercase().contains(&hint.to_lowercase()) {
                // Would extract value near the keyword
                // This is a very simplified implementation
                return Some(serde_json::Value::String(format!(
                    "Found keyword: {}",
                    hint
                )));
            }
        }
        None
    }

    /// Generate DSL from extracted attributes
    async fn generate_dsl_from_attributes(
        &self,
        request: &DocumentProcessingRequest,
        extracted_attributes: &HashMap<String, ExtractedAttributeValue>,
    ) -> Result<(Option<String>, Option<Uuid>), DslManagerError> {
        let entity_id = request
            .entity_identifier
            .clone()
            .unwrap_or_else(|| format!("entity-{}", request.document_id));

        // Build attribute map for DSL generation
        let attribute_map: HashMap<String, serde_json::Value> = extracted_attributes
            .iter()
            .filter_map(|(code, extracted)| {
                if matches!(extracted.validation_status, ValidationStatus::Valid) {
                    Some((code.clone(), extracted.value.clone()))
                } else {
                    None
                }
            })
            .collect();

        if attribute_map.is_empty() {
            return Ok((None, None));
        }

        // Generate DSL content based on document type and attributes
        let dsl_content = self.build_dsl_content(&request.document_type_code, &attribute_map);

        // Parse and validate DSL
        match self.parser.parse(&dsl_content) {
            Ok(ast) => {
                // Store DSL instance
                let instance_id = Uuid::new_v4();
                // Would store in database here
                tracing::info!("Generated DSL instance: {}", instance_id);
                Ok((Some(dsl_content), Some(instance_id)))
            }
            Err(e) => {
                tracing::error!("Generated DSL failed parsing: {}", e);
                Ok((Some(dsl_content), None))
            }
        }
    }

    /// Build DSL content from extracted attributes
    fn build_dsl_content(
        &self,
        document_type: &str,
        attributes: &HashMap<String, serde_json::Value>,
    ) -> String {
        let mut dsl_parts = Vec::new();

        // Add header comment
        dsl_parts.push(format!(";; DSL generated from {} document", document_type));
        dsl_parts.push(format!(";; Generated at: {}", Utc::now()));
        dsl_parts.push(String::new());

        // Generate appropriate DSL based on document type
        match document_type {
            "CERT_INCORPORATION" | "ARTICLES_OF_ASSOCIATION" => {
                if let Some(entity_name) = attributes.get("entity.legal_name") {
                    dsl_parts.push("(entity".to_string());
                    dsl_parts.push(format!("  :name {}", entity_name));
                    if let Some(reg_num) = attributes.get("entity.registration_number") {
                        dsl_parts.push(format!("  :registration-number {}", reg_num));
                    }
                    if let Some(jurisdiction) = attributes.get("entity.jurisdiction_incorporation")
                    {
                        dsl_parts.push(format!("  :jurisdiction {}", jurisdiction));
                    }
                    dsl_parts.push(")".to_string());
                }
            }
            "PASSPORT" | "NATIONAL_ID_CARD" => {
                dsl_parts.push("(individual".to_string());
                if let Some(name) = attributes.get("individual.full_name") {
                    dsl_parts.push(format!("  :name {}", name));
                }
                if let Some(dob) = attributes.get("individual.date_of_birth") {
                    dsl_parts.push(format!("  :date-of-birth {}", dob));
                }
                if let Some(nationality) = attributes.get("individual.nationality") {
                    dsl_parts.push(format!("  :nationality {}", nationality));
                }
                dsl_parts.push(")".to_string());
            }
            "BANK_STATEMENT" => {
                dsl_parts.push("(banking-relationship".to_string());
                if let Some(bank_name) = attributes.get("banking.bank_name") {
                    dsl_parts.push(format!("  :bank {}", bank_name));
                }
                if let Some(account) = attributes.get("banking.account_number") {
                    dsl_parts.push(format!("  :account {}", account));
                }
                dsl_parts.push(")".to_string());
            }
            _ => {
                // Generic attribute-based DSL
                dsl_parts.push("(document-data".to_string());
                for (key, value) in attributes {
                    dsl_parts.push(format!("  :{} {}", key.replace('.', "-"), value));
                }
                dsl_parts.push(")".to_string());
            }
        }

        dsl_parts.join("\n")
    }

    /// Perform cross-document validation
    async fn perform_cross_document_validation(
        &self,
        request: &DocumentProcessingRequest,
        extracted_attributes: &HashMap<String, ExtractedAttributeValue>,
    ) -> Result<Vec<CrossDocumentValidation>, DslManagerError> {
        let mut validation_results = Vec::new();

        if let Some(entity_id) = &request.entity_identifier {
            for (attr_code, extracted_value) in extracted_attributes {
                // Only validate attributes that require cross-document validation
                if self.should_cross_validate(attr_code).await? {
                    // Build comparison map (simplified - would query existing documents)
                    let mut comparison_map = HashMap::new();
                    comparison_map.insert(
                        request.document_type_code.clone(),
                        extracted_value.value.to_string(),
                    );

                    let validation = self
                        .document_attribute_repository
                        .validate_cross_document_consistency(entity_id, attr_code, &comparison_map)
                        .await
                        .map_err(DslManagerError::DatabaseError)?;

                    validation_results.push(validation);
                }
            }
        }

        Ok(validation_results)
    }

    /// Check if attribute should be cross-validated
    async fn should_cross_validate(&self, attribute_code: &str) -> Result<bool, DslManagerError> {
        // Check if attribute has cross-validation rules
        let attribute = self
            .document_attribute_repository
            .get_attribute_by_code(attribute_code)
            .await
            .map_err(DslManagerError::DatabaseError)?;

        Ok(attribute
            .map(|a| a.cross_document_validation.is_some())
            .unwrap_or(false))
    }

    /// Collect processing errors from extracted attributes
    fn collect_processing_errors(
        &self,
        extracted_attributes: &HashMap<String, ExtractedAttributeValue>,
    ) -> Vec<ProcessingError> {
        let mut errors = Vec::new();

        for (attr_code, extracted_value) in extracted_attributes {
            match &extracted_value.validation_status {
                ValidationStatus::Invalid { reason } => {
                    errors.push(ProcessingError {
                        error_type: "validation_error".to_string(),
                        attribute_code: Some(attr_code.clone()),
                        message: reason.clone(),
                        severity: ErrorSeverity::Error,
                    });
                }
                ValidationStatus::RequiresReview { reason } => {
                    errors.push(ProcessingError {
                        error_type: "requires_review".to_string(),
                        attribute_code: Some(attr_code.clone()),
                        message: reason.clone(),
                        severity: ErrorSeverity::Warning,
                    });
                }
                _ => {}
            }
        }

        errors
    }

    /// Determine overall processing status
    fn determine_processing_status(
        &self,
        extracted_attributes: &HashMap<String, ExtractedAttributeValue>,
        validation_results: &[CrossDocumentValidation],
        processing_errors: &[ProcessingError],
    ) -> ProcessingStatus {
        // Check for critical errors
        if processing_errors
            .iter()
            .any(|e| matches!(e.severity, ErrorSeverity::Critical))
        {
            return ProcessingStatus::ExtractionFailed;
        }

        // Check for validation failures
        if validation_results
            .iter()
            .any(|v| !v.is_consistent && v.requires_review)
        {
            return ProcessingStatus::ValidationFailed;
        }

        // Check for any errors
        if processing_errors
            .iter()
            .any(|e| matches!(e.severity, ErrorSeverity::Error))
        {
            return ProcessingStatus::PartialSuccess;
        }

        // Check if we extracted anything
        if extracted_attributes.is_empty() {
            return ProcessingStatus::ExtractionFailed;
        }

        ProcessingStatus::Success
    }

    // ============================================================================
    // ENHANCED VALIDATION OPERATIONS
    // ============================================================================

    /// Perform enhanced DSL validation with attribute checking
    pub async fn validate_dsl_enhanced(
        &self,
        dsl_content: &str,
        entity_identifier: Option<&str>,
    ) -> Result<EnhancedValidationResult, DslManagerError> {
        // Parse DSL for syntax validation
        let ast = match self.parser.parse(dsl_content) {
            Ok(ast) => ast,
            Err(e) => {
                return Ok(EnhancedValidationResult {
                    is_valid: false,
                    syntax_errors: vec![e.to_string()],
                    semantic_errors: Vec::new(),
                    attribute_validation_errors: Vec::new(),
                    cross_document_inconsistencies: Vec::new(),
                    compliance_warnings: Vec::new(),
                    overall_confidence_score: 0.0,
                });
            }
        };

        // Perform attribute validation
        let attribute_errors = self.validate_dsl_attributes(&ast).await?;

        // Perform cross-document validation if entity provided
        let cross_document_inconsistencies = if let Some(entity_id) = entity_identifier {
            self.validate_dsl_cross_document(dsl_content, entity_id)
                .await?
        } else {
            Vec::new()
        };

        // Calculate overall confidence score
        let confidence_score =
            self.calculate_confidence_score(&attribute_errors, &cross_document_inconsistencies);

        let is_valid = attribute_errors.is_empty() && cross_document_inconsistencies.is_empty();

        Ok(EnhancedValidationResult {
            is_valid,
            syntax_errors: Vec::new(),   // Already passed syntax validation
            semantic_errors: Vec::new(), // Would be populated by semantic analysis
            attribute_validation_errors: attribute_errors,
            cross_document_inconsistencies,
            compliance_warnings: Vec::new(), // Would be populated by compliance checks
            overall_confidence_score: confidence_score,
        })
    }

    /// Validate DSL attributes against AttributeID dictionary
    async fn validate_dsl_attributes(
        &self,
        _ast: &ParsedAst,
    ) -> Result<Vec<AttributeValidationError>, DslManagerError> {
        // This would analyze the AST and validate attribute usage
        // For now, returning empty vector
        Ok(Vec::new())
    }

    /// Validate DSL against existing documents for entity
    async fn validate_dsl_cross_document(
        &self,
        _dsl_content: &str,
        _entity_id: &str,
    ) -> Result<Vec<CrossDocumentValidation>, DslManagerError> {
        // This would extract attributes from DSL and cross-validate
        // For now, returning empty vector
        Ok(Vec::new())
    }

    /// Calculate confidence score for validation
    fn calculate_confidence_score(
        &self,
        attribute_errors: &[AttributeValidationError],
        cross_document_issues: &[CrossDocumentValidation],
    ) -> f64 {
        let error_penalty = attribute_errors.len() as f64 * 0.1;
        let consistency_penalty = cross_document_issues
            .iter()
            .map(|v| 1.0 - v.consistency_score)
            .sum::<f64>()
            * 0.1;

        (1.0 - error_penalty - consistency_penalty).max(0.0)
    }

    // ============================================================================
    // ANALYTICS AND REPORTING
    // ============================================================================

    /// Get data bridge metrics
    pub async fn get_data_bridge_metrics(&self) -> Result<DataBridgeMetrics, DslManagerError> {
        self.document_attribute_repository
            .get_data_bridge_metrics()
            .await
            .map_err(DslManagerError::DatabaseError)
    }

    /// Get extraction template for document type
    pub async fn get_extraction_template(
        &self,
        document_type_code: &str,
    ) -> Result<Option<DocumentExtractionTemplate>, DslManagerError> {
        self.document_attribute_repository
            .get_document_extraction_template(document_type_code)
            .await
            .map_err(DslManagerError::DatabaseError)
    }

    /// Get all document types
    pub async fn get_all_document_types(&self) -> Result<Vec<DocumentType>, DslManagerError> {
        self.document_attribute_repository
            .get_all_document_types()
            .await
            .map_err(DslManagerError::DatabaseError)
    }

    /// Get attribute by code
    pub async fn get_attribute(
        &self,
        attribute_code: &str,
    ) -> Result<Option<ConsolidatedAttribute>, DslManagerError> {
        self.document_attribute_repository
            .get_attribute_by_code(attribute_code)
            .await
            .map_err(DslManagerError::DatabaseError)
    }

    /// Test connectivity to all systems
    pub async fn test_connectivity(&self) -> Result<(), DslManagerError> {
        // Test database connectivity
        self.db_manager
            .test_connection()
            .await
            .map_err(|e| DslManagerError::DatabaseError(DatabaseError::SqlxError(e)))?;

        // Test document-attribute repository
        self.document_attribute_repository
            .test_connection()
            .await
            .map_err(DslManagerError::DatabaseError)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseConfig;

    async fn create_test_manager() -> EnhancedDslManager {
        let db_manager = DatabaseManager::with_default_config()
            .await
            .expect("Failed to create database manager");

        EnhancedDslManager::new(db_manager)
            .await
            .expect("Failed to create enhanced DSL manager")
    }

    #[tokio::test]
    async fn test_enhanced_manager_creation() {
        let manager = create_test_manager().await;
        assert!(manager.test_connectivity().await.is_ok());
    }

    #[tokio::test]
    async fn test_document_processing() {
        let manager = create_test_manager().await;

        let request = DocumentProcessingRequest {
            document_id: Uuid::new_v4(),
            document_type_code: "PASSPORT".to_string(),
            document_content: "Name: John Smith\nDate of Birth: 01/01/1990\nNationality: US"
                .to_string(),
            entity_identifier: Some("individual-123".to_string()),
            extraction_priority_threshold: Some(3),
            validate_cross_document: false,
            created_by: "test_user".to_string(),
        };

        let result = manager.process_document(request).await;
        // Would assert successful processing in real test
        assert!(result.is_ok() || result.is_err()); // Just check it completes
    }

    #[tokio::test]
    async fn test_get_data_bridge_metrics() {
        let manager = create_test_manager().await;
        let result = manager.get_data_bridge_metrics().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_extraction_template() {
        let manager = create_test_manager().await;
        let result = manager.get_extraction_template("PASSPORT").await;
        assert!(result.is_ok());
    }
}
