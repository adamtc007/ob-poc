//! Document Extraction Service
//!
//! This module provides the core trait and implementations for extracting
//! attribute values from uploaded documents (PDFs, images, etc.).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Instant;
use uuid::Uuid;

// Note: ObPocError removed - not available in current error module

/// Result type for extraction operations
pub type ExtractionResult<T> = Result<T, ExtractionError>;

/// Extraction-specific errors
#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("Attribute not found: {0}")]
    AttributeNotFound(Uuid),

    #[error("Extraction method failed: {method}, reason: {reason}")]
    ExtractionFailed { method: String, reason: String },

    #[error("Unsupported document type: {0}")]
    UnsupportedDocumentType(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Metadata about an extraction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMetadata {
    pub method: String,
    pub confidence: f64,
    pub processing_time_ms: u64,
    pub page_number: Option<u32>,
    pub bounding_box: Option<BoundingBox>,
    pub model_version: Option<String>,
}

/// Bounding box for extracted data location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Result of an extraction attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionAttempt {
    pub success: bool,
    pub value: Option<serde_json::Value>,
    pub metadata: ExtractionMetadata,
    pub error_message: Option<String>,
}

/// Core trait for document extraction services
#[async_trait]
pub trait ExtractionService: Send + Sync {
    /// Extract an attribute value from a document
    async fn extract(
        &self,
        doc_id: &Uuid,
        attribute_id: &Uuid,
    ) -> ExtractionResult<serde_json::Value>;

    /// Get the extraction method name
    fn method_name(&self) -> &'static str;

    /// Check if this service can extract from the given document type
    async fn can_extract(&self, doc_id: &Uuid) -> ExtractionResult<bool>;

    /// Batch extract multiple attributes from a document
    async fn batch_extract(
        &self,
        doc_id: &Uuid,
        attribute_ids: &[Uuid],
    ) -> ExtractionResult<Vec<ExtractionAttempt>> {
        let mut results = Vec::new();
        for attr_id in attribute_ids {
            let start = Instant::now();
            let attempt = match self.extract(doc_id, attr_id).await {
                Ok(value) => ExtractionAttempt {
                    success: true,
                    value: Some(value),
                    metadata: ExtractionMetadata {
                        method: self.method_name().to_string(),
                        confidence: 0.95,
                        processing_time_ms: start.elapsed().as_millis() as u64,
                        page_number: None,
                        bounding_box: None,
                        model_version: None,
                    },
                    error_message: None,
                },
                Err(e) => ExtractionAttempt {
                    success: false,
                    value: None,
                    metadata: ExtractionMetadata {
                        method: self.method_name().to_string(),
                        confidence: 0.0,
                        processing_time_ms: start.elapsed().as_millis() as u64,
                        page_number: None,
                        bounding_box: None,
                        model_version: None,
                    },
                    error_message: Some(e.to_string()),
                },
            };
            results.push(attempt);
        }
        Ok(results)
    }
}

/// Mock extraction service for testing
pub struct MockExtractionService {
    mock_data: std::collections::HashMap<(Uuid, Uuid), serde_json::Value>,
}

impl MockExtractionService {
    pub fn new() -> Self {
        Self {
            mock_data: std::collections::HashMap::new(),
        }
    }

    pub fn with_mock_data(mut self, doc_id: Uuid, attr_id: Uuid, value: serde_json::Value) -> Self {
        self.mock_data.insert((doc_id, attr_id), value);
        self
    }
}

impl Default for MockExtractionService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExtractionService for MockExtractionService {
    async fn extract(
        &self,
        doc_id: &Uuid,
        attribute_id: &Uuid,
    ) -> ExtractionResult<serde_json::Value> {
        self.mock_data
            .get(&(*doc_id, *attribute_id))
            .cloned()
            .ok_or_else(|| ExtractionError::ExtractionFailed {
                method: "mock".to_string(),
                reason: "No mock data configured".to_string(),
            })
    }

    fn method_name(&self) -> &'static str {
        "mock"
    }

    async fn can_extract(&self, _doc_id: &Uuid) -> ExtractionResult<bool> {
        Ok(true)
    }
}

/// OCR-based extraction service
pub struct OcrExtractionService {
    pool: PgPool,
}

impl OcrExtractionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get document content from database
    async fn get_document(&self, doc_id: &Uuid) -> ExtractionResult<DocumentContent> {
        let doc = sqlx::query_as::<_, DocumentContent>(
            r#"
            SELECT doc_id, mime_type, extracted_data
            FROM "ob-poc".document_catalog
            WHERE doc_id = $1
            "#,
        )
        .bind(doc_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ExtractionError::DocumentNotFound(*doc_id))?;

        Ok(doc)
    }

    /// Get attribute definition
    async fn get_attribute_definition(
        &self,
        attr_id: &Uuid,
    ) -> ExtractionResult<AttributeDefinition> {
        let attr = sqlx::query_as::<_, AttributeDefinition>(
            r#"
            SELECT attribute_id, name, mask as data_type
            FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
        )
        .bind(attr_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ExtractionError::AttributeNotFound(*attr_id))?;

        Ok(attr)
    }

    /// Extract date from document content
    fn extract_date(&self, content: &serde_json::Value) -> ExtractionResult<serde_json::Value> {
        // Placeholder: In real implementation, use regex or NLP to find dates
        if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
            // Simple pattern matching for ISO dates
            if let Some(date_match) = text
                .split_whitespace()
                .find(|s| s.contains('-') && s.len() == 10)
            {
                return Ok(serde_json::json!(date_match));
            }
        }
        Ok(serde_json::Value::Null)
    }

    /// Extract text from document content
    fn extract_text(
        &self,
        content: &serde_json::Value,
        field_name: &str,
    ) -> ExtractionResult<serde_json::Value> {
        // Placeholder: Extract specific text field
        if let Some(value) = content.get(field_name) {
            return Ok(value.clone());
        }
        Ok(serde_json::Value::Null)
    }

    /// Extract number from document content
    fn extract_number(&self, content: &serde_json::Value) -> ExtractionResult<serde_json::Value> {
        // Placeholder: Extract numeric values
        if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
            if let Some(number) = text.split_whitespace().find(|s| s.parse::<f64>().is_ok()) {
                if let Ok(num) = number.parse::<f64>() {
                    return Ok(serde_json::json!(num));
                }
            }
        }
        Ok(serde_json::Value::Null)
    }
}

#[async_trait]
impl ExtractionService for OcrExtractionService {
    async fn extract(
        &self,
        doc_id: &Uuid,
        attribute_id: &Uuid,
    ) -> ExtractionResult<serde_json::Value> {
        let doc = self.get_document(doc_id).await?;
        let attr_def = self.get_attribute_definition(attribute_id).await?;

        let content = doc.extracted_data.unwrap_or_else(|| serde_json::json!({}));

        // Apply extraction based on attribute type
        let result = match attr_def.data_type.as_str() {
            "date" => self.extract_date(&content)?,
            "string" | "text" => self.extract_text(&content, &attr_def.name)?,
            "number" | "integer" | "float" => self.extract_number(&content)?,
            _ => serde_json::Value::Null,
        };

        Ok(result)
    }

    fn method_name(&self) -> &'static str {
        "ocr"
    }

    async fn can_extract(&self, doc_id: &Uuid) -> ExtractionResult<bool> {
        let doc = self.get_document(doc_id).await?;
        // Can extract from PDFs and images
        Ok(doc
            .mime_type
            .map(|m| m.starts_with("image/") || m == "application/pdf")
            .unwrap_or(false))
    }
}

/// Document content from database
#[derive(Debug, sqlx::FromRow)]
struct DocumentContent {
    doc_id: Uuid,
    mime_type: Option<String>,
    extracted_data: Option<serde_json::Value>,
}

/// Attribute definition from dictionary
#[derive(Debug, sqlx::FromRow)]
struct AttributeDefinition {
    attribute_id: Uuid,
    name: String,
    data_type: String,
}

