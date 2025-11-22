//! Document Type Models
//!
//! Models for document types and their attribute mapping capabilities.
//! These models bridge document types to extractable attributes.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

/// Document type with extraction capabilities
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentType {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub domain: String,
    pub description: Option<String>,
}

/// Document to attribute mapping with extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentAttributeMapping {
    pub mapping_id: Uuid,
    pub document_type_id: Uuid,
    pub attribute_uuid: Uuid,
    pub extraction_method: ExtractionMethod,
    pub field_location: Option<sqlx::types::Json<FieldLocation>>,
    pub field_name: Option<String>,
    pub confidence_threshold: f64,
    pub is_required: bool,
    pub validation_pattern: Option<String>,
}

/// Extraction method enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
}

impl std::fmt::Display for ExtractionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractionMethod::OCR => write!(f, "OCR"),
            ExtractionMethod::MRZ => write!(f, "MRZ"),
            ExtractionMethod::Barcode => write!(f, "BARCODE"),
            ExtractionMethod::QrCode => write!(f, "QR_CODE"),
            ExtractionMethod::FormField => write!(f, "FORM_FIELD"),
            ExtractionMethod::Table => write!(f, "TABLE"),
            ExtractionMethod::Checkbox => write!(f, "CHECKBOX"),
            ExtractionMethod::Signature => write!(f, "SIGNATURE"),
            ExtractionMethod::Photo => write!(f, "PHOTO"),
            ExtractionMethod::NLP => write!(f, "NLP"),
            ExtractionMethod::AI => write!(f, "AI"),
        }
    }
}

impl std::str::FromStr for ExtractionMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "OCR" => Ok(ExtractionMethod::OCR),
            "MRZ" => Ok(ExtractionMethod::MRZ),
            "BARCODE" => Ok(ExtractionMethod::Barcode),
            "QR_CODE" => Ok(ExtractionMethod::QrCode),
            "FORM_FIELD" => Ok(ExtractionMethod::FormField),
            "TABLE" => Ok(ExtractionMethod::Table),
            "CHECKBOX" => Ok(ExtractionMethod::Checkbox),
            "SIGNATURE" => Ok(ExtractionMethod::Signature),
            "PHOTO" => Ok(ExtractionMethod::Photo),
            "NLP" => Ok(ExtractionMethod::NLP),
            "AI" => Ok(ExtractionMethod::AI),
            _ => Err(format!("Unknown extraction method: {}", s)),
        }
    }
}

// SQLX Type implementation for ExtractionMethod
#[cfg(feature = "database")]
impl sqlx::Type<sqlx::Postgres> for ExtractionMethod {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(feature = "database")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ExtractionMethod {
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        s.parse().map_err(|e: String| e.into())
    }
}

#[cfg(feature = "database")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ExtractionMethod {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&self.to_string(), buf)
    }
}

/// Field location information for extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldLocation {
    pub page: Option<u32>,
    pub region: Option<Region>,
}

/// Region coordinates for extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

/// Extracted attribute value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedAttribute {
    pub attribute_uuid: Uuid,
    pub value: serde_json::Value,
    pub confidence: f64,
    pub extraction_method: ExtractionMethod,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Document with type information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedDocument {
    pub document_id: Uuid,
    pub document_type: DocumentType,
    pub extractable_attributes: Vec<DocumentAttributeMapping>,
}

impl DocumentType {
    /// Create a new document type
    pub fn new(type_code: String, display_name: String, category: String, domain: String) -> Self {
        Self {
            type_id: Uuid::new_v4(),
            type_code,
            display_name,
            category,
            domain,
            description: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

impl DocumentAttributeMapping {
    /// Create a new document attribute mapping
    pub fn new(
        document_type_id: Uuid,
        attribute_uuid: Uuid,
        extraction_method: ExtractionMethod,
    ) -> Self {
        Self {
            mapping_id: Uuid::new_v4(),
            document_type_id,
            attribute_uuid,
            extraction_method,
            field_location: None,
            field_name: None,
            confidence_threshold: 0.80,
            is_required: false,
            validation_pattern: None,
        }
    }

    /// Set confidence threshold
    pub fn with_confidence(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Set as required
    pub fn required(mut self) -> Self {
        self.is_required = true;
        self
    }

    /// Set field location
    pub fn with_location(mut self, location: FieldLocation) -> Self {
        self.field_location = Some(sqlx::types::Json(location));
        self
    }

    /// Set field name
    pub fn with_field_name(mut self, name: String) -> Self {
        self.field_name = Some(name);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraction_method_from_str() {
        assert_eq!(
            "OCR".parse::<ExtractionMethod>().unwrap(),
            ExtractionMethod::OCR
        );
        assert_eq!(
            "mrz".parse::<ExtractionMethod>().unwrap(),
            ExtractionMethod::MRZ
        );
        assert_eq!(
            "FORM_FIELD".parse::<ExtractionMethod>().unwrap(),
            ExtractionMethod::FormField
        );
    }

    #[test]
    fn test_extraction_method_to_string() {
        assert_eq!(ExtractionMethod::OCR.to_string(), "OCR");
        assert_eq!(ExtractionMethod::MRZ.to_string(), "MRZ");
        assert_eq!(ExtractionMethod::QrCode.to_string(), "QR_CODE");
    }

    #[test]
    fn test_document_type_builder() {
        let doc_type = DocumentType::new(
            "PASSPORT".to_string(),
            "Passport".to_string(),
            "IDENTITY".to_string(),
            "KYC".to_string(),
        )
        .with_description("International travel document".to_string());

        assert_eq!(doc_type.type_code, "PASSPORT");
        assert_eq!(doc_type.category, "IDENTITY");
        assert!(doc_type.description.is_some());
    }

    #[test]
    fn test_mapping_builder() {
        let mapping =
            DocumentAttributeMapping::new(Uuid::new_v4(), Uuid::new_v4(), ExtractionMethod::MRZ)
                .with_confidence(0.95)
                .required();

        assert_eq!(mapping.extraction_method, ExtractionMethod::MRZ);
        assert_eq!(mapping.confidence_threshold, 0.95);
        assert!(mapping.is_required);
    }
}
