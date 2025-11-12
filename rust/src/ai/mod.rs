//! AI Integration Module - Gemini API Client
//!
//! This module provides integration with Google's Gemini AI API for intelligent
//! DSL transformation and generation capabilities. It supports the DSL Agent
//! with AI-powered features like natural language to DSL conversion and
//! intelligent DSL editing suggestions.

// pub mod agentic_crud_service;  // Temporarily disabled due to gemini dependency
pub mod agentic_dictionary_service;
pub(crate) mod agentic_document_service;
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

// Import AI types from dsl_types crate (Level 1 foundation)
pub use dsl_types::{AiConfig, AiDslRequest, AiDslResponse, AiError, AiResponseType, AiResult};

/// Trait for AI service implementations
#[async_trait::async_trait]
pub trait AiService {
    /// Generate DSL from natural language instruction
    async fn generate_dsl(&self, request: AiDslRequest) -> AiResult<AiDslResponse>;

    /// Check if the service is available
    async fn health_check(&self) -> AiResult<bool>;

    /// Get service name for identification
    fn service_name(&self) -> &str;

    /// Get supported response types
    fn supported_response_types(&self) -> Vec<AiResponseType>;
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
    pub(crate) fn extract_confidence(response: &str) -> f64 {
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
