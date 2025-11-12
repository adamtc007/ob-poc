//! Agents Module - High-Level Agentic Capabilities Facade
//!
//! This module provides a clean, unified interface for all agentic (AI-powered)
//! capabilities in the system. It abstracts away the complexity of individual
//! AI services and provides domain-specific intelligent automation.
//!
//! ## Architecture
//!
//! The agents module follows the facade pattern:
//! - **Public Interface**: Only essential agentic capabilities exposed
//! - **Internal Implementation**: AI service coordination hidden
//! - **Domain-Specific**: Organized by business capability, not technology
//!
//! ## Key Capabilities
//!
//! - **Dictionary Management**: AI-powered data dictionary operations
//! - **DSL Generation**: Natural language to DSL conversion
//! - **Document Processing**: Intelligent document extraction and analysis
//! - **KYC Automation**: Automated KYC workflow orchestration
//! - **UBO Analysis**: Ultimate beneficial ownership detection and analysis

// ============================================================================
// INTERNAL MODULES - Implementation Details (pub(crate))
// ============================================================================

// Re-export agentic services from the AI module but keep them internal
#[cfg(feature = "database")]
pub(crate) use crate::ai::agentic_dictionary_service;
#[cfg(feature = "database")]
pub(crate) use crate::ai::agentic_document_service;
pub(crate) use crate::ai::crud_prompt_builder;
pub(crate) use crate::ai::dsl_service;
pub(crate) use crate::ai::rag_system;
// pub(crate) use crate::ai::unified_agentic_service;  // Temporarily disabled

// ============================================================================
// PUBLIC FACADE - Agentic Capabilities for External Consumers
// ============================================================================

// Dictionary Management Agent
#[cfg(feature = "database")]
pub use crate::ai::agentic_dictionary_service::{
    AgenticDictionaryService, DictionaryServiceConfig,
};

// DSL Generation and Management Agent
#[cfg(feature = "database")]
pub use crate::ai::dsl_service::{AiDslService, KycCaseRequest, OwnershipLink, UboAnalysisRequest};

// Document Processing Agent (when available)
// #[cfg(feature = "database")]
// pub use crate::ai::agentic_document_service::{AgenticDocumentService, DocumentServiceConfig};

// Unified Agentic Service (when available)
// #[cfg(feature = "database")]
// pub use crate::ai::unified_agentic_service::{UnifiedAgenticService, AgenticCapability};

// ============================================================================
// AGENT COORDINATION TYPES
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Import types from dsl_types crate (Level 1 foundation)
use dsl_types::AgentMetadata;

/// High-level agent coordination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult<T> {
    /// Operation success status
    pub success: bool,
    /// Result data (if successful)
    pub data: Option<T>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Agent metadata
    pub metadata: AgentMetadata,
}

// AgentMetadata moved to dsl_types crate - import from there

/// Agent capability enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentCapability {
    /// Dictionary operations
    DictionaryManagement,
    /// DSL generation and processing
    DslGeneration,
    /// Document processing and extraction
    DocumentProcessing,
    /// KYC workflow automation
    KycAutomation,
    /// UBO analysis and detection
    UboAnalysis,
    /// Multi-domain orchestration
    DomainOrchestration,
}

/// Agent coordination error types
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent not available: {agent_id}")]
    AgentUnavailable { agent_id: String },

    #[error("Operation failed: {message}")]
    OperationFailed { message: String },

    #[error("Configuration error: {details}")]
    ConfigurationError { details: String },

    #[error("Dependency error: {dependency}")]
    DependencyError { dependency: String },
}

pub type AgentResult_<T> = Result<AgentResult<T>, AgentError>;

// ============================================================================
// FUTURE: AGENT ORCHESTRATION (placeholder for future development)
// ============================================================================

/// Future: Multi-agent orchestration coordinator
/// This will coordinate between multiple specialized agents
#[allow(dead_code)]
pub(crate) struct AgentOrchestrator {
    // Future implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_result_creation() {
        let result = AgentResult {
            success: true,
            data: Some("test_data".to_string()),
            error: None,
            metadata: AgentMetadata {
                agent_id: "test_agent".to_string(),
                operation: "test_op".to_string(),
                duration_ms: 100,
                confidence: 0.95,
                context: HashMap::new(),
            },
        };

        assert!(result.success);
        assert_eq!(result.data, Some("test_data".to_string()));
    }

    #[test]
    fn test_agent_capability_enum() {
        let capability = AgentCapability::DictionaryManagement;
        assert!(matches!(capability, AgentCapability::DictionaryManagement));
    }
}
