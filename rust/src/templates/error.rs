//! Template error types

use thiserror::Error;

/// Errors that can occur during template operations
#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Template not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Missing required parameter: {0}")]
    MissingParam(String),

    #[error("Invalid parameter value for '{param}': {message}")]
    InvalidParam { param: String, message: String },

    #[error("Template expansion error: {0}")]
    Expansion(String),
}
