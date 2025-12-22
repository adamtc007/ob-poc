//! Agent Pipeline
//!
//! Complete agent processing pipeline that integrates:
//! - IntentClassifier: Understand user intent from utterances
//! - EntityExtractor: Extract domain entities (managers, markets, instruments)
//! - DslGenerator: Generate DSL from classified intents and extracted entities
//! - Validation: Validate generated DSL before execution
//! - Execution: Execute validated DSL against the database (async)
//!
//! This implements the Phase 3 agent intelligence stack.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

#[cfg(feature = "database")]
use crate::dsl_v2::ExecutionContext;

use crate::agentic::dsl_generator::{DslGenerator, GenerationContext};
use crate::agentic::entity_extractor::{EntityExtractor, ExtractedEntities};
use crate::agentic::entity_types::EntityTypesConfig;
use crate::agentic::instrument_hierarchy::InstrumentHierarchyConfig;
use crate::agentic::intent_classifier::{
    ClassificationResult, ConversationContext, IntentClassifier,
};
use crate::agentic::market_regions::MarketRegionsConfig;
use crate::agentic::taxonomy::IntentTaxonomy;
use crate::agentic::validator::AgentValidator;

/// Complete agent processing pipeline
pub struct AgentPipeline {
    intent_classifier: IntentClassifier,
    entity_extractor: EntityExtractor,
    dsl_generator: DslGenerator,
    validator: AgentValidator,
    #[cfg(feature = "database")]
    executor: Option<crate::dsl_v2::DslExecutor>,
    /// Session contexts keyed by session ID
    sessions: HashMap<Uuid, SessionContext>,
}

/// Session context for multi-turn conversations
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    /// Conversation history
    pub turns: Vec<ConversationTurn>,
    /// Active CBU context
    pub active_cbu: Option<String>,
    /// Entity bindings from previous turns
    pub bindings: HashMap<String, String>,
    /// Session-level entities (for coreference)
    pub session_entities: HashMap<String, String>,
    /// Pending confirmation request
    pub pending_confirmation: Option<PendingConfirmation>,
}

/// A single conversation turn
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    pub user_message: String,
    pub classification: ClassificationResult,
    pub entities: ExtractedEntities,
    pub generated_dsl: Option<String>,
    pub execution_result: Option<TurnExecutionResult>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Execution result for a turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnExecutionResult {
    pub success: bool,
    pub bindings: Vec<(String, String)>,
    pub error: Option<String>,
}

/// Pending confirmation for high-impact operations
#[derive(Debug, Clone)]
pub struct PendingConfirmation {
    pub dsl: String,
    pub description: String,
    pub requires_explicit_yes: bool,
}

/// Response from the agent pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub response_type: ResponseType,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<TurnExecutionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification: Option<ClarificationRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
}

/// Type of agent response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseType {
    /// DSL was executed successfully
    Execution,
    /// Need clarification from user
    Clarification,
    /// Asking for confirmation before execution
    Confirmation,
    /// Errors occurred
    Error,
    /// Informational response (no action taken)
    Info,
}

/// Request for clarification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationRequest {
    pub question: String,
    pub options: Vec<String>,
    pub context: String,
}

/// Flow action determined by the pipeline
#[derive(Debug, Clone)]
pub enum FlowAction {
    /// Execute DSL immediately
    Execute(String),
    /// Ask for clarification
    Clarify(ClarificationRequest),
    /// Ask for entity disambiguation
    ClarifyEntity(ClarificationRequest),
    /// Ask for confirmation
    Confirm(PendingConfirmation),
    /// Report validation errors
    ReportErrors(Vec<String>),
}

/// Pipeline error
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Classification error: {0}")]
    Classification(String),
    #[error("Extraction error: {0}")]
    Extraction(String),
    #[error("Generation error: {0}")]
    Generation(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),
}

impl AgentPipeline {
    /// Create a new pipeline from configuration directory
    pub fn from_config_dir(config_dir: &Path) -> Result<Self, PipelineError> {
        // Load taxonomy
        let taxonomy_path = config_dir.join("intent_taxonomy.yaml");
        let taxonomy = IntentTaxonomy::load_from_file(&taxonomy_path)
            .map_err(|e| PipelineError::Config(format!("Failed to load taxonomy: {}", e)))?;

        // Load entity types
        let entity_types_path = config_dir.join("entity_types.yaml");
        let entity_types = EntityTypesConfig::load_from_file(&entity_types_path)
            .map_err(|e| PipelineError::Config(format!("Failed to load entity types: {}", e)))?;

        // Load market regions
        let market_regions_path = config_dir.join("market_regions.yaml");
        let market_regions = MarketRegionsConfig::load_from_file(&market_regions_path)
            .map_err(|e| PipelineError::Config(format!("Failed to load market regions: {}", e)))?;

        // Load instrument hierarchy
        let instrument_hierarchy_path = config_dir.join("instrument_hierarchy.yaml");
        let instrument_hierarchy =
            InstrumentHierarchyConfig::load_from_file(&instrument_hierarchy_path).map_err(|e| {
                PipelineError::Config(format!("Failed to load instrument hierarchy: {}", e))
            })?;

        // Load parameter mappings for DSL generator
        let mappings_path = config_dir.join("parameter_mappings.yaml");
        let dsl_generator = DslGenerator::from_file(&mappings_path).map_err(|e| {
            PipelineError::Config(format!("Failed to load parameter mappings: {}", e))
        })?;

        Ok(Self {
            intent_classifier: IntentClassifier::new(taxonomy),
            entity_extractor: EntityExtractor::new(
                entity_types,
                market_regions,
                instrument_hierarchy,
            ),
            dsl_generator,
            validator: AgentValidator::new()
                .map_err(|e| PipelineError::Config(format!("Failed to create validator: {}", e)))?,
            #[cfg(feature = "database")]
            executor: None,
            sessions: HashMap::new(),
        })
    }

    /// Create pipeline with database executor
    #[cfg(feature = "database")]
    pub fn with_executor(mut self, pool: sqlx::PgPool) -> Self {
        self.executor = Some(crate::dsl_v2::DslExecutor::new(pool));
        self
    }

    /// Process a user message through the complete pipeline
    pub async fn process(
        &mut self,
        message: &str,
        session_id: Uuid,
    ) -> Result<AgentResponse, PipelineError> {
        // Get or create session context
        let context = self.get_or_create_session(session_id);

        // Build conversation context for classifier
        let conv_context = self.build_conversation_context(&context);

        // Step 1: Classify intent
        let classification = self.intent_classifier.classify(message, &conv_context);

        // Step 2: Extract entities
        let entities = self.entity_extractor.extract(message, &conv_context);

        // Step 3: Determine flow action
        let action = self.determine_action(&classification, &entities, &context);

        // Step 4: Execute action and build response
        let response = self
            .execute_action(action, &classification, &entities, session_id)
            .await?;

        // Step 5: Record turn in session
        self.record_turn(session_id, message, &classification, &entities, &response);

        Ok(response)
    }

    /// Handle confirmation response
    pub async fn handle_confirmation(
        &mut self,
        session_id: Uuid,
        confirmed: bool,
    ) -> Result<AgentResponse, PipelineError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or(PipelineError::SessionNotFound(session_id))?;

        let pending = session
            .pending_confirmation
            .take()
            .ok_or_else(|| PipelineError::Execution("No pending confirmation".to_string()))?;

        if confirmed {
            // Execute the pending DSL
            self.execute_dsl(&pending.dsl, session_id).await
        } else {
            Ok(AgentResponse {
                response_type: ResponseType::Info,
                message: "Operation cancelled.".to_string(),
                dsl: None,
                execution: None,
                clarification: None,
                suggestions: None,
            })
        }
    }

    /// Handle clarification response
    pub async fn handle_clarification(
        &mut self,
        message: &str,
        session_id: Uuid,
    ) -> Result<AgentResponse, PipelineError> {
        // Just process as a normal message - the context will help
        self.process(message, session_id).await
    }

    /// Get or create a session context
    fn get_or_create_session(&mut self, session_id: Uuid) -> SessionContext {
        self.sessions.entry(session_id).or_default().clone()
    }

    /// Build conversation context from session
    fn build_conversation_context(&self, session: &SessionContext) -> ConversationContext {
        // Set last intent from most recent turn
        let last_intent = session
            .turns
            .last()
            .and_then(|turn| turn.classification.intents.first())
            .map(|intent| intent.intent_id.clone());

        ConversationContext {
            session_entities: session.session_entities.clone(),
            known_entities: session.bindings.clone(),
            last_intent,
            ..Default::default()
        }
    }

    /// Determine the appropriate flow action
    fn determine_action(
        &self,
        classification: &ClassificationResult,
        entities: &ExtractedEntities,
        session: &SessionContext,
    ) -> FlowAction {
        // Check if we have a clear intent with high confidence
        if classification.intents.is_empty() {
            return FlowAction::Clarify(ClarificationRequest {
                question: "I'm not sure what you'd like to do. Could you please clarify?"
                    .to_string(),
                options: vec![
                    "Assign an investment manager".to_string(),
                    "Configure pricing".to_string(),
                    "Set up cash sweep".to_string(),
                    "Define SLA".to_string(),
                ],
                context: "No clear intent detected".to_string(),
            });
        }

        let best_intent = &classification.intents[0];

        // Check confidence thresholds
        if best_intent.confidence < 0.45 {
            return FlowAction::Clarify(ClarificationRequest {
                question: format!(
                    "Did you mean to {}?",
                    best_intent.intent_id.replace('_', " ")
                ),
                options: classification
                    .intents
                    .iter()
                    .take(3)
                    .map(|i| i.intent_id.replace('_', " "))
                    .collect(),
                context: "Low confidence classification".to_string(),
            });
        }

        // Check for missing required entities
        // For now, we proceed if we have any entities
        if entities.is_empty() && best_intent.confidence < 0.85 {
            return FlowAction::Clarify(ClarificationRequest {
                question: "I need more details. Could you specify the entities involved?"
                    .to_string(),
                options: vec![],
                context: "Missing required entities".to_string(),
            });
        }

        // Build generation context
        let gen_context = GenerationContext {
            cbu_id: session.active_cbu.clone(),
            profile_id: None,
            available_symbols: session.bindings.keys().cloned().collect(),
            created_entities: session.bindings.clone(),
        };

        // Generate DSL
        let generated = self.dsl_generator.generate(
            &classification.intents,
            entities,
            &self.build_conversation_context(session),
            &gen_context,
        );

        if generated.statements.is_empty() {
            return FlowAction::ReportErrors(vec![
                "Could not generate DSL for this request".to_string()
            ]);
        }

        // Validate generated DSL
        let validation = self.validator.validate(&generated.to_dsl_string());

        if !validation.is_valid {
            // Check if errors are recoverable
            let errors: Vec<String> = validation
                .errors
                .iter()
                .map(|e| e.message.clone())
                .collect();

            return FlowAction::ReportErrors(errors);
        }

        // Check if confirmation is required (high-impact operations)
        let needs_confirmation = classification.intents.iter().any(|i| {
            i.confirmation_required
                || i.intent_id.contains("delete")
                || i.intent_id.contains("remove")
        });

        if needs_confirmation && best_intent.confidence < 0.95 {
            return FlowAction::Confirm(PendingConfirmation {
                dsl: generated.to_dsl_string(),
                description: format!(
                    "Execute {} operation?",
                    best_intent.intent_id.replace('_', " ")
                ),
                requires_explicit_yes: true,
            });
        }

        // Ready to execute
        FlowAction::Execute(generated.to_dsl_string())
    }

    /// Execute a flow action and return response
    async fn execute_action(
        &mut self,
        action: FlowAction,
        _classification: &ClassificationResult,
        _entities: &ExtractedEntities,
        session_id: Uuid,
    ) -> Result<AgentResponse, PipelineError> {
        match action {
            FlowAction::Execute(dsl) => self.execute_dsl(&dsl, session_id).await,

            FlowAction::Clarify(request) => Ok(AgentResponse {
                response_type: ResponseType::Clarification,
                message: request.question.clone(),
                dsl: None,
                execution: None,
                clarification: Some(request),
                suggestions: None,
            }),

            FlowAction::ClarifyEntity(request) => Ok(AgentResponse {
                response_type: ResponseType::Clarification,
                message: request.question.clone(),
                dsl: None,
                execution: None,
                clarification: Some(request),
                suggestions: None,
            }),

            FlowAction::Confirm(pending) => {
                // Store pending confirmation
                if let Some(session) = self.sessions.get_mut(&session_id) {
                    session.pending_confirmation = Some(pending.clone());
                }

                Ok(AgentResponse {
                    response_type: ResponseType::Confirmation,
                    message: pending.description.clone(),
                    dsl: Some(pending.dsl),
                    execution: None,
                    clarification: None,
                    suggestions: Some(vec!["yes".to_string(), "no".to_string()]),
                })
            }

            FlowAction::ReportErrors(errors) => Ok(AgentResponse {
                response_type: ResponseType::Error,
                message: errors.join("\n"),
                dsl: None,
                execution: None,
                clarification: None,
                suggestions: None,
            }),
        }
    }

    /// Execute DSL and return response
    async fn execute_dsl(
        &mut self,
        dsl: &str,
        session_id: Uuid,
    ) -> Result<AgentResponse, PipelineError> {
        #[cfg(feature = "database")]
        {
            if let Some(ref executor) = self.executor {
                match self.do_execute(executor, dsl, session_id).await {
                    Ok(result) => {
                        // Update session with new bindings
                        if let Some(session) = self.sessions.get_mut(&session_id) {
                            for (var, uuid) in &result.bindings {
                                session.bindings.insert(var.clone(), uuid.clone());
                            }
                        }

                        let message = if result.success {
                            format!(
                                "Executed successfully. Created {} bindings.",
                                result.bindings.len()
                            )
                        } else {
                            format!(
                                "Execution failed: {}",
                                result.error.as_deref().unwrap_or("Unknown error")
                            )
                        };

                        Ok(AgentResponse {
                            response_type: if result.success {
                                ResponseType::Execution
                            } else {
                                ResponseType::Error
                            },
                            message,
                            dsl: Some(dsl.to_string()),
                            execution: Some(result),
                            clarification: None,
                            suggestions: None,
                        })
                    }
                    Err(e) => Ok(AgentResponse {
                        response_type: ResponseType::Error,
                        message: format!("Execution error: {}", e),
                        dsl: Some(dsl.to_string()),
                        execution: None,
                        clarification: None,
                        suggestions: None,
                    }),
                }
            } else {
                // No executor - return DSL without execution
                Ok(AgentResponse {
                    response_type: ResponseType::Info,
                    message: "Generated DSL (execution not configured)".to_string(),
                    dsl: Some(dsl.to_string()),
                    execution: None,
                    clarification: None,
                    suggestions: None,
                })
            }
        }

        #[cfg(not(feature = "database"))]
        {
            let _ = session_id; // suppress unused warning
            Ok(AgentResponse {
                response_type: ResponseType::Info,
                message: "Generated DSL (database feature not enabled)".to_string(),
                dsl: Some(dsl.to_string()),
                execution: None,
                clarification: None,
                suggestions: None,
            })
        }
    }

    /// Execute DSL against the database using DslExecutor
    #[cfg(feature = "database")]
    async fn do_execute(
        &self,
        executor: &crate::dsl_v2::DslExecutor,
        dsl: &str,
        _session_id: Uuid,
    ) -> Result<TurnExecutionResult, PipelineError> {
        // Create execution context
        let mut ctx = ExecutionContext::new();

        // Execute the DSL - this parses, enriches, compiles, and executes
        match executor.execute_dsl(dsl, &mut ctx).await {
            Ok(results) => {
                // Extract bindings from execution context
                let bindings: Vec<(String, String)> = ctx
                    .symbols
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_string()))
                    .collect();

                // Check if any results indicate failure
                let all_success = !results.is_empty();

                Ok(TurnExecutionResult {
                    success: all_success,
                    bindings,
                    error: None,
                })
            }
            Err(e) => Ok(TurnExecutionResult {
                success: false,
                bindings: vec![],
                error: Some(e.to_string()),
            }),
        }
    }

    /// Record a conversation turn
    fn record_turn(
        &mut self,
        session_id: Uuid,
        message: &str,
        classification: &ClassificationResult,
        entities: &ExtractedEntities,
        response: &AgentResponse,
    ) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.turns.push(ConversationTurn {
                user_message: message.to_string(),
                classification: classification.clone(),
                entities: entities.clone(),
                generated_dsl: response.dsl.clone(),
                execution_result: response.execution.clone(),
                timestamp: chrono::Utc::now(),
            });

            // Update session entities for coreference
            for entity in entities.iter() {
                session
                    .session_entities
                    .insert(entity.entity_type.clone(), entity.value.clone());
            }
        }
    }

    /// Set active CBU for a session
    pub fn set_active_cbu(&mut self, session_id: Uuid, cbu_id: &str) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.active_cbu = Some(cbu_id.to_string());
            session
                .session_entities
                .insert("cbu_reference".to_string(), cbu_id.to_string());
        }
    }

    /// Clear a session
    pub fn clear_session(&mut self, session_id: Uuid) {
        self.sessions.remove(&session_id);
    }

    /// Get session context (for debugging/inspection)
    pub fn get_session(&self, session_id: Uuid) -> Option<&SessionContext> {
        self.sessions.get(&session_id)
    }
}

impl AgentResponse {
    /// Check if this is an execution response
    pub fn is_execution(&self) -> bool {
        self.response_type == ResponseType::Execution
    }

    /// Check if this is a clarification request
    pub fn is_clarification(&self) -> bool {
        self.response_type == ResponseType::Clarification
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        self.response_type == ResponseType::Error
    }

    /// Get executed DSL if available
    pub fn executed_dsl(&self) -> Option<&str> {
        if self.is_execution() {
            self.dsl.as_deref()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require config files
    // These are unit tests for the response types

    #[test]
    fn test_agent_response_execution() {
        let response = AgentResponse {
            response_type: ResponseType::Execution,
            message: "Success".to_string(),
            dsl: Some("(test.verb :arg value)".to_string()),
            execution: Some(TurnExecutionResult {
                success: true,
                bindings: vec![],
                error: None,
            }),
            clarification: None,
            suggestions: None,
        };

        assert!(response.is_execution());
        assert!(!response.is_clarification());
        assert_eq!(response.executed_dsl(), Some("(test.verb :arg value)"));
    }

    #[test]
    fn test_agent_response_clarification() {
        let response = AgentResponse {
            response_type: ResponseType::Clarification,
            message: "What did you mean?".to_string(),
            dsl: None,
            execution: None,
            clarification: Some(ClarificationRequest {
                question: "What did you mean?".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
                context: "test".to_string(),
            }),
            suggestions: None,
        };

        assert!(response.is_clarification());
        assert!(!response.is_execution());
        assert!(response.executed_dsl().is_none());
    }

    #[test]
    fn test_session_context_default() {
        let session = SessionContext::default();
        assert!(session.turns.is_empty());
        assert!(session.active_cbu.is_none());
        assert!(session.bindings.is_empty());
    }
}
