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
    AgentMode, ConversationMode, ReplStateV2, SessionFeedback, SessionScope, SubjectRef, VerbRef,
    WorkspaceFrame, WorkspaceHint, WorkspaceKind, WorkspaceStateView,
};
use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
use crate::journey::handoff::PackHandoff;
use crate::journey::pack::PackManifest;
use crate::lookup::LookupResult;

/// A v2 REPL session — the single source of truth for a user's work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplSessionV2 {
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
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    #[serde(default)]
    pub(super) next_runbook_version: u64,
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
    pub fn new() -> Self {
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
            created_at: now,
            last_active_at: now,
            next_runbook_version: 0,
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
    pub fn allocate_runbook_version(&mut self) -> u64 {
        self.runbook.next_version_counter += 1;
        self.next_runbook_version = self.runbook.next_version_counter;
        self.runbook.next_version_counter
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
    pub fn push_message(&mut self, role: MessageRole, content: String) {
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
    pub fn set_state(&mut self, new_state: ReplStateV2) {
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
    pub fn set_client_scope(&mut self, client_group_id: Uuid) {
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
    pub fn set_workspace(&mut self, workspace: WorkspaceKind) {
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
    pub fn set_workspace_root(&mut self, workspace: WorkspaceKind) {
        self.set_workspace(workspace);
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
    pub fn session_scope(&self) -> Option<SessionScope> {
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
    pub fn tos_frame(&self) -> Option<&WorkspaceFrame> {
        self.workspace_stack.last()
    }

    /// Return the mutable top-of-stack frame, if any.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    /// assert!(ReplSessionV2::new().tos_frame_mut().is_none());
    /// ```
    pub fn tos_frame_mut(&mut self) -> Option<&mut WorkspaceFrame> {
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
    pub fn increment_tos_writes(&mut self) {
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

    /// Append a trace entry for the given operation.
    pub fn append_trace(&mut self, op: super::session_trace::TraceOp) {
        self.trace_sequence += 1;
        let snapshot = self.stack_snapshot();
        self.trace.push(super::session_trace::TraceEntry::new(
            self.id,
            self.trace_sequence,
            self.agent_mode,
            op,
            snapshot,
        ));
    }

    /// Append an enriched trace entry with verb resolution and execution result.
    pub fn append_trace_enriched(
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
    pub fn push_workspace_frame(&mut self, frame: WorkspaceFrame) -> Result<()> {
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
    pub fn hydrate_tos(&mut self, state_view: WorkspaceStateView) {
        if let Some(tos) = self.workspace_stack.last_mut() {
            tos.hydrated_state = Some(state_view);
            tos.stale = false;
            self.active_workspace = Some(tos.workspace.clone());
        }
        self.last_active_at = Utc::now();
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
    pub fn pop_workspace_frame(&mut self) -> Option<WorkspaceFrame> {
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
            tos.stale = popped.as_ref().map_or(false, |p| p.writes_since_push > 0);
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
    pub fn commit_workspace_stack(&mut self) {
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
    pub fn build_session_feedback(&self, tos_is_peek_override: bool) -> SessionFeedback {
        let fallback_workspace = self.active_workspace.clone().unwrap_or(WorkspaceKind::Cbu);
        let fallback_registry = fallback_workspace.registry_entry();
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
                    constellation_family: fallback_registry
                        .default_constellation_family
                        .to_string(),
                    constellation_map: fallback_registry.default_constellation_map.to_string(),
                    subject_ref: None,
                    hydrated_constellation: None,
                    scoped_verb_surface: Vec::new(),
                    progress_summary: None,
                    available_actions: Vec::new(),
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
        SessionFeedback {
            stack_depth: self.workspace_stack.len(),
            tos: hydrated.clone(),
            tos_is_peek: tos_is_peek_override || self.workspace_stack.len() > 1,
            previous_workspace,
            stale_warning,
            scoped_verb_surface: hydrated.scoped_verb_surface.clone(),
            available_workspaces,
            pending_verb: self.pending_verb.clone(),
            conversation_mode: self.conversation_mode,
        }
    }

    /// Activate a journey pack.
    ///
    /// # Examples
    /// ```rust,ignore
    /// session.activate_pack(pack, hash, None);
    /// ```
    pub fn activate_pack(
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
    pub fn record_answer(&mut self, _field: String, _value: serde_json::Value) {
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
    pub fn clear_staged_pack(&mut self) {
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
    pub fn build_context_stack(
        &self,
        pack_router: Option<&crate::journey::router::PackRouter>,
    ) -> super::context_stack::ContextStack {
        let turn = self.messages.len() as u32;
        super::context_stack::ContextStack::from_runbook_with_router(
            &self.runbook,
            self.staged_pack.clone(),
            turn,
            pack_router,
        )
    }

    /// Whether a journey pack is currently active.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    ///
    /// assert!(!ReplSessionV2::new().has_active_pack());
    /// ```
    #[allow(deprecated)]
    pub fn has_active_pack(&self) -> bool {
        self.staged_pack.is_some() || self.journey_context.is_some()
    }

    /// Get the active pack ID.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::session_v2::ReplSessionV2;
    ///
    /// assert!(ReplSessionV2::new().active_pack_id().is_none());
    /// ```
    pub fn active_pack_id(&self) -> Option<String> {
        self.staged_pack.as_ref().map(|p| p.id.clone())
    }

    /// Rehydrate transient fields after loading from database.
    ///
    /// # Examples
    /// ```rust,ignore
    /// session.rehydrate(&pack_router);
    /// ```
    pub fn rehydrate(&mut self, pack_router: &crate::journey::router::PackRouter) {
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
}

impl Default for ReplSessionV2 {
    fn default() -> Self {
        Self::new()
    }
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

/// A single message in the session conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Who sent the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

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
        session
            .push_workspace_frame(WorkspaceFrame::new(WorkspaceKind::Kyc, scope))
            .unwrap();
        assert!(session.pop_workspace_frame().is_some());
        assert_eq!(session.workspace_stack.len(), 1);
        assert!(session.workspace_stack[0].stale);
    }

    #[test]
    fn test_activate_pack() {
        let mut session = ReplSessionV2::new();
        let pack = sample_pack();
        session.activate_pack(pack.clone(), "hash".to_string(), None);
        assert_eq!(session.active_pack_id(), Some(pack.id.clone()));
    }
}
