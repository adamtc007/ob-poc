//! Research macro error types

use thiserror::Error;

/// Errors that can occur during research macro operations
#[derive(Debug, Error)]
pub enum ResearchError {
    #[error("Unknown research macro: {0}")]
    UnknownMacro(String),

    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    #[error("Invalid parameter value for '{param}': {reason}")]
    InvalidParameter { param: String, reason: String },

    #[error("Schema validation failed: {0:?}")]
    SchemaValidation(Vec<String>),

    #[error("Failed to parse JSON from LLM response: {0}")]
    JsonParse(String),

    #[error("Template rendering error: {0}")]
    TemplateRender(String),

    #[error("LLM client error: {0}")]
    LlmClient(String),

    #[error("No pending research to approve")]
    NoPendingResearch,

    #[error("Session not found: {0}")]
    SessionNotFound(uuid::Uuid),

    #[error("Registry error: {0}")]
    Registry(String),

    #[error("LEI validation failed: {lei} - {reason}")]
    LeiValidation { lei: String, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for ResearchError {
    fn from(err: anyhow::Error) -> Self {
        ResearchError::Other(err.to_string())
    }
}

impl From<serde_json::Error> for ResearchError {
    fn from(err: serde_json::Error) -> Self {
        ResearchError::JsonParse(err.to_string())
    }
}

impl From<serde_yaml::Error> for ResearchError {
    fn from(err: serde_yaml::Error) -> Self {
        ResearchError::Registry(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ResearchError>;
