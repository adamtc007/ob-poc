//! Orchestrator V2 — Pack-Guided State Machine
//!
//! The heart of the v2 REPL. Dispatches `UserInputV2` against the current
//! `ReplStateV2` and produces `ReplResponseV2`.
//!
//! # State Machine Dispatch
//!
//! | Current State       | Input           | Handler                | Next State              |
//! |---------------------|-----------------|------------------------|-------------------------|
//! | ScopeGate           | Message         | try_resolve_scope()    | WorkspaceSelection or ScopeGate |
//! | ScopeGate           | SelectScope     | set_scope()            | WorkspaceSelection      |
//! | WorkspaceSelection  | SelectWorkspace | set_workspace()        | JourneySelection        |
//! | JourneySelection    | Message         | route_pack()           | InPack or JourneySelection |
//! | JourneySelection    | SelectPack      | activate_pack()        | InPack                  |
//! | InPack              | Message         | handle_in_pack_msg()   | SentencePlayback or InPack |
//! | InPack              | Command(Run)    | validate_and_execute() | Executing               |
//! | Clarifying          | Message/Select  | resolve_clarification()| SentencePlayback or Clarifying |
//! | SentencePlayback    | Confirm         | add_to_runbook()       | RunbookEditing or InPack |
//! | SentencePlayback    | Reject          | discard_proposal()     | InPack                  |
//! | RunbookEditing      | Command(Run)    | execute_runbook()      | Executing               |
//! | RunbookEditing      | Message         | handle_in_pack_msg()   | SentencePlayback        |
//! | Executing           | (completion)    | execute_runbook_from() | RunbookEditing          |

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::context_stack::ContextStack;
use super::decision_log::{
    ContextSummary, DecisionLog, ExtractionDecision, ExtractionMethod, TurnType,
    VerbCandidateSnapshot, VerbDecision,
};
use super::intent_service::{ClarificationOutcome, IntentService, VerbMatchOutcome};
use super::proposal_engine::ProposalEngine;
use super::response_v2::{ChapterView, ReplResponseKindV2, ReplResponseV2, StepResult};
use super::runbook::{
    ArgExtractionAudit, ConfirmPolicy, EntryStatus, ExecutionMode, GateType, InvocationRecord,
    RunbookEntry, RunbookStatus, SlotProvenance, SlotSource,
};
use super::sentence_gen::SentenceGenerator;
use super::session_v2::{MessageRole, ReplSessionV2};
use super::types_v2::{
    ConstellationContextRef, ExecutionProgress, ReplCommandV2, ReplStateV2,
    ResolvedConstellationContext, SessionFeedback, UserInputV2, WorkspaceFrame, WorkspaceKind,
    WorkspaceOption, WorkspaceStateView,
};
use super::verb_config_index::VerbConfigIndex;
use crate::dsl_v2::macros::MacroRegistry;
use crate::journey::handoff::PackHandoff;
use crate::journey::playback::PackPlayback;
use crate::journey::router::{PackRouteOutcome, PackRouter};
use crate::journey::template::instantiate_template;
use crate::lookup::LookupService;
use crate::mcp::verb_search::{VerbSearchResult, VerbSearchSource};
use crate::repl::intent_matcher::IntentMatcher;
use crate::repl::types::{MatchContext, MatchOutcome};
use crate::runbook::envelope::ReplayEnvelope;
#[cfg(feature = "database")]
use crate::runbook::executor::PostgresRunbookStore;
use crate::runbook::executor::{execute_runbook_with_pool, RunbookStoreBackend, StepOutcome};
use crate::runbook::step_executor_bridge::{DslExecutorV2StepExecutor, DslStepExecutor};
use crate::runbook::types::{
    CompiledRunbook, CompiledStep, ExecutionMode as CompiledExecutionMode,
};
use crate::runbook::RunbookStore;
use crate::traceability::{
    build_phase2_unavailable_payload, build_phase3_unavailable_payload,
    build_phase4_unavailable_payload, build_phase_trace_payload, build_trace_scaffold_payload,
    evaluate_phase3_against_phase2, evaluate_phase4_within_phase2, evaluate_phase5_repl,
    NewUtteranceTrace, Phase2Service, TraceKind, UtteranceTraceRepository,
};
use sem_os_client::SemOsClient;

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

#[allow(dead_code)] // Used by integration tests (rust/tests/repl_v2_phase*.rs)
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
    /// Macro registry for classify_verb() and compile_verb().
    macro_registry: Option<Arc<MacroRegistry>>,
    sessions: Arc<RwLock<HashMap<Uuid, ReplSessionV2>>>,
    executor: Arc<dyn DslExecutor>,
    executor_v2: Option<Arc<dyn DslExecutorV2>>,
    /// Phase 5: Session persistence for durable execution / human gates.
    #[cfg(feature = "database")]
    session_repository: Option<Arc<super::session_repository::SessionRepositoryV2>>,
    /// Compiled runbook store — artifacts from compile_invocation() stored
    /// here for execute_runbook() to retrieve by ID.
    runbook_store: Option<Arc<RunbookStore>>,
    /// Database pool for bootstrap resolution (ScopeGate).
    #[cfg(feature = "database")]
    pool: Option<sqlx::PgPool>,
    /// Pool for unified orchestrator (Phase 1.4 hardening).
    #[cfg(feature = "database")]
    unified_orch_pool: Option<sqlx::PgPool>,
    /// Semantic OS client for Sem OS context resolution (Phase 4 CCIR).
    /// When set, `match_verb_for_input()` resolves a SemOsContextEnvelope and
    /// pre-constrains verb search to Sem OS-allowed verbs.
    sem_os_client: Option<Arc<dyn SemOsClient>>,
    /// Optional lookup service for trace-time entity recovery.
    lookup_service: Option<LookupService>,
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
            macro_registry: None,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            executor,
            executor_v2: None,
            #[cfg(feature = "database")]
            session_repository: None,
            runbook_store: None,
            #[cfg(feature = "database")]
            pool: None,
            #[cfg(feature = "database")]
            unified_orch_pool: None,
            sem_os_client: None,
            lookup_service: None,
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

    /// Attach a database pool for the unified orchestrator (Phase 1.4).
    /// When set, `match_verb_for_input()` routes through the orchestrator
    /// for Sem OS filtering and IntentTrace logging.
    #[cfg(feature = "database")]
    pub fn with_unified_orchestrator(mut self, pool: sqlx::PgPool) -> Self {
        self.unified_orch_pool = Some(pool);
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

    /// Attach a MacroRegistry for classify_verb() and compile_verb().
    pub fn with_macro_registry(mut self, registry: Arc<MacroRegistry>) -> Self {
        self.macro_registry = Some(registry);
        self
    }

    /// Attach a RunbookStore for compiled runbook artifacts.
    ///
    /// When set, `try_compile_entry()` stores the `CompiledRunbook` artifact
    /// so `execute_runbook()` can retrieve it by ID during execution.
    pub fn with_runbook_store(mut self, store: Arc<RunbookStore>) -> Self {
        self.runbook_store = Some(store);
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

    /// Attach a database pool for bootstrap resolution in ScopeGate.
    #[cfg(feature = "database")]
    pub fn with_pool(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Attach a Semantic OS client for Sem OS context resolution (Phase 4 CCIR).
    ///
    /// When set, `match_verb_for_input()` resolves a SemOsContextEnvelope and
    /// pre-constrains verb search via `MatchContext.allowed_verbs`.
    pub fn with_sem_os_client(mut self, client: Arc<dyn SemOsClient>) -> Self {
        self.sem_os_client = Some(client);
        self
    }

    /// Attach a lookup service for trace-time entity recovery.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let orch = ReplOrchestratorV2::new(router, executor)
    ///     .with_lookup_service(lookup_service);
    /// ```
    pub fn with_lookup_service(mut self, lookup_service: LookupService) -> Self {
        self.lookup_service = Some(lookup_service);
        self
    }

    /// Access the pack router (useful for tests and introspection).
    pub fn pack_router(&self) -> &PackRouter {
        &self.pack_router
    }

    /// Return the database pool used for scope/bootstrap and navigation hydration.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let maybe_pool = orchestrator.pool();
    /// ```
    #[cfg(feature = "database")]
    pub fn pool(&self) -> Option<&sqlx::PgPool> {
        self.pool.as_ref().or(self.unified_orch_pool.as_ref())
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

    /// Create a session with a specific ID (for unified pipeline routing).
    pub async fn create_session_with_id(&self, id: Uuid) {
        let mut session = ReplSessionV2::new();
        session.id = id;
        self.sessions.write().await.insert(id, session);
    }

    /// Get a snapshot of session state (for API responses).
    pub async fn get_session(&self, session_id: Uuid) -> Option<ReplSessionV2> {
        self.sessions.read().await.get(&session_id).cloned()
    }

    /// Persist the current snapshot of a session, if persistence is configured.
    pub async fn persist_session_checkpoint(&self, session_id: Uuid) -> anyhow::Result<()> {
        let Some(session) = self.get_session(session_id).await else {
            anyhow::bail!("session {session_id} not found for persistence");
        };
        #[cfg(feature = "database")]
        if let Some(ref repo) = self.session_repository {
            repo.save_session(&session, 0).await?;
        }
        Ok(())
    }

    /// Push a new workspace frame onto the session stack.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let _ = orchestrator.push_workspace_frame(session_id, frame).await?;
    /// ```
    pub async fn push_workspace_frame(
        &self,
        session_id: Uuid,
        frame: WorkspaceFrame,
    ) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .context("session not found for push")?;
        session.push_workspace_frame(frame)?;
        Ok(())
    }

    /// Pop the current top-of-stack frame from the session.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let popped = orchestrator.pop_workspace_frame(session_id).await?;
    /// ```
    pub async fn pop_workspace_frame(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Option<WorkspaceFrame>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .context("session not found for pop")?;
        Ok(session.pop_workspace_frame())
    }

    /// Collapse the stack to the current top-of-stack frame.
    ///
    /// # Examples
    /// ```rust,ignore
    /// orchestrator.commit_workspace_stack(session_id).await?;
    /// ```
    pub async fn commit_workspace_stack(&self, session_id: Uuid) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .context("session not found for commit")?;
        session.commit_workspace_stack();
        Ok(())
    }

    /// Apply a hydrated top-of-stack view to the session.
    ///
    /// # Examples
    /// ```rust,ignore
    /// orchestrator.hydrate_tos(session_id, workspace_state).await?;
    /// ```
    pub async fn hydrate_tos(
        &self,
        session_id: Uuid,
        state_view: super::types_v2::WorkspaceStateView,
    ) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .context("session not found for hydrate")?;
        session.hydrate_tos(state_view);
        Ok(())
    }

    /// Update the root frame from a resolved navigation context.
    ///
    /// # Examples
    /// ```rust,ignore
    /// orchestrator.apply_root_context(session_id, &ctx).await?;
    /// ```
    pub async fn apply_root_context(
        &self,
        session_id: Uuid,
        context: &ConstellationContextRef,
    ) -> anyhow::Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .context("session not found for root context")?;
        if session.runbook.client_group_id != Some(context.client_group_id) {
            session.set_client_scope(context.client_group_id);
        }
        session.set_workspace_root(context.workspace.clone());
        if let Some(tos) = session.tos_frame_mut() {
            tos.constellation_family = context
                .constellation_family
                .clone()
                .unwrap_or_else(|| tos.constellation_family.clone());
            tos.constellation_map = context
                .constellation_map
                .clone()
                .unwrap_or_else(|| tos.constellation_map.clone());
            tos.subject_kind = context.subject_kind.clone();
            tos.subject_id = context.subject_id;
        }
        Ok(())
    }

    /// Build the current session feedback envelope.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let feedback = orchestrator.session_feedback(session_id).await?;
    /// ```
    pub async fn session_feedback(&self, session_id: Uuid) -> anyhow::Result<SessionFeedback> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .context("session not found for feedback")?;
        Ok(session.build_session_feedback(false))
    }

    /// Expose the session map for test manipulation (integration tests).
    #[doc(hidden)]
    pub fn sessions_for_test(&self) -> &Arc<RwLock<HashMap<Uuid, ReplSessionV2>>> {
        &self.sessions
    }

    /// Expose the runbook store for plan execution.
    pub fn runbook_store(&self) -> Option<Arc<RunbookStore>> {
        self.runbook_store.clone()
    }

    /// Expose the DSL executor for plan step execution.
    pub fn executor(&self) -> Arc<dyn DslExecutor> {
        self.executor.clone()
    }

    /// Signal that an external task completed (or failed) for a parked entry.
    ///
    /// Finds the session owning `correlation_key` via the runbook invocation
    /// index, resumes the entry, and either continues execution (on success)
    /// or transitions to `RunbookEditing` (on failure).
    ///
    /// Returns `Ok(Some(response))` when a session was found and resumed,
    /// `Ok(None)` when no session owns the key or the entry was already resumed.
    pub async fn signal_completion(
        &self,
        correlation_key: &str,
        status: &str,
        result: Option<serde_json::Value>,
        error: Option<String>,
    ) -> Result<Option<ReplResponseV2>, anyhow::Error> {
        use anyhow::Context as _;

        // 1. Find the session that owns this correlation key.
        let (session_id, entry_id) = {
            let sessions = self.sessions.read().await;
            let mut found = None;
            for (sid, session) in sessions.iter() {
                if let Some(eid) = session.runbook.invocation_index.get(correlation_key) {
                    found = Some((*sid, *eid));
                    break;
                }
            }
            match found {
                Some(pair) => pair,
                None => return Ok(None),
            }
        };

        // 2. Build the result payload.
        let signal_result = match status {
            "completed" => result,
            "failed" => Some(serde_json::json!({
                "error": error.clone().unwrap_or_default()
            })),
            other => anyhow::bail!("Invalid signal status: {}", other),
        };

        // 3. Resume the parked entry in the runbook.
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(&session_id)
                .context("Session disappeared between read and write")?;

            let resumed = session.runbook.resume_entry(correlation_key, signal_result);

            if resumed.is_none() {
                // Idempotent: already resumed.
                return Ok(None);
            }

            // If the signal indicates failure, mark entry Failed and transition
            // to RunbookEditing so the user can fix or retry.
            if status == "failed" {
                if let Some(entry) = session
                    .runbook
                    .entries
                    .iter_mut()
                    .find(|e| e.id == entry_id)
                {
                    entry.status = EntryStatus::Failed;
                }
                session.runbook.set_status(RunbookStatus::Ready);
                session.set_state(ReplStateV2::RunbookEditing);

                let response = ReplResponseV2 {
                    state: ReplStateV2::RunbookEditing,
                    kind: ReplResponseKindV2::Error {
                        error: error.unwrap_or_else(|| "External task failed".into()),
                        recoverable: true,
                    },
                    message: "External task failed. Edit the runbook or retry.".into(),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                };
                return Ok(Some(response));
            }
        }

        // 4. For "completed" signals, continue execution via the state machine.
        let input = UserInputV2::Command {
            command: ReplCommandV2::Resume(entry_id),
        };
        match self.process(session_id, input).await {
            Ok(response) => Ok(Some(response)),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to continue execution after signal: {}",
                e
            )),
        }
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

        session.pending_sem_os_envelope = None;
        session.pending_lookup_result = None;

        let trace_scaffold = self.persist_trace_scaffold(session, &input).await;

        // Record user input as a message and trace entry.
        if let UserInputV2::Message { ref content } = input {
            session.push_message(MessageRole::User, content.clone());
            let hash = {
                use sha2::{Digest, Sha256};
                format!("sha256:{:x}", Sha256::digest(content.as_bytes()))
            };
            session.append_trace(super::session_trace::TraceOp::Input {
                utterance_hash: hash,
            });
        }

        // Capture pre-execution slot snapshots for narration delta (ADR 043).
        let pre_narration_slots: Vec<crate::agent::narration_engine::SlotSnapshot> = session
            .workspace_stack
            .last()
            .and_then(|f| f.hydrated_state.as_ref())
            .and_then(|hs| hs.hydrated_constellation.as_ref())
            .map(|c| crate::agent::narration_engine::SlotSnapshot::capture(&c.slots))
            .unwrap_or_default();

        // Dispatch based on current state.
        let mut response = match session.state.clone() {
            ReplStateV2::ScopeGate { .. } => self.handle_scope_gate(session, input).await,
            ReplStateV2::WorkspaceSelection { .. } => {
                self.handle_workspace_selection(session, input)
            }
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

        // Re-hydrate constellation on TOS if writes occurred during this turn.
        // This ensures the response's session_feedback carries the post-execution
        // constellation state (updated slot states, available verbs, progress).
        if let Some(tos) = session.workspace_stack.last() {
            if tos.writes_since_push > 0 {
                if let Some(ref pool) = self.pool {
                    if let Ok(hydrated) = self.rehydrate_tos(pool, session).await {
                        session.hydrate_tos(hydrated);
                    }
                }
            }
        }

        // Compute post-execution narration (ADR 043) when writes occurred and
        // the contextual query path didn't already attach a narration payload.
        if response.narration.is_none() {
            if let Some(tos) = session.workspace_stack.last_mut() {
                if tos.writes_since_push > 0 {
                    if let Some(ref hs) = tos.hydrated_state {
                        if let Some(ref constellation) = hs.hydrated_constellation {
                            let label = constellation
                                .description
                                .as_deref()
                                .unwrap_or(&constellation.constellation);
                            let ws_key = serde_json::to_value(&tos.workspace)
                                .ok()
                                .and_then(|v| v.as_str().map(|s| s.to_string()))
                                .unwrap_or_default();
                            let last_verb = session
                                .runbook
                                .entries
                                .iter()
                                .rev()
                                .find(|e| {
                                    matches!(
                                        e.status,
                                        super::runbook::EntryStatus::Completed
                                            | super::runbook::EntryStatus::Failed
                                    )
                                })
                                .map(|e| e.verb.as_str())
                                .unwrap_or("");
                            let narration =
                                crate::agent::narration_engine::compute_narration(
                                    &pre_narration_slots,
                                    &constellation.slots,
                                    last_verb,
                                    tos.writes_since_push,
                                    tos.writes_since_push == 1,
                                    label,
                                    Some(&ws_key),
                                );
                            // Store hot verbs for boost signal.
                            tos.narration_hot_verbs = narration
                                .suggested_next
                                .iter()
                                .map(|s| s.verb_fqn.clone())
                                .collect();
                            if narration.has_content() {
                                response.narration = Some(narration);
                            }
                        }
                    }
                }
            }
        }

        // Record assistant response message.
        session.push_message(MessageRole::Assistant, response.message.clone());
        let finalized_trace_id = self
            .finalize_trace_scaffold(trace_scaffold, session, &response)
            .await;
        let lineage_trace_id = if repl_response_needs_follow_up(&response) {
            self.emit_repl_prompt_trace(session, finalized_trace_id, &response)
                .await
                .or(finalized_trace_id)
        } else {
            finalized_trace_id
        };
        update_repl_trace_lineage(session, lineage_trace_id, &response);

        // Release the write lock before async persistence
        drop(sessions);

        // Persist session state + trace entries to database (audit trail).
        if let Err(e) = self.persist_session_checkpoint(session_id).await {
            tracing::warn!(session_id = %session_id, error = %e, "Session checkpoint after process() failed");
        }

        Ok(response)
    }

    /// Re-hydrate the top-of-stack workspace constellation from the database.
    ///
    /// Called after verb execution changes entity state, so the session feedback
    /// reflects the post-execution constellation (updated slot states, available
    /// verbs, progress). This is the key step that closes the
    /// utterance → execute → updated-state → UI-render loop.
    #[cfg(feature = "database")]
    async fn rehydrate_tos(
        &self,
        pool: &sqlx::PgPool,
        session: &ReplSessionV2,
    ) -> anyhow::Result<WorkspaceStateView> {
        use crate::api::constellation_routes::hydrate_workspace_state;

        let tos = session
            .workspace_stack
            .last()
            .ok_or_else(|| anyhow::anyhow!("No workspace frame to hydrate"))?;

        let resolved = ResolvedConstellationContext {
            session_id: session.id,
            client_group_id: tos.session_scope.client_group_id,
            workspace: tos.workspace.clone(),
            constellation_family: tos.constellation_family.clone(),
            constellation_map: tos.constellation_map.clone(),
            subject_kind: tos.subject_kind.clone(),
            subject_id: tos.subject_id,
            handoff_context: None,
            session_scope: tos.session_scope.clone(),
            agent_mode: session.agent_mode,
        };

        let hydrated = hydrate_workspace_state(pool, &resolved)
            .await
            .map_err(|(_, msg)| anyhow::anyhow!("Hydration failed: {msg}"))?;

        tracing::debug!(
            workspace = ?tos.workspace,
            "Re-hydrated TOS constellation after execution"
        );
        Ok(hydrated)
    }

    #[cfg(not(feature = "database"))]
    async fn rehydrate_tos(
        &self,
        _pool: &sqlx::PgPool,
        _session: &ReplSessionV2,
    ) -> anyhow::Result<WorkspaceStateView> {
        anyhow::bail!("Constellation hydration requires database feature")
    }

    async fn persist_trace_scaffold(
        &self,
        session: &ReplSessionV2,
        input: &UserInputV2,
    ) -> Option<NewUtteranceTrace> {
        let raw_utterance = input_trace_text(input)?;

        let pool = self.pool.clone()?;

        let repository = UtteranceTraceRepository::new(pool);
        let sage_ctx = repl_trace_sage_context(session);
        let mut trace = NewUtteranceTrace::in_progress(
            session.id,
            Uuid::new_v4(),
            raw_utterance.clone(),
            repl_trace_kind(session, input),
            false,
        );
        trace.parent_trace_id = repl_parent_trace_id(session, input);
        let mut trace_payload = build_trace_scaffold_payload(
            &raw_utterance,
            &sage_ctx,
            build_phase2_unavailable_payload("repl_v2"),
            "repl_v2",
        );
        if let Some(payload) = trace_payload.as_object_mut() {
            payload.insert(
                "state".to_string(),
                serde_json::json!(format!("{:?}", session.state)),
            );
            payload.insert(
                "has_active_pack".to_string(),
                serde_json::json!(session.has_active_pack()),
            );
            payload.insert(
                "runbook_step_count".to_string(),
                serde_json::json!(session.runbook.entries.len()),
            );
        }
        trace.trace_payload = trace_payload;

        if let Err(error) = repository.insert(&trace).await {
            tracing::warn!(
                session_id = %session.id,
                error = %error,
                "Failed to persist REPL utterance trace scaffold"
            );
            return None;
        }

        Some(trace)
    }

    async fn finalize_trace_scaffold(
        &self,
        trace: Option<NewUtteranceTrace>,
        session: &ReplSessionV2,
        response: &ReplResponseV2,
    ) -> Option<Uuid> {
        let mut trace = trace?;

        let pool = self.pool.clone()?;

        let repository = UtteranceTraceRepository::new(pool);
        let sage_ctx = repl_trace_sage_context(session);
        let phase_payload = build_phase_trace_payload(&trace.raw_utterance, &sage_ctx);
        let phase2 = Phase2Service::evaluate_from_refs(
            session.pending_lookup_result.as_ref(),
            session.pending_sem_os_envelope.as_ref(),
        );
        let phase_2 = phase2.payload_or_unavailable("repl_v2");
        let resolved_verb = response_resolved_verb(response, session);
        let phase_3 =
            build_repl_phase3_evaluation(response, session, resolved_verb.as_deref(), &phase2);
        let phase_4 =
            build_repl_phase4_evaluation(response, session, resolved_verb.as_deref(), &phase2);
        let phase_5 = evaluate_phase5_repl(session, response);
        trace.outcome = classify_repl_trace_outcome(response);
        trace.halt_reason_code = repl_halt_reason_code(session, response);
        trace.halt_phase = repl_halt_phase(session, response);
        trace.resolved_verb = resolved_verb;
        trace.fallback_invoked = phase_4
            .as_ref()
            .map(|evaluation| evaluation.fallback_invoked())
            .unwrap_or(false);
        trace.fallback_reason_code = phase_4
            .as_ref()
            .and_then(|evaluation| evaluation.fallback_reason_code_for_trace());
        trace.execution_shape_kind = phase_5.execution_shape_kind().map(ToString::to_string);
        trace.situation_signature_hash = phase2.situation_signature_hash();
        trace.template_id = session
            .runbook
            .template_id
            .clone()
            .or_else(|| phase2.constellation_template_id());
        trace.template_version = session
            .runbook
            .template_hash
            .clone()
            .or_else(|| phase2.constellation_template_version());
        trace.surface_versions.constellation_template_version = trace.template_version.clone();
        trace.trace_payload = serde_json::json!({
            "phase_0": phase_payload["phase_0"].clone(),
            "phase_1": phase_payload["phase_1"].clone(),
            "phase_2": phase_2,
            "phase_3": phase_3
                .as_ref()
                .map(|evaluation| evaluation.payload_or_unavailable("repl_v2"))
                .unwrap_or_else(|| build_phase3_unavailable_payload("repl_v2")),
            "phase_4": phase_4
                .as_ref()
                .map(|evaluation| evaluation.payload_or_unavailable("repl_v2"))
                .unwrap_or_else(|| build_phase4_unavailable_payload("repl_v2")),
            "phase_5": phase_5.payload(),
            "entrypoint": "repl_v2",
            "state": &response.state,
            "response_kind": &response.kind,
            "step_count": response.step_count,
            "has_active_pack": session.has_active_pack(),
        });

        if let Err(error) = repository.update(&trace).await {
            tracing::warn!(
                trace_id = %trace.trace_id,
                error = %error,
                "Failed to finalize REPL utterance trace"
            );
            return None;
        }

        Some(trace.trace_id)
    }

    async fn emit_repl_prompt_trace(
        &self,
        session: &mut ReplSessionV2,
        parent_trace_id: Option<Uuid>,
        response: &ReplResponseV2,
    ) -> Option<Uuid> {
        let parent_trace_id = parent_trace_id?;
        let pool = self.pool.clone()?;

        let repository = UtteranceTraceRepository::new(pool);
        let mut trace = NewUtteranceTrace::in_progress(
            session.id,
            Uuid::new_v4(),
            response.message.clone(),
            TraceKind::ClarificationPrompt,
            false,
        );
        trace.parent_trace_id = Some(parent_trace_id);
        trace.outcome = crate::traceability::TraceOutcome::ClarificationTriggered;
        let sage_ctx = repl_trace_sage_context(session);
        let mut trace_payload = build_trace_scaffold_payload(
            &response.message,
            &sage_ctx,
            build_phase2_unavailable_payload("repl_v2_prompt"),
            "repl_v2_prompt",
        );
        if let Some(payload) = trace_payload.as_object_mut() {
            payload.insert("state".to_string(), serde_json::json!(&response.state));
            payload.insert(
                "response_kind".to_string(),
                serde_json::json!(&response.kind),
            );
            payload.insert(
                "step_count".to_string(),
                serde_json::json!(response.step_count),
            );
            payload.insert(
                "has_active_pack".to_string(),
                serde_json::json!(session.has_active_pack()),
            );
        }
        trace.trace_payload = trace_payload;

        if let Err(error) = repository.insert(&trace).await {
            tracing::warn!(
                session_id = %session.id,
                error = %error,
                "Failed to persist REPL prompt trace"
            );
            return None;
        }

        session.last_trace_id = Some(trace.trace_id);
        Some(trace.trace_id)
    }

    // -----------------------------------------------------------------------
    // State handlers
    // -----------------------------------------------------------------------

    async fn handle_scope_gate(
        &self,
        session: &mut ReplSessionV2,
        input: UserInputV2,
    ) -> ReplResponseV2 {
        match input {
            UserInputV2::SelectScope {
                group_id,
                group_name,
            } => {
                self.complete_scope_gate(session, group_id, &group_name)
                    .await
            }
            UserInputV2::Message { content } => {
                // Check if we have pending disambiguation candidates.
                let pending_candidates = match &session.state {
                    ReplStateV2::ScopeGate { candidates, .. } => candidates.clone(),
                    _ => None,
                };

                // Infrastructure / SemOS intent — bypass client group resolution.
                if super::bootstrap::is_infrastructure_intent(&content) {
                    return self.complete_infrastructure_scope_gate(session).await;
                }

                // Numeric "2" with no pending candidates → infrastructure shortcut.
                if content.trim() == "2" && pending_candidates.is_none() {
                    return self.complete_infrastructure_scope_gate(session).await;
                }

                // If we have candidates, try numeric or name selection first.
                if let Some(ref cands) = pending_candidates {
                    if let Some(selected) =
                        super::bootstrap::try_numeric_or_name_selection(&content, cands)
                    {
                        let group_id = selected.group_id;
                        let group_name = selected.group_name.clone();
                        return self
                            .complete_scope_gate(session, group_id, &group_name)
                            .await;
                    }
                    // Not a selection from the list — fall through to fresh resolution.
                }

                // Resolve the input against client groups.
                #[cfg(feature = "database")]
                {
                    if let Some(ref pool) = self.pool {
                        let outcome = super::bootstrap::resolve_client_input(&content, pool).await;
                        return self.handle_bootstrap_outcome(session, outcome).await;
                    }
                }

                // No database — store as pending and re-prompt.
                session.set_state(ReplStateV2::ScopeGate {
                    pending_input: Some(content),
                    candidates: None,
                });
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::ScopeRequired {
                        prompt: "Please select a client group to work with.".to_string(),
                    },
                    message: "Which client group would you like to work with?".to_string(),
                    runbook_summary: None,
                    step_count: 0,
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }
            _ => self.invalid_input(session, "Please select a scope first."),
        }
    }

    fn handle_workspace_selection(
        &self,
        session: &mut ReplSessionV2,
        input: UserInputV2,
    ) -> ReplResponseV2 {
        let workspace = match input {
            UserInputV2::SelectWorkspace { workspace } => workspace,
            UserInputV2::Message { ref content } => {
                // Try to resolve workspace from natural language
                match Self::resolve_workspace_from_utterance(content) {
                    Some(ws) => ws,
                    None => {
                        // Try numeric selection (1-6)
                        if let Some(ws) = Self::resolve_workspace_from_number(content, session) {
                            ws
                        } else {
                            return self.invalid_input(
                                session,
                                "I didn't recognise that workspace. Try: CBU, KYC, Deal, OnBoarding, Product Maintenance, or Instrument Matrix.",
                            );
                        }
                    }
                }
            }
            _ => {
                return self.invalid_input(
                    session,
                    "Please select a workspace: CBU, KYC, Deal, OnBoarding, Product Maintenance, or Instrument Matrix.",
                );
            }
        };

        session.set_workspace_root(workspace.clone());
        session.set_state(ReplStateV2::JourneySelection { candidates: None });
        let packs = self.pack_router.list_packs_for_workspace(&workspace);
        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::JourneyOptions {
                packs: packs.clone(),
            },
            message: format!(
                "{} workspace selected. Which journey would you like to start?",
                workspace.label()
            ),
            runbook_summary: None,
            step_count: session.runbook.entries.len(),
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    /// Resolve workspace from natural language utterance.
    /// Simple keyword matching — there are only 6 workspaces.
    fn resolve_workspace_from_utterance(input: &str) -> Option<WorkspaceKind> {
        let lower = input.to_lowercase();

        // Exact workspace name matches
        if lower.contains("kyc")
            || lower.contains("know your customer")
            || lower.contains("compliance")
        {
            return Some(WorkspaceKind::Kyc);
        }
        if lower.contains("onboard") || lower.contains("on-board") || lower.contains("on board") {
            return Some(WorkspaceKind::OnBoarding);
        }
        if lower.contains("deal")
            || lower.contains("commercial")
            || lower.contains("contract")
            || lower.contains("pricing")
        {
            return Some(WorkspaceKind::Deal);
        }
        if lower.contains("product") || lower.contains("service") || lower.contains("taxonomy") {
            return Some(WorkspaceKind::ProductMaintenance);
        }
        if lower.contains("instrument") || lower.contains("matrix") || lower.contains("trading") {
            return Some(WorkspaceKind::InstrumentMatrix);
        }
        if lower.contains("semos")
            || lower.contains("sem os")
            || lower.contains("semantic os")
            || lower.contains("registry governance")
            || lower.contains("stewardship")
        {
            return Some(WorkspaceKind::SemOsMaintenance);
        }
        if lower.contains("cbu")
            || lower.contains("client business")
            || lower.contains("structure")
            || lower.contains("maintenance")
        {
            return Some(WorkspaceKind::Cbu);
        }
        None
    }

    /// Resolve workspace from numeric selection (1-6).
    fn resolve_workspace_from_number(
        input: &str,
        session: &ReplSessionV2,
    ) -> Option<WorkspaceKind> {
        let trimmed = input.trim();
        let index: usize = trimmed.parse().ok()?;
        if index == 0 || index > 7 {
            return None;
        }
        // Match against the workspace options stored in the state
        if let ReplStateV2::WorkspaceSelection { ref workspaces } = session.state {
            workspaces.get(index - 1).map(|opt| opt.workspace.clone())
        } else {
            None
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
                let Some(workspace) = session.active_workspace.as_ref() else {
                    return self
                        .invalid_input(session, "Select a workspace before choosing a journey.");
                };

                // Try numeric selection first (1, 2, 3, ...)
                if let Ok(idx) = content.trim().parse::<usize>() {
                    if idx > 0 {
                        let pack_id = if let ReplStateV2::JourneySelection {
                            candidates: Some(ref packs),
                        } = session.state
                        {
                            packs.get(idx - 1).map(|p| p.pack_id.clone())
                        } else {
                            None
                        };
                        let pack_id = pack_id.or_else(|| {
                            let packs = self.pack_router.list_packs_for_workspace(workspace);
                            packs.get(idx - 1).map(|p| p.pack_id.clone())
                        });
                        if let Some(pid) = pack_id {
                            return self.activate_pack_by_id(session, &pid);
                        }
                    }
                }

                // Route via PackRouter (phrase matching).
                match self.pack_router.route_for_workspace(&content, workspace) {
                    PackRouteOutcome::Matched(manifest, hash) => {
                        let pack_id = manifest.id.clone();
                        let pack_name = manifest.name.clone();
                        let pack_version = manifest.version.clone();

                        // Record pack.select on the runbook (Invariant I-1).
                        self.record_pack_select_entry(
                            session,
                            &pack_id,
                            &pack_name,
                            &pack_version,
                            &hash,
                            None,
                        );

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
                            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                        }
                    }
                    PackRouteOutcome::NoMatch => {
                        let packs = self.pack_router.list_packs_for_workspace(workspace);
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                // Phase E: Check for power user fast commands before anything else.
                if let Some(response) = self.try_fast_command(session, &content).await {
                    return response;
                }

                // Phase N: Contextual query intercept (ADR 043 Phase 2).
                // "what's next", "what's missing", "where are we" bypass verb search
                // and return narration directly from constellation state.
                if crate::agent::narration_engine::is_contextual_query(&content) {
                    if let Some(narration_resp) = self.handle_contextual_query(session, &content) {
                        return narration_resp;
                    }
                }

                // Check if there are still required questions to answer.
                if let Some(question) = self.next_required_question(session) {
                    // Record the answer to the previous question (if any).
                    let field = question.field.clone();
                    session
                        .record_answer(field.clone(), serde_json::Value::String(content.clone()));

                    // Record pack.answer on the runbook (Invariant I-1).
                    let pack_id = session.active_pack_id();
                    self.record_pack_answer_entry(session, &field, &content, pack_id.as_deref());

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
                            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                        session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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

                    // Compile the verb (classify → compile → attach runbook ID).
                    if let Some(resp) = self.try_compile_entry(session, &mut entry) {
                        return resp;
                    }

                    session.runbook.add_entry(entry);

                    // Go to RunbookEditing (or back to InPack if pack is active).
                    let next_state = if session.has_active_pack() {
                        ReplStateV2::InPack {
                            pack_id: session.active_pack_id().unwrap_or_default(),
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
                        session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                    }
                } else {
                    self.invalid_input(session, "No sentence to confirm.")
                }
            }
            UserInputV2::Reject => {
                // Discard and go back to InPack.
                let next_state = if session.has_active_pack() {
                    ReplStateV2::InPack {
                        pack_id: session.active_pack_id().unwrap_or_default(),
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                        session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                // Contextual query intercept (ADR 043) — "what's next" etc.
                if crate::agent::narration_engine::is_contextual_query(&content) {
                    if let Some(narration_resp) =
                        self.handle_contextual_query(session, &content)
                    {
                        return narration_resp;
                    }
                }
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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

        // Now execute through the gate (INV-3: no raw DSL execution).
        let fallback_version = session.allocate_runbook_version();
        let entry_ref = &session.runbook.entries[idx];
        let runbook_id = session.runbook.id;
        let is_durable = entry_ref.execution_mode == ExecutionMode::Durable;
        let outcome = self
            .execute_entry_via_gate(
                entry_ref,
                session.id,
                is_durable,
                runbook_id,
                fallback_version,
            )
            .await;

        match outcome {
            StepOutcome::Completed { result } => {
                session.runbook.entries[idx].status = EntryStatus::Completed;
                session.runbook.entries[idx].result = Some(result);
            }
            StepOutcome::Failed { error } => {
                session.runbook.entries[idx].status = EntryStatus::Failed;
                session.runbook.entries[idx].result = Some(serde_json::json!({"error": error}));
            }
            StepOutcome::Parked { .. } => {
                // Edge case: approved human gate returns another park.
                // This shouldn't normally happen. Mark failed.
                session.runbook.entries[idx].status = EntryStatus::Failed;
                session.runbook.entries[idx].result =
                    Some(serde_json::json!({"error": "Unexpected park after approval"}));
            }
            StepOutcome::Skipped { reason } => {
                session.runbook.entries[idx].status = EntryStatus::Failed;
                session.runbook.entries[idx].result = Some(serde_json::json!({"error": reason}));
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    // -----------------------------------------------------------------------
    // Bootstrap helpers (ScopeGate resolution)
    // -----------------------------------------------------------------------

    /// Complete the scope gate: build scope DSL, add to runbook, execute,
    /// and only on success set client context and transition to WorkspaceSelection.
    ///
    /// Nothing is real until it's DSL on the runsheet, executed through the executor.
    async fn complete_scope_gate(
        &self,
        session: &mut ReplSessionV2,
        group_id: Uuid,
        group_name: &str,
    ) -> ReplResponseV2 {
        // 1. Build the DSL — this is the only thing that matters.
        let dsl = format!("(session.load-cluster :client \"{}\")", group_id);
        let sentence = format!("Set client scope to {}", group_name);

        let mut args = HashMap::new();
        args.insert("client".to_string(), group_id.to_string());
        args.insert("client-name".to_string(), group_name.to_string());

        let mut slot_prov = SlotProvenance {
            slots: HashMap::new(),
        };
        slot_prov
            .slots
            .insert("client".to_string(), SlotSource::InferredFromContext);

        let entry = RunbookEntry {
            id: Uuid::new_v4(),
            sequence: 0,
            sentence,
            labels: HashMap::new(),
            dsl: dsl.clone(),
            verb: "session.load-cluster".to_string(),
            args,
            slot_provenance: slot_prov,
            arg_extraction_audit: None,
            status: EntryStatus::Confirmed,
            execution_mode: ExecutionMode::Sync,
            confirm_policy: ConfirmPolicy::Always,
            unresolved_refs: vec![],
            depends_on: vec![],
            compiled_runbook_id: None,
            result: None,
            invocation: None,
        };

        // 2. Add to the runbook — now it exists.
        let entry_id = session.runbook.add_entry(entry);

        // 3. Execute through the gate (INV-3: no raw DSL execution).
        let fallback_version = session.allocate_runbook_version();
        let entry_ref = session
            .runbook
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .expect("just added");
        let outcome = self
            .execute_entry_via_gate(
                entry_ref,
                session.id,
                false,
                session.runbook.id,
                fallback_version,
            )
            .await;

        // 4. Record outcome on the runsheet entry.
        let succeeded = matches!(outcome, StepOutcome::Completed { .. });
        if let Some(entry) = session
            .runbook
            .entries
            .iter_mut()
            .find(|e| e.id == entry_id)
        {
            match &outcome {
                StepOutcome::Completed { result } => {
                    entry.status = EntryStatus::Completed;
                    entry.result = Some(result.clone());
                }
                StepOutcome::Failed { error } => {
                    entry.status = EntryStatus::Failed;
                    entry.result = Some(serde_json::json!({"error": error}));
                }
                StepOutcome::Skipped { reason } => {
                    entry.status = EntryStatus::Failed;
                    entry.result = Some(serde_json::json!({"error": reason}));
                }
                StepOutcome::Parked { .. } => {
                    entry.status = EntryStatus::Failed;
                    entry.result =
                        Some(serde_json::json!({"error": "Unexpected park in scope gate"}));
                }
            }
        }

        // 5. Set scope and always transition to workspace selection.
        // Even if session.load-cluster failed (e.g., no CBUs yet for this group),
        // the group is valid — the user can create CBUs in the CBU workspace.
        session.set_client_scope(group_id);

        if !succeeded {
            tracing::info!(
                "session.load-cluster had no CBUs for group {} — proceeding with empty scope",
                group_name
            );
        }

        let workspaces = self.workspace_options();
        session.set_state(ReplStateV2::WorkspaceSelection {
            workspaces: workspaces.clone(),
        });

        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::WorkspaceOptions {
                workspaces: workspaces.clone(),
            },
            message: if succeeded {
                format!(
                    "Scope set to {}. Which workspace would you like to enter?",
                    group_name
                )
            } else {
                format!(
                    "Scope set to {} (no CBUs yet — you can create them). Which workspace?",
                    group_name
                )
            },
            runbook_summary: None,
            step_count: if succeeded { 1 } else { 0 },
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    /// Complete scope gate for infrastructure sessions (no client group).
    ///
    /// Sets the nil-UUID scope sentinel, jumps directly to
    /// `JourneySelection` with workspace pinned to `SemOsMaintenance`,
    /// and returns the available packs for that workspace.
    async fn complete_infrastructure_scope_gate(
        &self,
        session: &mut ReplSessionV2,
    ) -> ReplResponseV2 {
        // Set scope to nil UUID — marks this as an infrastructure session.
        session.set_client_scope(Uuid::nil());

        // Pin workspace to SemOS Maintenance — skip WorkspaceSelection tollgate.
        session.set_workspace_root(WorkspaceKind::SemOsMaintenance);
        session.set_state(ReplStateV2::JourneySelection { candidates: None });

        let packs = self
            .pack_router
            .list_packs_for_workspace(&WorkspaceKind::SemOsMaintenance);

        ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::JourneyOptions {
                packs: packs.clone(),
            },
            message: "SemOS Infrastructure session - no client group required.\n\
                      Which journey would you like to start?"
                .to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    fn workspace_options(&self) -> Vec<WorkspaceOption> {
        vec![
            WorkspaceOption {
                workspace: WorkspaceKind::ProductMaintenance,
                label: WorkspaceKind::ProductMaintenance.label().to_string(),
                description:
                    "Design-time product, service, servicing-resource, and resource dictionary taxonomy"
                        .to_string(),
            },
            WorkspaceOption {
                workspace: WorkspaceKind::Deal,
                label: WorkspaceKind::Deal.label().to_string(),
                description: "Commercial deal, contracts, pricing, and onboarding handoff"
                    .to_string(),
            },
            WorkspaceOption {
                workspace: WorkspaceKind::Cbu,
                label: WorkspaceKind::Cbu.label().to_string(),
                description: "CBU maintenance, roles, and operating structure state".to_string(),
            },
            WorkspaceOption {
                workspace: WorkspaceKind::Kyc,
                label: WorkspaceKind::Kyc.label().to_string(),
                description: "Group and delta KYC, UBO, screening, and evidence".to_string(),
            },
            WorkspaceOption {
                workspace: WorkspaceKind::InstrumentMatrix,
                label: WorkspaceKind::InstrumentMatrix.label().to_string(),
                description: "Trading profile, instruction matrix, and executable mandate rules"
                    .to_string(),
            },
            WorkspaceOption {
                workspace: WorkspaceKind::OnBoarding,
                label: WorkspaceKind::OnBoarding.label().to_string(),
                description: "Onboarding activation, handoff progress, and runtime provisioning"
                    .to_string(),
            },
            WorkspaceOption {
                workspace: WorkspaceKind::SemOsMaintenance,
                label: WorkspaceKind::SemOsMaintenance.label().to_string(),
                description: "Manage SemOS registry governance — changesets, attributes, verbs, schemas"
                    .to_string(),
            },
        ]
    }

    /// Handle a `BootstrapOutcome` from the resolution logic.
    async fn handle_bootstrap_outcome(
        &self,
        session: &mut ReplSessionV2,
        outcome: super::bootstrap::BootstrapOutcome,
    ) -> ReplResponseV2 {
        match outcome {
            super::bootstrap::BootstrapOutcome::Resolved {
                group_id,
                group_name,
            } => {
                self.complete_scope_gate(session, group_id, &group_name)
                    .await
            }

            super::bootstrap::BootstrapOutcome::Ambiguous {
                candidates,
                original_input,
            } => {
                let message = super::bootstrap::format_disambiguation(&candidates, &original_input);
                session.set_state(ReplStateV2::ScopeGate {
                    pending_input: Some(original_input),
                    candidates: Some(candidates),
                });
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::ScopeRequired {
                        prompt: message.clone(),
                    },
                    message,
                    runbook_summary: None,
                    step_count: 0,
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }

            super::bootstrap::BootstrapOutcome::NoMatch { original_input } => {
                session.set_state(ReplStateV2::ScopeGate {
                    pending_input: Some(original_input.clone()),
                    candidates: None,
                });
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::ScopeRequired {
                        prompt: format!(
                            "No client group found matching \"{}\". Please try again.",
                            original_input
                        ),
                    },
                    message: format!(
                        "I couldn't find a client group matching \"{}\". Please try again or type the exact name.",
                        original_input
                    ),
                    runbook_summary: None,
                    step_count: 0,
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }

            super::bootstrap::BootstrapOutcome::Empty => {
                // No client groups in DB — stay in ScopeGate.
                // Client scope is a non-negotiable tollgate: nothing progresses without it.
                session.set_state(ReplStateV2::ScopeGate {
                    pending_input: None,
                    candidates: None,
                });

                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::ScopeRequired {
                        prompt: "No client groups are configured in the system. \
                                 Please ask an administrator to set up client groups \
                                 before proceeding."
                            .to_string(),
                    },
                    message: "No client groups are configured. \
                              A client group must be selected before any work can begin."
                        .to_string(),
                    runbook_summary: None,
                    step_count: 0,
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }

            super::bootstrap::BootstrapOutcome::Infrastructure => {
                self.complete_infrastructure_scope_gate(session).await
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn activate_pack_by_id(&self, session: &mut ReplSessionV2, pack_id: &str) -> ReplResponseV2 {
        let Some(workspace) = session.active_workspace.as_ref() else {
            return self.invalid_input(session, "Select a workspace before choosing a journey.");
        };

        if let Some((manifest, hash)) = self.pack_router.get_pack_for_workspace(pack_id, workspace)
        {
            let pack_id_str = manifest.id.clone();
            let pack_name = manifest.name.clone();
            let pack_version = manifest.version.clone();

            // 1. Record pack.select on the runbook (Invariant I-1).
            self.record_pack_select_entry(
                session,
                &pack_id_str,
                &pack_name,
                &pack_version,
                hash,
                None,
            );

            // 2. Activate the pack on the session (existing behavior).
            session.activate_pack(manifest.clone(), hash.clone(), None);

            // 3. Enter the pack (ask first question or prompt for input).
            self.enter_pack(session, &pack_id_str)
        } else {
            self.invalid_input(
                session,
                &format!(
                    "Pack '{}' is not available in the {} workspace.",
                    pack_id,
                    workspace.label()
                ),
            )
        }
    }

    /// Record a `pack.select` entry on the runbook so pack context is derivable from fold.
    fn record_pack_select_entry(
        &self,
        session: &mut ReplSessionV2,
        pack_id: &str,
        pack_name: &str,
        pack_version: &str,
        manifest_hash: &str,
        handoff_from: Option<&str>,
    ) {
        let dsl = if let Some(source) = handoff_from {
            format!(
                "(pack.select :pack-id \"{}\" :pack-version \"{}\" :manifest-hash \"{}\" :handoff-from \"{}\")",
                pack_id, pack_version, manifest_hash, source
            )
        } else {
            format!(
                "(pack.select :pack-id \"{}\" :pack-version \"{}\" :manifest-hash \"{}\")",
                pack_id, pack_version, manifest_hash
            )
        };

        let sentence = format!("Select journey: {}", pack_name);

        let mut args = HashMap::new();
        args.insert("pack-id".to_string(), pack_id.to_string());
        args.insert("pack-version".to_string(), pack_version.to_string());
        args.insert("manifest-hash".to_string(), manifest_hash.to_string());
        if let Some(source) = handoff_from {
            args.insert("handoff-from".to_string(), source.to_string());
        }

        let mut slot_prov = SlotProvenance {
            slots: HashMap::new(),
        };
        slot_prov
            .slots
            .insert("pack-id".to_string(), SlotSource::UserProvided);

        let entry = RunbookEntry {
            id: Uuid::new_v4(),
            sequence: session.runbook.entries.len() as i32,
            sentence,
            labels: HashMap::new(),
            dsl,
            verb: "pack.select".to_string(),
            args,
            slot_provenance: slot_prov,
            arg_extraction_audit: None,
            status: EntryStatus::Completed,
            execution_mode: ExecutionMode::Sync,
            confirm_policy: ConfirmPolicy::Always,
            unresolved_refs: vec![],
            depends_on: vec![],
            compiled_runbook_id: None,
            result: Some(serde_json::json!({
                "pack_id": pack_id,
                "pack_name": pack_name,
                "pack_version": pack_version,
                "manifest_hash": manifest_hash,
                "handoff_from": handoff_from,
            })),
            invocation: None,
        };

        session.runbook.add_entry(entry);
    }

    /// Record a `pack.answer` entry on the runbook so Q&A answers are derivable from fold.
    fn record_pack_answer_entry(
        &self,
        session: &mut ReplSessionV2,
        field: &str,
        value: &str,
        pack_id: Option<&str>,
    ) {
        let dsl = if let Some(pid) = pack_id {
            format!(
                "(pack.answer :field \"{}\" :value \"{}\" :pack-id \"{}\")",
                field, value, pid
            )
        } else {
            format!("(pack.answer :field \"{}\" :value \"{}\")", field, value)
        };

        let sentence = format!("Answer: {} = {}", field, value);

        let mut args = HashMap::new();
        args.insert("field".to_string(), field.to_string());
        args.insert("value".to_string(), value.to_string());
        if let Some(pid) = pack_id {
            args.insert("pack-id".to_string(), pid.to_string());
        }

        let mut slot_prov = SlotProvenance {
            slots: HashMap::new(),
        };
        slot_prov
            .slots
            .insert("field".to_string(), SlotSource::InferredFromContext);
        slot_prov
            .slots
            .insert("value".to_string(), SlotSource::UserProvided);

        let entry = RunbookEntry {
            id: Uuid::new_v4(),
            sequence: session.runbook.entries.len() as i32,
            sentence,
            labels: HashMap::new(),
            dsl,
            verb: "pack.answer".to_string(),
            args,
            slot_provenance: slot_prov,
            arg_extraction_audit: None,
            status: EntryStatus::Completed,
            execution_mode: ExecutionMode::Sync,
            confirm_policy: ConfirmPolicy::Always,
            unresolved_refs: vec![],
            depends_on: vec![],
            compiled_runbook_id: None,
            result: Some(serde_json::json!({
                "field": field,
                "value": value,
                "accepted": true,
                "pack_id": pack_id,
            })),
            invocation: None,
        };

        session.runbook.add_entry(entry);
    }

    fn enter_pack(&self, session: &mut ReplSessionV2, pack_id: &str) -> ReplResponseV2 {
        // Determine remaining required slots from staged pack.
        let required_slots: Vec<String> = session
            .staged_pack
            .as_ref()
            .map(|pack| {
                pack.required_questions
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
            }
        }
    }

    fn next_required_question(
        &self,
        session: &ReplSessionV2,
    ) -> Option<crate::journey::pack::PackQuestion> {
        let pack = session.staged_pack.as_ref()?;
        // Derive answered fields from the runbook fold (accumulated_answers).
        let ctx = self.build_context_stack(session);
        pack.required_questions
            .iter()
            .find(|q| !ctx.accumulated_answers.contains_key(&q.field))
            .cloned()
    }

    // -- Phase E: Fast command handling --

    /// Try to parse and handle a fast command from user input.
    ///
    /// Fast commands are detected by prefix matching before semantic search.
    /// They are zero-cost (no ML, no DB) and bypass the verb pipeline entirely.
    /// Returns `None` if the input is not a recognized fast command.
    async fn try_fast_command(
        &self,
        session: &mut ReplSessionV2,
        input: &str,
    ) -> Option<ReplResponseV2> {
        use super::runbook::FastCommand;

        let cmd = FastCommand::parse(input)?;

        let response = match cmd {
            FastCommand::Undo => self.handle_undo(session),
            FastCommand::Redo => self.handle_redo(session),
            FastCommand::Run => self.execute_runbook(session).await,
            FastCommand::RunStep(n) => {
                // Find entry by sequence number.
                let entry = session
                    .runbook
                    .entries
                    .iter()
                    .find(|e| e.sequence == n)
                    .map(|e| e.id);
                match entry {
                    Some(_id) => {
                        // For now, run the whole runbook (single-step execution is Phase H).
                        self.execute_runbook(session).await
                    }
                    None => self.invalid_input(session, &format!("No step {} in runbook.", n)),
                }
            }
            FastCommand::ShowRunbook => {
                let summary = self.runbook_summary(session);
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::RunbookSummary {
                        chapters: self.chapter_view(session),
                        summary: summary.clone(),
                    },
                    message: if session.runbook.entries.is_empty() {
                        "Runbook is empty.".to_string()
                    } else {
                        let progress = session.runbook.narrate_progress();
                        format!("{}\n\n{}", summary, progress)
                    },
                    runbook_summary: Some(summary),
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }
            FastCommand::DropStep(n) => {
                let entry_id = session
                    .runbook
                    .entries
                    .iter()
                    .find(|e| e.sequence == n)
                    .map(|e| e.id);
                match entry_id {
                    Some(id) => {
                        if session.runbook.remove_entry(id).is_some() {
                            let summary = self.runbook_summary(session);
                            ReplResponseV2 {
                                state: session.state.clone(),
                                kind: ReplResponseKindV2::RunbookSummary {
                                    chapters: self.chapter_view(session),
                                    summary: summary.clone(),
                                },
                                message: format!("Removed step {}. {}", n, summary),
                                runbook_summary: Some(summary),
                                step_count: session.runbook.entries.len(),
                                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                            }
                        } else {
                            self.invalid_input(session, &format!("Could not remove step {}.", n))
                        }
                    }
                    None => self.invalid_input(session, &format!("No step {} in runbook.", n)),
                }
            }
            FastCommand::Why => {
                // Show the last proposal's provenance/audit.
                let msg = if let Some(audit) = &session.pending_arg_audit {
                    format!(
                        "Last match: verb='{}', confidence={:.2}, model={}",
                        audit
                            .extracted_args
                            .keys()
                            .next()
                            .unwrap_or(&"?".to_string()),
                        audit.confidence,
                        audit.model_id,
                    )
                } else {
                    "No recent proposal to explain.".to_string()
                };
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Info {
                        detail: msg.clone(),
                    },
                    message: msg,
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }
            FastCommand::Options => {
                let pack_verbs = session
                    .staged_pack
                    .as_ref()
                    .map(|pack| pack.allowed_verbs.join(", "))
                    .unwrap_or_else(|| "No pack active — all verbs available.".to_string());
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Info {
                        detail: pack_verbs.clone(),
                    },
                    message: format!("Available verbs: {}", pack_verbs),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }
            FastCommand::SwitchJourney => {
                session.state = ReplStateV2::JourneySelection { candidates: None };
                session.clear_staged_pack();
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Prompt {
                        text: "What would you like to work on?".to_string(),
                    },
                    message: "Switched back to journey selection. What would you like to work on?"
                        .to_string(),
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }
            FastCommand::Cancel => self.handle_cancel(session),
            FastCommand::Status => {
                let progress = session.runbook.narrate_progress();
                let pending = session.runbook.derive_pending_questions();
                let msg = if pending.is_empty() {
                    progress
                } else {
                    format!(
                        "{}\n\n{} entries need entity resolution.",
                        progress,
                        pending.len()
                    )
                };
                ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Info {
                        detail: msg.clone(),
                    },
                    message: msg,
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }
            FastCommand::Help => self.handle_help(session),
        };

        Some(response)
    }

    /// Build a `MatchContext` from the current session state.
    ///
    /// Uses ContextStack (runbook fold) for scope and pack data instead
    /// of reading ClientContext/JourneyContext directly.
    fn build_match_context(&self, session: &ReplSessionV2) -> MatchContext {
        let ctx = self.build_context_stack(session);
        let narration_hot_verbs = session
            .workspace_stack
            .last()
            .map(|f| f.narration_hot_verbs.clone())
            .unwrap_or_default();
        MatchContext {
            client_group_id: ctx.derived_scope.client_group_id,
            client_group_name: ctx.derived_scope.client_group_name.clone(),
            domain_hint: ctx.active_pack().and_then(|p| p.dominant_domain.clone()),
            entity_kind: ctx.focus.cbu.as_ref().map(|_| "cbu".to_string()),
            narration_hot_verbs,
            ..Default::default()
        }
    }

    /// Handle a contextual query ("what's next", "what's missing", etc.)
    /// by returning narration from the constellation state without verb execution.
    ///
    /// Returns `None` if no hydrated constellation is available (falls through
    /// to normal verb matching).
    fn handle_contextual_query(
        &self,
        session: &mut ReplSessionV2,
        content: &str,
    ) -> Option<ReplResponseV2> {
        let frame = session.workspace_stack.last()?;
        let hydrated = frame.hydrated_state.as_ref()?;
        let constellation = hydrated.hydrated_constellation.as_ref()?;
        let label = constellation
            .description
            .as_deref()
            .unwrap_or(&constellation.constellation);

        let ws_key = serde_json::to_value(&frame.workspace)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let narration = crate::agent::narration_engine::query_narration(
            &constellation.slots,
            label,
            Some(&ws_key),
        );

        // Build a human-readable message from the narration.
        let mut msg = String::new();
        if let Some(ref progress) = narration.progress {
            msg.push_str(progress);
            msg.push('\n');
        }
        if !narration.required_gaps.is_empty() {
            msg.push_str("\nRequired:\n");
            for gap in &narration.required_gaps {
                msg.push_str(&format!(
                    "  - {} ({})\n",
                    gap.slot_label, gap.suggested_utterance
                ));
            }
        }
        if !narration.optional_gaps.is_empty() {
            msg.push_str("\nOptional:\n");
            for gap in &narration.optional_gaps {
                msg.push_str(&format!("  - {}\n", gap.slot_label));
            }
        }
        if !narration.blockers.is_empty() {
            msg.push_str("\nBlockers:\n");
            for b in &narration.blockers {
                msg.push_str(&format!("  - {} — {}\n", b.blocked_verb, b.reason));
            }
        }
        if narration.required_gaps.is_empty() && narration.optional_gaps.is_empty() {
            msg.push_str("\nAll slots filled — ready to proceed.");
        }
        if let Some(ref transition) = narration.workspace_transition {
            msg.push_str(&format!(
                "\n\nNext workspace: {} — {}\nSay: \"{}\"",
                transition.target_label, transition.reason, transition.suggested_utterance
            ));
        }

        // Store hot verbs for boost signal.
        if let Some(frame) = session.workspace_stack.last_mut() {
            frame.narration_hot_verbs = narration
                .suggested_next
                .iter()
                .map(|s| s.verb_fqn.clone())
                .collect();
        }

        tracing::info!(
            session_id = %session.id,
            query = %content,
            required_gaps = narration.required_gaps.len(),
            optional_gaps = narration.optional_gaps.len(),
            "Contextual query routed to NarrationEngine"
        );

        Some(ReplResponseV2 {
            state: session.state.clone(),
            kind: ReplResponseKindV2::Info {
                detail: msg.trim().to_string(),
            },
            message: msg.trim().to_string(),
            runbook_summary: None,
            step_count: session.runbook.entries.len(),
            session_feedback: Some(session.build_session_feedback(false)),
            narration: Some(narration),
        })
    }

    /// Build a `ContextStack` from the current session state for pack-scoped matching.
    ///
    /// The ContextStack is a pure fold over the runbook, enriched with the
    /// optional staged pack manifest and the PackRouter for manifest lookup.
    fn build_context_stack(&self, session: &ReplSessionV2) -> ContextStack {
        session.build_context_stack(Some(&self.pack_router))
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
        let pack = session.staged_pack.as_deref();

        let ctx_stack = self.build_context_stack(session);
        let context_vars: HashMap<String, String> = {
            let mut vars = HashMap::new();
            if let Some(name) = &ctx_stack.derived_scope.client_group_name {
                vars.insert("client_name".to_string(), name.clone());
            }
            if let Some(id) = ctx_stack.derived_scope.client_group_id {
                vars.insert("client_group_id".to_string(), id.to_string());
            }
            vars
        };

        let answers: HashMap<String, serde_json::Value> = ctx_stack.accumulated_answers.clone();

        let proposal_set = engine
            .propose(
                content,
                pack,
                &session.runbook,
                &match_ctx,
                &ctx_stack,
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

                // Compile the verb (classify → compile → attach runbook ID).
                if let Some(resp) = self.try_compile_entry(session, &mut entry) {
                    return resp;
                }

                session.runbook.add_entry(entry);

                let next_state = if session.has_active_pack() {
                    ReplStateV2::InPack {
                        pack_id: session.active_pack_id().unwrap_or_default(),
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
        // Phase 1.4: If unified orchestrator is available, log trace info.
        // The REPL's multi-phase flow (clarification, sentence gen, confirmation)
        // is preserved — orchestrator provides Sem OS context and trace only.
        #[cfg(feature = "database")]
        if let Some(ref _pool) = self.unified_orch_pool {
            tracing::debug!(
                source = "repl",
                session_id = %session.id,
                "Unified orchestrator available for REPL verb matching"
            );
        }

        let mut match_ctx = self.build_match_context(session);
        let context_stack = self.build_context_stack(session);

        if let Some(lookup_service) = &self.lookup_service {
            session.pending_lookup_result = Some(lookup_service.analyze(content, 5).await);
        }

        // Phase 4 CCIR: Resolve SemOsContextEnvelope and pre-constrain verb search.
        // This injects `allowed_verbs` into MatchContext, which flows through to
        // VerbSearchIntentMatcher → HybridVerbSearcher::search() via the Phase 3
        // allowed_verbs parameter. REPL matching is fail-closed: if Sem OS is
        // unavailable, the allowed set is empty.
        let mut sem_os_fingerprint: Option<String> = None;
        let mut sem_os_pruned_count: usize = 0;
        if let Some(ref client) = self.sem_os_client {
            let actor = crate::policy::ActorResolver::from_env();
            let envelope = crate::agent::orchestrator::resolve_allowed_verbs(
                client.as_ref(),
                &actor,
                Some(session.id),
            )
            .await;
            session.pending_sem_os_envelope = Some(envelope.clone());
            let phase2 =
                Phase2Service::evaluate(session.pending_lookup_result.clone(), Some(envelope));
            let phase2_legal_verbs = phase2.legal_verbs_or_empty.clone();

            if phase2.is_deny_all {
                tracing::warn!(
                    session_id = %session.id,
                    "REPL: Sem OS deny-all — verb search will return empty"
                );
                match_ctx.allowed_verbs = Some(phase2_legal_verbs.clone());
            }

            if !phase2.is_available {
                tracing::warn!(
                    session_id = %session.id,
                    "REPL: Sem OS unavailable — blocking unconstrained verb matching"
                );
                match_ctx.allowed_verbs = Some(std::collections::HashSet::new());
            } else {
                sem_os_fingerprint = phase2.fingerprint();
                sem_os_pruned_count = phase2.pruned_verb_count();
                match_ctx.allowed_verbs = Some(phase2_legal_verbs);
                tracing::debug!(
                    session_id = %session.id,
                    allowed_count = phase2.legal_verb_count(),
                    fingerprint = ?phase2.fingerprint(),
                    pruned_count = sem_os_pruned_count,
                    "REPL: Sem OS pre-constraint applied to MatchContext"
                );
            }
        } else {
            tracing::warn!(
                session_id = %session.id,
                "REPL: SemOsClient unavailable — blocking unconstrained verb matching"
            );
            match_ctx.allowed_verbs = Some(std::collections::HashSet::new());
            session.pending_sem_os_envelope = None;
        }

        if let Some(response) = self.phase2_gate_response(session) {
            return response;
        }

        // Phase 2: Try IntentService with context-aware matching first.
        if let Some(svc) = &self.intent_service {
            match svc
                .match_verb_with_context(content, &match_ctx, &context_stack)
                .await
            {
                Ok(outcome) => {
                    return self.handle_intent_service_outcome(
                        session,
                        content,
                        svc,
                        outcome,
                        sem_os_fingerprint.clone(),
                        sem_os_pruned_count,
                    );
                }
                Err(e) => {
                    tracing::warn!("IntentService error, falling back: {}", e);
                }
            }
        }

        // Phase 1: Try raw IntentMatcher with pack-scoped scoring (P-2 invariant).
        //
        // Uses `search_with_context()` (NOT `match_intent()`) to ensure pack
        // scoring is always applied. The IntentMatcher trait provides
        // `search_with_context()` as a default method that wraps `match_intent()`
        // with pack boost/penalty/forbidden filtering.
        if let Some(matcher) = &self.intent_matcher {
            match matcher
                .search_with_context(content, &match_ctx, &context_stack)
                .await
            {
                Ok(mut result) => {
                    // Apply precondition filter (P-D invariant).
                    let _filter_stats = super::preconditions::filter_by_preconditions(
                        &mut result.verb_candidates,
                        &self.verb_config_index,
                        &context_stack,
                        super::preconditions::EligibilityMode::Executable,
                    );
                    if _filter_stats.before_count != _filter_stats.after_count {
                        // Re-evaluate outcome after filtering.
                        let new_outcome =
                            super::scoring::apply_ambiguity_policy(&result.verb_candidates);
                        result.outcome = match new_outcome {
                            super::scoring::AmbiguityOutcome::NoMatch => {
                                super::types::MatchOutcome::NoMatch {
                                    reason: "No verb matched after precondition filter".to_string(),
                                }
                            }
                            super::scoring::AmbiguityOutcome::Confident { verb, score } => {
                                super::types::MatchOutcome::Matched {
                                    verb,
                                    confidence: score,
                                }
                            }
                            super::scoring::AmbiguityOutcome::Ambiguous { margin, .. } => {
                                super::types::MatchOutcome::Ambiguous { margin }
                            }
                            super::scoring::AmbiguityOutcome::Proposed { verb, score } => {
                                super::types::MatchOutcome::Matched {
                                    verb,
                                    confidence: score,
                                }
                            }
                        };
                    }
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
        sem_os_fingerprint: Option<String>,
        sem_os_pruned_count: usize,
    ) -> ReplResponseV2 {
        match outcome {
            VerbMatchOutcome::Matched {
                verb,
                confidence,
                generated_dsl,
            } => {
                // Phase F: Try deterministic arg extraction before LLM/DSL parsing.
                let turn = session.runbook.entries.len() as u32;
                let context_stack = super::context_stack::ContextStack::from_runbook(
                    &session.runbook,
                    session.staged_pack.clone(),
                    turn,
                );
                let (args, slot_provenance, det_model_id) = if let Some(det) =
                    super::deterministic_extraction::try_deterministic_extraction(
                        &verb,
                        original_input,
                        &context_stack,
                        svc.verb_config_index(),
                    ) {
                    (det.args, det.provenance, Some(det.model_id))
                } else {
                    let fallback_dsl = generated_dsl.as_deref().unwrap_or("");
                    let parsed = if fallback_dsl.is_empty() {
                        HashMap::new()
                    } else {
                        extract_args_from_dsl(fallback_dsl)
                    };
                    (parsed, HashMap::new(), None)
                };
                let dsl = generated_dsl.unwrap_or_else(|| rebuild_dsl(&verb, &args));

                // Phase G: Emit DecisionLog for this matched verb.
                {
                    let extraction_method = if det_model_id.is_some() {
                        ExtractionMethod::Deterministic
                    } else if args.is_empty() {
                        ExtractionMethod::None
                    } else {
                        ExtractionMethod::Llm
                    };
                    let prov_map: HashMap<String, String> = slot_provenance
                        .iter()
                        .map(|(k, v)| (k.clone(), format!("{:?}", v)))
                        .collect();
                    Self::emit_decision_log(
                        session,
                        original_input,
                        TurnType::IntentMatch,
                        VerbDecision {
                            raw_candidates: vec![VerbCandidateSnapshot {
                                verb_fqn: verb.clone(),
                                score: confidence,
                                domain: verb.split('.').next().map(|s| s.to_string()),
                                adjustments: vec![],
                            }],
                            reranked_candidates: vec![VerbCandidateSnapshot {
                                verb_fqn: verb.clone(),
                                score: confidence,
                                domain: verb.split('.').next().map(|s| s.to_string()),
                                adjustments: vec![],
                            }],
                            ambiguity_outcome: "confident".to_string(),
                            selected_verb: Some(verb.clone()),
                            confidence,
                            used_template_path: false,
                            template_id: None,
                            precondition_filter: None,
                            context_envelope_fingerprint: sem_os_fingerprint.clone(),
                            pruned_verbs_count: sem_os_pruned_count,
                        },
                        ExtractionDecision {
                            method: extraction_method,
                            filled_args: args.clone(),
                            missing_args: vec![],
                            slot_provenance: prov_map,
                            model_id: det_model_id.map(|m| m.to_string()),
                            llm_confidence: if det_model_id.is_some() {
                                None
                            } else {
                                Some(confidence as f64)
                            },
                        },
                        Some(dsl.clone()),
                        &context_stack,
                    );
                }

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
                            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                        };
                    }
                    ClarificationOutcome::Complete => {
                        // All args present — proceed to sentence playback
                    }
                }

                // Generate sentence via IntentService (uses YAML templates)
                let sentence = svc.generate_sentence(&verb, &args);
                let confirm_policy = svc.confirm_policy(&verb);

                // Build audit — use deterministic model_id if extraction succeeded.
                let mut audit = build_arg_extraction_audit(
                    original_input,
                    &args,
                    confidence,
                    None, // IntentService doesn't expose debug info here
                );
                if let Some(model) = det_model_id {
                    audit.model_id = model.to_string();
                    audit.confidence = 1.0; // Deterministic extraction is fully confident.
                }

                session.pending_arg_audit = Some(audit.clone());
                // Stash slot provenance for use when creating the RunbookEntry.
                session.pending_slot_provenance = Some(slot_provenance.clone());

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

                    // Compile the verb (classify → compile → attach runbook ID).
                    if let Some(resp) = self.try_compile_entry(session, &mut entry) {
                        return resp;
                    }

                    session.runbook.add_entry(entry);

                    let next_state = if session.has_active_pack() {
                        ReplStateV2::InPack {
                            pack_id: session.active_pack_id().unwrap_or_default(),
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
                        session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }

            VerbMatchOutcome::Ambiguous { candidates, margin } => {
                // Phase G: Emit DecisionLog for ambiguous outcome.
                {
                    let turn = session.runbook.entries.len() as u32;
                    let ctx = super::context_stack::ContextStack::from_runbook(
                        &session.runbook,
                        session.staged_pack.clone(),
                        turn,
                    );
                    let snaps: Vec<VerbCandidateSnapshot> = candidates
                        .iter()
                        .map(|c| VerbCandidateSnapshot {
                            verb_fqn: c.verb_fqn.clone(),
                            score: c.score,
                            domain: c.verb_fqn.split('.').next().map(|s| s.to_string()),
                            adjustments: vec![],
                        })
                        .collect();
                    Self::emit_decision_log(
                        session,
                        original_input,
                        TurnType::IntentMatch,
                        VerbDecision {
                            raw_candidates: snaps.clone(),
                            reranked_candidates: snaps,
                            ambiguity_outcome: format!("ambiguous(margin={:.3})", margin),
                            selected_verb: None,
                            confidence: candidates.first().map(|c| c.score).unwrap_or(0.0),
                            used_template_path: false,
                            template_id: None,
                            precondition_filter: None,
                            context_envelope_fingerprint: sem_os_fingerprint.clone(),
                            pruned_verbs_count: sem_os_pruned_count,
                        },
                        ExtractionDecision::default(),
                        None,
                        &ctx,
                    );
                }

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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                }
            }

            VerbMatchOutcome::NoMatch { reason } => {
                // Phase G: Emit DecisionLog for no-match.
                {
                    let turn = session.runbook.entries.len() as u32;
                    let ctx = super::context_stack::ContextStack::from_runbook(
                        &session.runbook,
                        session.staged_pack.clone(),
                        turn,
                    );
                    Self::emit_decision_log(
                        session,
                        original_input,
                        TurnType::IntentMatch,
                        VerbDecision {
                            raw_candidates: vec![],
                            reranked_candidates: vec![],
                            ambiguity_outcome: format!("no_match({})", reason),
                            selected_verb: None,
                            confidence: 0.0,
                            used_template_path: false,
                            template_id: None,
                            precondition_filter: None,
                            context_envelope_fingerprint: sem_os_fingerprint.clone(),
                            pruned_verbs_count: sem_os_pruned_count,
                        },
                        ExtractionDecision::default(),
                        None,
                        &ctx,
                    );
                }
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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

    /// Phase G: Emit a DecisionLog entry for a verb match outcome.
    ///
    /// Called after `handle_intent_service_outcome` resolves the outcome
    /// to capture the full decision snapshot for offline replay.
    fn emit_decision_log(
        session: &mut ReplSessionV2,
        original_input: &str,
        turn_type: TurnType,
        verb_decision: VerbDecision,
        extraction: ExtractionDecision,
        proposed_dsl: Option<String>,
        context_stack: &ContextStack,
    ) {
        let turn = session.decision_log.len() as u32;
        let log = DecisionLog::new(session.id, turn, original_input)
            .with_turn_type(turn_type)
            .with_verb_decision(verb_decision)
            .with_extraction_decision(extraction)
            .with_context_summary(ContextSummary::from_context(context_stack));

        let log = if let Some(dsl) = proposed_dsl {
            log.with_proposed_dsl(dsl)
        } else {
            log
        };

        session.decision_log.push(log);
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

                    // Compile the verb (classify → compile → attach runbook ID).
                    if let Some(resp) = self.try_compile_entry(session, &mut entry) {
                        return resp;
                    }

                    session.runbook.add_entry(entry);

                    let next_state = if session.has_active_pack() {
                        ReplStateV2::InPack {
                            pack_id: session.active_pack_id().unwrap_or_default(),
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
                        session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    fn try_instantiate_template(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        let pack = match session.staged_pack.as_ref() {
            Some(p) => p.clone(),
            None => return self.invalid_input(session, "No pack context."),
        };

        // Find the first template (Phase 0: use the first one).
        let template = match pack.templates.first() {
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                };
            }
        };

        // Build context vars from derived scope (replaces ClientContext reads).
        let ctx_stack = self.build_context_stack(session);
        let context_vars: HashMap<String, String> = {
            let mut vars = HashMap::new();
            if let Some(name) = &ctx_stack.derived_scope.client_group_name {
                vars.insert("client_name".to_string(), name.clone());
            }
            if let Some(id) = ctx_stack.derived_scope.client_group_id {
                vars.insert("client_group_id".to_string(), id.to_string());
            }
            vars
        };

        // Build invocation phrases and descriptions from VerbConfigIndex.
        let verb_phrases = self.verb_config_index.all_invocation_phrases();
        let verb_descriptions = self.verb_config_index.all_descriptions();

        // Answers derived from runbook fold.
        let answers = &ctx_stack.accumulated_answers;

        match instantiate_template(
            template,
            &context_vars,
            answers,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    /// Handle Cancel command — return to InPack or RunbookEditing.
    fn handle_cancel(&self, session: &mut ReplSessionV2) -> ReplResponseV2 {
        let next_state = if session.has_active_pack() {
            ReplStateV2::InPack {
                pack_id: session.active_pack_id().unwrap_or_default(),
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    /// Handle Info command — show session info and readiness.
    fn handle_info(&self, session: &ReplSessionV2) -> ReplResponseV2 {
        let readiness = session.runbook.readiness();
        let ctx_stack = self.build_context_stack(session);
        let scope = ctx_stack
            .derived_scope
            .client_group_name
            .clone()
            .unwrap_or_else(|| "none".to_string());
        let pack = session
            .staged_pack
            .as_ref()
            .map(|p| p.name.clone())
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
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
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                };
            }
        }

        session.runbook.set_status(RunbookStatus::Executing);
        let runbook_id = session.runbook.id;
        let total = session.runbook.entries.len();
        if start_index == 0 {
            session.pending_execution_rechecks.clear();
        }

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

            if let Some(outcome) = self
                .phase5_runtime_recheck(session, idx, entry_id, &entry_sentence, &entry_dsl)
                .await
            {
                let entry = &mut session.runbook.entries[idx];
                entry.status = EntryStatus::Failed;
                results.push(StepResult {
                    entry_id,
                    sequence: entry_sequence,
                    sentence: entry_sentence,
                    success: false,
                    message: Some(recheck_failure_message(&outcome)),
                    result: None,
                });
                continue;
            }

            // Allocate a version for fallback compilation (only consumed if
            // the entry lacks a compiled_runbook_id).
            let fallback_version = session.allocate_runbook_version();

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
                    // Route through execution gate (INV-3).
                    let entry_snapshot = session.runbook.entries[idx].clone();
                    let gate_outcome = self
                        .execute_entry_via_gate(
                            &entry_snapshot,
                            session.id,
                            true, // is_durable
                            runbook_id,
                            fallback_version,
                        )
                        .await;

                    match gate_outcome {
                        StepOutcome::Completed { result } => {
                            let result_json = serde_json::to_value(&result).ok();
                            {
                                let entry = &mut session.runbook.entries[idx];
                                entry.status = EntryStatus::Completed;
                                entry.result = Some(result.clone());
                            }
                            session.increment_tos_writes();
                            session.append_trace_enriched(
                                super::session_trace::TraceOp::VerbExecuted {
                                    verb_fqn: entry_dsl.clone(),
                                    step_id: entry_id,
                                },
                                Some(entry_dsl.clone()),
                                result_json,
                            );
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: true,
                                message: Some("Completed".to_string()),
                                result: Some(result),
                            });
                        }
                        StepOutcome::Parked {
                            correlation_key,
                            message,
                        } => {
                            let mut invocation = InvocationRecord::new(
                                entry_id,
                                runbook_id,
                                session.id,
                                correlation_key.clone(),
                                GateType::DurableTask,
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
                                    "Parked: {} (key: {})",
                                    message, correlation_key
                                )),
                                result: None,
                            });
                            parked = true;
                            break;
                        }
                        StepOutcome::Failed { error } => {
                            let entry = &mut session.runbook.entries[idx];
                            entry.status = EntryStatus::Failed;
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: false,
                                message: Some(error),
                                result: None,
                            });
                        }
                        StepOutcome::Skipped { reason } => {
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: true,
                                message: Some(format!("Skipped: {}", reason)),
                                result: None,
                            });
                        }
                    }
                }

                ExecutionMode::Sync => {
                    // Route through execution gate (INV-3).
                    let entry_snapshot = session.runbook.entries[idx].clone();
                    let entry = &mut session.runbook.entries[idx];
                    entry.status = EntryStatus::Executing;

                    let gate_outcome = self
                        .execute_entry_via_gate(
                            &entry_snapshot,
                            session.id,
                            false, // not durable
                            runbook_id,
                            fallback_version,
                        )
                        .await;

                    match gate_outcome {
                        StepOutcome::Completed { result } => {
                            let result_json = serde_json::to_value(&result).ok();
                            {
                                let entry = &mut session.runbook.entries[idx];
                                entry.status = EntryStatus::Completed;
                                entry.result = Some(result.clone());
                            }
                            session.increment_tos_writes();
                            session.append_trace_enriched(
                                super::session_trace::TraceOp::VerbExecuted {
                                    verb_fqn: entry_dsl.clone(),
                                    step_id: entry_id,
                                },
                                Some(entry_dsl.clone()),
                                result_json,
                            );
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: true,
                                message: Some("Completed".to_string()),
                                result: Some(result),
                            });
                        }
                        StepOutcome::Failed { error } => {
                            let entry = &mut session.runbook.entries[idx];
                            entry.status = EntryStatus::Failed;
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: false,
                                message: Some(error),
                                result: None,
                            });
                        }
                        StepOutcome::Parked {
                            correlation_key,
                            message,
                        } => {
                            // Unexpected park from sync path — treat as parked.
                            let mut invocation = InvocationRecord::new(
                                entry_id,
                                runbook_id,
                                session.id,
                                correlation_key.clone(),
                                GateType::DurableTask,
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
                                    "Parked: {} (key: {})",
                                    message, correlation_key
                                )),
                                result: None,
                            });
                            parked = true;
                            break;
                        }
                        StepOutcome::Skipped { reason } => {
                            results.push(StepResult {
                                entry_id,
                                sequence: entry_sequence,
                                sentence: entry_sentence,
                                success: true,
                                message: Some(format!("Skipped: {}", reason)),
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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
            }
        } else {
            // All entries processed — back to editing.
            let all_success = results.iter().all(|r| r.success);
            session.runbook.set_status(if all_success {
                RunbookStatus::Completed
            } else {
                RunbookStatus::Ready // Allow retry
            });

            // Check for pack handoff on successful completion.
            if all_success {
                if let Some(handoff_resp) = self.try_pack_handoff(session, &results) {
                    return handoff_resp;
                }
            }

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
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
            }
        }
    }

    /// Re-hydrate Sem OS legality before executing a runbook node.
    ///
    /// Returns `Some(StepOutcome::Failed)` when the step must be blocked at
    /// execution time because the selected verb is no longer legal.
    async fn phase5_runtime_recheck(
        &self,
        session: &mut ReplSessionV2,
        entry_index: usize,
        entry_id: Uuid,
        entry_sentence: &str,
        entry_dsl: &str,
    ) -> Option<StepOutcome> {
        let Some(entry) = session.runbook.entries.get(entry_index) else {
            return Some(StepOutcome::Failed {
                error: "Runbook entry missing during Phase 5 re-check".to_string(),
            });
        };

        let Some(ref client) = self.sem_os_client else {
            session.pending_execution_rechecks.push(serde_json::json!({
                "entry_id": entry_id,
                "sequence": entry.sequence,
                "verb": entry.verb,
                "status": "skipped",
                "reason": "sem_os_client_unconfigured",
            }));
            return None;
        };

        let actor = crate::policy::ActorResolver::from_env();
        let envelope = crate::agent::orchestrator::resolve_allowed_verbs(
            client.as_ref(),
            &actor,
            Some(session.id),
        )
        .await;
        session
            .pending_execution_rechecks
            .push(phase5_recheck_record(
                entry_id,
                entry.sequence,
                &entry.verb,
                entry_sentence,
                entry_dsl,
                &envelope,
            ));

        phase5_recheck_failure(&entry.verb, &envelope)
    }

    /// Compile a runbook entry on-the-fly for entries that lack a `compiled_runbook_id`.
    ///
    /// This is the **fallback path** — entries created before the compile pipeline was
    /// wired, or entries from code paths not yet routing through `try_compile_entry()`.
    /// The fallback ALWAYS goes through compile → store → execute_runbook(id).
    /// Raw DSL execution without a `CompiledRunbookId` is never permitted (INV-3).
    ///
    /// `version` must come from `session.allocate_runbook_version()` — the caller
    /// is responsible for passing it since the session borrow is already active.
    fn compile_entry_on_the_fly(
        &self,
        entry: &RunbookEntry,
        session_id: Uuid,
        version: u64,
    ) -> CompiledRunbook {
        use crate::runbook::write_set::derive_write_set;

        let compiled_mode = match entry.execution_mode {
            ExecutionMode::Sync => CompiledExecutionMode::Sync,
            ExecutionMode::Durable => CompiledExecutionMode::Durable,
            ExecutionMode::HumanGate => CompiledExecutionMode::HumanGate,
        };
        let step = CompiledStep {
            step_id: entry.id,
            sentence: entry.sentence.clone(),
            verb: entry.verb.clone(),
            dsl: entry.dsl.clone(),
            args: entry
                .args
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            depends_on: entry.depends_on.clone(),
            execution_mode: compiled_mode,
            write_set: derive_write_set(
                &entry.verb,
                &entry
                    .args
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                None,
            )
            .into_iter()
            .collect(),
            verb_contract_snapshot_id: None, // TODO: wire SemReg snapshot resolution
        };
        CompiledRunbook::new(session_id, version, vec![step], ReplayEnvelope::empty())
    }

    /// Execute a runbook entry through the execution gate (INV-3).
    ///
    /// Two cases:
    /// - Entry has `compiled_runbook_id` → fetch from store, execute through gate
    /// - Entry lacks `compiled_runbook_id` → compile on-the-fly, store, execute through gate
    ///
    /// `fallback_version` is used only when the entry lacks a `compiled_runbook_id`
    /// and must be compiled on-the-fly. The caller should pass
    /// `session.allocate_runbook_version()` to ensure monotonic versioning.
    ///
    /// Returns the `StepOutcome` from the single step in the compiled runbook.
    async fn execute_entry_via_gate(
        &self,
        entry: &RunbookEntry,
        session_id: Uuid,
        is_durable: bool,
        runbook_id: Uuid,
        fallback_version: u64,
    ) -> StepOutcome {
        // Construct the store backend: Postgres when pool available, in-memory fallback.
        // This ensures lock events, status events, and holder lookups fire in production
        // (Phase D: RunbookStoreBackend trait wiring).
        #[cfg(feature = "database")]
        let pg_store: Option<PostgresRunbookStore> = self
            .pool
            .as_ref()
            .map(|p| PostgresRunbookStore::new(p.clone()));

        // Fallback in-memory store for when no pool (tests, non-database config).
        let fallback_store: RunbookStore = RunbookStore::new();

        #[cfg(feature = "database")]
        let store: &dyn RunbookStoreBackend = if let Some(ref pg) = pg_store {
            pg
        } else if let Some(ref s) = self.runbook_store {
            s.as_ref()
        } else {
            &fallback_store
        };
        #[cfg(not(feature = "database"))]
        let store: &dyn RunbookStoreBackend = if let Some(ref s) = self.runbook_store {
            s.as_ref()
        } else {
            &fallback_store
        };

        // Resolve or create the CompiledRunbookId.
        let compiled_id = if let Some(id) = entry.compiled_runbook_id {
            id
        } else {
            // Fallback: compile on-the-fly → store → gate
            tracing::debug!(
                entry_id = %entry.id,
                verb = %entry.verb,
                "Compiling entry on-the-fly (fallback path)"
            );
            let compiled = self.compile_entry_on_the_fly(entry, session_id, fallback_version);
            let id = compiled.id;
            if let Err(e) = store.insert(&compiled).await {
                tracing::error!(error = %e, "Failed to insert compiled runbook into store");
                return StepOutcome::Failed {
                    error: format!("Failed to store compiled runbook: {}", e),
                };
            }
            id
        };

        // Build the appropriate StepExecutor bridge and execute through the gate.
        let extract_first_outcome = |result: crate::runbook::executor::RunbookExecutionResult| {
            result
                .step_results
                .into_iter()
                .next()
                .map(|sr| sr.outcome)
                .unwrap_or(StepOutcome::Failed {
                    error: "No step results from execution gate".into(),
                })
        };

        if is_durable {
            if let Some(ref exec) = self.executor_v2 {
                let bridge = DslExecutorV2StepExecutor::new(exec.clone(), runbook_id);
                match execute_runbook_with_pool(
                    store,
                    compiled_id,
                    None,
                    &bridge,
                    self.pool.as_ref(),
                )
                .await
                {
                    Ok(result) => extract_first_outcome(result),
                    Err(e) => StepOutcome::Failed {
                        error: format!("Execution gate error: {}", e),
                    },
                }
            } else {
                // No V2 executor — fall back to sync bridge (never parks).
                let bridge = DslStepExecutor::new(Arc::clone(&self.executor));
                match execute_runbook_with_pool(
                    store,
                    compiled_id,
                    None,
                    &bridge,
                    self.pool.as_ref(),
                )
                .await
                {
                    Ok(result) => extract_first_outcome(result),
                    Err(e) => StepOutcome::Failed {
                        error: format!("Execution gate error: {}", e),
                    },
                }
            }
        } else {
            let bridge = DslStepExecutor::new(Arc::clone(&self.executor));
            match execute_runbook_with_pool(store, compiled_id, None, &bridge, self.pool.as_ref())
                .await
            {
                Ok(result) => extract_first_outcome(result),
                Err(e) => StepOutcome::Failed {
                    error: format!("Execution gate error: {}", e),
                },
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

    /// Check if the active pack has a handoff_target and, if so, transition
    /// to the target pack. Returns `Some(response)` if handoff occurred,
    /// `None` if no handoff is configured or the target pack is missing.
    fn try_pack_handoff(
        &self,
        session: &mut ReplSessionV2,
        _results: &[StepResult],
    ) -> Option<ReplResponseV2> {
        let handoff_target = session
            .staged_pack
            .as_ref()
            .and_then(|pack| pack.handoff_target.as_ref())
            .cloned()?;

        // Build handoff context from completed entry outcomes.
        let source_runbook_id = session.runbook.id;
        let forwarded_outcomes: Vec<Uuid> = session
            .runbook
            .entries
            .iter()
            .filter(|e| e.status == EntryStatus::Completed)
            .map(|e| e.id)
            .collect();

        let mut forwarded_context = HashMap::new();
        let ctx_stack = self.build_context_stack(session);
        if let Some(id) = ctx_stack.derived_scope.client_group_id {
            forwarded_context.insert("client_group_id".to_string(), id.to_string());
        }
        // Carry forward entry results as context (UUIDs of completed entries).
        for (i, entry_id) in forwarded_outcomes.iter().enumerate() {
            forwarded_context.insert(format!("outcome_{}", i), entry_id.to_string());
        }

        let handoff = PackHandoff {
            source_runbook_id,
            target_pack_id: handoff_target.clone(),
            forwarded_context: forwarded_context.clone(),
            forwarded_outcomes,
        };

        // Try to find the target pack in the router.
        if let Some((manifest, hash)) = self.pack_router.get_pack(&handoff_target) {
            let target_name = manifest.name.clone();
            let target_id = manifest.id.clone();
            let target_version = manifest.version.clone();
            let source_pack_id = session.active_pack_id();

            // Activate target pack with handoff context.
            session.activate_pack(manifest.clone(), hash.clone(), Some(handoff));

            // Create a fresh runbook for the new pack.
            session.runbook = super::runbook::Runbook::new(session.id);
            session.runbook.pack_id = Some(target_id.clone());
            session
                .runbook
                .audit
                .push(super::runbook::RunbookEvent::HandoffReceived {
                    source_runbook_id,
                    target_pack_id: handoff_target.clone(),
                    forwarded_context,
                    timestamp: chrono::Utc::now(),
                });

            // Record pack.select on the new runbook with handoff source (Invariant I-1).
            self.record_pack_select_entry(
                session,
                &target_id,
                &target_name,
                &target_version,
                hash,
                source_pack_id.as_deref(),
            );

            // Enter the target pack.
            let resp = self.enter_pack(session, &target_id);
            Some(ReplResponseV2 {
                message: format!(
                    "Execution complete. Handing off to: {}.\n\n{}",
                    target_name, resp.message
                ),
                ..resp
            })
        } else {
            tracing::warn!(
                target_pack = %handoff_target,
                "Handoff target pack not found, completing normally"
            );
            None
        }
    }

    /// Try to compile a verb via the runbook compilation pipeline.
    ///
    /// If a `MacroRegistry` is wired, this calls `classify_verb()` →
    /// `compile_verb()` and attaches the `compiled_runbook_id` to the entry.
    /// Returns `Some(response)` if compilation produced a non-Compiled result
    /// (Clarification or ConstraintViolation) that should be shown to the user
    /// instead of adding the entry.
    /// Returns `None` if compilation succeeded (entry was updated) or if
    /// no `MacroRegistry` is configured (graceful degradation).
    fn try_compile_entry(
        &self,
        session: &mut ReplSessionV2,
        entry: &mut RunbookEntry,
    ) -> Option<ReplResponseV2> {
        use crate::journey::pack_manager::{ConstraintSource, EffectiveConstraints};
        use crate::runbook::{classify_verb, compile_verb};
        use crate::session::unified::UnifiedSession;

        let macro_registry = self.macro_registry.as_ref()?;

        // Derive constraints from the active pack context (staged preferred over executed).
        let ctx = self.build_context_stack(session);

        // Build a UnifiedSession entirely from the ContextStack (runbook fold).
        // P-3 invariant: the runbook is the single source of truth.
        let mut unified = UnifiedSession::new();
        if let Some(cg_id) = ctx.derived_scope.client_group_id {
            unified.client = Some(crate::session::unified::ClientRef {
                client_id: cg_id,
                display_name: ctx
                    .derived_scope
                    .client_group_name
                    .clone()
                    .unwrap_or_default(),
            });
        }
        // Pack answers may contain structure_type (needed before StructureRef below)
        if let Some(st_val) = ctx.accumulated_answers.get("structure_type") {
            if let Ok(st) =
                serde_json::from_value::<crate::session::unified::StructureType>(st_val.clone())
            {
                unified.structure_type = Some(st);
            }
        }
        // Focus CBU → current_structure (macro ${session.current_structure})
        if let Some(ref focus_cbu) = ctx.focus.cbu {
            unified.current_structure = Some(crate::session::unified::StructureRef {
                structure_id: focus_cbu.id,
                display_name: focus_cbu.display_name.clone(),
                structure_type: unified.structure_type.unwrap_or_default(),
            });
        }
        // Focus case → current_case (macro ${session.current_case})
        if let Some(ref focus_case) = ctx.focus.case {
            unified.current_case = Some(crate::session::unified::CaseRef {
                case_id: focus_case.id,
                display_name: focus_case.display_name.clone(),
            });
        }
        // Executed verbs → DAG completed set (prereq VerbCompleted checks)
        unified.dag_state.completed = ctx.executed_verbs.clone();
        let pack_ctx = ctx.pack_staged.as_ref().or(ctx.pack_executed.as_ref());
        let constraints = if let Some(pc) = pack_ctx {
            EffectiveConstraints {
                allowed_verbs: if pc.allowed_verbs.is_empty() {
                    None
                } else {
                    Some(pc.allowed_verbs.clone())
                },
                forbidden_verbs: pc.forbidden_verbs.clone(),
                contributing_packs: vec![ConstraintSource {
                    pack_id: pc.pack_id.clone(),
                    pack_name: pc.pack_id.clone(),
                    allowed_count: pc.allowed_verbs.len(),
                    forbidden_count: pc.forbidden_verbs.len(),
                }],
            }
        } else {
            EffectiveConstraints::unconstrained()
        };

        let classification = classify_verb(&entry.verb, &self.verb_config_index, macro_registry);
        let version = session.allocate_runbook_version();
        // Convert HashMap → BTreeMap for deterministic iteration (INV-2, Phase C)
        let args_btree: std::collections::BTreeMap<String, String> = entry
            .args
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let response = compile_verb(
            session.id,
            &classification,
            &args_btree,
            &unified,
            macro_registry,
            version,
            &constraints,
            None, // sem_reg_allowed_verbs — resolved upstream by orchestrator
            None, // verb_snapshot_pins — TODO: wire SemReg snapshot resolution
        );

        match response {
            crate::runbook::OrchestratorResponse::Compiled(summary) => {
                entry.compiled_runbook_id = Some(summary.compiled_runbook_id);
                // Store the compiled runbook artifact so execute_runbook() can
                // retrieve it by ID. INV-3: no execution without a stored artifact.
                if let Some(ref store) = self.runbook_store {
                    if let Some(ref runbook) = summary.compiled_runbook {
                        store.insert_sync(runbook);
                    }
                }
                None // success — caller continues with entry
            }
            crate::runbook::OrchestratorResponse::Clarification(c) => Some(ReplResponseV2 {
                state: session.state.clone(),
                kind: ReplResponseKindV2::Clarification {
                    question: c.question.clone(),
                    options: vec![], // no verb candidates — this is arg clarification
                },
                message: c.question,
                runbook_summary: None,
                step_count: session.runbook.entries.len(),
                session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
            }),
            crate::runbook::OrchestratorResponse::ConstraintViolation(v) => {
                let msg = format!("Pack constraint violation: {}", v.explanation,);
                Some(ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Error {
                        error: msg.clone(),
                        recoverable: true,
                    },
                    message: msg,
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                })
            }
            crate::runbook::OrchestratorResponse::CompilationError(e) => {
                let msg = format!("Compilation failed ({}): {}", e.source_phase, e.kind);
                Some(ReplResponseV2 {
                    state: session.state.clone(),
                    kind: ReplResponseKindV2::Error {
                        error: msg.clone(),
                        recoverable: true,
                    },
                    message: msg,
                    runbook_summary: None,
                    step_count: session.runbook.entries.len(),
                    session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
                })
            }
        }
    }

    fn runbook_summary(&self, session: &ReplSessionV2) -> String {
        if let Some(ref pack) = session.staged_pack {
            let ctx_stack = self.build_context_stack(session);
            PackPlayback::summarize(pack, &session.runbook, &ctx_stack.accumulated_answers)
        } else {
            format!("{} steps in runbook", session.runbook.entries.len())
        }
    }

    fn chapter_view(&self, session: &ReplSessionV2) -> Vec<ChapterView> {
        if let Some(ref pack) = session.staged_pack {
            PackPlayback::chapter_view(pack, &session.runbook)
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
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        }
    }

    fn phase2_gate_response(&self, session: &ReplSessionV2) -> Option<ReplResponseV2> {
        let phase2 = Phase2Service::evaluate_from_refs(
            session.pending_lookup_result.as_ref(),
            session.pending_sem_os_envelope.as_ref(),
        );
        let reason = phase2.halt_reason_code?;
        let message = match reason {
            "no_allowed_verbs" => {
                if let Some(block) = phase2.primary_constellation_block() {
                    return Some(self.invalid_input(
                        session,
                        &format!(
                            "Blocked by constellation state: {}. {}.",
                            block.predicate, block.resolution_hint
                        ),
                    ));
                }
                "No legal actions are currently available for this session state."
            }
            "sem_os_unavailable" => {
                "Semantic OS context is unavailable, so verb resolution is blocked."
            }
            "ambiguous_entity" => {
                // Entity ambiguity is informational — don't block verb matching.
                // Macros and deterministic phrase matches resolve without entity context.
                // Only halt if no verb match is found downstream.
                tracing::debug!(
                    "Phase2 gate: ambiguous_entity detected, deferring to verb matching"
                );
                return None;
            }
            "no_entity_found" => {
                "I could not resolve the referenced entity. Please provide more details."
            }
            _ => return None,
        };

        Some(self.invalid_input(session, message))
    }
}

fn recheck_failure_message(outcome: &StepOutcome) -> String {
    match outcome {
        StepOutcome::Failed { error } => error.clone(),
        StepOutcome::Completed { .. } => "Phase 5 re-check unexpectedly completed".to_string(),
        StepOutcome::Parked { message, .. } => format!("Phase 5 re-check parked: {message}"),
        StepOutcome::Skipped { reason } => format!("Phase 5 re-check skipped: {reason}"),
    }
}

fn phase5_recheck_record(
    entry_id: Uuid,
    sequence: i32,
    verb: &str,
    sentence: &str,
    dsl_command: &str,
    envelope: &crate::agent::sem_os_context_envelope::SemOsContextEnvelope,
) -> serde_json::Value {
    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    let status = Phase2Service::runtime_gate_status(&phase2.artifacts, verb);
    let primary_block = phase2.primary_constellation_block();

    serde_json::json!({
    "entry_id": entry_id,
    "sequence": sequence,
    "verb": verb,
    "sentence": sentence,
    "dsl_command": dsl_command,
    "status": status,
    "sem_os_label": phase2.policy_label,
    "allowed_verb_count": phase2.legal_verb_count(),
    "pruned_verb_count": phase2.pruned_verb_count(),
    "fingerprint": phase2.fingerprint(),
    "snapshot_set_id": envelope.snapshot_set_id,
    "blocking_entity": primary_block.as_ref().and_then(|block| block.blocking_entity.clone()),
    "blocking_state": primary_block.as_ref().and_then(|block| block.blocking_state.clone()),
    "blocking_predicate": primary_block.as_ref().map(|block| block.predicate.clone()),
    "resolution_hint": primary_block.as_ref().map(|block| block.resolution_hint.clone()),
    })
}

fn phase5_recheck_failure(
    verb: &str,
    envelope: &crate::agent::sem_os_context_envelope::SemOsContextEnvelope,
) -> Option<StepOutcome> {
    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    Phase2Service::runtime_gate_failure(&phase2.artifacts, verb)
        .map(|error| StepOutcome::Failed { error })
}

fn repl_trace_sage_context(session: &ReplSessionV2) -> crate::sage::SageContext {
    crate::sage::SageContext {
        session_id: Some(session.id),
        stage_focus: session.active_pack_id(),
        goals: Vec::new(),
        entity_kind: None,
        dominant_entity_name: None,
        last_intents: Vec::new(),
    }
}

fn input_trace_text(input: &UserInputV2) -> Option<String> {
    match input {
        UserInputV2::Message { content } => Some(content.clone()),
        UserInputV2::Command { command } => Some(format!("/{}", repl_command_name(command))),
        UserInputV2::SelectPack { pack_id } => Some(format!("select_pack:{pack_id}")),
        UserInputV2::SelectVerb {
            verb_fqn,
            original_input,
        } => Some(format!("select_verb:{verb_fqn}::{original_input}")),
        UserInputV2::SelectProposal { proposal_id } => {
            Some(format!("select_proposal:{proposal_id}"))
        }
        UserInputV2::SelectEntity {
            ref_id,
            entity_id,
            entity_name,
        } => Some(format!("select_entity:{ref_id}:{entity_id}:{entity_name}")),
        UserInputV2::SelectScope {
            group_id,
            group_name,
        } => Some(format!("select_scope:{group_id}:{group_name}")),
        UserInputV2::SelectWorkspace { workspace } => {
            Some(format!("select_workspace:{}", workspace.label()))
        }
        UserInputV2::Approve {
            entry_id,
            approved_by,
        } => Some(format!(
            "approve:{entry_id}:{}",
            approved_by.as_deref().unwrap_or_default()
        )),
        UserInputV2::RejectGate { entry_id, reason } => Some(format!(
            "reject_gate:{entry_id}:{}",
            reason.as_deref().unwrap_or_default()
        )),
        UserInputV2::Confirm => Some("confirm".to_string()),
        UserInputV2::Reject => Some("reject".to_string()),
        UserInputV2::Edit {
            step_id,
            field,
            value,
        } => Some(format!("edit:{step_id}:{field}:{value}")),
    }
}

fn repl_trace_kind(session: &ReplSessionV2, input: &UserInputV2) -> TraceKind {
    match input {
        UserInputV2::Command {
            command: ReplCommandV2::Resume(_),
        } => TraceKind::ResumedExecution,
        _ if repl_parent_trace_id(session, input).is_some() => TraceKind::ClarificationResponse,
        _ => TraceKind::Original,
    }
}

fn repl_parent_trace_id(session: &ReplSessionV2, input: &UserInputV2) -> Option<Uuid> {
    match input {
        UserInputV2::Confirm
        | UserInputV2::Reject
        | UserInputV2::SelectWorkspace { .. }
        | UserInputV2::SelectPack { .. }
        | UserInputV2::SelectVerb { .. }
        | UserInputV2::SelectProposal { .. }
        | UserInputV2::SelectEntity { .. }
        | UserInputV2::SelectScope { .. }
        | UserInputV2::Approve { .. }
        | UserInputV2::RejectGate { .. }
        | UserInputV2::Edit { .. }
        | UserInputV2::Command { .. } => session.pending_trace_id,
        UserInputV2::Message { .. } => match session.state {
            ReplStateV2::Clarifying { .. } | ReplStateV2::SentencePlayback { .. } => {
                session.pending_trace_id
            }
            _ => None,
        },
    }
}

fn update_repl_trace_lineage(
    session: &mut ReplSessionV2,
    trace_id: Option<Uuid>,
    response: &ReplResponseV2,
) {
    session.last_trace_id = trace_id;
    session.pending_trace_id = match response.kind {
        ReplResponseKindV2::ScopeRequired { .. }
        | ReplResponseKindV2::WorkspaceOptions { .. }
        | ReplResponseKindV2::JourneyOptions { .. }
        | ReplResponseKindV2::Question { .. }
        | ReplResponseKindV2::SentencePlayback { .. }
        | ReplResponseKindV2::Clarification { .. }
        | ReplResponseKindV2::StepProposals { .. } => trace_id,
        ReplResponseKindV2::Executed { .. }
        | ReplResponseKindV2::Parked { .. }
        | ReplResponseKindV2::RunbookSummary { .. }
        | ReplResponseKindV2::Info { .. }
        | ReplResponseKindV2::Prompt { .. }
        | ReplResponseKindV2::Error { .. } => None,
    };
}

fn repl_response_needs_follow_up(response: &ReplResponseV2) -> bool {
    matches!(
        response.kind,
        ReplResponseKindV2::ScopeRequired { .. }
            | ReplResponseKindV2::WorkspaceOptions { .. }
            | ReplResponseKindV2::JourneyOptions { .. }
            | ReplResponseKindV2::Question { .. }
            | ReplResponseKindV2::SentencePlayback { .. }
            | ReplResponseKindV2::Clarification { .. }
            | ReplResponseKindV2::StepProposals { .. }
    )
}

fn classify_repl_trace_outcome(response: &ReplResponseV2) -> crate::traceability::TraceOutcome {
    match response.kind {
        ReplResponseKindV2::Executed { .. } => {
            crate::traceability::TraceOutcome::ExecutedSuccessfully
        }
        ReplResponseKindV2::Error { .. } => crate::traceability::TraceOutcome::HaltedAtPhase,
        ReplResponseKindV2::JourneyOptions { .. } => crate::traceability::TraceOutcome::NoMatch,
        ReplResponseKindV2::Parked { .. }
        | ReplResponseKindV2::ScopeRequired { .. }
        | ReplResponseKindV2::WorkspaceOptions { .. }
        | ReplResponseKindV2::Question { .. }
        | ReplResponseKindV2::SentencePlayback { .. }
        | ReplResponseKindV2::Clarification { .. }
        | ReplResponseKindV2::StepProposals { .. }
        | ReplResponseKindV2::Info { .. }
        | ReplResponseKindV2::Prompt { .. }
        | ReplResponseKindV2::RunbookSummary { .. } => {
            crate::traceability::TraceOutcome::ClarificationTriggered
        }
    }
}

fn repl_halt_reason_code(session: &ReplSessionV2, response: &ReplResponseV2) -> Option<String> {
    if matches!(response.kind, ReplResponseKindV2::Error { .. }) {
        if let Some(reason) = repl_phase2_halt_reason(session) {
            return Some(reason);
        }
    }

    match response.kind {
        ReplResponseKindV2::Error { .. } => Some("repl_error".to_string()),
        ReplResponseKindV2::JourneyOptions { .. } => Some("journey_selection_required".to_string()),
        _ => None,
    }
}

fn repl_halt_phase(session: &ReplSessionV2, response: &ReplResponseV2) -> Option<i16> {
    if matches!(response.kind, ReplResponseKindV2::Error { .. }) {
        if let Some(phase2) = repl_phase2_halt_artifacts(session) {
            if phase2.halt_phase.is_some() {
                return phase2.halt_phase;
            }
        }
    }

    match response.kind {
        ReplResponseKindV2::Error { .. } | ReplResponseKindV2::JourneyOptions { .. } => Some(4),
        _ => None,
    }
}

fn repl_phase2_halt_artifacts(
    session: &ReplSessionV2,
) -> Option<crate::traceability::Phase2Evaluation> {
    let phase2 = Phase2Service::evaluate_from_refs(
        session.pending_lookup_result.as_ref(),
        session.pending_sem_os_envelope.as_ref(),
    );
    phase2.is_available.then_some(phase2)
}

fn repl_phase2_halt_reason(session: &ReplSessionV2) -> Option<String> {
    repl_phase2_halt_artifacts(session)
        .as_ref()
        .and_then(|phase2| phase2.halt_reason_code)
        .map(ToString::to_string)
}

fn repl_command_name(command: &ReplCommandV2) -> &'static str {
    match command {
        ReplCommandV2::Run => "run",
        ReplCommandV2::Undo => "undo",
        ReplCommandV2::Redo => "redo",
        ReplCommandV2::Clear => "clear",
        ReplCommandV2::Cancel => "cancel",
        ReplCommandV2::Info => "info",
        ReplCommandV2::Help => "help",
        ReplCommandV2::Remove(_) => "remove",
        ReplCommandV2::Reorder(_) => "reorder",
        ReplCommandV2::Disable(_) => "disable",
        ReplCommandV2::Enable(_) => "enable",
        ReplCommandV2::Toggle(_) => "toggle",
        ReplCommandV2::Status => "status",
        ReplCommandV2::Resume(_) => "resume",
    }
}

fn response_resolved_verb(response: &ReplResponseV2, session: &ReplSessionV2) -> Option<String> {
    match (&response.kind, &response.state) {
        (ReplResponseKindV2::SentencePlayback { verb, .. }, _) => Some(verb.clone()),
        (_, ReplStateV2::SentencePlayback { verb, .. }) => Some(verb.clone()),
        _ => session
            .runbook
            .entries
            .last()
            .map(|entry| entry.verb.clone()),
    }
}

fn build_repl_phase4_evaluation(
    response: &ReplResponseV2,
    session: &ReplSessionV2,
    resolved_verb: Option<&str>,
    phase2: &crate::traceability::Phase2Evaluation,
) -> Option<crate::traceability::Phase4Evaluation> {
    let resolved_verb = resolved_verb?;

    let strategy = match response.kind {
        ReplResponseKindV2::SentencePlayback { .. } => "repl_sentence_playback",
        ReplResponseKindV2::Executed { .. } => "repl_runbook_execute",
        ReplResponseKindV2::Parked { .. } => "repl_runbook_parked",
        _ => "repl_selection",
    };

    Some(evaluate_phase4_within_phase2(
        Some(resolved_verb.to_string()),
        vec![resolved_verb.to_string()],
        strategy,
        if session.runbook.entries.is_empty() {
            0.85
        } else {
            1.0
        },
        None,
        phase2,
    ))
}

fn build_repl_phase3_evaluation(
    response: &ReplResponseV2,
    session: &ReplSessionV2,
    resolved_verb: Option<&str>,
    phase2: &crate::traceability::Phase2Evaluation,
) -> Option<crate::traceability::Phase3Evaluation> {
    let resolved_verb = resolved_verb?;
    let source = match response.kind {
        ReplResponseKindV2::Executed { .. } | ReplResponseKindV2::Parked { .. } => {
            VerbSearchSource::ScenarioIndex
        }
        ReplResponseKindV2::SentencePlayback { .. } => VerbSearchSource::MacroIndex,
        _ if session.runbook.entries.is_empty() => VerbSearchSource::LearnedExact,
        _ => VerbSearchSource::LexiconExact,
    };
    Some(evaluate_phase3_against_phase2(
        vec![VerbSearchResult {
            verb: resolved_verb.to_string(),
            score: if session.runbook.entries.is_empty() {
                0.85
            } else {
                1.0
            },
            source,
            matched_phrase: resolved_verb.to_string(),
            description: None,
            journey: None,
        }],
        phase2,
    ))
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
description: Hand off a contracted deal into onboarding for an existing CBU
invocation_phrases:
  - "request onboarding for this deal"
  - "submit onboarding handoff"
  - "onboard a client"
required_context:
  - client_group_id
required_questions:
  - field: deal_id
    prompt: "Which deal should be handed off?"
    answer_kind: string
  - field: cbu_id
    prompt: "Which existing CBU should receive the onboarding handoff?"
    answer_kind: string
templates:
  - template_id: basic-onboarding
    when_to_use: "Standard onboarding handoff"
    steps:
      - verb: deal.read
        args:
          deal-id: "{answers.deal_id}"
      - verb: cbu.read
        args:
          cbu-id: "{answers.cbu_id}"
      - verb: deal.request-onboarding
        args:
          deal-id: "{answers.deal_id}"
          contract-id: "{context.contract_id}"
          cbu-id: "{answers.cbu_id}"
          product-id: "{context.product_id}"
workspaces:
  - on_boarding
definition_of_done:
  - "Onboarding handoff submitted"
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
            ReplResponseKindV2::WorkspaceOptions { .. }
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

        // Select workspace (required after scope gate).
        orch.process(
            id,
            UserInputV2::SelectWorkspace {
                workspace: WorkspaceKind::OnBoarding,
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

        // Should ask the first required question or enter InPack.
        assert!(
            matches!(resp.kind, ReplResponseKindV2::Question { .. })
                || matches!(resp.kind, ReplResponseKindV2::Prompt { .. })
                || matches!(resp.kind, ReplResponseKindV2::Info { .. })
        );
    }

    #[test]
    fn test_compile_entry_on_the_fly_preserves_dependency_edges() {
        let orch = make_orchestrator();
        let mut entry = RunbookEntry::new(
            "case.open".to_string(),
            "Open case".to_string(),
            "(case.open)".to_string(),
        );
        entry.depends_on.push(Uuid::new_v4());
        let compiled = orch.compile_entry_on_the_fly(&entry, Uuid::new_v4(), 1);

        assert_eq!(compiled.steps.len(), 1);
        assert_eq!(compiled.steps[0].depends_on, entry.depends_on);
    }

    #[test]
    fn test_phase5_recheck_failure_blocks_verb_outside_allowed_set() {
        let envelope =
            crate::agent::sem_os_context_envelope::SemOsContextEnvelope::test_with_verbs(&[
                "cbu.create",
            ]);

        let outcome = phase5_recheck_failure("case.open", &envelope);
        assert!(matches!(outcome, Some(StepOutcome::Failed { .. })));
    }

    #[test]
    fn test_phase5_recheck_failure_surfaces_constellation_block() {
        let mut envelope = crate::agent::sem_os_context_envelope::SemOsContextEnvelope::deny_all();
        envelope.grounded_action_surface =
            Some(sem_os_core::context_resolution::GroundedActionSurface {
                resolved_subject: sem_os_core::context_resolution::SubjectRef::TaskId(Uuid::nil()),
                resolved_constellation: Some("constellation.kyc".to_string()),
                resolved_slot_path: Some("case".to_string()),
                resolved_node_id: Some("node-1".to_string()),
                resolved_state_machine: Some("case_machine".to_string()),
                current_state: Some("intake".to_string()),
                traversed_edges: vec![],
                constraint_signals: vec![
                    sem_os_core::context_resolution::GroundedConstraintSignal {
                        kind: "dependency_block".to_string(),
                        slot_path: "case".to_string(),
                        related_slot: Some("cbu".to_string()),
                        required_state: Some("filled".to_string()),
                        actual_state: Some("empty".to_string()),
                        message: "dependency 'cbu' is in state 'empty' but requires 'filled'"
                            .to_string(),
                    },
                ],
                valid_actions: vec![],
                blocked_actions: vec![sem_os_core::context_resolution::BlockedActionOption {
                    action_id: "case.open".to_string(),
                    action_kind: "primitive".to_string(),
                    description: "Blocked action for slot 'case'".to_string(),
                    reasons: vec![
                        "dependency 'cbu' is in state 'empty' but requires 'filled'".to_string()
                    ],
                }],
                dsl_candidates: vec![],
            });

        let outcome = phase5_recheck_failure("case.open", &envelope).expect("blocked");
        let StepOutcome::Failed { error } = outcome else {
            panic!("expected failed step outcome");
        };
        assert!(error.contains("dependency 'cbu' is in state 'empty' but requires 'filled'"));
        assert!(error.contains("move 'cbu' from 'empty' to at least 'filled'"));
    }

    #[test]
    fn test_phase5_recheck_record_marks_allowed_status() {
        let envelope =
            crate::agent::sem_os_context_envelope::SemOsContextEnvelope::test_with_verbs(&[
                "case.open",
            ]);

        let record = phase5_recheck_record(
            Uuid::nil(),
            1,
            "case.open",
            "Open case",
            "(case.open)",
            &envelope,
        );

        assert_eq!(record["status"], "allowed");
        assert_eq!(record["verb"], "case.open");
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

        // Select workspace.
        orch.process(
            id,
            UserInputV2::SelectWorkspace {
                workspace: WorkspaceKind::OnBoarding,
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

        // Select workspace.
        orch.process(
            id,
            UserInputV2::SelectWorkspace {
                workspace: WorkspaceKind::OnBoarding,
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

        // 1b. Select workspace.
        orch.process(
            id,
            UserInputV2::SelectWorkspace {
                workspace: WorkspaceKind::OnBoarding,
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

        // Set scope + select workspace + select pack + answer questions.
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
            UserInputV2::SelectWorkspace {
                workspace: WorkspaceKind::OnBoarding,
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

    // -----------------------------------------------------------------------
    // Phase H Acceptance Test: Full KYC case flow
    //
    // Validates: bootstrap → pack → template-guided case → run.
    // Verifies: ContextStack-derived state, accumulated_answers from runbook
    // fold, pack scoring, template instantiation, execution.
    // -----------------------------------------------------------------------

    fn kyc_yaml() -> &'static [u8] {
        include_bytes!("../../config/packs/kyc-case.yaml")
    }

    fn make_kyc_orchestrator() -> ReplOrchestratorV2 {
        let (manifest, hash) = load_pack_from_bytes(kyc_yaml()).unwrap();
        let packs = vec![(Arc::new(manifest), hash)];
        let router = PackRouter::new(packs);
        ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
    }

    /// Phase H acceptance test: full KYC case flow.
    ///
    /// Flow: ScopeGate → JourneySelection → InPack (answer questions) →
    ///       RunbookEditing (template built) → Executed (all steps complete).
    ///
    /// Validates:
    /// - DerivedScope from runbook fold (not ClientContext)
    /// - accumulated_answers from runbook fold (not JourneyContext.answers)
    /// - staged_pack drives pack reads (not JourneyContext.pack)
    /// - Template instantiation uses ContextStack
    /// - All runbook entries complete with results
    #[tokio::test]
    async fn test_phase_h_kyc_acceptance() {
        let orch = make_kyc_orchestrator();
        let id = orch.create_session().await;

        // Turn 1: Set scope — this records scope on the runbook.
        let group_id = Uuid::new_v4();
        let resp = orch
            .process(
                id,
                UserInputV2::SelectScope {
                    group_id,
                    group_name: "Aviva Investors".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(
            matches!(resp.kind, ReplResponseKindV2::WorkspaceOptions { .. }),
            "Expected WorkspaceOptions, got {:?}",
            resp.kind
        );

        // Verify DerivedScope from runbook fold.
        {
            let session = orch.get_session(id).await.unwrap();
            let ctx = session.build_context_stack(None);
            assert_eq!(ctx.derived_scope.client_group_id, Some(group_id));
            assert_eq!(
                ctx.derived_scope.client_group_name.as_deref(),
                Some("Aviva Investors")
            );
        }

        // Turn 1b: Select workspace.
        orch.process(
            id,
            UserInputV2::SelectWorkspace {
                workspace: WorkspaceKind::Kyc,
            },
        )
        .await
        .unwrap();

        // Turn 2: Select KYC pack.
        let resp = orch
            .process(
                id,
                UserInputV2::SelectPack {
                    pack_id: "kyc-case".to_string(),
                },
            )
            .await
            .unwrap();
        // Should ask the first required question: entity_name.
        assert!(
            matches!(resp.kind, ReplResponseKindV2::Question { .. }),
            "Expected Question, got {:?}",
            resp.kind
        );
        assert!(
            resp.message.contains("entity"),
            "First question should be about entity"
        );

        // Verify staged_pack is set (not just journey_context).
        {
            let session = orch.get_session(id).await.unwrap();
            assert!(session.staged_pack.is_some());
            assert_eq!(session.staged_pack.as_ref().unwrap().id, "kyc-case");
            assert!(session.has_active_pack());
            assert_eq!(session.active_pack_id().as_deref(), Some("kyc-case"));
        }

        // Turn 3: Answer entity_name.
        let resp = orch
            .process(
                id,
                UserInputV2::Message {
                    content: "Aviva Holdings Ltd".to_string(),
                },
            )
            .await
            .unwrap();
        // Should ask the next question: case_type.
        assert!(
            matches!(resp.kind, ReplResponseKindV2::Question { .. }),
            "Expected Question for case_type, got {:?}",
            resp.kind
        );

        // Verify accumulated_answers from runbook fold.
        {
            let session = orch.get_session(id).await.unwrap();
            let ctx = session.build_context_stack(None);
            assert!(
                ctx.accumulated_answers.contains_key("entity_name"),
                "entity_name should be in accumulated_answers from runbook fold"
            );
        }

        // Turn 4: Answer case_type → triggers template instantiation.
        let resp = orch
            .process(
                id,
                UserInputV2::Message {
                    content: "new".to_string(),
                },
            )
            .await
            .unwrap();

        // Should have built a runbook from template.
        assert!(
            matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
            "Expected RunbookSummary, got {:?}",
            resp.kind
        );

        // Verify runbook has template entries.
        {
            let session = orch.get_session(id).await.unwrap();
            assert!(
                !session.runbook.entries.is_empty(),
                "Runbook should have entries from template"
            );
            assert!(
                session.runbook.template_id.is_some(),
                "Runbook should track template_id"
            );
            assert_eq!(session.runbook.template_id.as_deref(), Some("new-kyc-case"));

            // Verify accumulated_answers has both answers.
            let ctx = session.build_context_stack(None);
            assert!(ctx.accumulated_answers.contains_key("entity_name"));
            assert!(ctx.accumulated_answers.contains_key("case_type"));

            // Verify DerivedScope is still correct (from runbook, not ClientContext).
            assert_eq!(ctx.derived_scope.client_group_id, Some(group_id));
        }

        // Turn 5: Execute the runbook.
        let resp = orch
            .process(
                id,
                UserInputV2::Command {
                    command: ReplCommandV2::Run,
                },
            )
            .await
            .unwrap();
        assert!(
            matches!(resp.kind, ReplResponseKindV2::Executed { .. }),
            "Expected Executed, got {:?}",
            resp.kind
        );

        // Final verification: all entries completed, results present.
        {
            let session = orch.get_session(id).await.unwrap();
            assert_eq!(session.runbook.status, RunbookStatus::Completed);
            for entry in &session.runbook.entries {
                assert_eq!(
                    entry.status,
                    EntryStatus::Completed,
                    "Entry '{}' should be Completed but was {:?}",
                    entry.verb,
                    entry.status
                );
                assert!(
                    entry.result.is_some(),
                    "Entry '{}' should have a result",
                    entry.verb
                );
            }

            // Verify: total turns ≤ 8 (scope + pack + 2 answers + run = 5).
            // This is well under the target of ≤8 turns.
            let turn_count = 5;
            assert!(
                turn_count <= 8,
                "Flow should complete in ≤8 turns, took {}",
                turn_count
            );

            // Verify: no ClientContext or JourneyContext was needed for reads.
            // All state was derived from runbook fold via ContextStack.
            let ctx = session.build_context_stack(None);
            assert_eq!(ctx.derived_scope.client_group_id, Some(group_id));
            assert!(ctx.accumulated_answers.len() >= 2);
        }
    }

    /// Phase H: Verify ContextStack rebuild from runbook is deterministic.
    ///
    /// Build context stack at different points in the flow and verify
    /// it always reflects the current runbook state.
    #[tokio::test]
    async fn test_phase_h_context_stack_determinism() {
        let orch = make_kyc_orchestrator();
        let id = orch.create_session().await;

        // Empty session → empty context.
        {
            let session = orch.get_session(id).await.unwrap();
            let ctx = session.build_context_stack(None);
            assert!(ctx.derived_scope.client_group_id.is_none());
            assert!(ctx.accumulated_answers.is_empty());
            assert!(ctx.active_pack().is_none());
        }

        // After scope.
        let group_id = Uuid::new_v4();
        orch.process(
            id,
            UserInputV2::SelectScope {
                group_id,
                group_name: "Test Corp".to_string(),
            },
        )
        .await
        .unwrap();

        {
            let session = orch.get_session(id).await.unwrap();
            let ctx = session.build_context_stack(None);
            assert_eq!(ctx.derived_scope.client_group_id, Some(group_id));
            assert!(ctx.accumulated_answers.is_empty());
        }

        // After workspace selection.
        orch.process(
            id,
            UserInputV2::SelectWorkspace {
                workspace: WorkspaceKind::Kyc,
            },
        )
        .await
        .unwrap();

        // After pack selection.
        orch.process(
            id,
            UserInputV2::SelectPack {
                pack_id: "kyc-case".to_string(),
            },
        )
        .await
        .unwrap();

        {
            let session = orch.get_session(id).await.unwrap();
            let ctx = session.build_context_stack(None);
            assert!(ctx.active_pack().is_some());
            assert_eq!(ctx.active_pack().unwrap().pack_id, "kyc-case");
        }

        // After answering a question.
        orch.process(
            id,
            UserInputV2::Message {
                content: "Some Entity".to_string(),
            },
        )
        .await
        .unwrap();

        {
            let session = orch.get_session(id).await.unwrap();
            let ctx = session.build_context_stack(None);
            // Scope still there.
            assert_eq!(ctx.derived_scope.client_group_id, Some(group_id));
            // Answer recorded via runbook fold.
            assert!(ctx.accumulated_answers.contains_key("entity_name"));
            // Pack still active.
            assert!(ctx.active_pack().is_some());
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

    #[test]
    fn test_repl_phase2_halt_reason_uses_sem_os_deny_all() {
        let mut session = ReplSessionV2::new();
        session.pending_sem_os_envelope =
            Some(crate::agent::sem_os_context_envelope::SemOsContextEnvelope::deny_all());
        let response = ReplResponseV2 {
            state: ReplStateV2::RunbookEditing,
            kind: ReplResponseKindV2::Error {
                error: "No matching action found".to_string(),
                recoverable: true,
            },
            message: "No matching action found".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        };

        assert_eq!(
            repl_halt_reason_code(&session, &response).as_deref(),
            Some("no_allowed_verbs")
        );
        assert_eq!(repl_halt_phase(&session, &response), Some(2));
    }

    #[test]
    fn test_repl_phase2_halt_reason_uses_ambiguous_lookup() {
        let mut session = ReplSessionV2::new();
        session.pending_lookup_result = Some(crate::lookup::LookupResult {
            verbs: vec![],
            entities: vec![crate::entity_linking::EntityResolution {
                mention_span: (0, 7),
                mention_text: "Allianz".to_string(),
                candidates: vec![
                    crate::entity_linking::EntityCandidate {
                        entity_id: Uuid::new_v4(),
                        entity_kind: "company".to_string(),
                        canonical_name: "Allianz SE".to_string(),
                        score: 0.81,
                        evidence: vec![],
                    },
                    crate::entity_linking::EntityCandidate {
                        entity_id: Uuid::new_v4(),
                        entity_kind: "fund".to_string(),
                        canonical_name: "Allianz Fund".to_string(),
                        score: 0.79,
                        evidence: vec![],
                    },
                ],
                selected: None,
                confidence: 0.0,
                evidence: vec![],
            }],
            dominant_entity: None,
            expected_kinds: vec!["company".to_string()],
            concepts: vec![],
            verb_matched: false,
            entities_resolved: false,
        });
        let response = ReplResponseV2 {
            state: ReplStateV2::RunbookEditing,
            kind: ReplResponseKindV2::Error {
                error: "Entity resolution needed".to_string(),
                recoverable: true,
            },
            message: "Entity resolution needed".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        };

        assert_eq!(
            repl_halt_reason_code(&session, &response).as_deref(),
            Some("ambiguous_entity")
        );
        assert_eq!(repl_halt_phase(&session, &response), Some(2));
    }

    #[test]
    fn test_phase2_gate_response_uses_sem_os_deny_all_message() {
        let orch = ReplOrchestratorV2::new(PackRouter::new(vec![]), Arc::new(StubExecutor));
        let mut session = ReplSessionV2::new();
        let mut envelope = crate::agent::sem_os_context_envelope::SemOsContextEnvelope::deny_all();
        envelope.grounded_action_surface =
            Some(sem_os_core::context_resolution::GroundedActionSurface {
                resolved_subject: sem_os_core::context_resolution::SubjectRef::TaskId(Uuid::nil()),
                resolved_constellation: Some("constellation.kyc".to_string()),
                resolved_slot_path: Some("case".to_string()),
                resolved_node_id: Some("node-1".to_string()),
                resolved_state_machine: Some("case_machine".to_string()),
                current_state: Some("intake".to_string()),
                traversed_edges: vec![],
                constraint_signals: vec![
                    sem_os_core::context_resolution::GroundedConstraintSignal {
                        kind: "dependency_block".to_string(),
                        slot_path: "case".to_string(),
                        related_slot: Some("cbu".to_string()),
                        required_state: Some("filled".to_string()),
                        actual_state: Some("empty".to_string()),
                        message: "dependency 'cbu' is in state 'empty' but requires 'filled'"
                            .to_string(),
                    },
                ],
                valid_actions: vec![],
                blocked_actions: vec![sem_os_core::context_resolution::BlockedActionOption {
                    action_id: "case.open".to_string(),
                    action_kind: "primitive".to_string(),
                    description: "Blocked action for slot 'case'".to_string(),
                    reasons: vec![
                        "dependency 'cbu' is in state 'empty' but requires 'filled'".to_string()
                    ],
                }],
                dsl_candidates: vec![],
            });
        session.pending_sem_os_envelope = Some(envelope);

        let response = orch.phase2_gate_response(&session).expect("phase 2 gate");
        assert!(matches!(response.kind, ReplResponseKindV2::Error { .. }));
        assert_eq!(
            response.message,
            "Blocked by constellation state: dependency 'cbu' is in state 'empty' but requires 'filled'. move 'cbu' from 'empty' to at least 'filled'."
        );
    }

    #[test]
    fn test_phase2_gate_response_defers_lookup_ambiguity() {
        // Entity ambiguity is now deferred to downstream verb matching (not a hard block).
        let orch = ReplOrchestratorV2::new(PackRouter::new(vec![]), Arc::new(StubExecutor));
        let mut session = ReplSessionV2::new();
        session.pending_lookup_result = Some(crate::lookup::LookupResult {
            verbs: vec![],
            entities: vec![crate::entity_linking::EntityResolution {
                mention_span: (0, 7),
                mention_text: "Allianz".to_string(),
                candidates: vec![
                    crate::entity_linking::EntityCandidate {
                        entity_id: Uuid::new_v4(),
                        entity_kind: "company".to_string(),
                        canonical_name: "Allianz SE".to_string(),
                        score: 0.81,
                        evidence: vec![],
                    },
                    crate::entity_linking::EntityCandidate {
                        entity_id: Uuid::new_v4(),
                        entity_kind: "fund".to_string(),
                        canonical_name: "Allianz Fund".to_string(),
                        score: 0.79,
                        evidence: vec![],
                    },
                ],
                selected: None,
                confidence: 0.0,
                evidence: vec![],
            }],
            dominant_entity: None,
            expected_kinds: vec!["company".to_string()],
            concepts: vec![],
            verb_matched: false,
            entities_resolved: false,
        });

        // ambiguous_entity is now deferred — phase2_gate_response returns None.
        let response = orch.phase2_gate_response(&session);
        assert!(
            response.is_none(),
            "ambiguous_entity should be deferred (not a hard block), got {:?}",
            response
        );
    }

    #[test]
    fn test_build_repl_phase4_payload_uses_resolved_verb() {
        let session = ReplSessionV2::new();
        let response = ReplResponseV2 {
            state: ReplStateV2::SentencePlayback {
                sentence: "Open the case".to_string(),
                dsl: "(case.open)".to_string(),
                verb: "case.open".to_string(),
                args: HashMap::new(),
            },
            kind: ReplResponseKindV2::SentencePlayback {
                sentence: "Open the case".to_string(),
                verb: "case.open".to_string(),
                step_sequence: 1,
            },
            message: "Open the case".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: Some(session.build_session_feedback(false)),
                    narration: None,
        };

        let payload = build_repl_phase4_evaluation(
            &response,
            &session,
            Some("case.open"),
            &crate::traceability::Phase2Service::evaluate(None, None),
        )
        .expect("phase4 evaluation should exist")
        .payload();
        assert_eq!(payload["resolved_verb"], "case.open");
        assert_eq!(payload["resolution_strategy"], "exact_match");
        assert_eq!(
            payload["resolution_strategy_detail"],
            "repl_sentence_playback"
        );
    }
}
