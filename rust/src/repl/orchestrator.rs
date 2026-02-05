//! REPL Orchestrator
//!
//! Main state machine for the REPL. Single entry point for all user interactions.
//!
//! ## Design Principles
//!
//! 1. **Explicit State Machine**: All states and transitions are visible here
//! 2. **Single Entry Point**: `process(session_id, input)` handles everything
//! 3. **Ledger Logging**: Every interaction logged before state transition
//! 4. **Pure Intent Matching**: IntentMatcher is called with no side effects
//! 5. **Persistent Sessions**: Sessions survive page reloads

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::intent_matcher::IntentMatcher;
use super::response::ReplResponse;
use super::session::ReplSession;
use super::types::{
    ClarifyingKind, ClarifyingState, ClientGroupOption, EntryStatus, LedgerEntry,
    LedgerExecutionResult, MatchContext, MatchOutcome, ReplCommand, ReplState, UserInput,
};

// ============================================================================
// ReplOrchestrator
// ============================================================================

/// Main REPL state machine coordinator
///
/// Handles all user interactions through a single entry point.
/// Maintains session state and coordinates with services.
pub struct ReplOrchestrator {
    /// Intent matching service (stateless, no side effects)
    intent_matcher: Arc<dyn IntentMatcher>,

    /// DSL executor (for "run" command)
    executor: Option<Arc<dyn DslExecutor>>,

    /// Session store (in-memory for now, can be backed by DB)
    sessions: Arc<RwLock<HashMap<Uuid, ReplSession>>>,

    /// Client group provider (for prompting user to select)
    client_group_provider: Option<Arc<dyn ClientGroupProvider>>,
}

impl ReplOrchestrator {
    /// Create a new orchestrator with required dependencies
    pub fn new(intent_matcher: Arc<dyn IntentMatcher>) -> Self {
        Self {
            intent_matcher,
            executor: None,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            client_group_provider: None,
        }
    }

    /// Add DSL executor
    pub fn with_executor(mut self, executor: Arc<dyn DslExecutor>) -> Self {
        self.executor = Some(executor);
        self
    }

    /// Add client group provider
    pub fn with_client_group_provider(mut self, provider: Arc<dyn ClientGroupProvider>) -> Self {
        self.client_group_provider = Some(provider);
        self
    }

    /// Use an existing session store
    pub fn with_sessions(mut self, sessions: Arc<RwLock<HashMap<Uuid, ReplSession>>>) -> Self {
        self.sessions = sessions;
        self
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Single entry point - handles ALL user interactions
    ///
    /// This is the main API for the REPL. All inputs flow through here.
    pub async fn process(&self, session_id: Uuid, input: UserInput) -> Result<ReplResponse> {
        // Get or create session
        let mut session = self.get_or_create_session(session_id).await;

        // State machine dispatch
        let response = match (&session.state.clone(), &input) {
            // ================================================================
            // IDLE state transitions
            // ================================================================
            (ReplState::Idle, UserInput::Message { content }) => {
                self.handle_message(&mut session, content).await?
            }
            (ReplState::Idle, UserInput::Command { command }) => {
                self.handle_command(&mut session, *command).await?
            }

            // ================================================================
            // CLARIFYING state transitions
            // ================================================================
            (
                ReplState::Clarifying(ClarifyingState::VerbSelection { .. }),
                UserInput::VerbSelection { .. },
            ) => self.handle_verb_selection(&mut session, &input).await?,

            (
                ReplState::Clarifying(ClarifyingState::ScopeSelection { .. }),
                UserInput::ScopeSelection { .. },
            ) => self.handle_scope_selection(&mut session, &input).await?,

            (
                ReplState::Clarifying(ClarifyingState::EntityResolution { .. }),
                UserInput::EntitySelection { .. },
            ) => self.handle_entity_selection(&mut session, &input).await?,

            (
                ReplState::Clarifying(ClarifyingState::Confirmation { .. }),
                UserInput::Confirmation { confirmed },
            ) => self.handle_confirmation(&mut session, *confirmed).await?,

            (
                ReplState::Clarifying(ClarifyingState::IntentTier { .. }),
                UserInput::IntentTierSelection { .. },
            ) => {
                self.handle_intent_tier_selection(&mut session, &input)
                    .await?
            }

            (
                ReplState::Clarifying(ClarifyingState::ClientGroupSelection { .. }),
                UserInput::ClientGroupSelection {
                    group_id,
                    group_name,
                },
            ) => {
                self.handle_client_group_selection(&mut session, *group_id, group_name.clone())
                    .await?
            }

            // Clarifying state but user sends new message - abandon clarification
            (ReplState::Clarifying(_), UserInput::Message { content }) => {
                session.supersede_pending();
                session.transition_to_idle();
                self.handle_message(&mut session, content).await?
            }

            // Cancel clarification
            (
                ReplState::Clarifying(_),
                UserInput::Command {
                    command: ReplCommand::Cancel,
                },
            ) => {
                session.supersede_pending();
                session.transition_to_idle();
                ReplResponse::ack(ReplState::Idle, "Cancelled")
            }

            // ================================================================
            // DSL_READY state transitions
            // ================================================================
            (
                ReplState::DslReady { .. },
                UserInput::Command {
                    command: ReplCommand::Run,
                },
            ) => self.execute_dsl(&mut session).await?,

            // New message while DSL pending - discard and restart
            (ReplState::DslReady { .. }, UserInput::Message { content }) => {
                session.supersede_pending();
                session.transition_to_idle();
                self.handle_message(&mut session, content).await?
            }

            // Cancel pending DSL
            (
                ReplState::DslReady { .. },
                UserInput::Command {
                    command: ReplCommand::Cancel,
                },
            ) => {
                session.supersede_pending();
                session.transition_to_idle();
                ReplResponse::ack(ReplState::Idle, "Cancelled")
            }

            // ================================================================
            // Invalid transitions
            // ================================================================
            _ => ReplResponse::error(
                format!(
                    "Invalid input for current state. State: {:?}, Input type: {}",
                    session.state,
                    input_type_name(&input)
                ),
                true,
            ),
        };

        // Persist session
        self.save_session(&session).await;

        Ok(response)
    }

    /// Get session by ID (returns None if not found)
    pub async fn get_session(&self, session_id: Uuid) -> Option<ReplSession> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).cloned()
    }

    /// Create a new session
    pub async fn create_session(&self) -> ReplSession {
        let session = ReplSession::new();
        let id = session.id;
        let mut sessions = self.sessions.write().await;
        sessions.insert(id, session.clone());
        session
    }

    // ========================================================================
    // Internal Handlers
    // ========================================================================

    /// Get or create a session
    async fn get_or_create_session(&self, session_id: Uuid) -> ReplSession {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            return session.clone();
        }
        drop(sessions);

        // Create new session
        let session = ReplSession::with_id(session_id);
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session.clone());
        session
    }

    /// Save session to store
    async fn save_session(&self, session: &ReplSession) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, session.clone());
    }

    /// Handle natural language message
    async fn handle_message(
        &self,
        session: &mut ReplSession,
        message: &str,
    ) -> Result<ReplResponse> {
        // Check if session needs client group selection first
        if session.needs_client_group() {
            if let Some(provider) = &self.client_group_provider {
                let groups = provider.get_available_groups(session.user_id).await?;
                if !groups.is_empty() {
                    // Log the attempt
                    let entry = LedgerEntry::new(UserInput::message(message.to_string()))
                        .with_status(EntryStatus::Clarifying {
                            kind: ClarifyingKind::ClientGroupSelection,
                        });
                    session.add_entry(entry);

                    // Prompt for client group
                    session.transition_to_clarifying(ClarifyingState::ClientGroupSelection {
                        options: groups.clone(),
                        prompt: "Please select a client group to work with:".to_string(),
                    });

                    return Ok(ReplResponse::client_group_selection(
                        groups,
                        "Please select a client group to work with:".to_string(),
                    ));
                }
            }
        }

        // Build match context from session
        let context = MatchContext {
            client_group_id: session.client_group_id,
            client_group_name: session.client_group_name.clone(),
            scope: session.scope.clone(),
            dominant_entity_id: session.dominant_entity_id,
            user_id: session.user_id,
            domain_hint: session.domain_hint.clone(),
            bindings: session
                .bindings()
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        };

        // Call intent matcher (pure, no side effects)
        let result = self.intent_matcher.match_intent(message, &context).await?;

        // Create ledger entry BEFORE state transition
        let mut entry = LedgerEntry::new(UserInput::message(message.to_string()))
            .with_intent_result(result.clone());

        // Transition based on outcome
        let response = match &result.outcome {
            MatchOutcome::Matched { verb, confidence } => {
                if let Some(dsl) = &result.generated_dsl {
                    let can_auto = self.can_auto_execute(verb);
                    entry.status = EntryStatus::Ready;
                    session.add_entry(entry);

                    session.transition_to_dsl_ready(dsl.clone(), verb.clone(), can_auto);

                    if can_auto {
                        // Auto-execute navigation verbs
                        return self.execute_dsl(session).await;
                    }

                    ReplResponse::dsl_ready(
                        dsl.clone(),
                        verb.clone(),
                        format!(
                            "Ready to execute {} (confidence: {:.0}%)",
                            verb,
                            confidence * 100.0
                        ),
                        can_auto,
                    )
                } else {
                    entry.status = EntryStatus::Failed {
                        error: "Verb matched but DSL generation failed".to_string(),
                    };
                    session.add_entry(entry);
                    session.transition_to_idle();
                    ReplResponse::error("Verb matched but I couldn't generate the command.", true)
                }
            }

            MatchOutcome::Ambiguous { margin } => {
                entry.status = EntryStatus::Clarifying {
                    kind: ClarifyingKind::VerbSelection,
                };
                session.add_entry(entry);

                session.transition_to_clarifying(ClarifyingState::VerbSelection {
                    options: result.verb_candidates.clone(),
                    original_input: message.to_string(),
                    margin: *margin,
                });

                ReplResponse::verb_disambiguation(
                    result.verb_candidates,
                    message.to_string(),
                    *margin,
                )
            }

            MatchOutcome::NeedsScopeSelection => {
                let options = result.scope_candidates.clone().unwrap_or_default();
                entry.status = EntryStatus::Clarifying {
                    kind: ClarifyingKind::ScopeSelection,
                };
                session.add_entry(entry);

                session.transition_to_clarifying(ClarifyingState::ScopeSelection {
                    options: options.clone(),
                    original_input: message.to_string(),
                });

                ReplResponse::scope_selection(options, message.to_string())
            }

            MatchOutcome::NeedsEntityResolution => {
                entry.status = EntryStatus::Clarifying {
                    kind: ClarifyingKind::EntityResolution,
                };
                session.add_entry(entry);

                let partial_dsl = result.generated_dsl.clone().unwrap_or_default();
                session.transition_to_clarifying(ClarifyingState::EntityResolution {
                    unresolved_refs: result.unresolved_refs.clone(),
                    partial_dsl: partial_dsl.clone(),
                });

                ReplResponse::entity_resolution(result.unresolved_refs, partial_dsl)
            }

            MatchOutcome::NeedsClientGroup { options } => {
                entry.status = EntryStatus::Clarifying {
                    kind: ClarifyingKind::ClientGroupSelection,
                };
                session.add_entry(entry);

                session.transition_to_clarifying(ClarifyingState::ClientGroupSelection {
                    options: options.clone(),
                    prompt: "Please select a client group:".to_string(),
                });

                ReplResponse::client_group_selection(
                    options.clone(),
                    "Please select a client group:".to_string(),
                )
            }

            MatchOutcome::NeedsIntentTier { options } => {
                entry.status = EntryStatus::Clarifying {
                    kind: ClarifyingKind::IntentTier,
                };
                session.add_entry(entry);

                session.transition_to_clarifying(ClarifyingState::IntentTier {
                    tier_number: 1,
                    options: options.clone(),
                    original_input: message.to_string(),
                });

                ReplResponse::intent_tier_selection(
                    1,
                    options.clone(),
                    message.to_string(),
                    "What are you trying to do?".to_string(),
                )
            }

            MatchOutcome::NoMatch { reason } => {
                entry.status = EntryStatus::Failed {
                    error: reason.clone(),
                };
                session.add_entry(entry);
                session.transition_to_idle();
                ReplResponse::no_match(reason.clone(), vec![])
            }

            MatchOutcome::DirectDsl { source } => {
                entry.dsl = Some(source.clone());
                entry.status = EntryStatus::Ready;
                session.add_entry(entry);

                session.transition_to_dsl_ready(source.clone(), "direct".to_string(), false);

                ReplResponse::dsl_ready(
                    source.clone(),
                    "direct".to_string(),
                    "Direct DSL input ready for execution".to_string(),
                    false,
                )
            }
        };

        Ok(response)
    }

    /// Handle REPL command
    async fn handle_command(
        &self,
        session: &mut ReplSession,
        command: ReplCommand,
    ) -> Result<ReplResponse> {
        match command {
            ReplCommand::Run => {
                // Nothing to run in idle state
                Ok(ReplResponse::error("No command pending to run", true))
            }
            ReplCommand::Undo => {
                // TODO: Implement undo
                Ok(ReplResponse::ack(
                    session.state.clone(),
                    "Undo not yet implemented",
                ))
            }
            ReplCommand::Redo => {
                // TODO: Implement redo
                Ok(ReplResponse::ack(
                    session.state.clone(),
                    "Redo not yet implemented",
                ))
            }
            ReplCommand::Clear => {
                session.ledger.clear();
                session.derived = Default::default();
                session.transition_to_idle();
                Ok(ReplResponse::ack(ReplState::Idle, "Session cleared"))
            }
            ReplCommand::Cancel => {
                // Nothing to cancel in idle state
                Ok(ReplResponse::ack(ReplState::Idle, "Nothing to cancel"))
            }
            ReplCommand::Info => {
                let info = format!(
                    "Session: {}\nEntries: {}\nCBUs in scope: {}\nBindings: {}\nState: {:?}",
                    session.id,
                    session.entry_count(),
                    session.cbu_ids().len(),
                    session.bindings().len(),
                    session.state
                );
                Ok(ReplResponse::ack(session.state.clone(), info))
            }
            ReplCommand::Help => {
                let help = "Available commands:\n\
                    - Type a message to interact with the system\n\
                    - 'run' to execute pending DSL\n\
                    - 'undo' to undo the last action\n\
                    - 'redo' to redo an undone action\n\
                    - 'clear' to reset the session\n\
                    - 'cancel' to cancel current operation\n\
                    - 'info' to show session info";
                Ok(ReplResponse::ack(session.state.clone(), help))
            }
        }
    }

    /// Handle verb selection from disambiguation
    async fn handle_verb_selection(
        &self,
        session: &mut ReplSession,
        input: &UserInput,
    ) -> Result<ReplResponse> {
        let UserInput::VerbSelection { selected_verb, .. } = input else {
            return Err(anyhow!("Expected VerbSelection input"));
        };

        // Log the selection
        let entry = LedgerEntry::new(input.clone()).with_status(EntryStatus::Draft);
        session.add_entry(entry);

        // Re-run intent matching with the selected verb as a hint
        // For now, just generate DSL with the selected verb
        // TODO: Call LLM to generate args for the selected verb

        let dsl = format!("({} )", selected_verb);
        session.transition_to_dsl_ready(dsl.clone(), selected_verb.clone(), false);

        Ok(ReplResponse::dsl_ready(
            dsl,
            selected_verb.clone(),
            format!("Selected: {}", selected_verb),
            false,
        ))
    }

    /// Handle scope selection
    async fn handle_scope_selection(
        &self,
        session: &mut ReplSession,
        input: &UserInput,
    ) -> Result<ReplResponse> {
        let UserInput::ScopeSelection { option_name, .. } = input else {
            return Err(anyhow!("Expected ScopeSelection input"));
        };

        // Log the selection
        let entry = LedgerEntry::new(input.clone()).with_status(EntryStatus::Draft);
        session.add_entry(entry);

        // TODO: Update session scope and re-run intent matching
        session.transition_to_idle();

        Ok(ReplResponse::ack(
            ReplState::Idle,
            format!("Selected scope: {}", option_name),
        ))
    }

    /// Handle entity selection for resolution
    async fn handle_entity_selection(
        &self,
        session: &mut ReplSession,
        input: &UserInput,
    ) -> Result<ReplResponse> {
        let UserInput::EntitySelection {
            ref_id,
            entity_name,
            ..
        } = input
        else {
            return Err(anyhow!("Expected EntitySelection input"));
        };

        // Log the selection
        let entry = LedgerEntry::new(input.clone()).with_status(EntryStatus::Draft);
        session.add_entry(entry);

        // TODO: Update DSL with resolved entity and check if more refs need resolution
        session.transition_to_idle();

        Ok(ReplResponse::ack(
            ReplState::Idle,
            format!("Resolved {} to {}", ref_id, entity_name),
        ))
    }

    /// Handle confirmation (yes/no)
    async fn handle_confirmation(
        &self,
        session: &mut ReplSession,
        confirmed: bool,
    ) -> Result<ReplResponse> {
        // Log the confirmation
        let entry =
            LedgerEntry::new(UserInput::Confirmation { confirmed }).with_status(EntryStatus::Draft);
        session.add_entry(entry);

        if confirmed {
            // Get DSL from clarifying state
            if let ReplState::Clarifying(ClarifyingState::Confirmation { dsl, verb, .. }) =
                &session.state
            {
                let dsl = dsl.clone();
                let verb = verb.clone();
                session.transition_to_dsl_ready(dsl.clone(), verb, false);
                return self.execute_dsl(session).await;
            }
        }

        session.transition_to_idle();
        Ok(ReplResponse::ack(ReplState::Idle, "Cancelled"))
    }

    /// Handle intent tier selection
    async fn handle_intent_tier_selection(
        &self,
        session: &mut ReplSession,
        input: &UserInput,
    ) -> Result<ReplResponse> {
        let UserInput::IntentTierSelection { tier, selected_id } = input else {
            return Err(anyhow!("Expected IntentTierSelection input"));
        };

        // Log the selection
        let entry = LedgerEntry::new(input.clone()).with_status(EntryStatus::Draft);
        session.add_entry(entry);

        // TODO: Filter verbs by selected tier and re-show disambiguation
        session.transition_to_idle();

        Ok(ReplResponse::ack(
            ReplState::Idle,
            format!("Selected tier {} option: {}", tier, selected_id),
        ))
    }

    /// Handle client group selection
    async fn handle_client_group_selection(
        &self,
        session: &mut ReplSession,
        group_id: Uuid,
        group_name: String,
    ) -> Result<ReplResponse> {
        // Log the selection
        let entry = LedgerEntry::new(UserInput::ClientGroupSelection {
            group_id,
            group_name: group_name.clone(),
        })
        .with_status(EntryStatus::Executed);
        session.add_entry(entry);

        // Update session context
        session.client_group_id = Some(group_id);
        session.client_group_name = Some(group_name.clone());
        session.transition_to_idle();

        Ok(ReplResponse::ack(
            ReplState::Idle,
            format!("Working with: {}", group_name),
        ))
    }

    /// Execute the pending DSL
    async fn execute_dsl(&self, session: &mut ReplSession) -> Result<ReplResponse> {
        let (dsl, verb) = match &session.state {
            ReplState::DslReady { dsl, verb, .. } => (dsl.clone(), verb.clone()),
            _ => return Err(anyhow!("No DSL ready to execute")),
        };

        // Transition to executing
        session.transition_to_executing(dsl.clone());

        // Execute via executor (if available)
        if let Some(executor) = &self.executor {
            match executor.execute(&dsl, session).await {
                Ok(result) => {
                    session.update_last_execution(result.clone());
                    session.transition_to_idle();

                    Ok(ReplResponse::executed(
                        dsl,
                        result.message.clone(),
                        result.affected_cbu_ids.clone(),
                        result.bindings.clone(),
                    ))
                }
                Err(e) => {
                    session.mark_last_failed(e.to_string());
                    session.transition_to_idle();
                    Ok(ReplResponse::error(
                        format!("Execution failed: {}", e),
                        true,
                    ))
                }
            }
        } else {
            // No executor - just acknowledge
            let result = LedgerExecutionResult {
                message: format!("Executed: {}", verb),
                affected_cbu_ids: vec![],
                bindings: vec![],
                view_state_update: None,
                duration_ms: Some(0),
            };
            session.update_last_execution(result);
            session.transition_to_idle();

            Ok(ReplResponse::executed(
                dsl,
                format!("Executed: {} (no executor configured)", verb),
                vec![],
                vec![],
            ))
        }
    }

    /// Check if verb should auto-execute
    fn can_auto_execute(&self, verb: &str) -> bool {
        matches!(
            verb,
            "session.load-galaxy"
                | "session.load-cbu"
                | "session.load-jurisdiction"
                | "session.unload-cbu"
                | "session.clear"
                | "session.undo"
                | "session.redo"
                | "session.info"
                | "view.drill"
                | "view.surface"
                | "view.universe"
                | "view.cbu"
        )
    }
}

// ============================================================================
// Supporting Traits
// ============================================================================

/// DSL executor trait
#[async_trait::async_trait]
pub trait DslExecutor: Send + Sync {
    /// Execute DSL and return result
    async fn execute(&self, dsl: &str, session: &ReplSession) -> Result<LedgerExecutionResult>;
}

/// Client group provider trait
#[async_trait::async_trait]
pub trait ClientGroupProvider: Send + Sync {
    /// Get available client groups for user
    async fn get_available_groups(&self, user_id: Option<Uuid>) -> Result<Vec<ClientGroupOption>>;
}

// ============================================================================
// Helpers
// ============================================================================

fn input_type_name(input: &UserInput) -> &'static str {
    match input {
        UserInput::Message { .. } => "message",
        UserInput::VerbSelection { .. } => "verb_selection",
        UserInput::ScopeSelection { .. } => "scope_selection",
        UserInput::EntitySelection { .. } => "entity_selection",
        UserInput::Confirmation { .. } => "confirmation",
        UserInput::IntentTierSelection { .. } => "intent_tier_selection",
        UserInput::ClientGroupSelection { .. } => "client_group_selection",
        UserInput::Command { .. } => "command",
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::verb_search::HybridVerbSearcher;
    use crate::repl::intent_matcher::HybridIntentMatcher;

    fn make_orchestrator() -> ReplOrchestrator {
        let verb_searcher = Arc::new(HybridVerbSearcher::minimal());
        let intent_matcher = Arc::new(HybridIntentMatcher::new(verb_searcher));
        ReplOrchestrator::new(intent_matcher)
    }

    #[tokio::test]
    async fn test_create_session() {
        let orchestrator = make_orchestrator();
        let session = orchestrator.create_session().await;

        assert!(matches!(session.state, ReplState::Idle));
        assert!(session.ledger.is_empty());
    }

    #[tokio::test]
    async fn test_get_or_create_session() {
        let orchestrator = make_orchestrator();
        let session_id = Uuid::new_v4();

        // First call creates
        let session1 = orchestrator.get_or_create_session(session_id).await;
        assert_eq!(session1.id, session_id);

        // Second call retrieves
        let session2 = orchestrator.get_or_create_session(session_id).await;
        assert_eq!(session2.id, session_id);
    }

    #[tokio::test]
    async fn test_handle_clear_command() {
        let orchestrator = make_orchestrator();
        let session = orchestrator.create_session().await;

        let response = orchestrator
            .process(session.id, UserInput::command(ReplCommand::Clear))
            .await
            .unwrap();

        assert!(matches!(response.state, ReplState::Idle));
        assert!(response.message.contains("cleared"));
    }

    #[tokio::test]
    async fn test_handle_info_command() {
        let orchestrator = make_orchestrator();
        let session = orchestrator.create_session().await;

        let response = orchestrator
            .process(session.id, UserInput::command(ReplCommand::Info))
            .await
            .unwrap();

        assert!(response.message.contains("Session:"));
    }

    #[tokio::test]
    async fn test_handle_help_command() {
        let orchestrator = make_orchestrator();
        let session = orchestrator.create_session().await;

        let response = orchestrator
            .process(session.id, UserInput::command(ReplCommand::Help))
            .await
            .unwrap();

        assert!(response.message.contains("Available commands"));
    }

    #[tokio::test]
    async fn test_direct_dsl_input() {
        let orchestrator = make_orchestrator();
        let session = orchestrator.create_session().await;

        // Set client group to avoid prompting
        {
            let mut sessions = orchestrator.sessions.write().await;
            if let Some(s) = sessions.get_mut(&session.id) {
                s.client_group_id = Some(Uuid::new_v4());
                s.client_group_name = Some("Test".to_string());
            }
        }

        let response = orchestrator
            .process(
                session.id,
                UserInput::message("(cbu.create :name \"test\")"),
            )
            .await
            .unwrap();

        // Should recognize as direct DSL and go to DslReady
        assert!(matches!(response.state, ReplState::DslReady { .. }));
    }
}
