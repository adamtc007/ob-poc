//! AI-Powered DSL Agent System
//!
//! This module provides intelligent DSL generation, transformation, and validation
//! capabilities using AI agents. It consolidates functionality previously spread
//! across the Go implementation into a unified Rust system.
//!
//! # Features
//!
//! - **DSL Generation**: Create DSL from natural language instructions
//! - **DSL Transformation**: Modify existing DSL based on business requirements
//! - **DSL Validation**: Comprehensive syntax and semantic validation
//! - **Verb Validation**: Ensure only approved vocabulary verbs are used
//! - **Semantic Context**: Rich metadata-driven verb selection and usage
//!
//! # Architecture
//!
//! The agent system follows a layered approach:
//! - **Agent Interface**: High-level API for DSL operations
//! - **Validation Engine**: Syntax, semantic, and vocabulary validation
//! - **Context Engine**: Business context and workflow awareness
//! - **Template Engine**: Pattern-based DSL generation
//!
//! # Usage
//!
//! ```rust,ignore
//! use ob_poc::agents::{DslAgent, DslTransformationRequest};
//!
//! let agent = DslAgent::new().await?;
//! let response = agent.transform_dsl(DslTransformationRequest {
//!     current_dsl: "(case.create (cbu.id \"CBU-1234\"))".to_string(),
//!     instruction: "Add custody product".to_string(),
//!     target_state: "products_added".to_string(),
//!     context: HashMap::new(),
//! }).await?;
//! ```

pub mod dsl_agent;
pub mod templates;
pub mod validation;

pub use dsl_agent::{
    DslAgent, DslGenerationRequest, DslGenerationResponse, DslTransformationRequest,
    DslTransformationResponse, DslValidationResponse,
};
pub use templates::{DslTemplate, DslTemplateEngine, TemplateContext, TemplateValue};
pub use validation::{DslValidator, ValidationError, ValidationResult, VerbValidator};

/// Common result type for agent operations
pub type AgentResult<T> = Result<T, AgentError>;

/// Errors that can occur during agent operations
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent initialization failed: {0}")]
    InitializationError(String),

    #[error("DSL validation failed: {0}")]
    ValidationError(String),

    #[error("DSL transformation failed: {0}")]
    TransformationError(String),

    #[error("Template processing failed: {0}")]
    TemplateError(String),

    #[error("Semantic analysis failed: {0}")]
    SemanticError(String),

    #[error("External API error: {0}")]
    ExternalApiError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Configuration for the agent system
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// External AI service API key (e.g., Gemini, OpenAI)
    pub api_key: Option<String>,

    /// Model name to use for AI operations
    pub model_name: String,

    /// Maximum confidence threshold for AI responses (0.0-1.0)
    pub confidence_threshold: f64,

    /// Maximum number of retry attempts for failed operations
    pub max_retries: u32,

    /// Timeout for AI service calls in seconds
    pub timeout_seconds: u64,

    /// Enable verbose logging for debugging
    pub verbose_logging: bool,

    /// Custom verb vocabulary file path
    pub custom_vocabulary_path: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("GEMINI_API_KEY").ok(),
            model_name: "gemini-2.5-flash-preview-09-2025".to_string(),
            confidence_threshold: 0.8,
            max_retries: 3,
            timeout_seconds: 30,
            verbose_logging: false,
            custom_vocabulary_path: None,
        }
    }
}

/// Quality metrics for agent operations
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QualityMetrics {
    /// Confidence score from AI model (0.0-1.0)
    pub confidence: f64,

    /// Validation score for generated DSL (0.0-1.0)
    pub validation_score: f64,

    /// Completeness score (how complete the DSL is) (0.0-1.0)
    pub completeness: f64,

    /// Semantic coherence score (0.0-1.0)
    pub coherence: f64,

    /// Number of approved vocabulary verbs used
    pub approved_verbs_count: usize,

    /// Number of unapproved verbs detected
    pub unapproved_verbs_count: usize,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

impl QualityMetrics {
    /// Calculate overall quality score combining all metrics
    pub fn overall_score(&self) -> f64 {
        let base_score = (self.confidence * 0.3)
            + (self.validation_score * 0.25)
            + (self.completeness * 0.2)
            + (self.coherence * 0.15)
            + (if self.unapproved_verbs_count == 0 {
                1.0
            } else {
                0.0
            } * 0.1);

        // Penalize for unapproved verbs
        if self.unapproved_verbs_count > 0 {
            base_score * 0.5
        } else {
            base_score
        }
    }

    /// Check if quality meets minimum acceptable standards
    pub fn meets_quality_standards(&self, threshold: f64) -> bool {
        self.overall_score() >= threshold && self.unapproved_verbs_count == 0
    }
}

/// Utility functions for the agent system
pub mod utils {
    use super::*;
    use std::collections::HashMap;

    /// Clean JSON response from AI models (remove markdown, etc.)
    pub fn clean_json_response(response: &str) -> String {
        let cleaned = response.trim();

        // Remove markdown JSON code blocks
        let cleaned = if cleaned.starts_with("```json") {
            if let Some(start) = cleaned.find('\n') {
                &cleaned[start + 1..]
            } else {
                cleaned
            }
        } else {
            cleaned
        };

        let cleaned = cleaned.trim_end_matches("```").trim_start_matches("```");

        // Extract JSON object if embedded in text
        if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
            if end > start {
                return cleaned[start..=end].to_string();
            }
        }

        cleaned.trim().to_string()
    }

    /// Convert context map to JSON string safely
    pub fn context_to_json(context: &HashMap<String, serde_json::Value>) -> String {
        serde_json::to_string(context).unwrap_or_else(|_| "{}".to_string())
    }

    /// Extract business context from DSL content
    pub fn extract_business_context(dsl: &str) -> HashMap<String, String> {
        let mut context = HashMap::new();

        // Extract CBU ID
        if let Some(cbu_match) = regex::Regex::new(r#"cbu\.id\s+"([^"]+)""#)
            .ok()
            .and_then(|re| re.captures(dsl))
        {
            context.insert("cbu_id".to_string(), cbu_match[1].to_string());
        }

        // Extract nature-purpose
        if let Some(purpose_match) = regex::Regex::new(r#"nature-purpose\s+"([^"]+)""#)
            .ok()
            .and_then(|re| re.captures(dsl))
        {
            context.insert("nature_purpose".to_string(), purpose_match[1].to_string());
        }

        // Extract domain from verbs
        if let Some(domain_match) = regex::Regex::new(r#"\(([a-z]+)\."#)
            .ok()
            .and_then(|re| re.captures(dsl))
        {
            context.insert("primary_domain".to_string(), domain_match[1].to_string());
        }

        context
    }

    /// Estimate DSL complexity score
    pub fn calculate_complexity_score(dsl: &str) -> f64 {
        let line_count = dsl.lines().count() as f64;
        let verb_count = regex::Regex::new(r#"\([a-z]+\.[a-z]"#)
            .map(|re| re.find_iter(dsl).count() as f64)
            .unwrap_or(0.0);
        let nesting_depth = calculate_max_nesting_depth(dsl);

        // Complexity formula: weighted combination of factors
        (line_count * 0.1) + (verb_count * 0.5) + (nesting_depth * 0.4)
    }

    fn calculate_max_nesting_depth(dsl: &str) -> f64 {
        let mut max_depth = 0;
        let mut current_depth: i32 = 0;

        for ch in dsl.chars() {
            match ch {
                '(' => {
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth);
                }
                ')' => {
                    current_depth = current_depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        max_depth as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.model_name, "gemini-2.5-flash-preview-09-2025");
        assert_eq!(config.confidence_threshold, 0.8);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_quality_metrics_overall_score() {
        let metrics = QualityMetrics {
            confidence: 0.9,
            validation_score: 0.8,
            completeness: 0.7,
            coherence: 0.6,
            approved_verbs_count: 5,
            unapproved_verbs_count: 0,
            processing_time_ms: 1000,
        };

        let score = metrics.overall_score();
        assert!(score > 0.7);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_quality_metrics_with_unapproved_verbs() {
        let metrics = QualityMetrics {
            confidence: 0.9,
            validation_score: 0.8,
            completeness: 0.7,
            coherence: 0.6,
            approved_verbs_count: 5,
            unapproved_verbs_count: 2,
            processing_time_ms: 1000,
        };

        let score = metrics.overall_score();
        assert!(score < 0.5); // Should be penalized
        assert!(!metrics.meets_quality_standards(0.8));
    }

    #[test]
    fn test_clean_json_response() {
        let markdown_json = "```json\n{\"test\": \"value\"}\n```";
        let cleaned = utils::clean_json_response(markdown_json);
        assert_eq!(cleaned, r#"{"test": "value"}"#);
    }

    #[test]
    fn test_extract_business_context() {
        let dsl = r#"(case.create (cbu.id "CBU-1234") (nature-purpose "Test fund"))"#;
        let context = utils::extract_business_context(dsl);

        assert_eq!(context.get("cbu_id"), Some(&"CBU-1234".to_string()));
        assert_eq!(
            context.get("nature_purpose"),
            Some(&"Test fund".to_string())
        );
        assert_eq!(context.get("primary_domain"), Some(&"case".to_string()));
    }

    #[test]
    fn test_calculate_complexity_score() {
        let simple_dsl = "(case.create (cbu.id \"test\"))";
        let complex_dsl = r#"
            (case.create (cbu.id "test"))
            (products.add "CUSTODY" "FUND_ACCOUNTING")
            (kyc.start
              (documents
                (document "CertificateOfIncorporation")
                (document "ArticlesOfAssociation"))
              (jurisdictions
                (jurisdiction "LU")))
        "#;

        let simple_score = utils::calculate_complexity_score(simple_dsl);
        let complex_score = utils::calculate_complexity_score(complex_dsl);

        assert!(complex_score > simple_score);
    }
}
