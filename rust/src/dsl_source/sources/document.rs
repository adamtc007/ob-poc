//! Document Extraction Source - Generates DSL for extracting attributes from documents
//!
//! This module generates DSL.CRUD statements for document attribute extraction.
//! It reads the document type's attribute index from the dictionary and generates
//! appropriate extraction DSL that will be executed by the Forth engine.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{AttributeSource, SourceContext, SourceError};

/// Document extraction source - generates DSL for attribute extraction
pub struct DocumentSource {
    pool: PgPool,
}

/// Extraction method for document attributes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtractionMethod {
    OCR,
    MRZ,
    Barcode,
    QrCode,
    FormField,
    Table,
    Checkbox,
    Signature,
    Photo,
    NLP,
    AI,
    Manual,
}

/// Extracted value from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedValue {
    pub attribute_id: Uuid,
    pub value: serde_json::Value,
    pub confidence: f64,
    pub extraction_method: ExtractionMethod,
    pub metadata: Option<serde_json::Value>,
}

/// Document type attribute mapping from dictionary
#[derive(Debug, Clone)]
pub struct DocumentAttributeMapping {
    pub attribute_uuid: Uuid,
    pub attribute_name: String,
    pub extraction_method: ExtractionMethod,
    pub is_required: bool,
    pub confidence_threshold: f64,
    pub field_name: Option<String>,
}

/// Generated DSL for document extraction
#[derive(Debug, Clone)]
pub struct ExtractionDsl {
    pub document_id: Uuid,
    pub document_type_code: String,
    pub dsl_statements: Vec<String>,
    pub attribute_mappings: Vec<DocumentAttributeMapping>,
}

impl DocumentSource {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get extractable attributes for a document type
    pub async fn get_document_type_attributes(
        &self,
        document_type_id: Uuid,
    ) -> Result<Vec<DocumentAttributeMapping>, SourceError> {
        // Query document_attribute_mappings joined with dictionary
        // DB schema: dam.document_type_id (uuid), dam.attribute_uuid (uuid)
        let mappings = sqlx::query!(
            r#"
            SELECT
                dam.attribute_uuid,
                d.name as attribute_name,
                dam.extraction_method,
                dam.is_required,
                dam.confidence_threshold,
                dam.field_name
            FROM "ob-poc".document_attribute_mappings dam
            JOIN "ob-poc".dictionary d ON dam.attribute_uuid = d.attribute_id
            WHERE dam.document_type_id = $1
            ORDER BY dam.created_at
            "#,
            document_type_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(mappings
            .into_iter()
            .map(|row| DocumentAttributeMapping {
                attribute_uuid: row.attribute_uuid,
                attribute_name: row.attribute_name,
                extraction_method: parse_extraction_method(&row.extraction_method),
                is_required: row.is_required.unwrap_or(false),
                confidence_threshold: row
                    .confidence_threshold
                    .map(|d| d.to_string().parse().unwrap_or(0.8))
                    .unwrap_or(0.8),
                field_name: row.field_name,
            })
            .collect())
    }

    /// Generate DSL for extracting all attributes from a document
    pub async fn generate_extraction_dsl(
        &self,
        doc_id: Uuid,
        cbu_id: Uuid,
    ) -> Result<ExtractionDsl, SourceError> {
        // Get document type from catalog - DB uses doc_id as PK
        let doc = sqlx::query!(
            r#"
            SELECT dc.doc_id, dc.document_type_id, dt.type_code
            FROM "ob-poc".document_catalog dc
            JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
            WHERE dc.doc_id = $1
            "#,
            doc_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SourceError::DocumentNotFound(doc_id))?;

        let document_type_id = doc.document_type_id.ok_or_else(|| {
            SourceError::ExtractionFailed("Document has no document_type_id".to_string())
        })?;

        // Get attribute mappings for this document type
        let mappings = self.get_document_type_attributes(document_type_id).await?;

        if mappings.is_empty() {
            return Err(SourceError::ExtractionFailed(format!(
                "No extractable attributes configured for document type: {}",
                doc.type_code
            )));
        }

        // Generate DSL statements for each attribute
        let mut dsl_statements = Vec::new();

        for mapping in &mappings {
            let dsl = format!(
                r#"(document.extract :doc-id "{}" :attr-id "{}" :cbu-id "{}" :method "{}" :required {})"#,
                doc_id,
                mapping.attribute_uuid,
                cbu_id,
                extraction_method_to_str(&mapping.extraction_method),
                if mapping.is_required { "true" } else { "false" }
            );
            dsl_statements.push(dsl);
        }

        Ok(ExtractionDsl {
            document_id: doc_id,
            document_type_code: doc.type_code,
            dsl_statements,
            attribute_mappings: mappings,
        })
    }

    /// Generate DSL for a single attribute extraction
    pub fn generate_single_extraction_dsl(
        &self,
        doc_id: Uuid,
        attribute_id: Uuid,
        cbu_id: Uuid,
        method: &ExtractionMethod,
    ) -> String {
        format!(
            r#"(document.extract :doc-id "{}" :attr-id "{}" :cbu-id "{}" :method "{}")"#,
            doc_id,
            attribute_id,
            cbu_id,
            extraction_method_to_str(method)
        )
    }

    /// Generate a complete DSL sheet for document extraction
    pub async fn generate_extraction_sheet(
        &self,
        doc_id: Uuid,
        cbu_id: Uuid,
    ) -> Result<String, SourceError> {
        let extraction = self.generate_extraction_dsl(doc_id, cbu_id).await?;

        let mut sheet = format!(
            r#"; Document Extraction Sheet
; Document: {} (type: {})
; Generated: {}

"#,
            doc_id,
            extraction.document_type_code,
            chrono::Utc::now().to_rfc3339()
        );

        for stmt in &extraction.dsl_statements {
            sheet.push_str(stmt);
            sheet.push('\n');
        }

        Ok(sheet)
    }
}

#[async_trait]
impl AttributeSource for DocumentSource {
    fn source_type(&self) -> &str {
        "document"
    }

    async fn fetch_value(
        &self,
        attribute_id: Uuid,
        context: &SourceContext,
    ) -> Result<Option<ExtractedValue>, SourceError> {
        let doc_id = context.document_id.ok_or_else(|| {
            SourceError::ExtractionFailed("No document_id in context".to_string())
        })?;

        // Check if this attribute was already extracted from this document
        // DB schema: doc_id, attribute_id (uuid), value (jsonb), extraction_confidence, extraction_method
        let existing = sqlx::query!(
            r#"
            SELECT
                dm.attribute_id,
                dm.value,
                dm.extraction_confidence,
                dm.extraction_method
            FROM "ob-poc".document_metadata dm
            WHERE dm.doc_id = $1 AND dm.attribute_id = $2
            "#,
            doc_id,
            attribute_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = existing {
            Ok(Some(ExtractedValue {
                attribute_id: row.attribute_id,
                value: row.value,
                confidence: row
                    .extraction_confidence
                    .map(|d| d.to_string().parse().unwrap_or(0.0))
                    .unwrap_or(0.0),
                extraction_method: parse_extraction_method(
                    &row.extraction_method.unwrap_or_default(),
                ),
                metadata: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn can_provide(&self, attribute_id: Uuid) -> bool {
        // Check if any document type can provide this attribute
        // DB schema: uses attribute_uuid
        let result = sqlx::query!(
            r#"
            SELECT COUNT(*) as count
            FROM "ob-poc".document_attribute_mappings
            WHERE attribute_uuid = $1
            "#,
            attribute_id
        )
        .fetch_one(&self.pool)
        .await;

        result.map(|r| r.count.unwrap_or(0) > 0).unwrap_or(false)
    }
}

fn parse_extraction_method(s: &str) -> ExtractionMethod {
    match s.to_uppercase().as_str() {
        "OCR" => ExtractionMethod::OCR,
        "MRZ" => ExtractionMethod::MRZ,
        "BARCODE" => ExtractionMethod::Barcode,
        "QR_CODE" | "QRCODE" => ExtractionMethod::QrCode,
        "FORM_FIELD" | "FORMFIELD" => ExtractionMethod::FormField,
        "TABLE" => ExtractionMethod::Table,
        "CHECKBOX" => ExtractionMethod::Checkbox,
        "SIGNATURE" => ExtractionMethod::Signature,
        "PHOTO" => ExtractionMethod::Photo,
        "NLP" => ExtractionMethod::NLP,
        "AI" => ExtractionMethod::AI,
        _ => ExtractionMethod::Manual,
    }
}

fn extraction_method_to_str(method: &ExtractionMethod) -> &'static str {
    match method {
        ExtractionMethod::OCR => "OCR",
        ExtractionMethod::MRZ => "MRZ",
        ExtractionMethod::Barcode => "BARCODE",
        ExtractionMethod::QrCode => "QR_CODE",
        ExtractionMethod::FormField => "FORM_FIELD",
        ExtractionMethod::Table => "TABLE",
        ExtractionMethod::Checkbox => "CHECKBOX",
        ExtractionMethod::Signature => "SIGNATURE",
        ExtractionMethod::Photo => "PHOTO",
        ExtractionMethod::NLP => "NLP",
        ExtractionMethod::AI => "AI",
        ExtractionMethod::Manual => "MANUAL",
    }
}
