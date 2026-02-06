//! Orchestrator V2 — Pack-Guided State Machine
//!
//! The heart of the v2 REPL. Dispatches `UserInputV2` against the current
//! `ReplStateV2` and produces `ReplResponseV2`.
//!
//! # State Machine Dispatch
//!
//! | Current State       | Input           | Handler                | Next State              |
//! |---------------------|-----------------|------------------------|-------------------------|
//! | ScopeGate           | Message         | try_resolve_scope()    | JourneySelection or ScopeGate |
//! | ScopeGate           | SelectScope     | set_scope()            | JourneySelection        |
//! | JourneySelection    | Message         | route_pack()           | InPack or JourneySelection |
//! | JourneySelection    | SelectPack      | activate_pack()        | InPack                  |
//! | InPack              | Message         | handle_in_pack_msg()   | SentencePlayback or InPack |
//! | InPack              | Command(Run)    | validate_and_execute() | Executing               |
//! | Clarifying          | Message/Select  | resolve_clarification()| SentencePlayback or Clarifying |
//! | SentencePlayback    | Confirm         | add_to_runbook()       | RunbookEditing or InPack |
//! | SentencePlayback    | Reject          | discard_proposal()     | InPack                  |
//! | RunbookEditing      | Command(Run)    | execute_runbook()      | Executing               |
//! | RunbookEditing      | Message         | handle_in_pack_msg()   | SentencePlayback        |
//! | Executing           | (completion)    | record_outcomes()      | RunbookEditing          |

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::intent_service::{ClarificationOutcome, IntentService, VerbMatchOutcome};
use super::proposal_engine::ProposalEngine;
use super::response_v2::{ChapterView, ReplResponseKindV2, ReplResponseV2, StepResult};
use super::runbook::{
    ArgExtractionAudit, ConfirmPolicy, EntryStatus, ExecutionMode, GateType, InvocationRecord,
    RunbookEntry, RunbookStatus,
};
use super::sentence_gen::SentenceGenerator;
use super::session_v2::{ClientContext, MessageRole, ReplSessionV2};
use super::types_v2::{ExecutionProgress, ReplCommandV2, ReplStateV2, UserInputV2};
use super::verb_config_index::VerbConfigIndex;
use crate::journey::playback::PackPlayback;
use crate::journey::router::{PackRouteOutcome, PackRouter};
use crate::journey::template::instantiate_template;
use crate::repl::intent_matcher::IntentMatcher;
use crate::repl::types::{MatchContext, MatchOutcome};

// ---------------------------------------------------------------------------
// DslExecutor trait (abstraction for stub/real execution)
// ---------------------------------------------------------------------------

/// Trait for DSL execution — allows stub execution in Phase 0.
#[async_trait::async_trait]
pub trait DslExecutor: Send + Sync {
    async fn execute(&self, dsl: &str) -> Result<serde_json::Value, String>;
}

/// Stub executor that returns success for all DSL.
pub struct StubExecutor;

#[async_trait::async_trait]
impl DslExecutor for StubExecutor {
    async fn execute(&self, _dsl: &str) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"status": "stub_success"}))
    }
}

// ---------------------------------------------------------------------------
// DslExecutorV2 trait (extended execution with parking signals)
// ---------------------------------------------------------------------------

/// Extended result from DSL execution that can signal parking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DslExecutionOutcome {
    /// Execution completed successfully.
    Completed(serde_json::Value),
    /// Execution needs to park — waiting for external signal.
    Parked {
        task_id: Uuid,
        correlation_key: String,
        timeout: Option<chrono::Duration>,
        message: String,
    },
    /// Execution failed.
    Failed(String),
}

/// Extended executor that can return parking signals.
#[async_trait::async_trait]
pub trait DslExecutorV2: Send + Sync {
    async fn execute_v2(&self, dsl: &str, entry_id: Uuid, runbook_id: Uuid) -> DslExecutionOutcome;
}

/// Adapts any DslExecutor to DslExecutorV2 (sync-only path: never parks).
#[async_trait::async_trait]
impl<T: DslExecutor> DslExecutorV2 for T {
    async fn execute_v2(
        &self,
        dsl: &str,
        _entry_id: Uuid,
        _runbook_id: Uuid,
    ) -> DslExecutionOutcome {
        match self.execute(dsl).await {
            Ok(v) => DslExecutionOutcome::Completed(v),
            Err(e) => DslExecutionOutcome::Failed(e),
        }
    }
}

/// Test executor that parks entries whose DSL contains ":park" or ":durable" markers.
pub struct ParkableStubExecutor;

#[async_trait::async_trait]
impl DslExecutorV2 for ParkableStubExecutor {
    async fn execute_v2(&self, dsl: &str, entry_id: Uuid, runbook_id: Uuid) -> DslExecutionOutcome {
        if dsl.contains(":park") || dsl.contains(":durable") {
            DslExecutionOutcome::Parked {
                task_id: Uuid::new_v4(),
                correlation_key: format!("{}:{}", runbook_id, entry_id),
                timeout: None,
                message: "Waiting for external completion".into(),
            }
        } else {
            DslExecutionOutcome::Completed(serde_json::json!({"status": "stub_success"}))
        }
    }
}

// ---------------------------------------------------------------------------
// ReplOrchestratorV2
// ---------------------------------------------------------------------------

/// The v2 REPL orchestrator — dispatches input against state machine.
pub struct ReplOrchestratorV2 {
    pack_router: PackRouter,
    sentence_gen: SentenceGenerator,
    verb_config_index: Arc<VerbConfigIndex>,
    intent_matcher: Option<Arc<dyn IntentMatcher>>,
    /// Phase 2: Unified pipeline facade (preferred over separate intent_matcher).
    intent_service: Option<Arc<IntentService>>,
    /// Phase 3: Proposal engine (preferred over direct match_verb_for_input).
    proposal_engine: Option<Arc<ProposalEngine>>,
    sessions: Arc<RwLock<HashMap<Uuid, ReplSessionV2>>>,
    executor: Arc<dyn DslExecutor>,
    executor_v2: Option<Arc<dyn DslExecutorV2>>,
    /// Phase 5: Session persistence for durable execution / human gates.
    #[cfg(feature = "database")]
    session_repository: Option<Arc<super::session_repository::SessionRepositoryV2>>,
}

impl ReplOrchestratorV2 {
    /// Create a new orchestrator.
    pub fn new(pack_router: PackRouter, executor: Arc<dyn DslExecutor>) -> Self {
        Self {
            pack_router,
            sentence_gen: SentenceGenerator,
            verb_config_index: Arc::new(VerbConfigIndex::empty()),
            intent_matcher: None,
            intent_service: None,
            proposal_engine: None,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            executor,
            executor_v2: None,
            #[cfg(feature = "database")]
            session_repository: None,
        }
    }

    /// Attach a VerbConfigIndex for sentence generation and confirm policies.
    pub fn with_verb_config_index(mut self, index: Arc<VerbConfigIndex>) -> Self {
        self.verb_config_index = index;
        self
    }

    /// Attach an IntentMatcher for real verb matching.
    pub fn with_intent_matcher(mut self, matcher: Arc<dyn IntentMatcher>) -> Self {
        self.intent_matcher = Some(matcher);
        self
    }

    /// Attach an IntentService (Phase 2 unified pipeline).
    ///
    /// When set, `match_verb_for_input()` uses IntentService instead of
    /// the separate IntentMatcher. IntentService provides clarification
    /// checking and sentence generation through a unified interface.
    pub fn with_intent_service(mut self, svc: Arc<IntentService>) -> Self {
        self.intent_service = Some(svc);
        self
    }

    /// Attach a ProposalEngine (Phase 3).
    ///
    /// When set, `handle_in_pack` and `handle_runbook_editing` use
    /// `propose_for_input()` instead of `match_verb_for_input()`.
    /// The proposal engine returns ranked alternatives with evidence.
    pub fn with_proposal_engine(mut self, engine: Arc<ProposalEngine>) -> Self {
        self.proposal_engine = Some(engine);
        self
    }

    /// Attach an extended executor that can signal parking.
    pub fn with_executor_v2(mut self, executor: Arc<dyn DslExecutorV2>) -> Self {
        self.executor_v2 = Some(executor);
        self
    }

    /// Attach a session repository for durable execution persistence.
    #[cfg(feature = "database")]
    pub fn with_session_repository(
        mut self,
        repo: Arc<super::session_repository::SessionRepositoryV2>,
    ) -> Self {
        self.session_repository = Some(repo);
        self
    }

    /// Access the pack router (useful for tests and introspection).
    pub fn pack_router(&self) -> &PackRouter {
        &self.pack_router
    }

    /// Insert a previously-persisted session into the in-memory map.
    ///
    /// Used during session recovery (GET session not found in memory → load from DB → restore).
    pub async fn restore_session(&self, session: ReplSessionV2) {
        let id = session.id;
        self.sessions.write().await.insert(id, session);
    }

    /// Delete a session from memory and (if configured) from persistent storage.
    pub async fn delete_session(&self, session_id: Uuid) -> bool {
        let removed = self.sessions.write().await.remove(&session_id).is_some();
        if removed {
            self.maybe_delete_persisted_session(session_id).await;
        }
        removed
    }

    /// Create a new session and return its ID.
    pub async fn create_session(&self) -> Uuid {
        let session = ReplSessionV2::new();
        let id = session.id;
        self.sessions.write().await.insert(id, session);
        id
    }

    /// Get a snapshot of session state (for API responses).
    pub async fn get_session(&self, session_id: Uuid) -> Option<ReplSessionV2> {
        self.sessions.read().await.get(&session_id).cloned()
    }

    /// Expose the session map for test manipulation (integration tests).
    #[doc(hidden)]
    pub fn sessions_for_test(&self) -> &Arc<RwLock<HashMap<Uuid, ReplSessionV2>>> {
        &self.sessions
    }

    /// Process user input and return a response.
    pub async fn process(
        &self,
        session_id: Uuid,
        input: UserInputV2,
    ) -> Result<ReplResponseV2, OrchestratorError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or(OrchestratorError::SessionNotFound(session_id))?;

        // Record user input as a message.
        if let UserInputV2::Message { ref content } = input {
            session.push_message(MessageRole::User, content.clone());
        }

        // Dispatch based on current state.
        let response = match session.state.clone() {
            ReplStateV2::ScopeGate { .. } => self.handle_scope_gate(session, input),
            ReplStateV2::JourneySelection { .. } => self.handle_journey_selection(session, input),
            ReplStateV2::InPack { .. } => self.handle_in_pack(session, input).await,
            ReplStateV2::Clarifying { .. } => self.handle_clarifying(session, input),
            ReplStateV2::SentencePlayback { .. } => self.handle_sentence_playback(session, input),
            ReplStateV2::RunbookEditing => self.handle_runbook_editing(session, input).await,
            ReplStateV2::Executing {
                runbook_id,
                progress,
            } => {
                self.handle_executing(session, input, runbook_id, progress)
                    .await
            }
        };

        // Record assistant response message.
        session.push_message(MessageRole::Assistant, response.message.clone());

        Ok(response)
    }

    // -----------------------------------------------------------------------
    // State handlers
    // -----------------------------------------------------------------------

    fn handle_scope_gate(&self, session: &mut ReplSessionV2, input: UserInputV2) -> ReplResponseV2 {
        match input {
            UserInputV2::SelectScope {
                group_id,
                group_name,
            } => {
                session.set_client_context(ClientContext {
                    client_group_id: group_id,
                    client_group_name: group_name.clone(),
                    default_cbu: None,
                    default_book: None,
                });
                session.set_state(ReplStateV2::JourneySelection { candidates: None });

                let packs = self.pack_router.list_packs();
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::JourneyOptions {
                        packs: packs.clone(),
                    },
                    message: format!(
                        "Scope set to {}. Which journey would you like to start?",
                        group_name
                    ),
                    runbook_summary: None,
                    step_count: 0,
                }
            }
            UserInputV2::Message { content } => {
                // Try to interpret the message as a scope selection.
                // Phase 0: simple approach — store as pending and ask.
                session.set_state(ReplStateV2::ScopeGate {
                    pending_input: Some(content),
                });
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::ScopeRequired {
                        prompt: "Please select a client group to work with.".to_string(),
                    },
                    message: "Which client group would you like to work with?".to_string(),
                    runbook_summary: None,
                    step_count: 0,
                }
            }
            _ => self.invalid_input(session, "Please select a scope first."),
        }
    }

    fn handle_journey_selection(
        &self,
        session: &mut ReplSessionV2,
        input: UserInputV2,
    ) -> ReplResponseV2 {
        match input {
            UserInputV2::SelectPack { pack_id } => self.activate_pack_by_id(session, &pack_id),
            UserInputV2::Message { content } => {
                // Route via PackRouter.
                match self.pack_router.route(&content) {
                    PackRouteOutcome::Matched(manifest, hash) => {
                        let pack_id = manifest.id.clone();
                        session.activate_pack(manifest, hash, None);
                        self.enter_pack(session, &pack_id)
                    }
                    PackRouteOutcome::Ambiguous(candidates) => {
                        session.set_state(ReplStateV2::JourneySelection {
                            candidates: Some(candidates.clone()),
                        });
                        ReplResponseV2 {
                            state: session.state.clone(),
                            kind: ReplResponseKindV2::JourneyOptions { packs: candidates },
                            message: "Multiple journeys match. Which one would you like?"
                                .to_string(),
                            runbook_summary: None,
                            step_count: 0,
                        }
                    }
                    PackRouteOutcome::NoMatch => {
                        let packs = self.pack_router.list_packs();
                        session.set_state(ReplStateV2::JourneySelection {
                            candidates: Some(packs.clone()),
                        });
                        ReplResponseV2 {
                            state: session.state.clone(),
                            kind: ReplResponseKindV2::JourneyOptions { packs },
                            message:
                                "I couldn't match that to a journey. Here are the available options:"
                                    .to_string(),
                            runbook_summary: None,
                            step_count: 0,
                        }
                    }
                }
            }
            _ => self.invalid_input(session, "Please select or describe a journey."),
        }
    }

    async fn handle_in_pack(
        &self,
        session: &mut ReplSessionV2,
        input: UserInputV2,
    ) -> ReplResponseV2 {
        match input {
            UserInputV2::Message { content } => {
                // Check if there are still required questions to answer.
                if let Some(question) = self.next_required_question(session) {
                    // Record the answer to the previous question (if any).
                    session
                        .record_answer(question.field.clone(), serde_json::Value::String(content));

                    // Check for next question.
                    if let Some(next) = self.next_required_question(session) {
                        return ReplResponseV2 {
                            state: session.state.clone(),
                            kind: ReplResponseKindV2::Question {
                                field: next.field.clone(),
                                prompt: next.prompt.clone(),
                                answer_kind: format!("{:?}", next.answer_kind),
                            },
                            message: next.prompt.clone(),
                            runbook_summary: None,
                            step_count: session.runbook.entries.len(),
                        };
                    }

                    // All questions answered — try to instantiate template.
                    return self.try_instantiate_template(session);
                }

                // No more questions — propose via engine (or fallback to match_verb).
                return self.propose_for_input(session, &content).await;
            }
            UserInputV2::SelectProposal { proposal_id } => {
                self.handle_select_proposal(session, proposal_id)
            }
            UserInputV2::Edit {
                step_id,
                field,
                value,
            } => self.handle_edit_step(session, step_id, &field, &value),
            UserInputV2::Command { command } => match command {
                ReplCommandV2::Run => self.execute_runbook(session).await,
                ReplCommandV2::Undo => self.handle_undo(session),
                ReplCommandV2::Redo => self.handle_redo(session),
                ReplCommandV2::Clear => self.handle_clear(session),
                ReplCommandV2::Cancel => self.handle_cancel(session),
                ReplCommandV2::Info => self.handle_info(session),
                ReplCommandV2::Help => self.handle_help(session),
                ReplCommandV2::Remove(id) => {
                    if session.runbook.remove_entry(id).is_some() {
                        let summary = self.runbook_summary(session);
                        ReplResponseV2 {
                            state: session.state.clone(),
                            kind: ReplResponseKindV2::RunbookSummary {
                                chapters: self.chapter_view(session),
                                summary: summary.clone(),
                            },
                            message: format!("Removed step. {}", summary),
                            runbook_summary: Some(summary),
                            step_count: session.runbook.entries.len(),
                        }
                    } else {
                        self.invalid_input(session, "Step not found.")
                    }
                }
                ReplCommandV2::Reorder(ids) => {
                    session.runbook.reorder(&ids);
                    let summary = self.runbook_summary(session);
                    ReplResponseV2 {
                        state: session.state.clone(),
                        kind: ReplResponseKindV2::RunbookSummary {
                            chapters: self.chapter_view(session),
                            summary: summary.clone(),
                        },
                        message: format!("Reordered. {}", summary),
                        runbook_summary: Some(summary),
                        step_count: session.runbook.entries.len(),
                    }
                }
                ReplCommandV2::Disable(id) => self.handle_disable(session, id),
                ReplCommandV2::Enable(id) => self.handle_enable(session, id),
                ReplCommandV2::Toggle(id) => self.handle_toggle(session, id),
                ReplCommandV2::Status => self.handle_info(session),
                ReplCommandV2::Resume(_) => {
                    self.invalid_input(session, "Resume is only valid when runbook is parked.")
                }
            },
            _ => self.invalid_input(session, "Send a message or use /run to execute."),
        }
    }

    fn handle_clarifying(&self, session: &mut ReplSessionV2, input: UserInputV2) -> ReplResponseV2 {
        match input {
            UserInputV2::SelectVerb {
                verb_fqn,
                original_input,
            } => {
                let sentence =
                    self.sentence_gen
                        .generate(&verb_fqn, &HashMap::new(), &[], &original_input);
                session.set_state(ReplStateV2::SentencePlayback {
                    sentence: sentence.clone(),
                    verb: verb_fqn.clone(),
                    dsl: format!("({})", verb_fqn),
                    args: HashMap::new(),
                });
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::SentencePlayback {
                        sentence: sentence.clone(),
                        verb: verb_fqn,
                        step_sequence: (session.runbook.entries.len() + 1) as i32,
                    },
                    message: format!("Proposed: {}\n\nConfirm or reject?", sentence),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }
            UserInputV2::Command {
                command: ReplCommandV2::Cancel,
            } => self.handle_cancel(session),
            _ => self.invalid_input(session, "Please select an option or provide more details."),
        }
    }

    fn handle_sentence_playback(
        &self,
        session: &mut ReplSessionV2,
        input: UserInputV2,
    ) -> ReplResponseV2 {
        match input {
            UserInputV2::Command {
                command: ReplCommandV2::Cancel,
            } => self.handle_cancel(session),
            UserInputV2::Confirm => {
                // Add the proposed sentence to the runbook.
                if let ReplStateV2::SentencePlayback {
                    sentence,
                    verb,
                    dsl,
                    args,
                } = session.state.clone()
                {
                    let mut entry = RunbookEntry::new(verb, sentence.clone(), dsl);
                    entry.args = args;
                    entry.arg_extraction_audit = session.pending_arg_audit.take();
                    entry.status = EntryStatus::Confirmed;
                    session.runbook.add_entry(entry);

                    // Go to RunbookEditing (or back to InPack if pack is active).
                    let next_state = if session.journey_context.is_some() {
                        let pack_id = session
                            .journey_context
                            .as_ref()
                            .map(|c| c.pack.id.clone())
                            .unwrap_or_default();
                        ReplStateV2::InPack {
                            pack_id,
                            required_slots_remaining: vec![],
                            last_proposal_id: None,
                        }
                    } else {
                        ReplStateV2::RunbookEditing
                    };
                    session.set_state(next_state);

                    let summary = self.runbook_summary(session);
                    ReplResponseV2 {
                        state: session.state.clone(),
                        kind: ReplResponseKindV2::RunbookSummary {
                            chapters: self.chapter_view(session),
                            summary: summary.clone(),
                        },
                        message: format!("Added: {}\n\n{}", sentence, summary),
                        runbook_summary: Some(summary),
                        step_count: session.runbook.entries.len(),
                    }
                } else {
                    self.invalid_input(session, "No sentence to confirm.")
                }
            }
            UserInputV2::Reject => {
                // Discard and go back to InPack.
                let next_state = if session.journey_context.is_some() {
                    let pack_id = session
                        .journey_context
                        .as_ref()
                        .map(|c| c.pack.id.clone())
                        .unwrap_or_default();
                    ReplStateV2::InPack {
                        pack_id,
                        required_slots_remaining: vec![],
                        last_proposal_id: None,
                    }
                } else {
                    ReplStateV2::RunbookEditing
                };
                session.set_state(next_state);
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Question {
                        field: String::new(),
                        prompt: "What would you like to do instead?".to_string(),
                        answer_kind: "string".to_string(),
                    },
                    message: "Rejected. What would you like to do instead?".to_string(),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }
            _ => self.invalid_input(session, "Please confirm or reject the proposed step."),
        }
    }

    async fn handle_runbook_editing(
        &self,
        session: &mut ReplSessionV2,
        input: UserInputV2,
    ) -> ReplResponseV2 {
        match input {
            UserInputV2::Edit {
                step_id,
                field,
                value,
            } => self.handle_edit_step(session, step_id, &field, &value),
            UserInputV2::Command { command } => match command {
                ReplCommandV2::Run => self.execute_runbook(session).await,
                ReplCommandV2::Undo => self.handle_undo(session),
                ReplCommandV2::Redo => self.handle_redo(session),
                ReplCommandV2::Clear => self.handle_clear(session),
                ReplCommandV2::Cancel => self.handle_cancel(session),
                ReplCommandV2::Info => self.handle_info(session),
                ReplCommandV2::Help => self.handle_help(session),
                ReplCommandV2::Remove(entry_id) => {
                    if session.runbook.remove_entry(entry_id).is_some() {
                        let summary = self.runbook_summary(session);
                        ReplResponseV2 {
                            state: session.state.clone(),
                            kind: ReplResponseKindV2::RunbookSummary {
                                chapters: self.chapter_view(session),
                                summary: summary.clone(),
                            },
                            message: format!("Removed step. {}", summary),
                            runbook_summary: Some(summary),
                            step_count: session.runbook.entries.len(),
                        }
                    } else {
                        self.invalid_input(session, "Step not found.")
                    }
                }
                ReplCommandV2::Reorder(ids) => {
                    session.runbook.reorder(&ids);
                    let summary = self.runbook_summary(session);
                    ReplResponseV2 {
                        state: session.state.clone(),
                        kind: ReplResponseKindV2::RunbookSummary {
                            chapters: self.chapter_view(session),
                            summary: summary.clone(),
                        },
                        message: format!("Reordered. {}", summary),
                        runbook_summary: Some(summary),
                        step_count: session.runbook.entries.len(),
                    }
                }
                ReplCommandV2::Disable(id) => self.handle_disable(session, id),
                ReplCommandV2::Enable(id) => self.handle_enable(session, id),
                ReplCommandV2::Toggle(id) => self.handle_toggle(session, id),
                ReplCommandV2::Status => self.handle_info(session),
                ReplCommandV2::Resume(_) => {
                    self.invalid_input(session, "Resume is only valid when runbook is parked.")
                }
            },
            UserInputV2::Message { content } => {
                // Treat as new verb matching — same as InPack message handling.
                return self.propose_for_input(session, &content).await;
            }
            UserInputV2::SelectProposal { proposal_id } => {
                self.handle_select_proposal(session, proposal_id)
            }
            _ => self.invalid_input(session, "Use /run to execute, or add more steps."),
        }
    }

    async fn handle_executing(
        &self,
        session: &mut ReplSessionV2,
        input: UserInputV2,
        _runbook_id: Uuid,
        _progress: ExecutionProgress,
    ) -> ReplResponseV2 {
        match input {
            UserInputV2::Command {
                command: ReplCommandV2::Status,
            }
            | UserInputV2::Command {
                command: ReplCommandV2::Info,
            } => self.handle_parked_status(session),

            UserInputV2::Approve {
                entry_id,
                approved_by,
            } => {
                self.handle_human_gate_approval(session, entry_id, approved_by)
                    .await
            }

            UserInputV2::RejectGate { entry_id, reason } => {
                self.handle_human_gate_rejection(session, entry_id, reason)
                    .await
            }

            UserInputV2::Command {
                command: ReplCommandV2::Cancel,
            } => self.handle_cancel_parked(session).await,

            UserInputV2::Command {
                command: ReplCommandV2::Resume(entry_id),
            } => self.continue_execution(session, entry_id).await,

            _ => self.invalid_input(
                session,
                "Runbook is parked. Use /status to check, approve/reject a gate, or /cancel.",
            ),
        }
    }

    /// Show status of parked entries.
    fn handle_parked_status(&self, session: &ReplSessionV2) -> ReplResponseV2 {
        let parked: Vec<_> = session
            .runbook
            .entries
            .iter()
            .filter(|e| e.status == EntryStatus::Parked)
            .collect();

        if parked.is_empty() {
            return self.invalid_input(session, "No entries are currently parked.");
        }

        let info_lines: Vec<String> = parked
            .iter()
            .map(|e| {
                let gate = e
                    .invocation
                    .as_ref()
                    .map(|inv| format!("{:?}", inv.gate_type))
                    .unwrap_or_else(|| "Unknown".to_string());
                let key = e
                    .invocation
                    .as_ref()
                    .map(|inv| inv.correlation_key.clone())
                    .unwrap_or_default();
                format!(
                    "  Step {} ({}): {} — gate: {}, key: {}",
                    e.sequence, e.id, e.sentence, gate, key
                )
            })
            .collect();

        let summary = self.runbook_summary(session);
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::RunbookSummary {
                chapters: self.chapter_view(session),
                summary: summary.clone(),
            },
            message: format!(
                "Parked entries ({}):\n{}\n\n{}",
                parked.len(),
                info_lines.join("\n"),
                summary
            ),
            runbook_summary: Some(summary),
            step_count: session.runbook.entries.len(),
        }
    }

    /// Approve a human-gated entry: execute its DSL, then continue.
    async fn handle_human_gate_approval(
        &self,
        session: &mut ReplSessionV2,
        entry_id: Uuid,
        approved_by: Option<String>,
    ) -> ReplResponseV2 {
        // Validate the entry is parked with HumanApproval gate.
        let entry_idx = session
            .runbook
            .entries
            .iter()
            .position(|e| e.id == entry_id);

        let idx = match entry_idx {
            Some(idx) => idx,
            None => {
                return self.invalid_input(session, &format!("Entry {} not found.", entry_id));
            }
        };

        {
            let entry = &session.runbook.entries[idx];
            if entry.status != EntryStatus::Parked {
                return self.invalid_input(
                    session,
                    &format!(
                        "Entry {} is not parked (status: {:?}).",
                        entry_id, entry.status
                    ),
                );
            }
            let is_human_gate = entry
                .invocation
                .as_ref()
                .map(|inv| inv.gate_type == GateType::HumanApproval)
                .unwrap_or(false);
            if !is_human_gate {
                return self.invalid_input(
                    session,
                    "Entry is parked for durable task, not human approval. Wait for signal.",
                );
            }
        }

        // Emit approval event.
        let invocation_id = session.runbook.entries[idx]
            .invocation
            .as_ref()
            .map(|inv| inv.invocation_id)
            .unwrap_or_default();
        session
            .runbook
            .audit
            .push(super::runbook::RunbookEvent::HumanGateApproved {
                entry_id,
                invocation_id,
                approved_by,
                timestamp: chrono::Utc::now(),
            });

        // Resume the entry (marks Completed in runbook, clears invocation index).
        let correlation_key = session.runbook.entries[idx]
            .invocation
            .as_ref()
            .map(|inv| inv.correlation_key.clone())
            .unwrap_or_default();
        session.runbook.resume_entry(&correlation_key, None);

        // Now execute the DSL (was NOT executed before for HumanGate).
        let dsl = session.runbook.entries[idx].dsl.clone();
        let runbook_id = session.runbook.id;
        let outcome = self.execute_entry_v2(&dsl, entry_id, runbook_id).await;

        match outcome {
            DslExecutionOutcome::Completed(result) => {
                session.runbook.entries[idx].status = EntryStatus::Completed;
                session.runbook.entries[idx].result = Some(result);
            }
            DslExecutionOutcome::Failed(err) => {
                session.runbook.entries[idx].status = EntryStatus::Failed;
                session.runbook.entries[idx].result = Some(serde_json::json!({"error": err}));
            }
            DslExecutionOutcome::Parked { .. } => {
                // Edge case: approved human gate returns another park.
                // This shouldn't normally happen. Mark failed.
                session.runbook.entries[idx].status = EntryStatus::Failed;
                session.runbook.entries[idx].result =
                    Some(serde_json::json!({"error": "Unexpected park after approval"}));
            }
        }

        // Persist after approval (required — state changed from Parked).
        if let Err(e) = self.persist_session_required(session).await {
            tracing::error!(session_id = %session.id, error = %e, "Failed to persist after gate approval");
        }

        // Continue executing remaining entries.
        self.continue_execution(session, entry_id).await
    }

    /// Reject a human-gated entry: mark failed, return to editing.
    async fn handle_human_gate_rejection(
        &self,
        session: &mut ReplSessionV2,
        entry_id: Uuid,
        reason: Option<String>,
    ) -> ReplResponseV2 {
        let entry_idx = session
            .runbook
            .entries
            .iter()
            .position(|e| e.id == entry_id);

        let idx = match entry_idx {
            Some(idx) => idx,
            None => {
                return self.invalid_input(session, &format!("Entry {} not found.", entry_id));
            }
        };

        let entry = &session.runbook.entries[idx];
        if entry.status != EntryStatus::Parked {
            return self.invalid_input(session, &format!("Entry {} is not parked.", entry_id));
        }

        let invocation_id = entry
            .invocation
            .as_ref()
            .map(|inv| inv.invocation_id)
            .unwrap_or_default();

        // Emit rejection event.
        session
            .runbook
            .audit
            .push(super::runbook::RunbookEvent::HumanGateRejected {
                entry_id,
                invocation_id,
                rejected_by: None,
                reason: reason.clone(),
                timestamp: chrono::Utc::now(),
            });

        // Mark entry as Failed and clear invocation.
        session.runbook.entries[idx].status = EntryStatus::Failed;
        session.runbook.entries[idx].result =
            Some(serde_json::json!({"rejected": true, "reason": reason}));
        if let Some(ref inv) = session.runbook.entries[idx].invocation {
            session
                .runbook
                .invocation_index
                .remove(&inv.correlation_key);
        }
        session.runbook.entries[idx].invocation = None;

        // Back to editing state.
        session.runbook.set_status(RunbookStatus::Ready);
        session.set_state(ReplStateV2::RunbookEditing);

        // Persist after rejection (required — state changed from Parked).
        if let Err(e) = self.persist_session_required(session).await {
            tracing::error!(session_id = %session.id, error = %e, "Failed to persist after gate rejection");
        }

        let summary = self.runbook_summary(session);
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::RunbookSummary {
                chapters: self.chapter_view(session),
                summary: summary.clone(),
            },
            message: format!(
                "Gate rejected for step {}. Runbook paused.\n\n{}",
                entry_id, summary
            ),
            runbook_summary: Some(summary),
            step_count: session.runbook.entries.len(),
        }
    }

    /// Cancel all parked entries and return to editing.
    async fn handle_cancel_parked(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        let cancelled = session.runbook.cancel_parked_entries();
        if cancelled == 0 {
            return self.invalid_input(session, "No entries to cancel.");
        }

        session.runbook.set_status(RunbookStatus::Ready);
        session.set_state(ReplStateV2::RunbookEditing);

        // Persist after cancel (required — parked state cleared).
        if let Err(e) = self.persist_session_required(session).await {
            tracing::error!(session_id = %session.id, error = %e, "Failed to persist after cancel");
        }

        let summary = self.runbook_summary(session);
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::RunbookSummary {
                chapters: self.chapter_view(session),
                summary: summary.clone(),
            },
            message: format!("{} parked entries cancelled.\n\n{}", cancelled, summary),
            runbook_summary: Some(summary),
            step_count: session.runbook.entries.len(),
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn activate_pack_by_id(&self, session: &mut ReplSessionV2, pack_id: &str) -> ReplResponseV2 {
        if let Some((manifest, hash)) = self.pack_router.get_pack(pack_id) {
            let pack_id = manifest.id.clone();
            session.activate_pack(manifest.clone(), hash.clone(), None);
            self.enter_pack(session, &pack_id)
        } else {
            self.invalid_input(session, &format!("Pack '{}' not found.", pack_id))
        }
    }

    fn enter_pack(&self, session: &mut ReplSessionV2, pack_id: &str) -> ReplResponseV2 {
        // Determine remaining required slots.
        let required_slots: Vec<String> = session
            .journey_context
            .as_ref()
            .map(|ctx| {
                ctx.pack
                    .required_questions
                    .iter()
                    .map(|q| q.field.clone())
                    .collect()
            })
            .unwrap_or_default();

        session.set_state(ReplStateV2::InPack {
            pack_id: pack_id.to_string(),
            required_slots_remaining: required_slots,
            last_proposal_id: None,
        });

        // Ask the first required question (if any).
        if let Some(question) = self.next_required_question(session) {
            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::Question {
                    field: question.field.clone(),
                    prompt: question.prompt.clone(),
                    answer_kind: format!("{:?}", question.answer_kind),
                },
                message: question.prompt.clone(),
                runbook_summary: None,
                step_count: 0,
            }
        } else {
            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::Question {
                    field: String::new(),
                    prompt: "What would you like to do?".to_string(),
                    answer_kind: "string".to_string(),
                },
                message: "Pack activated. What would you like to do?".to_string(),
                runbook_summary: None,
                step_count: 0,
            }
        }
    }

    fn next_required_question(
        &self,
        session: &ReplSessionV2,
    ) -> Option<crate::journey::pack::PackQuestion> {
        let ctx = session.journey_context.as_ref()?;
        ctx.pack
            .required_questions
            .iter()
            .find(|q| !ctx.answers.contains_key(&q.field))
            .cloned()
    }

    /// Build a `MatchContext` from the current session state.
    fn build_match_context(&self, session: &ReplSessionV2) -> MatchContext {
        MatchContext {
            client_group_id: session.client_context.as_ref().map(|c| c.client_group_id),
            client_group_name: session
                .client_context
                .as_ref()
                .map(|c| c.client_group_name.clone()),
            domain_hint: session.journey_context.as_ref().and_then(|ctx| {
                ctx.pack
                    .allowed_verbs
                    .first()
                    .and_then(|v| v.split('.').next().map(|s| s.to_string()))
            }),
            ..Default::default()
        }
    }

    /// Phase 3: Propose steps using the ProposalEngine.
    ///
    /// If no proposal engine is configured, falls back to `match_verb_for_input()`.
    /// If exactly 1 proposal with high confidence (>= 0.85), auto-advances to
    /// SentencePlayback. Otherwise, returns StepProposals for user selection.
    async fn propose_for_input(
        &self,
        session: &mut ReplSessionV2,
        content: &str,
    ) -> ReplResponseV2 {
        let engine = match &self.proposal_engine {
            Some(e) => e,
            None => return self.match_verb_for_input(session, content).await,
        };

        let match_ctx = self.build_match_context(session);
        let pack = session.journey_context.as_ref().map(|c| c.pack.as_ref());

        let context_vars: HashMap<String, String> = session
            .client_context
            .as_ref()
            .map(|c| {
                HashMap::from([
                    ("client_name".to_string(), c.client_group_name.clone()),
                    ("client_group_id".to_string(), c.client_group_id.to_string()),
                ])
            })
            .unwrap_or_default();

        let answers = session
            .journey_context
            .as_ref()
            .map(|c| c.answers.clone())
            .unwrap_or_default();

        let proposal_set = engine
            .propose(
                content,
                pack,
                &session.runbook,
                &match_ctx,
                &context_vars,
                &answers,
            )
            .await;

        // Single high-confidence proposal → auto-advance to SentencePlayback.
        if proposal_set.proposals.len() == 1
            && proposal_set.proposals[0].evidence.confidence
                >= super::proposal_engine::AUTO_ADVANCE_THRESHOLD
        {
            let p = &proposal_set.proposals[0];
            let confirm_policy = p.confirm_policy;

            session.set_state(ReplStateV2::SentencePlayback {
                sentence: p.sentence.clone(),
                verb: p.verb.clone(),
                dsl: p.dsl.clone(),
                args: p.args.clone(),
            });
            session.last_proposal_set = Some(proposal_set);

            // QuickConfirm auto-confirms.
            if confirm_policy == ConfirmPolicy::QuickConfirm {
                let p = &session.last_proposal_set.as_ref().unwrap().proposals[0];
                let mut entry =
                    RunbookEntry::new(p.verb.clone(), p.sentence.clone(), p.dsl.clone());
                entry.args = p.args.clone();
                entry.status = EntryStatus::Confirmed;
                entry.confirm_policy = ConfirmPolicy::QuickConfirm;
                session.runbook.add_entry(entry);

                let next_state = if session.journey_context.is_some() {
                    let pack_id = session
                        .journey_context
                        .as_ref()
                        .map(|c| c.pack.id.clone())
                        .unwrap_or_default();
                    ReplStateV2::InPack {
                        pack_id,
                        required_slots_remaining: vec![],
                        last_proposal_id: None,
                    }
                } else {
                    ReplStateV2::RunbookEditing
                };
                session.set_state(next_state);

                let summary = format!(
                    "Auto-confirmed (quick): {}",
                    session.last_proposal_set.as_ref().unwrap().proposals[0].sentence
                );
                return ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::RunbookSummary {
                        chapters: self.chapter_view(session),
                        summary: summary.clone(),
                    },
                    message: summary,
                    runbook_summary: Some(self.runbook_summary(session)),
                    step_count: session.runbook.entries.len(),
                };
            }

            return ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::SentencePlayback {
                    sentence: session.last_proposal_set.as_ref().unwrap().proposals[0]
                        .sentence
                        .clone(),
                    verb: session.last_proposal_set.as_ref().unwrap().proposals[0]
                        .verb
                        .clone(),
                    step_sequence: (session.runbook.entries.len() + 1) as i32,
                },
                message: format!(
                    "Proposed: {} (confidence: {:.0}%)\n\nConfirm or reject?",
                    session.last_proposal_set.as_ref().unwrap().proposals[0].sentence,
                    session.last_proposal_set.as_ref().unwrap().proposals[0]
                        .evidence
                        .confidence
                        * 100.0
                ),
                runbook_summary: None,
                step_count: session.runbook.entries.len(),
            };
        }

        // No proposals → fall back to no-match response.
        if proposal_set.proposals.is_empty() {
            return self.invalid_input(
                session,
                &format!("No matching action found for: {}", content),
            );
        }

        // Multiple proposals → return StepProposals for user selection.
        let response = ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::StepProposals {
                proposals: proposal_set.proposals.clone(),
                template_fast_path: proposal_set.template_fast_path,
                proposal_hash: proposal_set.proposal_hash.clone(),
            },
            message: format!(
                "I found {} options. Select one or provide more details:",
                proposal_set.proposals.len()
            ),
            runbook_summary: None,
            step_count: session.runbook.entries.len(),
        };
        session.last_proposal_set = Some(proposal_set);
        response
    }

    /// Handle `SelectProposal` input — look up proposal from last set and
    /// transition to SentencePlayback.
    fn handle_select_proposal(
        &self,
        session: &mut ReplSessionV2,
        proposal_id: Uuid,
    ) -> ReplResponseV2 {
        let proposal = session
            .last_proposal_set
            .as_ref()
            .and_then(|ps| ps.proposals.iter().find(|p| p.id == proposal_id))
            .cloned();

        match proposal {
            Some(p) => {
                session.set_state(ReplStateV2::SentencePlayback {
                    sentence: p.sentence.clone(),
                    verb: p.verb.clone(),
                    dsl: p.dsl.clone(),
                    args: p.args.clone(),
                });

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::SentencePlayback {
                        sentence: p.sentence.clone(),
                        verb: p.verb.clone(),
                        step_sequence: (session.runbook.entries.len() + 1) as i32,
                    },
                    message: format!(
                        "Selected: {} (confidence: {:.0}%)\n\nConfirm or reject?",
                        p.sentence,
                        p.evidence.confidence * 100.0
                    ),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }
            None => self.invalid_input(session, "Proposal not found. Please try again."),
        }
    }

    /// Match a verb from free-text input using IntentService > IntentMatcher > stub.
    ///
    /// This is the core verb matching logic shared by `handle_in_pack` and
    /// `handle_runbook_editing` once required questions are exhausted.
    ///
    /// Priority:
    /// 1. IntentService (Phase 2 unified pipeline — preferred)
    /// 2. IntentMatcher (Phase 1 direct — fallback)
    /// 3. Stub (Phase 0 — last resort)
    async fn match_verb_for_input(
        &self,
        session: &mut ReplSessionV2,
        content: &str,
    ) -> ReplResponseV2 {
        let match_ctx = self.build_match_context(session);

        // Phase 2: Try IntentService first (unified pipeline with clarification).
        if let Some(svc) = &self.intent_service {
            match svc.match_verb(content, &match_ctx).await {
                Ok(outcome) => {
                    return self.handle_intent_service_outcome(session, content, svc, outcome);
                }
                Err(e) => {
                    tracing::warn!("IntentService error, falling back: {}", e);
                }
            }
        }

        // Phase 1: Try raw IntentMatcher if available.
        if let Some(matcher) = &self.intent_matcher {
            match matcher.match_intent(content, &match_ctx).await {
                Ok(result) => {
                    return self.handle_intent_result(session, content, result);
                }
                Err(e) => {
                    tracing::warn!("IntentMatcher error, falling back to stub: {}", e);
                }
            }
        }

        // Stub fallback (Phase 0 behavior): generate a placeholder sentence.
        self.stub_verb_match(session, content)
    }

    /// Handle outcome from IntentService (Phase 2 path).
    ///
    /// Key difference from `handle_intent_result`: when a verb is matched,
    /// checks `sentences.clarify` for missing required args BEFORE going to
    /// SentencePlayback. This produces conversational clarification prompts
    /// instead of raw validation errors.
    fn handle_intent_service_outcome(
        &self,
        session: &mut ReplSessionV2,
        original_input: &str,
        svc: &IntentService,
        outcome: VerbMatchOutcome,
    ) -> ReplResponseV2 {
        match outcome {
            VerbMatchOutcome::Matched {
                verb,
                confidence,
                generated_dsl,
            } => {
                let dsl = generated_dsl.unwrap_or_else(|| format!("({})", verb));
                let args = extract_args_from_dsl(&dsl);

                // Phase 2: Check clarification via sentences.clarify
                match svc.check_clarification(&verb, &args) {
                    ClarificationOutcome::NeedsClarification { prompts, .. } => {
                        // Use the first conversational clarify prompt
                        let (_arg_name, prompt) = &prompts[0];
                        session.set_state(ReplStateV2::Clarifying {
                            question: prompt.clone(),
                            candidates: vec![],
                            original_input: original_input.to_string(),
                        });
                        return ReplResponseV2 {
                            state: session.state.clone(),
                            kind: ReplResponseKindV2::Error {
                                error: prompt.clone(),
                                recoverable: true,
                            },
                            message: prompt.clone(),
                            runbook_summary: None,
                            step_count: session.runbook.entries.len(),
                        };
                    }
                    ClarificationOutcome::Complete => {
                        // All args present — proceed to sentence playback
                    }
                }

                // Generate sentence via IntentService (uses YAML templates)
                let sentence = svc.generate_sentence(&verb, &args);
                let confirm_policy = svc.confirm_policy(&verb);

                // Build audit
                let audit = build_arg_extraction_audit(
                    original_input,
                    &args,
                    confidence,
                    None, // IntentService doesn't expose debug info here
                );

                session.pending_arg_audit = Some(audit.clone());

                session.set_state(ReplStateV2::SentencePlayback {
                    sentence: sentence.clone(),
                    verb: verb.clone(),
                    dsl: dsl.clone(),
                    args: args.clone(),
                });

                // QuickConfirm auto-confirms (same logic as Phase 1)
                if confirm_policy == ConfirmPolicy::QuickConfirm {
                    let mut entry = RunbookEntry::new(verb.clone(), sentence.clone(), dsl);
                    entry.args = args;
                    entry.arg_extraction_audit = Some(audit);
                    entry.status = EntryStatus::Confirmed;
                    entry.confirm_policy = ConfirmPolicy::QuickConfirm;
                    session.runbook.add_entry(entry);

                    let next_state = if session.journey_context.is_some() {
                        let pack_id = session
                            .journey_context
                            .as_ref()
                            .map(|c| c.pack.id.clone())
                            .unwrap_or_default();
                        ReplStateV2::InPack {
                            pack_id,
                            required_slots_remaining: vec![],
                            last_proposal_id: None,
                        }
                    } else {
                        ReplStateV2::RunbookEditing
                    };
                    session.set_state(next_state);

                    let summary = format!("Auto-confirmed (quick): {}", sentence);
                    return ReplResponseV2 {
                        state: session.state.clone(),
                        kind: ReplResponseKindV2::RunbookSummary {
                            chapters: self.chapter_view(session),
                            summary: summary.clone(),
                        },
                        message: summary,
                        runbook_summary: Some(self.runbook_summary(session)),
                        step_count: session.runbook.entries.len(),
                    };
                }

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::SentencePlayback {
                        sentence: sentence.clone(),
                        verb: verb.clone(),
                        step_sequence: (session.runbook.entries.len() + 1) as i32,
                    },
                    message: format!(
                        "Proposed: {} (confidence: {:.0}%)\n\nConfirm or reject?",
                        sentence,
                        confidence * 100.0
                    ),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }

            VerbMatchOutcome::Ambiguous { candidates, margin } => {
                let v2_candidates: Vec<_> = candidates
                    .iter()
                    .take(5)
                    .map(|c| super::types_v2::VerbCandidate {
                        verb_fqn: c.verb_fqn.clone(),
                        description: c.description.clone(),
                        score: c.score,
                    })
                    .collect();

                session.set_state(ReplStateV2::Clarifying {
                    question: "Which action did you mean?".to_string(),
                    candidates: v2_candidates.clone(),
                    original_input: original_input.to_string(),
                });

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Error {
                        error: format!("Ambiguous match (margin: {:.3}). Please select:", margin),
                        recoverable: true,
                    },
                    message: format!(
                        "I found multiple matching actions (margin: {:.3}):\n{}",
                        margin,
                        v2_candidates
                            .iter()
                            .enumerate()
                            .map(|(i, c)| format!(
                                "  {}. {} — {}",
                                i + 1,
                                c.verb_fqn,
                                c.description
                            ))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }

            VerbMatchOutcome::NoMatch { reason } => {
                self.invalid_input(session, &format!("No matching action found: {}", reason))
            }

            VerbMatchOutcome::DirectDsl { source } => {
                let verb = source
                    .trim()
                    .trim_start_matches('(')
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();

                let sentence = format!("Execute: {}", source.trim());
                session.set_state(ReplStateV2::SentencePlayback {
                    sentence: sentence.clone(),
                    verb: verb.clone(),
                    dsl: source.clone(),
                    args: HashMap::new(),
                });

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::SentencePlayback {
                        sentence: sentence.clone(),
                        verb,
                        step_sequence: (session.runbook.entries.len() + 1) as i32,
                    },
                    message: format!("Direct DSL: {}\n\nConfirm or reject?", sentence),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }

            VerbMatchOutcome::NeedsScopeSelection => {
                self.invalid_input(session, "Please select a scope first.")
            }

            VerbMatchOutcome::NeedsEntityResolution => self.invalid_input(
                session,
                "Entity resolution needed. Please provide more details.",
            ),

            VerbMatchOutcome::Other(result) => {
                let result = *result;
                // Fallback: delegate to handle_intent_result for cases
                // not explicitly handled above (intent tier, client group, etc.)
                self.handle_intent_result(session, original_input, result)
            }
        }
    }

    /// Handle the result of an IntentMatcher call.
    fn handle_intent_result(
        &self,
        session: &mut ReplSessionV2,
        original_input: &str,
        result: crate::repl::types::IntentMatchResult,
    ) -> ReplResponseV2 {
        match result.outcome {
            MatchOutcome::Matched { verb, confidence } => {
                // Look up verb config for sentence generation.
                let (phrases, description) = self
                    .verb_config_index
                    .get(&verb)
                    .map(|e| {
                        let tmpl = if !e.sentence_templates.is_empty() {
                            e.sentence_templates.clone()
                        } else {
                            e.invocation_phrases.clone()
                        };
                        (tmpl, e.description.clone())
                    })
                    .unwrap_or_else(|| (vec![], String::new()));

                let dsl = result
                    .generated_dsl
                    .clone()
                    .unwrap_or_else(|| format!("({})", verb));

                // Extract args from generated DSL.
                let args = extract_args_from_dsl(&dsl);

                // Build ArgExtractionAudit from IntentMatchResult debug info.
                let audit = build_arg_extraction_audit(
                    original_input,
                    &args,
                    confidence,
                    result.debug.as_ref(),
                );

                let sentence = self
                    .sentence_gen
                    .generate(&verb, &args, &phrases, &description);

                let confirm_policy = self.verb_config_index.confirm_policy(&verb);

                // Store the audit on the session — consumed on Confirm.
                session.pending_arg_audit = Some(audit.clone());

                session.set_state(ReplStateV2::SentencePlayback {
                    sentence: sentence.clone(),
                    verb: verb.clone(),
                    dsl: dsl.clone(),
                    args: args.clone(),
                });

                // For QuickConfirm verbs (navigation), auto-confirm.
                if confirm_policy == ConfirmPolicy::QuickConfirm {
                    let mut entry = RunbookEntry::new(verb.clone(), sentence.clone(), dsl);
                    entry.args = args;
                    entry.arg_extraction_audit = Some(audit);
                    entry.status = EntryStatus::Confirmed;
                    entry.confirm_policy = ConfirmPolicy::QuickConfirm;
                    session.runbook.add_entry(entry);

                    let next_state = if session.journey_context.is_some() {
                        let pack_id = session
                            .journey_context
                            .as_ref()
                            .map(|c| c.pack.id.clone())
                            .unwrap_or_default();
                        ReplStateV2::InPack {
                            pack_id,
                            required_slots_remaining: vec![],
                            last_proposal_id: None,
                        }
                    } else {
                        ReplStateV2::RunbookEditing
                    };
                    session.set_state(next_state);

                    let summary = format!("Auto-confirmed (quick): {}", sentence);
                    return ReplResponseV2 {
                        state: session.state.clone(),
                        kind: ReplResponseKindV2::RunbookSummary {
                            chapters: self.chapter_view(session),
                            summary: summary.clone(),
                        },
                        message: summary,
                        runbook_summary: Some(self.runbook_summary(session)),
                        step_count: session.runbook.entries.len(),
                    };
                }

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::SentencePlayback {
                        sentence: sentence.clone(),
                        verb: verb.clone(),
                        step_sequence: (session.runbook.entries.len() + 1) as i32,
                    },
                    message: format!(
                        "Proposed: {} (confidence: {:.0}%)\n\nConfirm or reject?",
                        sentence,
                        confidence * 100.0
                    ),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }

            MatchOutcome::Ambiguous { margin } => {
                // Present verb candidates for clarification.
                let candidates: Vec<_> = result
                    .verb_candidates
                    .iter()
                    .take(5)
                    .map(|c| super::types_v2::VerbCandidate {
                        verb_fqn: c.verb_fqn.clone(),
                        description: c.description.clone(),
                        score: c.score,
                    })
                    .collect();

                session.set_state(ReplStateV2::Clarifying {
                    question: "Which action did you mean?".to_string(),
                    candidates: candidates.clone(),
                    original_input: original_input.to_string(),
                });

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Error {
                        error: format!("Ambiguous match (margin: {:.3}). Please select:", margin),
                        recoverable: true,
                    },
                    message: format!(
                        "I found multiple matching actions (margin: {:.3}):\n{}",
                        margin,
                        candidates
                            .iter()
                            .enumerate()
                            .map(|(i, c)| format!(
                                "  {}. {} — {}",
                                i + 1,
                                c.verb_fqn,
                                c.description
                            ))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }

            MatchOutcome::DirectDsl { source } => {
                // Direct DSL input — go straight to sentence playback.
                let verb = source
                    .trim()
                    .trim_start_matches('(')
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();

                let sentence = format!("Execute: {}", source.trim());
                session.set_state(ReplStateV2::SentencePlayback {
                    sentence: sentence.clone(),
                    verb: verb.clone(),
                    dsl: source.clone(),
                    args: HashMap::new(),
                });

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::SentencePlayback {
                        sentence: sentence.clone(),
                        verb,
                        step_sequence: (session.runbook.entries.len() + 1) as i32,
                    },
                    message: format!("Direct DSL: {}\n\nConfirm or reject?", sentence),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                }
            }

            MatchOutcome::NoMatch { ref reason } => {
                self.invalid_input(session, &format!("No matching action found: {}", reason))
            }

            MatchOutcome::NeedsScopeSelection => {
                self.invalid_input(session, "Please select a scope first.")
            }

            MatchOutcome::NeedsEntityResolution => {
                // TODO Phase 2: entity resolution flow
                self.invalid_input(
                    session,
                    "Entity resolution needed. Please provide more details.",
                )
            }

            MatchOutcome::NeedsClientGroup { .. } => {
                self.invalid_input(session, "Please select a client group first.")
            }

            MatchOutcome::NeedsIntentTier { .. } => {
                // TODO Phase 2: intent tier disambiguation
                self.invalid_input(
                    session,
                    "Multiple action types match. Please be more specific.",
                )
            }
        }
    }

    /// Phase 0 stub: generate a placeholder sentence from input.
    fn stub_verb_match(&self, session: &mut ReplSessionV2, content: &str) -> ReplResponseV2 {
        let sentence = self.sentence_gen.generate(
            "user.request",
            &HashMap::from([("input".to_string(), content.to_string())]),
            &[],
            content,
        );
        session.set_state(ReplStateV2::SentencePlayback {
            sentence: sentence.clone(),
            verb: "user.request".to_string(),
            dsl: format!("(user.request :input \"{}\")", content),
            args: HashMap::from([("input".to_string(), content.to_string())]),
        });
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::SentencePlayback {
                sentence: sentence.clone(),
                verb: "user.request".to_string(),
                step_sequence: (session.runbook.entries.len() + 1) as i32,
            },
            message: format!("Proposed: {}\n\nConfirm or reject?", sentence),
            runbook_summary: None,
            step_count: session.runbook.entries.len(),
        }
    }

    fn try_instantiate_template(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        let ctx = match session.journey_context.as_ref() {
            Some(c) => c,
            None => return self.invalid_input(session, "No pack context."),
        };

        // Find the first template (Phase 0: use the first one).
        let template = match ctx.pack.templates.first() {
            Some(t) => t,
            None => {
                // No templates — go to InPack for freeform input.
                return ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Question {
                        field: String::new(),
                        prompt: "All questions answered. What would you like to do?".to_string(),
                        answer_kind: "string".to_string(),
                    },
                    message: "All questions answered. What would you like to do?".to_string(),
                    runbook_summary: None,
                    step_count: 0,
                };
            }
        };

        // Build context vars from client context.
        let context_vars: HashMap<String, String> = session
            .client_context
            .as_ref()
            .map(|c| {
                HashMap::from([
                    ("client_name".to_string(), c.client_group_name.clone()),
                    ("client_group_id".to_string(), c.client_group_id.to_string()),
                ])
            })
            .unwrap_or_default();

        // Build invocation phrases and descriptions from VerbConfigIndex.
        let verb_phrases = self.verb_config_index.all_invocation_phrases();
        let verb_descriptions = self.verb_config_index.all_descriptions();

        match instantiate_template(
            template,
            &context_vars,
            &ctx.answers,
            &self.sentence_gen,
            &verb_phrases,
            &verb_descriptions,
        ) {
            Ok((entries, template_hash)) => {
                // Set template provenance on runbook.
                session.runbook.template_id = Some(template.template_id.clone());
                session.runbook.template_hash = Some(template_hash);

                // Add all entries to runbook and mark as Confirmed
                // (user confirmed by answering all pack questions).
                for entry in entries {
                    let id = session.runbook.add_entry(entry);
                    session.runbook.set_entry_status(id, EntryStatus::Confirmed);
                }
                session.runbook.set_status(RunbookStatus::Building);

                let summary = self.runbook_summary(session);
                session.set_state(ReplStateV2::RunbookEditing);

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::RunbookSummary {
                        chapters: self.chapter_view(session),
                        summary: summary.clone(),
                    },
                    message: format!(
                        "Runbook built with {} steps:\n\n{}\n\nReview and use /run to execute.",
                        session.runbook.entries.len(),
                        session.runbook.display_sentences().join("\n")
                    ),
                    runbook_summary: Some(summary),
                    step_count: session.runbook.entries.len(),
                }
            }
            Err(e) => ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::Error {
                    error: e.to_string(),
                    recoverable: true,
                },
                message: format!("Template error: {}", e),
                runbook_summary: None,
                step_count: 0,
            },
        }
    }

    /// Regenerate sentence and DSL for an entry after an arg change.
    fn regenerate_entry_sentence(&self, entry: &super::runbook::RunbookEntry) -> (String, String) {
        let sentence = if let Some(ref svc) = self.intent_service {
            svc.generate_sentence(&entry.verb, &entry.args)
        } else {
            let (phrases, desc) = self
                .verb_config_index
                .get(&entry.verb)
                .map(|e| {
                    let tmpl = if !e.sentence_templates.is_empty() {
                        e.sentence_templates.clone()
                    } else {
                        e.invocation_phrases.clone()
                    };
                    (tmpl, e.description.clone())
                })
                .unwrap_or_default();
            self.sentence_gen
                .generate(&entry.verb, &entry.args, &phrases, &desc)
        };

        let dsl = rebuild_dsl(&entry.verb, &entry.args);
        (sentence, dsl)
    }

    /// Handle editing a specific arg on a runbook entry.
    fn handle_edit_step(
        &self,
        session: &mut ReplSessionV2,
        step_id: Uuid,
        field: &str,
        value: &str,
    ) -> ReplResponseV2 {
        let entry = match session.runbook.entry_by_id(step_id) {
            Some(e) => e.clone(),
            None => return self.invalid_input(session, "Step not found."),
        };

        let old_value = entry.args.get(field).cloned();
        let old_sentence = entry.sentence.clone();

        session
            .runbook
            .update_entry_arg(step_id, field, value.to_string());

        let entry_ref = session.runbook.entry_by_id(step_id).unwrap();
        let (new_sentence, new_dsl) = self.regenerate_entry_sentence(entry_ref);

        session.runbook.update_entry_sentence(
            step_id,
            new_sentence.clone(),
            new_dsl,
            &old_sentence,
            field,
            old_value,
            value,
        );

        if let Some(entry_mut) = session.runbook.entry_by_id_mut(step_id) {
            entry_mut
                .slot_provenance
                .slots
                .insert(field.to_string(), super::runbook::SlotSource::UserProvided);
        }

        let summary = self.runbook_summary(session);
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::RunbookSummary {
                chapters: self.chapter_view(session),
                summary: summary.clone(),
            },
            message: format!(
                "Updated step {}: {} = \"{}\"\n\nNew sentence: {}",
                entry.sequence, field, value, new_sentence
            ),
            runbook_summary: Some(summary),
            step_count: session.runbook.entries.len(),
        }
    }

    /// Handle Cancel command — return to InPack or RunbookEditing.
    fn handle_cancel(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        let next_state = if session.journey_context.is_some() {
            let pack_id = session
                .journey_context
                .as_ref()
                .map(|c| c.pack.id.clone())
                .unwrap_or_default();
            ReplStateV2::InPack {
                pack_id,
                required_slots_remaining: vec![],
                last_proposal_id: None,
            }
        } else {
            ReplStateV2::RunbookEditing
        };
        session.set_state(next_state);
        let summary = self.runbook_summary(session);
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::RunbookSummary {
                chapters: self.chapter_view(session),
                summary: summary.clone(),
            },
            message: "Cancelled.".to_string(),
            runbook_summary: Some(summary),
            step_count: session.runbook.entries.len(),
        }
    }

    /// Handle Undo command — remove last entry, push to undo stack.
    fn handle_undo(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        if let Some(last) = session.runbook.entries.last().cloned() {
            let sentence = last.sentence.clone();
            session.runbook.remove_entry(last.id);
            session.runbook.push_undo_entry(last);
            let summary = self.runbook_summary(session);
            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::RunbookSummary {
                    chapters: self.chapter_view(session),
                    summary: summary.clone(),
                },
                message: format!("Undone: {}\n\n{}", sentence, summary),
                runbook_summary: Some(summary),
                step_count: session.runbook.entries.len(),
            }
        } else {
            self.invalid_input(session, "Nothing to undo.")
        }
    }

    /// Handle Redo command — restore entry from undo stack.
    fn handle_redo(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        if let Some(entry) = session.runbook.pop_undo_entry() {
            let sentence = entry.sentence.clone();
            session.runbook.add_entry(entry);
            let summary = self.runbook_summary(session);
            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::RunbookSummary {
                    chapters: self.chapter_view(session),
                    summary: summary.clone(),
                },
                message: format!("Restored: {}\n\n{}", sentence, summary),
                runbook_summary: Some(summary),
                step_count: session.runbook.entries.len(),
            }
        } else {
            self.invalid_input(session, "Nothing to redo.")
        }
    }

    /// Handle Clear command — remove all entries.
    fn handle_clear(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        let count = session.runbook.clear();
        let summary = self.runbook_summary(session);
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::RunbookSummary {
                chapters: self.chapter_view(session),
                summary: summary.clone(),
            },
            message: format!("Cleared {} steps.", count),
            runbook_summary: Some(summary),
            step_count: session.runbook.entries.len(),
        }
    }

    /// Handle Info command — show session info and readiness.
    fn handle_info(&self, session: &ReplSessionV2) -> ReplResponseV2 {
        let readiness = session.runbook.readiness();
        let scope = session
            .client_context
            .as_ref()
            .map(|c| c.client_group_name.clone())
            .unwrap_or_else(|| "none".to_string());
        let pack = session
            .journey_context
            .as_ref()
            .map(|c| c.pack.name.clone())
            .unwrap_or_else(|| "none".to_string());

        let mut info = format!(
            "Session: {}\nScope: {}\nPack: {}\nSteps: {} ({} enabled, {} disabled)\nStatus: {:?}\nReady: {}",
            session.id,
            scope,
            pack,
            readiness.total_entries,
            readiness.enabled_entries,
            readiness.disabled_entries,
            session.runbook.status,
            if readiness.ready { "Yes" } else { "No" }
        );

        if !readiness.issues.is_empty() {
            info.push_str("\n\nIssues:");
            for issue in &readiness.issues {
                info.push_str(&format!("\n  Step {}: {}", issue.sequence, issue.issue));
            }
        }

        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::RunbookSummary {
                chapters: self.chapter_view(session),
                summary: info.clone(),
            },
            message: info,
            runbook_summary: None,
            step_count: session.runbook.entries.len(),
        }
    }

    /// Handle Help command — context-appropriate help text.
    fn handle_help(&self, session: &ReplSessionV2) -> ReplResponseV2 {
        let help = match &session.state {
            ReplStateV2::RunbookEditing | ReplStateV2::InPack { .. } => {
                "Commands:\n  /run — Execute the runbook\n  /undo — Undo last action\n  /redo — Restore last undone action\n  /clear — Remove all steps\n  /cancel — Cancel current action\n  /info — Show session status\n  /help — Show this help\n\nYou can also type a message to add steps, or use Edit to modify step arguments."
            }
            ReplStateV2::SentencePlayback { .. } => {
                "Confirm or reject the proposed step, or /cancel to go back."
            }
            _ => "Type a message or use /help for commands.",
        };
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::Error {
                error: help.to_string(),
                recoverable: true,
            },
            message: help.to_string(),
            runbook_summary: None,
            step_count: session.runbook.entries.len(),
        }
    }

    /// Handle Disable command.
    fn handle_disable(&self, session: &mut ReplSessionV2, entry_id: Uuid) -> ReplResponseV2 {
        if session.runbook.disable_entry(entry_id) {
            let summary = self.runbook_summary(session);
            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::RunbookSummary {
                    chapters: self.chapter_view(session),
                    summary: summary.clone(),
                },
                message: format!("Step disabled.\n\n{}", summary),
                runbook_summary: Some(summary),
                step_count: session.runbook.entries.len(),
            }
        } else {
            self.invalid_input(session, "Step not found or already disabled.")
        }
    }

    /// Handle Enable command.
    fn handle_enable(&self, session: &mut ReplSessionV2, entry_id: Uuid) -> ReplResponseV2 {
        if session.runbook.enable_entry(entry_id) {
            let summary = self.runbook_summary(session);
            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::RunbookSummary {
                    chapters: self.chapter_view(session),
                    summary: summary.clone(),
                },
                message: format!("Step enabled.\n\n{}", summary),
                runbook_summary: Some(summary),
                step_count: session.runbook.entries.len(),
            }
        } else {
            self.invalid_input(session, "Step not found or not disabled.")
        }
    }

    /// Handle Toggle command.
    fn handle_toggle(&self, session: &mut ReplSessionV2, entry_id: Uuid) -> ReplResponseV2 {
        match session.runbook.toggle_entry(entry_id) {
            Some(new_status) => {
                let label = if new_status == EntryStatus::Disabled {
                    "disabled"
                } else {
                    "enabled"
                };
                let summary = self.runbook_summary(session);
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::RunbookSummary {
                        chapters: self.chapter_view(session),
                        summary: summary.clone(),
                    },
                    message: format!("Step {}.\n\n{}", label, summary),
                    runbook_summary: Some(summary),
                    step_count: session.runbook.entries.len(),
                }
            }
            None => self.invalid_input(session, "Step not found."),
        }
    }

    async fn execute_runbook(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        self.execute_runbook_from(session, 0).await
    }

    /// Execute the runbook starting from `start_index`.
    ///
    /// Used both for initial execution (start_index=0) and for continuation
    /// after a parked entry resumes (start_index = entry after resumed one).
    async fn execute_runbook_from(
        &self,
        session: &mut ReplSessionV2,
        start_index: usize,
    ) -> ReplResponseV2 {
        if session.runbook.entries.is_empty() {
            return self.invalid_input(session, "Runbook is empty. Add steps first.");
        }

        // Readiness gate — check all entries before executing (only on fresh start).
        if start_index == 0 {
            let report = session.runbook.readiness();
            if !report.ready {
                let issues_text = report
                    .issues
                    .iter()
                    .map(|i| format!("  Step {}: {}", i.sequence, i.issue))
                    .collect::<Vec<_>>()
                    .join("\n");
                return ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Error {
                        error: format!("Runbook not ready:\n{}", issues_text),
                        recoverable: true,
                    },
                    message: format!(
                        "Cannot execute. {} issue(s):\n{}",
                        report.issues.len(),
                        issues_text
                    ),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                };
            }
        }

        session.runbook.set_status(RunbookStatus::Executing);
        let runbook_id = session.runbook.id;
        let total = session.runbook.entries.len();

        session.set_state(ReplStateV2::Executing {
            runbook_id,
            progress: ExecutionProgress::new(total),
        });

        // Execute entries starting from start_index.
        // Stop-on-first-park: when any entry parks, we stop the loop.
        let mut results = Vec::new();
        let mut parked = false;

        for idx in start_index..session.runbook.entries.len() {
            let entry = &session.runbook.entries[idx];
            let entry_id = entry.id;
            let entry_dsl = entry.dsl.clone();
            let entry_sequence = entry.sequence;
            let entry_sentence = entry.sentence.clone();
            let entry_status = entry.status;
            let execution_mode = entry.execution_mode;

            // Skip disabled entries.
            if entry_status == EntryStatus::Disabled {
                results.push(StepResult {
                    entry_id,
                    sequence: entry_sequence,
                    sentence: entry_sentence,
                    success: true,
                    message: Some("Skipped (disabled)".to_string()),
                    result: None,
                });
                continue;
            }

            // Skip already-completed entries (for continuation after resume).
            if entry_status == EntryStatus::Completed {
                continue;
            }

            match execution_mode {
                ExecutionMode::HumanGate => {
                    // Park BEFORE execution — DSL is NOT called.
                    let correlation_key =
                        InvocationRecord::make_correlation_key(runbook_id, entry_id);
                    let mut invocation = InvocationRecord::new(
                        entry_id,
                        runbook_id,
                        session.id,
                        correlation_key.clone(),
                        GateType::HumanApproval,
                    );
                    invocation.captured_context = serde_json::json!({"dsl": entry_dsl});
                    session.runbook.park_entry(entry_id, invocation);
                    session.runbook.set_status(RunbookStatus::Parked);

                    results.push(StepResult {
                        entry_id,
                        sequence: entry_sequence,
                        sentence: entry_sentence,
                        success: true,
                        message: Some(format!(
                            "Awaiting human approval (key: {})",
                            correlation_key
                        )),
                        result: None,
                    });
                    parked = true;
                    break;
                }

                ExecutionMode::Durable => {
                    // Execute, but handle Parked outcome from executor_v2.
                    let outcome = self
                        .execute_entry_v2(&entry_dsl, entry_id, runbook_id)
                        .await;

                    match outcome {
                        DslExecutionOutcome::Completed(result) => {
                            let entry = &mut session.runbook.entries[idx];
                            entry.status = EntryStatus::Completed;
                            entry.result = Some(result.clone());
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: true,
                                message: Some("Completed".to_string()),
                                result: Some(result),
                            });
                        }
                        DslExecutionOutcome::Parked {
                            task_id,
                            correlation_key,
                            timeout,
                            message,
                        } => {
                            let mut invocation = InvocationRecord::new(
                                entry_id,
                                runbook_id,
                                session.id,
                                correlation_key.clone(),
                                GateType::DurableTask,
                            );
                            invocation.task_id = Some(task_id);
                            invocation.timeout_at = timeout.map(|d| chrono::Utc::now() + d);
                            invocation.captured_context = serde_json::json!({"dsl": entry_dsl});
                            session.runbook.park_entry(entry_id, invocation);
                            session.runbook.set_status(RunbookStatus::Parked);

                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: true,
                                message: Some(format!(
                                    "Parked: {} (key: {})",
                                    message, correlation_key
                                )),
                                result: None,
                            });
                            parked = true;
                            break;
                        }
                        DslExecutionOutcome::Failed(err) => {
                            let entry = &mut session.runbook.entries[idx];
                            entry.status = EntryStatus::Failed;
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: false,
                                message: Some(err),
                                result: None,
                            });
                        }
                    }
                }

                ExecutionMode::Sync => {
                    // Standard synchronous execution (unchanged from Phase 4).
                    let entry = &mut session.runbook.entries[idx];
                    entry.status = EntryStatus::Executing;
                    match self.executor.execute(&entry_dsl).await {
                        Ok(result) => {
                            entry.status = EntryStatus::Completed;
                            entry.result = Some(result.clone());
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: true,
                                message: Some("Completed".to_string()),
                                result: Some(result),
                            });
                        }
                        Err(err) => {
                            entry.status = EntryStatus::Failed;
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: false,
                                message: Some(err),
                                result: None,
                            });
                        }
                    }
                }
            }
        }

        if parked {
            // Stay in Executing state — session is parked.
            let parked_entry_id = session
                .runbook
                .entries
                .iter()
                .find(|e| e.status == EntryStatus::Parked)
                .map(|e| e.id);
            let completed = results.iter().filter(|r| r.success).count();

            session.set_state(ReplStateV2::Executing {
                runbook_id,
                progress: ExecutionProgress {
                    total_steps: total,
                    completed_steps: completed.saturating_sub(1), // Parked entry counted as "success" in results
                    failed_steps: results.iter().filter(|r| !r.success).count(),
                    parked_steps: 1,
                    current_step: parked_entry_id,
                    parked_entry_id,
                },
            });

            // Persist session on park (required — durable execution guarantee).
            if let Err(e) = self.persist_session_required(session).await {
                tracing::error!(session_id = %session.id, error = %e, "Failed to persist parked session");
            }

            let summary = self.runbook_summary(session);
            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::Executed { results },
                message: format!(
                    "Execution parked: {} completed, 1 awaiting signal.\n\n{}",
                    completed.saturating_sub(1),
                    summary
                ),
                runbook_summary: Some(summary),
                step_count: session.runbook.entries.len(),
            }
        } else {
            // All entries processed — back to editing.
            let all_success = results.iter().all(|r| r.success);
            session.runbook.set_status(if all_success {
                RunbookStatus::Completed
            } else {
                RunbookStatus::Ready // Allow retry
            });

            session.set_state(ReplStateV2::RunbookEditing);

            // Best-effort persist on completion (non-critical).
            self.maybe_persist_session(session).await;

            let summary = self.runbook_summary(session);
            let succeeded = results.iter().filter(|r| r.success).count();
            let failed = results.iter().filter(|r| !r.success).count();

            ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::Executed { results },
                message: format!(
                    "Execution complete: {} succeeded, {} failed.\n\n{}",
                    succeeded, failed, summary
                ),
                runbook_summary: Some(summary),
                step_count: session.runbook.entries.len(),
            }
        }
    }

    /// Execute an entry using executor_v2 if available, falling back to executor.
    async fn execute_entry_v2(
        &self,
        dsl: &str,
        entry_id: Uuid,
        runbook_id: Uuid,
    ) -> DslExecutionOutcome {
        if let Some(ref exec) = self.executor_v2 {
            exec.execute_v2(dsl, entry_id, runbook_id).await
        } else {
            // Fallback: use the legacy executor (never parks).
            match self.executor.execute(dsl).await {
                Ok(v) => DslExecutionOutcome::Completed(v),
                Err(e) => DslExecutionOutcome::Failed(e),
            }
        }
    }

    /// Continue execution after a parked entry resumes.
    ///
    /// Finds the index of the just-resumed entry and continues from the next entry.
    pub async fn continue_execution(
        &self,
        session: &mut ReplSessionV2,
        resumed_entry_id: Uuid,
    ) -> ReplResponseV2 {
        let resume_idx = session
            .runbook
            .entries
            .iter()
            .position(|e| e.id == resumed_entry_id);

        match resume_idx {
            Some(idx) => self.execute_runbook_from(session, idx + 1).await,
            None => self.invalid_input(session, "Resumed entry not found in runbook."),
        }
    }

    // -----------------------------------------------------------------------
    // Persistence helpers (Phase 5)
    // -----------------------------------------------------------------------

    /// Best-effort persist — logs on failure, doesn't block the caller.
    ///
    /// Used for non-critical checkpoints (e.g., execution completes normally).
    #[allow(unused_variables)]
    async fn maybe_persist_session(&self, session: &ReplSessionV2) {
        #[cfg(feature = "database")]
        {
            if let Some(ref repo) = self.session_repository {
                if let Err(e) = repo.save_session(session, 0).await {
                    tracing::warn!(
                        session_id = %session.id,
                        error = %e,
                        "Best-effort session persist failed"
                    );
                }
            }
        }
    }

    /// Required persist — returns error on failure.
    ///
    /// Used for critical state changes (park, resume, approve, reject)
    /// where losing state would break durable execution guarantees.
    #[allow(unused_variables)]
    async fn persist_session_required(
        &self,
        session: &ReplSessionV2,
    ) -> Result<(), OrchestratorError> {
        #[cfg(feature = "database")]
        {
            if let Some(ref repo) = self.session_repository {
                repo.save_session(session, 0)
                    .await
                    .map_err(|e| OrchestratorError::PersistenceFailed(e.to_string()))?;
                // Also persist any active invocation records.
                for entry in &session.runbook.entries {
                    if let Some(ref inv) = entry.invocation {
                        if let Err(e) = repo.save_invocation(inv).await {
                            tracing::warn!(
                                invocation_id = %inv.invocation_id,
                                error = %e,
                                "Failed to persist invocation record"
                            );
                        }
                    }
                }
                return Ok(());
            }
        }
        // No repository configured — this is fine for test scenarios.
        Ok(())
    }

    /// Delete a session from persistence (if configured).
    #[allow(unused_variables)]
    async fn maybe_delete_persisted_session(&self, session_id: Uuid) {
        #[cfg(feature = "database")]
        {
            if let Some(ref repo) = self.session_repository {
                if let Err(e) = repo.delete_session(session_id).await {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "Failed to delete persisted session"
                    );
                }
            }
        }
    }

    fn runbook_summary(&self, session: &ReplSessionV2) -> String {
        if let Some(ref ctx) = session.journey_context {
            PackPlayback::summarize(&ctx.pack, &session.runbook, &ctx.answers)
        } else {
            format!("{} steps in runbook", session.runbook.entries.len())
        }
    }

    fn chapter_view(&self, session: &ReplSessionV2) -> Vec<ChapterView> {
        if let Some(ref ctx) = session.journey_context {
            PackPlayback::chapter_view(&ctx.pack, &session.runbook)
                .into_iter()
                .map(|c| ChapterView {
                    chapter: c.chapter,
                    steps: c.steps,
                })
                .collect()
        } else {
            vec![ChapterView {
                chapter: "Steps".to_string(),
                steps: session
                    .runbook
                    .entries
                    .iter()
                    .map(|e| (e.sequence, e.sentence.clone()))
                    .collect(),
            }]
        }
    }

    fn invalid_input(&self, session: &ReplSessionV2, message: &str) -> ReplResponseV2 {
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::Error {
                error: message.to_string(),
                recoverable: true,
            },
            message: message.to_string(),
            runbook_summary: None,
            step_count: session.runbook.entries.len(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Extract keyword arguments from a DSL s-expression string.
///
/// Parses patterns like `(verb :key "value" :key2 value2)` into a HashMap.
/// This is a lightweight extractor — not a full parser. It handles quoted
/// string values and bare word values.
fn extract_args_from_dsl(dsl: &str) -> HashMap<String, String> {
    let mut args = HashMap::new();
    let content = dsl.trim().trim_start_matches('(').trim_end_matches(')');
    let mut chars = content.chars().peekable();
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();

    // Tokenize: split on whitespace, respecting quoted strings.
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                // Consume quoted string.
                let mut quoted = String::new();
                loop {
                    match chars.next() {
                        Some('"') | None => break,
                        Some('\\') => {
                            if let Some(escaped) = chars.next() {
                                quoted.push(escaped);
                            }
                        }
                        Some(c) => quoted.push(c),
                    }
                }
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(format!("\"{}\"", quoted));
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // Skip the verb (first token), then parse :key value pairs.
    let mut i = 1; // skip verb
    while i < tokens.len() {
        if tokens[i].starts_with(':') {
            let key = tokens[i][1..].to_string();
            if i + 1 < tokens.len() && !tokens[i + 1].starts_with(':') {
                let val = tokens[i + 1].trim_matches('"').to_string();
                args.insert(key, val);
                i += 2;
            } else {
                // Flag-style keyword with no value — store as "true".
                args.insert(key, "true".to_string());
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    args
}

/// Rebuild an s-expression DSL string from verb + args.
/// Inverse of `extract_args_from_dsl()`.
pub fn rebuild_dsl(verb: &str, args: &HashMap<String, String>) -> String {
    if args.is_empty() {
        return format!("({})", verb);
    }
    let mut parts = vec![format!("({}", verb)];
    let mut sorted_keys: Vec<_> = args.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys {
        let val = &args[key];
        if val.contains(' ') || val.contains('"') {
            parts.push(format!(":{} \"{}\"", key, val.replace('"', "\\\"")));
        } else {
            parts.push(format!(":{} {}", key, val));
        }
    }
    format!("{})", parts.join(" "))
}

/// Build an `ArgExtractionAudit` from an IntentMatcher result.
///
/// Uses debug info from the match when available, with sensible defaults
/// for fields not yet provided by the current pipeline.
fn build_arg_extraction_audit(
    user_input: &str,
    extracted_args: &HashMap<String, String>,
    confidence: f32,
    debug: Option<&crate::repl::types::MatchDebugInfo>,
) -> ArgExtractionAudit {
    use sha2::{Digest, Sha256};

    // Derive a prompt hash from debug notes (or use a placeholder).
    let prompt_hash = if let Some(dbg) = debug {
        let material = dbg.notes.join("|");
        let hash = Sha256::digest(material.as_bytes());
        format!("{:x}", hash)[..16].to_string()
    } else {
        "no_debug_info".to_string()
    };

    // Extract model_id from debug search_tier if available.
    let model_id = debug
        .and_then(|d| d.search_tier.clone())
        .unwrap_or_else(|| "hybrid_intent_matcher".to_string());

    ArgExtractionAudit {
        model_id,
        prompt_hash,
        user_input: user_input.to_string(),
        extracted_args: extracted_args.clone(),
        confidence: confidence as f64,
        timestamp: chrono::Utc::now(),
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum OrchestratorError {
    SessionNotFound(Uuid),
    /// A required persistence operation failed (e.g., parking checkpoint).
    PersistenceFailed(String),
    /// A persistence-requiring operation was attempted but no repository is configured.
    NoPersistenceConfigured,
}

impl std::fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionNotFound(id) => write!(f, "Session not found: {}", id),
            Self::PersistenceFailed(msg) => write!(f, "Session persistence failed: {}", msg),
            Self::NoPersistenceConfigured => {
                write!(
                    f,
                    "Session persistence required but no repository configured"
                )
            }
        }
    }
}

impl std::error::Error for OrchestratorError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey::pack::load_pack_from_bytes;

    fn onboarding_yaml() -> &'static str {
        r#"
id: onboarding-request
name: Onboarding Request
version: "1.0"
description: Onboard a new client structure
invocation_phrases:
  - "onboard a client"
  - "set up new client"
required_context:
  - client_group_id
required_questions:
  - field: products
    prompt: "Which products should be added?"
    answer_kind: list
  - field: jurisdiction
    prompt: "Which jurisdiction?"
    answer_kind: string
    default: "LU"
templates:
  - template_id: basic-onboarding
    when_to_use: "Standard onboarding"
    steps:
      - verb: cbu.create
        args:
          name: "{context.client_name}"
          jurisdiction: "{answers.jurisdiction}"
      - verb: cbu.assign-product
        repeat_for: "answers.products"
        args:
          product: "{item}"
definition_of_done:
  - "All products assigned"
"#
    }

    fn make_orchestrator() -> ReplOrchestratorV2 {
        let (manifest, hash) = load_pack_from_bytes(onboarding_yaml().as_bytes()).unwrap();
        let packs = vec![(Arc::new(manifest), hash)];
        let router = PackRouter::new(packs);
        ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
    }

    #[tokio::test]
    async fn test_create_session() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;
        let session = orch.get_session(id).await.unwrap();
        assert!(matches!(session.state, ReplStateV2::ScopeGate { .. }));
    }

    #[tokio::test]
    async fn test_scope_gate_requires_scope() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;

        let resp = orch
            .process(
                id,
                UserInputV2::Message {
                    content: "hello".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            resp.kind,
            ReplResponseKindV2::ScopeRequired { .. }
        ));
    }

    #[tokio::test]
    async fn test_select_scope_transitions_to_journey_selection() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;

        let resp = orch
            .process(
                id,
                UserInputV2::SelectScope {
                    group_id: Uuid::new_v4(),
                    group_name: "Allianz".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            resp.kind,
            ReplResponseKindV2::JourneyOptions { .. }
        ));
        assert!(resp.message.contains("Allianz"));
    }

    #[tokio::test]
    async fn test_select_pack_transitions_to_in_pack() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;

        // Set scope.
        orch.process(
            id,
            UserInputV2::SelectScope {
                group_id: Uuid::new_v4(),
                group_name: "Allianz".to_string(),
            },
        )
        .await
        .unwrap();

        // Select pack.
        let resp = orch
            .process(
                id,
                UserInputV2::SelectPack {
                    pack_id: "onboarding-request".to_string(),
                },
            )
            .await
            .unwrap();

        // Should ask the first required question.
        assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
        assert!(resp.message.contains("products"));
    }

    #[tokio::test]
    async fn test_route_pack_by_phrase() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;

        // Set scope.
        orch.process(
            id,
            UserInputV2::SelectScope {
                group_id: Uuid::new_v4(),
                group_name: "Allianz".to_string(),
            },
        )
        .await
        .unwrap();

        // Route via phrase.
        let resp = orch
            .process(
                id,
                UserInputV2::Message {
                    content: "onboard a client".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
    }

    #[tokio::test]
    async fn test_force_select_pack() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;

        // Set scope.
        orch.process(
            id,
            UserInputV2::SelectScope {
                group_id: Uuid::new_v4(),
                group_name: "Allianz".to_string(),
            },
        )
        .await
        .unwrap();

        // Force-select by name.
        let resp = orch
            .process(
                id,
                UserInputV2::Message {
                    content: "use the onboarding request journey".to_string(),
                },
            )
            .await
            .unwrap();

        // Should activate the pack (first question).
        assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
    }

    #[tokio::test]
    async fn test_golden_loop_stub() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;

        // 1. Set scope.
        orch.process(
            id,
            UserInputV2::SelectScope {
                group_id: Uuid::new_v4(),
                group_name: "Allianz".to_string(),
            },
        )
        .await
        .unwrap();

        // 2. Select pack.
        orch.process(
            id,
            UserInputV2::SelectPack {
                pack_id: "onboarding-request".to_string(),
            },
        )
        .await
        .unwrap();

        // 3. Answer Q1: products.
        orch.process(
            id,
            UserInputV2::Message {
                content: "IRS, EQUITY".to_string(),
            },
        )
        .await
        .unwrap();

        // 4. Answer Q2: jurisdiction → triggers template instantiation.
        let resp = orch
            .process(
                id,
                UserInputV2::Message {
                    content: "LU".to_string(),
                },
            )
            .await
            .unwrap();

        // Should have built a runbook.
        assert!(matches!(
            resp.kind,
            ReplResponseKindV2::RunbookSummary { .. }
        ));

        // Verify session state.
        let session = orch.get_session(id).await.unwrap();
        assert!(!session.runbook.entries.is_empty());
        assert!(session.runbook.template_id.is_some());
        assert!(session.runbook.template_hash.is_some());

        // 5. Execute.
        let resp = orch
            .process(
                id,
                UserInputV2::Command {
                    command: ReplCommandV2::Run,
                },
            )
            .await
            .unwrap();

        assert!(matches!(resp.kind, ReplResponseKindV2::Executed { .. }));

        // Verify all entries completed.
        let session = orch.get_session(id).await.unwrap();
        for entry in &session.runbook.entries {
            assert_eq!(entry.status, EntryStatus::Completed);
            assert!(entry.result.is_some());
        }
        assert_eq!(session.runbook.status, RunbookStatus::Completed);
    }

    #[tokio::test]
    async fn test_session_not_found() {
        let orch = make_orchestrator();
        let result = orch.process(Uuid::new_v4(), UserInputV2::Confirm).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sentence_playback_reject() {
        let orch = make_orchestrator();
        let id = orch.create_session().await;

        // Set scope + select pack + answer questions.
        orch.process(
            id,
            UserInputV2::SelectScope {
                group_id: Uuid::new_v4(),
                group_name: "Test".to_string(),
            },
        )
        .await
        .unwrap();

        orch.process(
            id,
            UserInputV2::SelectPack {
                pack_id: "onboarding-request".to_string(),
            },
        )
        .await
        .unwrap();

        orch.process(
            id,
            UserInputV2::Message {
                content: "IRS".to_string(),
            },
        )
        .await
        .unwrap();

        // Answer Q2 → builds runbook.
        orch.process(
            id,
            UserInputV2::Message {
                content: "LU".to_string(),
            },
        )
        .await
        .unwrap();

        // Now add a new step via message → goes to SentencePlayback.
        let resp = orch
            .process(
                id,
                UserInputV2::Message {
                    content: "also add custody account".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            resp.kind,
            ReplResponseKindV2::SentencePlayback { .. }
        ));

        // Reject it.
        let resp = orch.process(id, UserInputV2::Reject).await.unwrap();

        assert!(resp.message.contains("Rejected"));
    }

    // -----------------------------------------------------------------------
    // extract_args_from_dsl tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_args_simple() {
        let args = extract_args_from_dsl(r#"(cbu.create :name "Allianz Lux" :jurisdiction "LU")"#);
        assert_eq!(args.get("name").unwrap(), "Allianz Lux");
        assert_eq!(args.get("jurisdiction").unwrap(), "LU");
    }

    #[test]
    fn test_extract_args_bare_values() {
        let args = extract_args_from_dsl("(session.load-galaxy :apex-name allianz)");
        assert_eq!(args.get("apex-name").unwrap(), "allianz");
    }

    #[test]
    fn test_extract_args_flag_keyword() {
        let args = extract_args_from_dsl("(cbu.create :name test :dry-run)");
        assert_eq!(args.get("name").unwrap(), "test");
        assert_eq!(args.get("dry-run").unwrap(), "true");
    }

    #[test]
    fn test_extract_args_empty_dsl() {
        let args = extract_args_from_dsl("(cbu.create)");
        assert!(args.is_empty());
    }

    #[test]
    fn test_extract_args_escaped_quotes() {
        let args = extract_args_from_dsl(r#"(entity.create :name "O\"Brien Corp")"#);
        assert_eq!(args.get("name").unwrap(), "O\"Brien Corp");
    }

    // -----------------------------------------------------------------------
    // build_arg_extraction_audit tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_audit_without_debug() {
        let args = HashMap::from([("name".to_string(), "Allianz".to_string())]);
        let audit = build_arg_extraction_audit("create allianz", &args, 0.85, None);

        assert_eq!(audit.user_input, "create allianz");
        assert_eq!(audit.extracted_args.get("name").unwrap(), "Allianz");
        assert!((audit.confidence - 0.85).abs() < 0.001);
        assert_eq!(audit.prompt_hash, "no_debug_info");
        assert_eq!(audit.model_id, "hybrid_intent_matcher");
    }

    #[test]
    fn test_build_audit_with_debug() {
        let debug = crate::repl::types::MatchDebugInfo {
            timing: vec![],
            search_tier: Some("global_semantic".to_string()),
            entity_linking: None,
            notes: vec!["matched via bge-small".to_string()],
        };
        let args = HashMap::new();
        let audit = build_arg_extraction_audit("load allianz", &args, 0.92, Some(&debug));

        assert_eq!(audit.model_id, "global_semantic");
        assert!(!audit.prompt_hash.is_empty());
        assert_ne!(audit.prompt_hash, "no_debug_info");
    }

    // -----------------------------------------------------------------------
    // ArgExtractionAudit on runbook entries
    // -----------------------------------------------------------------------

    #[test]
    fn test_template_entries_have_no_audit() {
        // Template-derived entries should NOT have arg_extraction_audit.
        let entry = super::RunbookEntry::new(
            "cbu.create".to_string(),
            "Create fund".to_string(),
            "(cbu.create :name test)".to_string(),
        );
        assert!(entry.arg_extraction_audit.is_none());
    }

    // -----------------------------------------------------------------------
    // DslExecutorV2 / ParkableStubExecutor tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_stub_executor_adapts_to_v2() {
        let stub = StubExecutor;
        let result = stub
            .execute_v2(
                "(cbu.create :name \"test\")",
                Uuid::new_v4(),
                Uuid::new_v4(),
            )
            .await;
        match result {
            DslExecutionOutcome::Completed(v) => {
                assert_eq!(v["status"], "stub_success");
            }
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_parkable_stub_parks_on_marker() {
        let executor = ParkableStubExecutor;
        let entry_id = Uuid::new_v4();
        let runbook_id = Uuid::new_v4();
        let result = executor
            .execute_v2("(doc.solicit :park)", entry_id, runbook_id)
            .await;
        match result {
            DslExecutionOutcome::Parked {
                correlation_key,
                message,
                ..
            } => {
                assert!(correlation_key.contains(&runbook_id.to_string()));
                assert!(correlation_key.contains(&entry_id.to_string()));
                assert!(!message.is_empty());
            }
            other => panic!("Expected Parked, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_parkable_stub_completes_on_normal_dsl() {
        let executor = ParkableStubExecutor;
        let result = executor
            .execute_v2(
                "(cbu.create :name \"test\")",
                Uuid::new_v4(),
                Uuid::new_v4(),
            )
            .await;
        match result {
            DslExecutionOutcome::Completed(v) => {
                assert_eq!(v["status"], "stub_success");
            }
            other => panic!("Expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_dsl_execution_outcome_serialization() {
        let outcomes = vec![
            DslExecutionOutcome::Completed(serde_json::json!({"id": "abc"})),
            DslExecutionOutcome::Parked {
                task_id: Uuid::new_v4(),
                correlation_key: "key-123".into(),
                timeout: None,
                message: "Waiting".into(),
            },
            DslExecutionOutcome::Failed("error".into()),
        ];
        for outcome in &outcomes {
            let json = serde_json::to_string(outcome).unwrap();
            let deserialized: DslExecutionOutcome = serde_json::from_str(&json).unwrap();
            // Just verify roundtrip doesn't panic.
            let _ = format!("{:?}", deserialized);
        }
    }
}
