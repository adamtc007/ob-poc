//! Document type definition for the semantic registry.
//!
//! A document type defines a class of physical or digital documents
//! that can serve as evidence. Examples: passport, utility bill,
//! certificate of incorporation, bank statement.

use serde::{Deserialize, Serialize};

/// Body for a document type definition snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTypeDefBody {
    /// Fully qualified name, e.g. `"doc.passport"`
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Category: `identity`, `address`, `corporate`, `financial`, `regulatory`
    pub category: String,
    /// Maximum age in days before the document is considered expired
    #[serde(default)]
    pub max_age_days: Option<u32>,
    /// Accepted file formats (e.g., `["pdf", "jpg", "png"]`)
    #[serde(default)]
    pub accepted_formats: Vec<String>,
    /// Data extraction rules (what attributes can be extracted from this document)
    #[serde(default)]
    pub extraction_rules: Vec<DocumentExtractionRule>,
}

/// A rule for extracting structured data from a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentExtractionRule {
    /// Attribute FQN to extract to
    pub target_attribute_fqn: String,
    /// Extraction method: `ocr`, `barcode`, `manual`, `metadata`
    pub method: String,
    /// Confidence threshold for automated extraction
    #[serde(default)]
    pub min_confidence: Option<f64>,
    /// Whether human review is required after extraction
    #[serde(default)]
    pub requires_review: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_type_def_serde() {
        let body = DocumentTypeDefBody {
            fqn: "doc.passport".into(),
            name: "Passport".into(),
            description: "National passport for identity verification".into(),
            category: "identity".into(),
            max_age_days: Some(3650),
            accepted_formats: vec!["pdf".into(), "jpg".into(), "png".into()],
            extraction_rules: vec![DocumentExtractionRule {
                target_attribute_fqn: "entity.full-name".into(),
                method: "ocr".into(),
                min_confidence: Some(0.90),
                requires_review: false,
            }],
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: DocumentTypeDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "doc.passport");
        assert_eq!(round.category, "identity");
        assert_eq!(round.accepted_formats.len(), 3);
        assert_eq!(round.extraction_rules.len(), 1);
    }

    #[test]
    fn test_document_type_defaults() {
        let json = serde_json::json!({
            "fqn": "doc.test",
            "name": "Test",
            "description": "Test doc",
            "category": "identity"
        });
        let body: DocumentTypeDefBody = serde_json::from_value(json).unwrap();
        assert!(body.max_age_days.is_none());
        assert!(body.accepted_formats.is_empty());
        assert!(body.extraction_rules.is_empty());
    }
}
