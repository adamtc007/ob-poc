//! Source Executor Framework
//!
//! This module provides the framework for fetching attribute values from various sources
//! during DSL execution. Each source has a priority and can handle specific attributes.

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::domains::attributes::execution_context::{ExecutionContext, ValueSource};

pub type SourceResult<T> = Result<T, SourceError>;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("API call failed: {0}")]
    ApiError(String),

    #[error("No valid source for attribute: {0}")]
    NoValidSource(Uuid),
}

#[derive(Debug, Clone)]
pub struct AttributeValue {
    pub uuid: Uuid,
    pub semantic_id: String,
    pub value: JsonValue,
    pub source: ValueSource,
}

#[async_trait]
pub trait SourceExecutor: Send + Sync {
    /// Attempt to fetch a value for the given attribute
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue>;

    /// Check if this source can handle the given attribute
    fn can_handle(&self, attr_uuid: &Uuid) -> bool;

    /// Priority of this source (lower = higher priority)
    fn priority(&self) -> u32;
}

pub mod default;
pub mod document_extraction;
pub mod user_input;

pub use default::DefaultValueSource;
pub use document_extraction::DocumentExtractionSource;
pub use user_input::UserInputSource;
