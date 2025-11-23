//! Attribute value sources for DSL generation
//!
//! Sources provide attribute values that can be used in DSL generation.
//! They retrieve values from documents, external APIs, databases, etc.

pub mod document;

pub use document::{DocumentSource, ExtractionMethod, ExtractedValue, ExtractionDsl, DocumentAttributeMapping};

use async_trait::async_trait;
use uuid::Uuid;

/// Trait for attribute value sources
#[async_trait]
pub trait AttributeSource: Send + Sync {
    /// Get the source type identifier
    fn source_type(&self) -> &str;
    
    /// Fetch attribute value from this source
    async fn fetch_value(
        &self,
        attribute_id: Uuid,
        context: &SourceContext,
    ) -> Result<Option<ExtractedValue>, SourceError>;
    
    /// Check if this source can provide the given attribute
    async fn can_provide(&self, attribute_id: Uuid) -> bool;
}

/// Context for source operations
#[derive(Debug, Clone)]
pub struct SourceContext {
    pub cbu_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
}

/// Errors from source operations
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),
    
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),
    
    #[error("Confidence below threshold: {0} < {1}")]
    LowConfidence(f64, f64),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
