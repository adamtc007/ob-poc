//! Lexicon-based Agent Pipeline Integration
//!
//! This module provides `LexiconAgentPipeline`, the primary agent pipeline
//! using formal grammar-based tokenization and nom parsing.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let pipeline = LexiconAgentPipeline::load_from_config(config_dir)?;
//! let response = pipeline.process(session_id, message).await?;
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::lexicon::{
    DatabaseEntityResolver, EntityResolver, Lexicon, LexiconPipeline, LexiconPipelineResult,
};

// ============================================================================
// Response Types
// ============================================================================

/// Session context for maintaining conversation state.
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    /// Active CBU ID for this session.
    pub active_cbu: Option<String>,

    /// Last intent type processed.
    pub last_intent: Option<String>,

    /// Session-level entity bindings.
    pub bindings: HashMap<String, String>,
}

/// Response type indicating what kind of action is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseType {
    /// DSL is ready for execution.
    Execution,

    /// Need user to clarify something.
    Clarification,

    /// Need user to confirm before execution.
    Confirmation,

    /// An error occurred.
    Error,
}

/// Clarification request with options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationRequest {
    /// Question to ask the user.
    pub question: String,

    /// Possible options (if any).
    pub options: Vec<String>,

    /// Additional context.
    pub context: String,
}

/// Execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution succeeded.
    pub success: bool,

    /// Created entity IDs.
    pub created_ids: Vec<String>,

    /// Any error message.
    pub error: Option<String>,
}

/// Agent response from processing a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// Type of response.
    pub response_type: ResponseType,

    /// Human-readable message.
    pub message: String,

    /// Generated DSL (if any).
    pub dsl: Option<String>,

    /// Execution result (if executed).
    pub execution: Option<ExecutionResult>,

    /// Clarification request (if needed).
    pub clarification: Option<ClarificationRequest>,

    /// Suggested next actions.
    pub suggestions: Option<Vec<String>>,
}

/// Lexicon-based agent pipeline.
///
/// This is the Phase 3 replacement for `AgentPipeline` using formal grammar
/// parsing instead of regex pattern matching.
pub struct LexiconAgentPipeline {
    /// The core lexicon pipeline.
    pipeline: LexiconPipeline,

    /// Entity resolver for database lookups.
    entity_resolver: Option<Arc<dyn EntityResolver>>,

    /// Session contexts keyed by session ID.
    sessions: HashMap<Uuid, SessionContext>,
}

impl LexiconAgentPipeline {
    /// Create a new pipeline with the given lexicon.
    pub fn new(lexicon: Arc<Lexicon>) -> Self {
        Self {
            pipeline: LexiconPipeline::new(lexicon),
            entity_resolver: None,
            sessions: HashMap::new(),
        }
    }

    /// Load pipeline from configuration directory.
    ///
    /// Expects `config/agent/lexicon.yaml` in the config directory.
    pub fn load_from_config(config_dir: impl AsRef<Path>) -> Result<Self> {
        let lexicon_path = config_dir.as_ref().join("agent").join("lexicon.yaml");
        let lexicon = Lexicon::load_from_file(&lexicon_path)
            .with_context(|| format!("Failed to load lexicon from {:?}", lexicon_path))?;

        Ok(Self::new(Arc::new(lexicon)))
    }

    /// Set the entity resolver for database lookups.
    pub fn with_entity_resolver(mut self, resolver: Arc<dyn EntityResolver>) -> Self {
        self.entity_resolver = Some(resolver);
        self
    }

    /// Set the entity resolver from an EntityGateway URL.
    #[cfg(feature = "database")]
    pub async fn with_gateway_resolver(mut self, gateway_url: &str) -> Result<Self> {
        let resolver = DatabaseEntityResolver::connect(gateway_url).await?;
        self.entity_resolver = Some(Arc::new(resolver));
        Ok(self)
    }

    /// Get or create a session context.
    pub fn get_or_create_session(&mut self, session_id: Uuid) -> &mut SessionContext {
        self.sessions.entry(session_id).or_default()
    }

    /// Set the active CBU for a session.
    pub fn set_active_cbu(&mut self, session_id: Uuid, cbu_id: String, cbu_name: String) {
        let session = self.get_or_create_session(session_id);
        session.active_cbu = Some(cbu_id.clone());

        // Also update the pipeline's internal state
        self.pipeline.set_active_cbu(cbu_id, cbu_name);
    }

    /// Process a user message in a session.
    pub async fn process(&mut self, session_id: Uuid, message: &str) -> Result<AgentResponse> {
        // Ensure session exists
        let _session = self.get_or_create_session(session_id);

        // Process through lexicon pipeline
        let result = self.pipeline.process(message).await;

        // Convert to AgentResponse
        Ok(self.convert_result(result))
    }

    /// Convert LexiconPipelineResult to AgentResponse.
    fn convert_result(&self, result: LexiconPipelineResult) -> AgentResponse {
        let response_type = if !result.errors.is_empty() {
            ResponseType::Error
        } else if result.intent_type == "unknown" {
            ResponseType::Clarification
        } else if result.needs_confirmation {
            ResponseType::Confirmation
        } else if !result.unresolved_entities.is_empty() {
            ResponseType::Clarification
        } else {
            ResponseType::Execution
        };

        // Build clarification if needed
        let clarification = if !result.unresolved_entities.is_empty() {
            Some(ClarificationRequest {
                question: format!(
                    "I need to resolve these entities: {}",
                    result.unresolved_entities.join(", ")
                ),
                options: vec![],
                context: result.description.clone(),
            })
        } else if result.intent_type == "unknown" {
            Some(ClarificationRequest {
                question: "I didn't understand that. Could you rephrase?".to_string(),
                options: vec![],
                context: String::new(),
            })
        } else {
            None
        };

        // Build message with domain info if available
        let message = if let Some(ref domain) = result.domain {
            format!("[{}] {}", domain, result.description)
        } else {
            result.description
        };

        AgentResponse {
            response_type,
            message,
            dsl: result.dsl,
            execution: None,
            clarification,
            suggestions: None,
        }
    }

    /// Clear a session.
    pub fn clear_session(&mut self, session_id: Uuid) {
        self.sessions.remove(&session_id);
        self.pipeline.clear_active_cbu();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::lexicon::{
        EntitiesConfig, InstrumentsConfig, LexiconConfig, PrepositionsConfig, VerbsConfig,
    };

    fn test_lexicon() -> Arc<Lexicon> {
        let config = LexiconConfig {
            verbs: VerbsConfig {
                create: vec!["add".to_string(), "create".to_string()],
                link: vec!["assign".to_string()],
                query: vec!["list".to_string(), "show".to_string()],
                ..Default::default()
            },
            entities: EntitiesConfig {
                counterparty: vec!["counterparty".to_string()],
                isda: vec!["isda".to_string()],
                person: vec!["person".to_string()],
                ..Default::default()
            },
            instruments: InstrumentsConfig {
                otc: vec!["irs".to_string(), "cds".to_string()],
                exchange_traded: vec!["equity".to_string()],
            },
            roles: vec!["director".to_string()],
            prepositions: PrepositionsConfig {
                as_: vec!["as".to_string()],
                for_: vec!["for".to_string()],
                ..Default::default()
            },
            articles: vec!["a".to_string(), "the".to_string()],
            ..Default::default()
        };

        Arc::new(Lexicon::from_config(config).unwrap())
    }

    #[tokio::test]
    async fn test_lexicon_agent_pipeline() {
        let lexicon = test_lexicon();
        let mut pipeline = LexiconAgentPipeline::new(lexicon);
        let session_id = Uuid::new_v4();

        pipeline.set_active_cbu(session_id, "cbu-123".to_string(), "Test Fund".to_string());

        let response = pipeline
            .process(session_id, "add counterparty for irs")
            .await
            .unwrap();

        // Check that DSL was generated and domain detected
        assert!(response.dsl.is_some());
        assert!(response.message.contains("otc"));
    }

    #[tokio::test]
    async fn test_unknown_intent() {
        let lexicon = test_lexicon();
        let mut pipeline = LexiconAgentPipeline::new(lexicon);
        let session_id = Uuid::new_v4();

        let response = pipeline.process(session_id, "hello world").await.unwrap();

        assert_eq!(response.response_type, ResponseType::Clarification);
        assert!(response.clarification.is_some());
    }
}
