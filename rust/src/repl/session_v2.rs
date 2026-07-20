//! Session V2 — Owns a Runbook instead of a ledger.
//!
//! `ReplSessionV2` is the single container for a user's in-progress work.
//! It holds the state machine, runbook, staged pack, conversation history,
//! and the session-scoped workspace stack used by the v0.5 navigation model.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::decision_log::SessionDecisionLog;
use super::proposal_engine::ProposalSet;
use super::runbook::{ArgExtractionAudit, Runbook, SlotSource};
use super::types_v2::{
    ActionHint, AgentMode, ConversationMode, ReplStateV2, SessionEntityResolutionFeedback,
    SessionFeedback, SessionScope, SubjectKind, SubjectRef, VerbRef, WorkspaceFrame, WorkspaceHint,
    WorkspaceKind, WorkspaceStateView,
};
use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
use crate::journey::handoff::PackHandoff;
use crate::journey::pack::PackManifest;
use crate::lookup::LookupResult;

/// A v2 REPL session — the single source of truth for a user's work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReplSessionV2 {
    pub id: Uuid,
    pub state: ReplStateV2,
    /// Deprecated — retained as opaque JSON for deserialization of legacy sessions.
    #[serde(default)]
    pub client_context: Option<serde_json::Value>,
    /// Deprecated — retained as opaque JSON for deserialization of legacy sessions.
    #[serde(default)]
    pub journey_context: Option<serde_json::Value>,
    /// The active pack manifest (not serialized — reloaded from pack files).
    #[serde(skip)]
    pub staged_pack: Option<Arc<PackManifest>>,
    /// Hash of the staged pack manifest (for rehydration).
    #[serde(skip)]
    pub staged_pack_hash: Option<String>,
    pub runbook: Runbook,
    pub messages: Vec<ChatMessage>,
    #[serde(skip)]
    pub pending_arg_audit: Option<ArgExtractionAudit>,
    #[serde(skip)]
    pub pending_slot_provenance: Option<HashMap<String, SlotSource>>,
    #[serde(skip)]
    pub last_proposal_set: Option<ProposalSet>,
    #[serde(skip)]
    pub decision_log: SessionDecisionLog,
    #[serde(default)]
    pub last_trace_id: Option<Uuid>,
    #[serde(default)]
    pub pending_trace_id: Option<Uuid>,
    #[serde(skip)]
    pub pending_sem_os_envelope: Option<SemOsContextEnvelope>,
    #[serde(skip)]
    pub pending_lookup_result: Option<LookupResult>,
    /// Last entity-resolution projection produced by the Sage lookup service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_entity_resolution: Option<SessionEntityResolutionFeedback>,
    #[serde(skip)]
    pub pending_execution_rechecks: Vec<serde_json::Value>,
    #[serde(default)]
    pub active_workspace: Option<WorkspaceKind>,
    #[serde(default)]
    pub workspace_stack: Vec<WorkspaceFrame>,
    #[serde(default)]
    pub pending_verb: Option<VerbRef>,
    #[serde(default)]
    pub conversation_mode: ConversationMode,
    /// Current agent mode — determines permission gates for stack ops vs execution.
    #[serde(default)]
    pub agent_mode: AgentMode,
    /// Append-only trace log capturing every session mutation.
    #[serde(default, skip)]
    pub trace: Vec<super::session_trace::TraceEntry>,
    /// Monotonic trace sequence counter.
    #[serde(default)]
    pub trace_sequence: u64,
    /// Controls when hydrated snapshots are captured in trace entries.
    #[serde(default)]
    pub snapshot_policy: super::session_trace::SnapshotPolicy,
    /// Current runbook plan (multi-workspace orchestration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runbook_plan: Option<crate::runbook::plan_types::RunbookPlan>,
    /// Cursor within the runbook plan (which step to execute next).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runbook_plan_cursor: Option<usize>,
    /// Results of executed plan steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution_log: Vec<crate::runbook::plan_types::StepResult>,
    /// Symbol table for @reference resolution (`:as @myEntity` bindings).
    /// Session-scoped — persists across workspace switches.
    /// Synced from ExecutionContext.pending_session.bindings after verb execution.
    #[serde(default)]
    pub bindings: std::collections::HashMap<String, serde_json::Value>,

    /// CBU IDs in the current working set.
    /// Insertion-ordered Vec (not HashSet) because subject_id is set from
    /// the LAST entry (most recently created/loaded CBU).
    /// Populated during ScopeGate (group loads CBUs) and updated by verb
    /// execution (cbu.create appends). Dedup via contains() before push.
    #[serde(default)]
    pub cbu_ids: Vec<Uuid>,

    /// Session display name (e.g., "Allianz Global Investors").
    /// Set during ScopeGate from the client group name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    #[serde(default)]
    pub(super) next_runbook_version: u64,
    /// When true, `append_trace` is a no-op (used during replay).
    #[serde(default, skip)]
    pub(crate) tracing_suppressed: bool,
    #[serde(default)]
    pub is_test_session: bool,
}

impl ReplSessionV2 {
    /// Create a new session starting in `ScopeGate`.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::ReplStateV2;
    ///
    /// let session = ReplSessionV2::new();
    /// assert!(matches!(session.state, ReplStateV2::ScopeGate { .. }));
    /// ```
    pub(crate) fn new() -> Self {
        let id = Uuid::new_v4();
        let now = Utc::now();
        Self {
            id,
            state: ReplStateV2::ScopeGate {
                pending_input: None,
                candidates: None,
            },
            client_context: None,
            journey_context: None,
            staged_pack: None,
            staged_pack_hash: None,
            runbook: Runbook::new(id),
            messages: Vec::new(),
            pending_arg_audit: None,
            pending_slot_provenance: None,
            last_proposal_set: None,
            decision_log: SessionDecisionLog::new(id),
            last_trace_id: None,
            pending_trace_id: None,
            pending_sem_os_envelope: None,
            pending_lookup_result: None,
            last_entity_resolution: None,
            pending_execution_rechecks: Vec::new(),
            active_workspace: None,
            workspace_stack: Vec::new(),
            pending_verb: None,
            conversation_mode: ConversationMode::Inspect,
            agent_mode: AgentMode::default(),
            trace: Vec::new(),
            trace_sequence: 0,
            snapshot_policy: super::session_trace::SnapshotPolicy::default(),
            runbook_plan: None,
            runbook_plan_cursor: None,
            execution_log: Vec::new(),
            bindings: std::collections::HashMap::new(),
            cbu_ids: Vec::new(),
            name: None,
            created_at: now,
            last_active_at: now,
            next_runbook_version: 0,
            tracing_suppressed: false,
            is_test_session: false,
        }
    }

    /// Allocate the next monotonic runbook version.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    ///
    /// let mut session = ReplSessionV2::new();
    /// assert_eq!(session.allocate_runbook_version(), 1);
    /// assert_eq!(session.allocate_runbook_version(), 2);
    /// ```
    pub(crate) fn allocate_runbook_version(&mut self) -> u64 {
        self.runbook.next_version_counter += 1;
        self.next_runbook_version = self.runbook.next_version_counter;
        self.runbook.next_version_counter
    }

    /// Set the active session persona used for stack and execution gating.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::AgentMode;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.set_agent_mode(AgentMode::Repl);
    /// assert_eq!(session.agent_mode, AgentMode::Repl);
    /// ```
    pub(crate) fn set_agent_mode(&mut self, mode: AgentMode) {
        self.agent_mode = mode;
        self.last_active_at = Utc::now();
    }

    /// Enter Sage persona.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::AgentMode;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.enter_sage_mode();
    /// assert_eq!(session.agent_mode, AgentMode::Sage);
    /// ```
    pub(crate) fn enter_sage_mode(&mut self) {
        self.set_agent_mode(AgentMode::Sage);
    }

    /// Enter REPL persona.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::AgentMode;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.enter_repl_mode();
    /// assert_eq!(session.agent_mode, AgentMode::Repl);
    /// ```
    pub(crate) fn enter_repl_mode(&mut self) {
        self.set_agent_mode(AgentMode::Repl);
    }

    /// Add a message to the conversation history.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::{MessageRole, ReplSessionV2};
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.push_message(MessageRole::User, "hello".to_string());
    /// assert_eq!(session.messages.len(), 1);
    /// ```
    pub(crate) fn push_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Utc::now(),
        });
        self.last_active_at = Utc::now();
    }

    /// Transition to a new state.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::ReplStateV2;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.set_state(ReplStateV2::RunbookEditing);
    /// assert!(matches!(session.state, ReplStateV2::RunbookEditing));
    /// ```
    pub(crate) fn set_state(&mut self, new_state: ReplStateV2) {
        let from = format!("{:?}", self.state);
        let to = format!("{:?}", new_state);
        self.state = new_state;
        self.append_trace(super::session_trace::TraceOp::StateTransition { from, to });
        self.last_active_at = Utc::now();
    }

    /// Set the client scope (client group id).
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.set_client_scope(Uuid::nil());
    /// assert_eq!(session.runbook.client_group_id, Some(Uuid::nil()));
    /// ```
    pub(crate) fn set_client_scope(&mut self, client_group_id: Uuid) {
        self.runbook.client_group_id = Some(client_group_id);
        self.last_active_at = Utc::now();
    }

    /// Set the active workspace and replace the stack with a root frame.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::WorkspaceKind;
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.set_client_scope(Uuid::nil());
    /// session.set_workspace(WorkspaceKind::Deal);
    /// assert_eq!(session.workspace_stack.len(), 1);
    /// ```
    pub(crate) fn set_workspace(&mut self, workspace: WorkspaceKind) {
        self.active_workspace = Some(workspace.clone());
        self.workspace_stack.clear();
        if let Some(scope) = self.session_scope() {
            self.workspace_stack
                .push(WorkspaceFrame::new(workspace, scope));
        }
        self.last_active_at = Utc::now();
    }

    /// Alias for setting the root workspace frame.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::WorkspaceKind;
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.set_client_scope(Uuid::nil());
    /// session.set_workspace_root(WorkspaceKind::Kyc);
    /// assert_eq!(session.active_workspace, Some(WorkspaceKind::Kyc));
    /// ```
    pub(crate) fn set_workspace_root(&mut self, workspace: WorkspaceKind) {
        self.set_workspace(workspace);
    }

    /// Apply an entity lookup result to the persisted REPL/DAG session state.
    ///
    /// Returns `true` when the top-of-stack DAG subject changed and should be
    /// rehydrated.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let changed = session.apply_lookup_result(&lookup_result);
    /// if changed {
    ///     // Rehydrate the top-of-stack workspace before returning feedback.
    /// }
    /// ```
    pub(crate) fn apply_lookup_result(&mut self, result: &LookupResult) -> bool {
        self.last_entity_resolution = Some(result.into());
        self.clear_last_entity_bindings();

        let Some(entity) = result.dominant_entity.as_ref() else {
            self.last_active_at = Utc::now();
            return false;
        };
        if entity.entity_id.is_nil() {
            self.last_active_at = Utc::now();
            return false;
        }

        self.bindings.insert(
            "last_entity_id".to_string(),
            serde_json::json!(entity.entity_id.to_string()),
        );
        self.bindings.insert(
            "last_entity_kind".to_string(),
            serde_json::json!(entity.entity_kind.as_str()),
        );
        self.bindings.insert(
            "last_entity_name".to_string(),
            serde_json::json!(entity.canonical_name.as_str()),
        );
        let normalized_kind = binding_key_fragment(&entity.entity_kind);
        if !normalized_kind.is_empty() {
            self.bindings.insert(
                format!("last_{normalized_kind}_id"),
                serde_json::json!(entity.entity_id.to_string()),
            );
        }

        let subject_kind = subject_kind_for_entity_kind(&entity.entity_kind);
        if matches!(subject_kind, Some(SubjectKind::Cbu)) {
            self.cbu_ids.retain(|id| *id != entity.entity_id);
            self.cbu_ids.push(entity.entity_id);
        }

        let mut subject_changed = false;
        if let Some(kind) = subject_kind {
            if let Some(tos) = self.workspace_stack.last_mut() {
                let supported = tos.workspace.registry_entry().subject_kinds.contains(&kind);
                if supported
                    && (tos.subject_kind.as_ref() != Some(&kind)
                        || tos.subject_id != Some(entity.entity_id))
                {
                    tos.subject_kind = Some(kind);
                    tos.subject_id = Some(entity.entity_id);
                    tos.hydrated_state = None;
                    tos.stale = true;
                    subject_changed = true;
                }
            }
        }

        self.last_active_at = Utc::now();
        subject_changed
    }

    fn clear_last_entity_bindings(&mut self) {
        let previous_kind_key = self
            .bindings
            .get("last_entity_kind")
            .and_then(|value| value.as_str())
            .map(binding_key_fragment);

        self.bindings.remove("last_entity_id");
        self.bindings.remove("last_entity_kind");
        self.bindings.remove("last_entity_name");
        if let Some(kind_key) = previous_kind_key {
            if !kind_key.is_empty() {
                self.bindings.remove(&format!("last_{kind_key}_id"));
            }
        }
    }

    /// Return the current session scope if the client group is known.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.set_client_scope(Uuid::nil());
    /// assert!(session.session_scope().is_some());
    /// ```
    pub(crate) fn session_scope(&self) -> Option<SessionScope> {
        self.runbook
            .client_group_id
            .map(|client_group_id| SessionScope {
                client_group_id,
                client_group_name: None,
            })
    }

    /// Return the top-of-stack frame, if any.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// assert!(ReplSessionV2::new().tos_frame().is_none());
    /// ```
    pub(crate) fn tos_frame(&self) -> Option<&WorkspaceFrame> {
        self.workspace_stack.last()
    }

    /// Return the mutable top-of-stack frame, if any.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// assert!(ReplSessionV2::new().tos_frame_mut().is_none());
    /// ```
    pub(crate) fn tos_frame_mut(&mut self) -> Option<&mut WorkspaceFrame> {
        self.workspace_stack.last_mut()
    }

    /// Increment the write counter on the top-of-stack frame.
    ///
    /// Called after each verb execution to track whether a pop should mark
    /// the restored frame as stale.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::{SessionScope, WorkspaceFrame, WorkspaceKind};
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// let scope = SessionScope { client_group_id: Uuid::nil(), client_group_name: None };
    /// session.push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Deal, scope)).unwrap();
    /// session.increment_tos_writes();
    /// assert_eq!(session.tos_frame().unwrap().writes_since_push, 1);
    /// ```
    pub(crate) fn increment_tos_writes(&mut self) {
        if let Some(tos) = self.workspace_stack.last_mut() {
            tos.writes_since_push += 1;
        }
    }

    /// Build a lightweight snapshot of the current workspace stack for trace entries.
    pub(crate) fn stack_snapshot(&self) -> Vec<super::session_trace::FrameRef> {
        self.workspace_stack
            .iter()
            .map(|f| super::session_trace::FrameRef {
                workspace: f.workspace.clone(),
                constellation_map: f.constellation_map.clone(),
                subject_id: f.subject_id,
                stale: f.stale,
            })
            .collect()
    }

    fn should_capture_snapshot(&self, op: &super::session_trace::TraceOp, sequence: u64) -> bool {
        match self.snapshot_policy {
            super::session_trace::SnapshotPolicy::Never => false,
            super::session_trace::SnapshotPolicy::EveryN(n) => {
                n > 0 && sequence.is_multiple_of(u64::from(n))
            }
            super::session_trace::SnapshotPolicy::OnStackOp => matches!(
                op,
                super::session_trace::TraceOp::StackPush { .. }
                    | super::session_trace::TraceOp::StackPop { .. }
                    | super::session_trace::TraceOp::StackCommit
            ),
            super::session_trace::SnapshotPolicy::OnExecution => {
                matches!(op, super::session_trace::TraceOp::VerbExecuted { .. })
            }
        }
    }

    fn current_trace_snapshot(&self) -> Option<serde_json::Value> {
        self.tos_frame()
            .and_then(|tos| tos.hydrated_state.as_ref())
            .and_then(|state| serde_json::to_value(state).ok())
    }

    /// Append a trace entry for the given operation.
    pub(crate) fn append_trace(&mut self, op: super::session_trace::TraceOp) {
        if self.tracing_suppressed {
            return;
        }
        self.trace_sequence += 1;
        let snapshot = self.stack_snapshot();
        let mut entry = super::session_trace::TraceEntry::new(
            self.id,
            self.trace_sequence,
            self.agent_mode,
            op,
            snapshot,
        );
        let feedback = self.build_session_feedback(false);
        if let Ok(fb_json) = serde_json::to_value(&feedback) {
            entry = entry.with_session_feedback(fb_json);
        }
        if self.should_capture_snapshot(&entry.op, self.trace_sequence) {
            if let Some(snapshot) = self.current_trace_snapshot() {
                entry = entry.with_snapshot(snapshot);
            }
        }
        self.trace.push(entry);
    }

    /// Append an enriched trace entry with verb resolution and execution result.
    pub(crate) fn append_trace_enriched(
        &mut self,
        op: super::session_trace::TraceOp,
        verb_fqn: Option<String>,
        execution_result: Option<serde_json::Value>,
    ) {
        self.trace_sequence += 1;
        let snapshot = self.stack_snapshot();
        let mut entry = super::session_trace::TraceEntry::new(
            self.id,
            self.trace_sequence,
            self.agent_mode,
            op,
            snapshot,
        );
        if let Some(v) = verb_fqn {
            entry = entry.with_verb_resolved(v);
        }
        if let Some(r) = execution_result {
            entry = entry.with_execution_result(r);
        }
        // Attach lightweight session feedback (without hydrated constellation)
        let feedback = self.build_session_feedback(false);
        if let Ok(fb_json) = serde_json::to_value(&feedback) {
            entry = entry.with_session_feedback(fb_json);
        }
        if self.should_capture_snapshot(&entry.op, self.trace_sequence) {
            if let Some(snapshot) = self.current_trace_snapshot() {
                entry = entry.with_snapshot(snapshot);
            }
        }
        self.trace.push(entry);
    }

    /// Push a new frame onto the workspace stack.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::{SessionScope, WorkspaceFrame, WorkspaceKind};
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// let ok = session.push_workspace_frame(WorkspaceFrame::new(
    ///     WorkspaceKind::Kyc,
    ///     SessionScope { client_group_id: Uuid::nil(), client_group_name: None },
    /// ));
    /// assert!(ok.is_ok());
    /// ```
    pub(crate) fn push_workspace_frame(&mut self, frame: WorkspaceFrame) -> Result<()> {
        anyhow::ensure!(
            self.workspace_stack.len() < 3,
            "workspace stack depth exceeds max depth 3"
        );
        let ws = frame.workspace.clone();
        self.active_workspace = Some(ws.clone());
        self.workspace_stack.push(frame);
        self.append_trace(super::session_trace::TraceOp::StackPush { workspace: ws });
        self.last_active_at = Utc::now();
        Ok(())
    }

    /// Replace the hydrated state on the top-of-stack frame.
    ///
    /// # Examples
    /// ```rust,ignore
    /// session.hydrate_tos(view);
    /// ```
    pub(crate) fn hydrate_tos(&mut self, state_view: WorkspaceStateView) {
        if let Some(tos) = self.workspace_stack.last_mut() {
            // Build constellation verb index from hydrated slots (Tier -0.5 in verb search)
            tos.constellation_verb_index = state_view.hydrated_constellation.as_ref().map(|c| {
                std::sync::Arc::new(
                    crate::agent::constellation_verb_index::ConstellationVerbIndex::build(&c.slots),
                )
            });
            tos.hydrated_state = Some(state_view);
            tos.stale = false;
            self.active_workspace = Some(tos.workspace.clone());
        }
        self.last_active_at = Utc::now();
    }

    // ── Viewport state accessors (observation frame, NOT resource truth) ──

    /// Current Observatory view level from TOS.
    pub(crate) fn tos_view_level(&self) -> ob_poc_types::galaxy::ViewLevel {
        self.workspace_stack
            .last()
            .map(|f| f.view_level)
            .unwrap_or(ob_poc_types::galaxy::ViewLevel::Universe)
    }

    /// Set the Observatory view level on TOS.
    pub(crate) fn set_tos_view_level(&mut self, level: ob_poc_types::galaxy::ViewLevel) {
        if let Some(tos) = self.workspace_stack.last_mut() {
            tos.view_level = level;
        }
    }

    /// Current focus slot path from TOS.
    pub(crate) fn tos_focus_slot_path(&self) -> Option<&str> {
        self.workspace_stack
            .last()
            .and_then(|f| f.focus_slot_path.as_deref())
    }

    /// Set the focus slot path on TOS.
    pub(crate) fn set_tos_focus_slot(&mut self, path: Option<String>) {
        if let Some(tos) = self.workspace_stack.last_mut() {
            tos.focus_slot_path = path;
        }
    }

    /// Push a viewport snapshot to TOS navigation history (for back/forward).
    /// Truncates any forward history beyond the current cursor.
    pub(crate) fn push_nav_snapshot(&mut self) {
        if let Some(tos) = self.workspace_stack.last_mut() {
            let snapshot = super::types_v2::ViewportSnapshot {
                view_level: tos.view_level,
                focus_slot_path: tos.focus_slot_path.clone(),
                timestamp: chrono::Utc::now(),
            };
            // Truncate forward history
            if tos.nav_cursor + 1 < tos.nav_snapshots.len() {
                tos.nav_snapshots.truncate(tos.nav_cursor + 1);
            }
            tos.nav_snapshots.push(snapshot);
            tos.nav_cursor = tos.nav_snapshots.len() - 1;
        }
    }

    /// Navigate back in viewport history. Returns true if cursor moved.
    pub(crate) fn nav_back(&mut self) -> bool {
        if let Some(tos) = self.workspace_stack.last_mut() {
            if tos.nav_cursor > 0 {
                tos.nav_cursor -= 1;
                let snap = &tos.nav_snapshots[tos.nav_cursor];
                tos.view_level = snap.view_level;
                tos.focus_slot_path = snap.focus_slot_path.clone();
                return true;
            }
        }
        false
    }

    /// Navigate forward in viewport history. Returns true if cursor moved.
    pub(crate) fn nav_forward(&mut self) -> bool {
        if let Some(tos) = self.workspace_stack.last_mut() {
            if tos.nav_cursor + 1 < tos.nav_snapshots.len() {
                tos.nav_cursor += 1;
                let snap = &tos.nav_snapshots[tos.nav_cursor];
                tos.view_level = snap.view_level;
                tos.focus_slot_path = snap.focus_slot_path.clone();
                return true;
            }
        }
        false
    }

    /// Pop the top-of-stack frame and mark the restored frame stale.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::{SessionScope, WorkspaceFrame, WorkspaceKind};
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// let scope = SessionScope { client_group_id: Uuid::nil(), client_group_name: None };
    /// session.push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Deal, scope.clone())).unwrap();
    /// session.push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Kyc, scope)).unwrap();
    /// assert!(session.pop_workspace_frame().is_some());
    /// ```
    pub(crate) fn pop_workspace_frame(&mut self) -> Option<WorkspaceFrame> {
        if self.workspace_stack.len() <= 1 {
            return None;
        }
        let popped = self.workspace_stack.pop();
        if let Some(ref p) = popped {
            self.append_trace(super::session_trace::TraceOp::StackPop {
                workspace: p.workspace.clone(),
            });
        }
        if let Some(tos) = self.workspace_stack.last_mut() {
            // Only mark stale if the popped frame had writes — a pure peek doesn't
            // invalidate the frame underneath.
            tos.stale = popped.as_ref().is_some_and(|p| p.writes_since_push > 0);
            self.active_workspace = Some(tos.workspace.clone());
        }
        self.last_active_at = Utc::now();
        popped
    }

    /// Collapse the stack to the current top-of-stack frame.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// use ob_poc::repl::types_v2::{SessionScope, WorkspaceFrame, WorkspaceKind};
    /// use uuid::Uuid;
    ///
    /// let mut session = ReplSessionV2::new();
    /// let scope = SessionScope { client_group_id: Uuid::nil(), client_group_name: None };
    /// session.push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Deal, scope.clone())).unwrap();
    /// session.push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Kyc, scope)).unwrap();
    /// session.commit_workspace_stack();
    /// assert_eq!(session.workspace_stack.len(), 1);
    /// ```
    pub(crate) fn commit_workspace_stack(&mut self) {
        if let Some(tos) = self.workspace_stack.last().cloned() {
            self.workspace_stack.clear();
            self.active_workspace = Some(tos.workspace.clone());
            self.workspace_stack.push(WorkspaceFrame {
                stale: false,
                is_peek: false,
                ..tos
            });
            self.append_trace(super::session_trace::TraceOp::StackCommit);
        }
        self.last_active_at = Utc::now();
    }

    /// Build session feedback from the current top-of-stack state.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// let feedback = ReplSessionV2::new().build_session_feedback(false);
    /// assert_eq!(feedback.stack_depth, 0);
    /// ```
    pub(crate) fn build_session_feedback(&self, tos_is_peek_override: bool) -> SessionFeedback {
        let fallback_workspace = self.active_workspace.clone().unwrap_or(WorkspaceKind::Cbu);
        let _fallback_registry = fallback_workspace.registry_entry();
        let (hydrated, stale_warning) = if let Some(tos) = self.workspace_stack.last() {
            (
                tos.hydrated_state
                    .clone()
                    .unwrap_or_else(|| WorkspaceStateView {
                        workspace: tos.workspace.clone(),
                        constellation_family: tos.constellation_family.clone(),
                        constellation_map: tos.constellation_map.clone(),
                        subject_ref: tos
                            .subject_id
                            .zip(tos.subject_kind.clone())
                            .map(|(id, kind)| SubjectRef { kind, id }),
                        hydrated_constellation: None,
                        scoped_verb_surface: Vec::new(),
                        progress_summary: None,
                        available_actions: Vec::new(),
                    }),
                tos.stale,
            )
        } else {
            (
                WorkspaceStateView {
                    workspace: fallback_workspace.clone(),
                    constellation_family: "universe".to_string(),
                    constellation_map: "universe.root".to_string(),
                    subject_ref: None,
                    hydrated_constellation: None,
                    scoped_verb_surface: universe_root_verbs(),
                    progress_summary: None,
                    available_actions: universe_root_actions(),
                },
                false,
            )
        };
        let previous_workspace = if self.workspace_stack.len() > 1 {
            self.workspace_stack
                .get(self.workspace_stack.len().saturating_sub(2))
                .map(|frame| frame.workspace.clone())
        } else {
            None
        };
        let available_workspaces = workspace_hints();
        let stale_shared_facts = self
            .workspace_stack
            .last()
            .map(|tos| tos.stale_shared_facts.clone())
            .unwrap_or_default();
        let has_stale_shared_facts = !stale_shared_facts.is_empty();
        SessionFeedback {
            stack_depth: self.workspace_stack.len(),
            tos: hydrated.clone(),
            tos_is_peek: tos_is_peek_override
                || self
                    .workspace_stack
                    .last()
                    .map(|frame| frame.is_peek)
                    .unwrap_or(false),
            previous_workspace,
            stale_warning: stale_warning || has_stale_shared_facts,
            stale_shared_facts,
            scoped_verb_surface: hydrated.scoped_verb_surface.clone(),
            available_workspaces,
            pending_verb: self.pending_verb.clone(),
            conversation_mode: self.conversation_mode,
            entity_resolution: self.last_entity_resolution.clone(),
        }
    }

    /// Activate a journey pack.
    ///
    /// # Examples
    /// ```rust,ignore
    /// session.activate_pack(pack, hash, None);
    /// ```
    pub(crate) fn activate_pack(
        &mut self,
        pack: Arc<PackManifest>,
        manifest_hash: String,
        _handoff: Option<PackHandoff>,
    ) {
        self.runbook.pack_id = Some(pack.id.clone());
        self.runbook.pack_version = Some(pack.version.clone());
        self.runbook.pack_manifest_hash = Some(manifest_hash.clone());
        self.staged_pack = Some(pack);
        self.staged_pack_hash = Some(manifest_hash);
        self.last_active_at = Utc::now();
    }

    /// Record an answer to a pack question.
    ///
    /// # Examples
    /// ```rust,ignore
    /// session.record_answer("field".into(), serde_json::json!("value"));
    /// ```
    pub(crate) fn record_answer(&mut self, _field: String, _value: serde_json::Value) {
        self.last_active_at = Utc::now();
    }

    /// Clear the staged pack.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    ///
    /// let mut session = ReplSessionV2::new();
    /// session.clear_staged_pack();
    /// assert!(session.staged_pack.is_none());
    /// ```
    pub(crate) fn clear_staged_pack(&mut self) {
        self.staged_pack = None;
        self.staged_pack_hash = None;
        self.last_active_at = Utc::now();
    }

    /// Build a `ContextStack` from the current session state.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let stack = session.build_context_stack(None);
    /// ```
    pub(crate) fn build_context_stack(
        &self,
        pack_router: Option<&crate::journey::router::PackRouter>,
    ) -> super::context_stack::ContextStack {
        let turn = self.messages.len() as u32;
        let mut stack = super::context_stack::ContextStack::from_runbook_with_router(
            &self.runbook,
            self.staged_pack.clone(),
            turn,
            pack_router,
        );
        stack.is_test_session = self.is_test_session;
        stack
    }

    /// Whether a journey pack is currently active.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    ///
    /// assert!(!ReplSessionV2::new().has_active_pack());
    /// ```
    pub(crate) fn has_active_pack(&self) -> bool {
        self.active_pack_id().is_some()
    }

    /// Get the active pack ID.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    ///
    /// assert!(ReplSessionV2::new().active_pack_id().is_none());
    /// ```
    pub(crate) fn active_pack_id(&self) -> Option<String> {
        self.staged_pack
            .as_ref()
            .map(|p| p.id.clone())
            .or_else(|| self.runbook.pack_id.clone())
    }

    /// Rehydrate transient fields after loading from database.
    ///
    /// # Examples
    /// ```rust,ignore
    /// session.rehydrate(&pack_router);
    /// ```
    pub(crate) fn rehydrate(&mut self, pack_router: &crate::journey::router::PackRouter) {
        let hash = self
            .staged_pack_hash
            .as_deref()
            .or(self.runbook.pack_manifest_hash.as_deref());
        if let Some(hash) = hash {
            if let Some((manifest, _)) = pack_router.get_pack_by_hash(hash) {
                self.staged_pack = Some(manifest.clone());
                self.staged_pack_hash = Some(hash.to_string());
            }
        }
        self.runbook.rebuild_invocation_index();
    }

    /// Build the canonical stack/context value shared with BPMN-lite.
    pub(crate) fn build_session_stack_state(&self) -> ob_poc_types::session_stack::SessionStackState {
        use ob_poc_types::session_stack::{
            ConstraintCascadeState, SessionScopeState, SessionStackFrame, SessionStackState,
        };

        let scope = self.session_scope().map(|scope| SessionScopeState {
            client_group_id: scope.client_group_id,
            client_group_name: scope.client_group_name,
        });

        let workspace_stack = self
            .workspace_stack
            .iter()
            .map(|frame| SessionStackFrame {
                workspace: workspace_kind_to_shared(&frame.workspace),
                constellation_family: frame.constellation_family.clone(),
                constellation_map: frame.constellation_map.clone(),
                subject_kind: frame.subject_kind.as_ref().map(subject_kind_to_shared),
                subject_id: frame.subject_id,
                pushed_at: frame.pushed_at,
                stale: frame.stale,
                writes_since_push: frame.writes_since_push,
                is_peek: frame.is_peek,
                constraints: ConstraintCascadeState {
                    structure_id: frame.current_structure_id,
                    structure_name: frame.current_structure_name.clone(),
                    case_id: frame.current_case_id,
                    mandate_id: frame.current_mandate_id,
                    deal_id: frame.deal_id,
                    deal_name: frame.deal_name.clone(),
                },
                view_level: frame.view_level,
                focus_slot_path: frame.focus_slot_path.clone(),
            })
            .collect();

        SessionStackState {
            session_id: self.id,
            scope,
            active_workspace: self.active_workspace.as_ref().map(workspace_kind_to_shared),
            workspace_stack,
            trace_sequence: self.trace_sequence,
        }
    }
}

impl Default for ReplSessionV2 {
    fn default() -> Self {
        Self::new()
    }
}

fn universe_root_verbs() -> Vec<VerbRef> {
    vec![
        VerbRef {
            verb_fqn: "session.start".into(),
            display_name: "New Session".into(),
        },
        VerbRef {
            verb_fqn: "session.resume".into(),
            display_name: "Resume Session".into(),
        },
        VerbRef {
            verb_fqn: "session.load-cbu".into(),
            display_name: "Load Client Group".into(),
        },
        VerbRef {
            verb_fqn: "session.load-galaxy".into(),
            display_name: "Load Galaxy".into(),
        },
        VerbRef {
            verb_fqn: "session.load-jurisdiction".into(),
            display_name: "Load Jurisdiction".into(),
        },
        VerbRef {
            verb_fqn: "client-group.search".into(),
            display_name: "Search Client Groups".into(),
        },
        VerbRef {
            verb_fqn: "gleif.search".into(),
            display_name: "Search LEI Registry".into(),
        },
        VerbRef {
            verb_fqn: "session.info".into(),
            display_name: "Session Info".into(),
        },
    ]
}

fn universe_root_actions() -> Vec<ActionHint> {
    vec![
        ActionHint {
            label: "Select a client group".into(),
            verb_fqn: Some("session.load-cbu".into()),
            action_type: "scope".into(),
        },
        ActionHint {
            label: "Search for a client group".into(),
            verb_fqn: Some("client-group.search".into()),
            action_type: "scope".into(),
        },
        ActionHint {
            label: "SemOS infrastructure maintenance".into(),
            verb_fqn: None,
            action_type: "workspace".into(),
        },
    ]
}

fn workspace_hints() -> Vec<WorkspaceHint> {
    WorkspaceKind::all()
        .into_iter()
        .map(|workspace| {
            let registry = workspace.registry_entry();
            WorkspaceHint {
                workspace,
                label: registry.display_name.to_string(),
                default_constellation_family: registry.default_constellation_family.to_string(),
                default_constellation_map: registry.default_constellation_map.to_string(),
            }
        })
        .collect()
}

fn workspace_kind_to_shared(
    workspace: &WorkspaceKind,
) -> ob_poc_types::session_stack::SessionWorkspaceKind {
    use ob_poc_types::session_stack::SessionWorkspaceKind;

    match workspace {
        WorkspaceKind::ProductMaintenance => SessionWorkspaceKind::ProductMaintenance,
        WorkspaceKind::Catalogue => SessionWorkspaceKind::Catalogue,
        WorkspaceKind::Deal => SessionWorkspaceKind::Deal,
        WorkspaceKind::Cbu => SessionWorkspaceKind::Cbu,
        WorkspaceKind::Kyc => SessionWorkspaceKind::Kyc,
        WorkspaceKind::InstrumentMatrix => SessionWorkspaceKind::InstrumentMatrix,
        WorkspaceKind::OnBoarding => SessionWorkspaceKind::OnBoarding,
        WorkspaceKind::SemOsMaintenance => SessionWorkspaceKind::SemOsMaintenance,
        WorkspaceKind::LifecycleResources => SessionWorkspaceKind::LifecycleResources,
        WorkspaceKind::Bpmn => SessionWorkspaceKind::Bpmn,
    }
}

fn subject_kind_to_shared(
    subject_kind: &SubjectKind,
) -> ob_poc_types::session_stack::SessionSubjectKind {
    use ob_poc_types::session_stack::SessionSubjectKind;

    match subject_kind {
        SubjectKind::ClientGroup => SessionSubjectKind::ClientGroup,
        SubjectKind::Cbu => SessionSubjectKind::Cbu,
        SubjectKind::Deal => SessionSubjectKind::Deal,
        SubjectKind::Case => SessionSubjectKind::Case,
        SubjectKind::Handoff => SessionSubjectKind::Handoff,
        SubjectKind::Matrix => SessionSubjectKind::Matrix,
        SubjectKind::Product => SessionSubjectKind::Product,
        SubjectKind::Service => SessionSubjectKind::Service,
        SubjectKind::Resource => SessionSubjectKind::Resource,
        SubjectKind::Attribute => SessionSubjectKind::Attribute,
    }
}

fn subject_kind_for_entity_kind(entity_kind: &str) -> Option<SubjectKind> {
    match binding_key_fragment(entity_kind).as_str() {
        "client_group" | "clientgroup" => Some(SubjectKind::ClientGroup),
        "cbu" | "fund" | "client_business_unit" | "client_business_unit_entity" => {
            Some(SubjectKind::Cbu)
        }
        "deal" => Some(SubjectKind::Deal),
        "case" | "kyc_case" => Some(SubjectKind::Case),
        "handoff" => Some(SubjectKind::Handoff),
        "matrix" | "instrument_matrix" => Some(SubjectKind::Matrix),
        "product" => Some(SubjectKind::Product),
        "service" => Some(SubjectKind::Service),
        "resource" => Some(SubjectKind::Resource),
        "attribute" => Some(SubjectKind::Attribute),
        "" | "company" | "entity" | "document" | "legal_entity" | "jurisdiction" => None,
        _ => None,
    }
}

fn binding_key_fragment(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// A single message in the session conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Who sent the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MessageRole {
    User,
    Assistant,
    System,
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use ob_poc_types::galaxy::ViewLevel;

    fn sample_pack() -> Arc<PackManifest> {
        let yaml = r#"
id: onboarding-request
name: Onboarding Request
version: "1.0"
description: Hand off a contracted deal into onboarding for an existing CBU
invocation_phrases:
  - "request onboarding for this deal"
required_context:
  - client_group_id
"#;
        let (manifest, _) = crate::journey::pack::load_pack_from_bytes(yaml.as_bytes()).unwrap();
        Arc::new(manifest)
    }

    fn lookup_with_dominant(
        entity_id: Uuid,
        entity_kind: &str,
        canonical_name: &str,
    ) -> crate::lookup::LookupResult {
        crate::lookup::LookupResult {
            entity_snapshot: crate::lookup::EntitySnapshotMetadata {
                hash: "snapshot-hash".to_string(),
                version: 1,
                entity_count: 7,
            },
            verbs: Vec::new(),
            entities: vec![ob_poc_entity_linking::EntityResolution {
                mention_span: (0, 7),
                mention_text: canonical_name.to_string(),
                candidates: vec![ob_poc_entity_linking::EntityCandidate {
                    entity_id,
                    entity_kind: entity_kind.to_string(),
                    canonical_name: canonical_name.to_string(),
                    score: 0.92,
                    evidence: Vec::new(),
                }],
                selected: Some(entity_id),
                confidence: 0.92,
                evidence: Vec::new(),
            }],
            dominant_entity: Some(crate::lookup::service::DominantEntity {
                entity_id,
                canonical_name: canonical_name.to_string(),
                entity_kind: entity_kind.to_string(),
                confidence: 0.92,
                mention_span: (0, 7),
            }),
            expected_kinds: vec![entity_kind.to_string()],
            concepts: Vec::new(),
            verb_matched: false,
            entities_resolved: true,
        }
    }

    fn lookup_without_dominant() -> crate::lookup::LookupResult {
        crate::lookup::LookupResult {
            entity_snapshot: crate::lookup::EntitySnapshotMetadata {
                hash: "snapshot-hash".to_string(),
                version: 1,
                entity_count: 7,
            },
            verbs: Vec::new(),
            entities: Vec::new(),
            dominant_entity: None,
            expected_kinds: Vec::new(),
            concepts: Vec::new(),
            verb_matched: false,
            entities_resolved: false,
        }
    }

    #[test]
    fn test_new_session() {
        let session = ReplSessionV2::new();
        assert!(matches!(session.state, ReplStateV2::ScopeGate { .. }));
        assert!(session.workspace_stack.is_empty());
    }

    #[test]
    fn test_set_workspace_root() {
        let mut session = ReplSessionV2::new();
        session.set_client_scope(Uuid::nil());
        session.set_workspace_root(WorkspaceKind::Deal);
        assert_eq!(session.workspace_stack.len(), 1);
        assert_eq!(session.active_workspace, Some(WorkspaceKind::Deal));
    }

    #[test]
    fn test_push_pop_stack() {
        let mut session = ReplSessionV2::new();
        let scope = SessionScope {
            client_group_id: Uuid::nil(),
            client_group_name: None,
        };
        session
            .push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Deal, scope.clone()))
            .unwrap();
        let mut kyc_frame = WorkspaceFrame::new(WorkspaceKind::Kyc, scope);
        // Simulate a write so pop marks the underlying frame stale
        kyc_frame.writes_since_push = 1;
        session.push_workspace_frame(kyc_frame).unwrap();
        assert!(session.pop_workspace_frame().is_some());
        assert_eq!(session.workspace_stack.len(), 1);
        // Frame underneath is stale because popped frame had writes
        assert!(session.workspace_stack[0].stale);
    }

    #[test]
    fn test_push_pop_peek_not_stale() {
        let mut session = ReplSessionV2::new();
        let scope = SessionScope {
            client_group_id: Uuid::nil(),
            client_group_name: None,
        };
        session
            .push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Deal, scope.clone()))
            .unwrap();
        // Peek frame with no writes — pop should NOT mark underlying stale
        session
            .push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Kyc, scope))
            .unwrap();
        assert!(session.pop_workspace_frame().is_some());
        assert_eq!(session.workspace_stack.len(), 1);
        assert!(!session.workspace_stack[0].stale);
    }

    #[test]
    fn test_activate_pack() {
        let mut session = ReplSessionV2::new();
        let pack = sample_pack();
        session.activate_pack(pack.clone(), "hash".to_string(), None);
        assert_eq!(session.active_pack_id(), Some(pack.id.clone()));
    }

    #[test]
    fn test_session_feedback_exposes_entity_resolution() {
        let entity_id = Uuid::new_v4();
        let lookup = crate::lookup::LookupResult {
            entity_snapshot: crate::lookup::EntitySnapshotMetadata {
                hash: "snapshot-hash".to_string(),
                version: 1,
                entity_count: 7,
            },
            verbs: Vec::new(),
            entities: vec![ob_poc_entity_linking::EntityResolution {
                mention_span: (0, 7),
                mention_text: "Allianz".to_string(),
                candidates: vec![ob_poc_entity_linking::EntityCandidate {
                    entity_id,
                    entity_kind: "company".to_string(),
                    canonical_name: "Allianz SE".to_string(),
                    score: 0.92,
                    evidence: Vec::new(),
                }],
                selected: Some(entity_id),
                confidence: 0.92,
                evidence: Vec::new(),
            }],
            dominant_entity: Some(crate::lookup::service::DominantEntity {
                entity_id,
                canonical_name: "Allianz SE".to_string(),
                entity_kind: "company".to_string(),
                confidence: 0.92,
                mention_span: (0, 7),
            }),
            expected_kinds: vec!["company".to_string()],
            concepts: Vec::new(),
            verb_matched: false,
            entities_resolved: true,
        };

        let mut session = ReplSessionV2::new();
        session.last_entity_resolution = Some((&lookup).into());

        let feedback = session.build_session_feedback(false);
        let entity_resolution = feedback
            .entity_resolution
            .expect("entity resolution feedback");

        assert_eq!(entity_resolution.snapshot_hash, "snapshot-hash");
        assert_eq!(
            entity_resolution
                .dominant_entity
                .as_ref()
                .map(|entity| entity.entity_id),
            Some(entity_id)
        );
        assert_eq!(entity_resolution.mentions.len(), 1);
        assert_eq!(entity_resolution.mentions[0].selected_id, Some(entity_id));
    }

    #[test]
    fn test_apply_lookup_result_updates_repl_and_dag_subject() {
        let entity_id = Uuid::new_v4();
        let lookup = lookup_with_dominant(entity_id, "cbu", "Allianz Fund");

        let mut session = ReplSessionV2::new();
        session.set_client_scope(Uuid::new_v4());
        session.set_workspace_root(WorkspaceKind::Cbu);

        assert!(session.apply_lookup_result(&lookup));
        assert_eq!(session.cbu_ids, vec![entity_id]);
        assert_eq!(
            session
                .bindings
                .get("last_entity_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            Some(entity_id.to_string())
        );
        assert_eq!(
            session
                .tos_frame()
                .and_then(|frame| frame.subject_kind.clone()),
            Some(SubjectKind::Cbu)
        );
        assert_eq!(
            session.tos_frame().and_then(|frame| frame.subject_id),
            Some(entity_id)
        );
        assert_eq!(
            session.build_session_stack_state().workspace_stack[0].subject_id,
            Some(entity_id)
        );
    }

    #[test]
    fn test_apply_lookup_result_clears_last_entity_when_no_entity_selected() {
        let entity_id = Uuid::new_v4();
        let lookup = lookup_with_dominant(entity_id, "cbu", "Allianz Fund");

        let mut session = ReplSessionV2::new();
        session.set_client_scope(Uuid::new_v4());
        session.set_workspace_root(WorkspaceKind::Cbu);
        assert!(session.apply_lookup_result(&lookup));

        assert!(!session.apply_lookup_result(&lookup_without_dominant()));
        assert!(!session.bindings.contains_key("last_entity_id"));
        assert!(!session.bindings.contains_key("last_entity_kind"));
        assert!(!session.bindings.contains_key("last_entity_name"));
        assert!(!session.bindings.contains_key("last_cbu_id"));
        assert_eq!(
            session.tos_frame().and_then(|frame| frame.subject_id),
            Some(entity_id)
        );
        assert!(session
            .last_entity_resolution
            .as_ref()
            .is_some_and(|resolution| resolution.dominant_entity.is_none()));
    }

    #[test]
    fn test_apply_lookup_result_ignores_nil_entity_id() {
        let lookup = lookup_with_dominant(Uuid::nil(), "cbu", "Empty Fund");

        let mut session = ReplSessionV2::new();
        session.set_client_scope(Uuid::new_v4());
        session.set_workspace_root(WorkspaceKind::Cbu);

        assert!(!session.apply_lookup_result(&lookup));
        assert!(session.cbu_ids.is_empty());
        assert!(!session.bindings.contains_key("last_entity_id"));
        assert!(session
            .tos_frame()
            .is_some_and(|frame| frame.subject_id.is_none()));
    }

    #[test]
    fn test_apply_lookup_result_reselection_updates_cbu_recency_without_duplicates() {
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let first_lookup = lookup_with_dominant(first_id, "cbu", "First Fund");
        let second_lookup = lookup_with_dominant(second_id, "cbu", "Second Fund");

        let mut session = ReplSessionV2::new();
        session.set_client_scope(Uuid::new_v4());
        session.set_workspace_root(WorkspaceKind::Cbu);

        assert!(session.apply_lookup_result(&first_lookup));
        assert!(session.apply_lookup_result(&second_lookup));
        assert!(session.apply_lookup_result(&first_lookup));

        assert_eq!(session.cbu_ids, vec![second_id, first_id]);
        assert_eq!(
            session.tos_frame().and_then(|frame| frame.subject_id),
            Some(first_id)
        );
        assert_eq!(
            session
                .bindings
                .get("last_cbu_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            Some(first_id.to_string())
        );
    }

    #[test]
    fn test_session_stack_sync_tracks_scope_and_workspace_frames() {
        let mut session = ReplSessionV2::new();
        let client_group_id = Uuid::new_v4();
        let subject_id = Uuid::new_v4();

        session.set_client_scope(client_group_id);
        session.set_workspace_root(WorkspaceKind::Cbu);
        if let Some(tos) = session.workspace_stack.last_mut() {
            tos.subject_kind = Some(SubjectKind::Cbu);
            tos.subject_id = Some(subject_id);
            tos.constellation_family = "operating".to_string();
            tos.constellation_map = "struct.lux.ucits.sicav".to_string();
        }
        session.set_tos_view_level(ViewLevel::Surface);
        session.set_tos_focus_slot(Some("overview.summary".to_string()));

        let stack = session.build_session_stack_state();
        assert_eq!(stack.session_id, session.id);
        assert_eq!(
            stack.scope.as_ref().map(|scope| scope.client_group_id),
            Some(client_group_id)
        );
        assert_eq!(
            stack.active_workspace,
            Some(ob_poc_types::session_stack::SessionWorkspaceKind::Cbu)
        );
        assert_eq!(stack.workspace_stack.len(), 1);

        let frame = &stack.workspace_stack[0];
        assert_eq!(
            frame.workspace,
            ob_poc_types::session_stack::SessionWorkspaceKind::Cbu
        );
        assert_eq!(
            frame.subject_kind,
            Some(ob_poc_types::session_stack::SessionSubjectKind::Cbu)
        );
        assert_eq!(frame.subject_id, Some(subject_id));
        assert_eq!(frame.constellation_family, "operating");
        assert_eq!(frame.constellation_map, "struct.lux.ucits.sicav");
        assert_eq!(frame.view_level, ViewLevel::Surface);
        assert_eq!(frame.focus_slot_path.as_deref(), Some("overview.summary"));
    }
}
