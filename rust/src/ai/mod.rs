//! AI Integration Module - Gemini API Client
//!
//! This module provides integration with Google's Gemini AI API for intelligent
//! DSL transformation and generation capabilities. It supports the DSL Agent
//! with AI-powered features like natural language to DSL conversion and
//! intelligent DSL editing suggestions.

// pub mod agentic_crud_service;  // Temporarily disabled due to gemini dependency
pub mod agentic_dictionary_service;
pub mod agentic_document_service;
pub mod crud_prompt_builder;
pub mod dsl_service;
// pub mod gemini;  // Temporarily disabled due to API compatibility issues
pub mod openai;
pub mod rag_system;
#[cfg(test)]
mod tests;
// pub mod unified_agentic_service;  // Temporarily disabled due to agentic_crud_service dependency

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AI service configuration
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// API key for the AI service
    pub api_key: String,

    /// Model name/version to use
    pub model: String,

    /// Maximum tokens in response
    pub max_tokens: Option<u32>,

    /// Temperature for response generation (0.0 - 1.0)
    pub temperature: Option<f32>,

    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            model: "gemini-2.5-flash-preview-09-2025".to_string(),
            max_tokens: Some(8192),
            temperature: Some(0.1),
            timeout_seconds: 30,
        }
    }
}

/// AI request for DSL operations
#[derive(Debug, Clone, Serialize)]
pub struct AiDslRequest {
    /// The instruction or question for the AI
    pub instruction: String,

    /// Optional context for the request
    pub context: Option<HashMap<String, String>>,

    /// Expected response type
    pub response_type: AiResponseType,

    /// Temperature for AI generation
    pub temperature: Option<f64>,

    /// Maximum tokens for response
    pub max_tokens: Option<u32>,
}

/// Type of AI response expected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiResponseType {
    /// Generate new DSL from scratch
    DslGeneration,

    /// Transform existing DSL
    DslTransformation,

    /// Validate DSL and provide feedback
    DslValidation,

    /// Explain DSL structure and meaning
    DslExplanation,

    /// Suggest improvements to DSL
    DslSuggestions,
}

/// AI response containing DSL and metadata
#[derive(Debug, Clone, Deserialize)]
pub struct AiDslResponse {
    /// Generated or transformed DSL content
    pub generated_dsl: String,

    /// Explanation of what was done
    pub explanation: String,

    /// Confidence score (0.0 - 1.0)
    pub confidence: Option<f64>,

    /// List of changes made (for transformations)
    pub changes: Option<Vec<String>>,

    /// Warnings or concerns about the DSL
    pub warnings: Option<Vec<String>>,

    /// Suggestions for improvement
    pub suggestions: Option<Vec<String>>,
}

/// Errors that can occur during AI operations
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Authentication error: missing or invalid API key")]
    AuthenticationError,

    #[error("Rate limit exceeded")]
    RateLimitError,

    #[error("AI service timeout")]
    TimeoutError,

    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

/// Result type for AI operations
pub type AiResult<T> = Result<T, AiError>;

/// Trait for AI service implementations
#[async_trait::async_trait]
pub trait AiService {
    /// Generate DSL from natural language instruction
    async fn generate_dsl(&self, request: AiDslRequest) -> AiResult<AiDslResponse>;

    /// Check if the service is available
    async fn health_check(&self) -> AiResult<bool>;

    /// Get service configuration
    fn config(&self) -> &AiConfig;
}

/// Utility functions for AI integration
pub mod utils {
    use super::*;

    /// Clean and validate AI-generated DSL
    pub fn clean_dsl_response(raw_response: &str) -> String {
        let cleaned = raw_response.trim();

        // Remove markdown code blocks if present
        let cleaned = if cleaned.starts_with("```") {
            if let Some(start) = cleaned.find('\n') {
                let content = &cleaned[start + 1..];
                if let Some(end) = content.rfind("```") {
                    &content[..end]
                } else {
                    content
                }
            } else {
                cleaned
            }
        } else {
            cleaned
        };

        cleaned.trim().to_string()
    }

    /// Extract confidence score from AI response
    pub fn extract_confidence(response: &str) -> f64 {
        // Look for confidence indicators in the response
        if response.contains("high confidence") || response.contains("very confident") {
            0.9
        } else if response.contains("medium confidence") || response.contains("confident") {
            0.7
        } else if response.contains("low confidence") || response.contains("uncertain") {
            0.5
        } else {
            0.8 // Default confidence
        }
    }

    /// Parse structured JSON response from AI
    pub fn parse_structured_response(response: &str) -> AiResult<serde_json::Value> {
        // Try to find JSON in the response
        let cleaned = clean_dsl_response(response);

        // Look for JSON object
        if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
            let json_str = &cleaned[start..=end];
            serde_json::from_str(json_str).map_err(AiError::JsonError)
        } else {
            // Return a simple structure if no JSON found
            Ok(serde_json::json!({
                "dsl_content": cleaned,
                "explanation": "AI response",
                "confidence": extract_confidence(&cleaned),
                "changes": [],
                "warnings": [],
                "suggestions": []
            }))
        }
    }
}
