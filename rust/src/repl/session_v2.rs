//! Session V2 — Owns a Runbook instead of a ledger
//!
//! `ReplSessionV2` is the single container for a user's in-progress work.
//! It holds the state machine, runbook, staged pack, and conversation history.
//! Session state is derived from the runbook via `ContextStack::from_runbook()`.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::decision_log::SessionDecisionLog;
use super::proposal_engine::ProposalSet;
use super::runbook::{ArgExtractionAudit, Runbook, SlotSource};
use super::types_v2::ReplStateV2;
use crate::journey::handoff::PackHandoff;
use crate::journey::pack::PackManifest;

// ---------------------------------------------------------------------------
// ReplSessionV2
// ---------------------------------------------------------------------------

/// A v2 REPL session — the single source of truth for a user's work.
///
/// Session state is derived from the runbook via `ContextStack::from_runbook()`.
/// The `staged_pack` field holds the active pack manifest for the current turn;
/// everything else (scope, answers, progress) is a left fold over executed
/// runbook entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplSessionV2 {
    pub id: Uuid,
    pub state: ReplStateV2,
    /// Deprecated — retained as opaque JSON for deserialization of legacy sessions.
    /// No code should read or write this field.
    #[serde(default)]
    pub client_context: Option<serde_json::Value>,
    /// Deprecated — retained as opaque JSON for deserialization of legacy sessions.
    /// No code should read or write this field.
    #[serde(default)]
    pub journey_context: Option<serde_json::Value>,
    /// The active pack manifest (not serialized — reloaded from pack files).
    /// This is the staged pack that ContextStack reads. It replaces the
    /// `journey_context.pack` field for all read-side access.
    #[serde(skip)]
    pub staged_pack: Option<Arc<PackManifest>>,
    /// Hash of the staged pack manifest (for rehydration).
    #[serde(skip)]
    pub staged_pack_hash: Option<String>,
    pub runbook: Runbook,
    pub messages: Vec<ChatMessage>,
    /// Transient: audit from the most recent IntentMatcher result.
    /// Set when a verb is matched, consumed when the entry is confirmed.
    #[serde(skip)]
    pub pending_arg_audit: Option<ArgExtractionAudit>,
    /// Transient: slot provenance from deterministic extraction (Phase F).
    /// Set when deterministic extraction succeeds, consumed when the entry is confirmed.
    #[serde(skip)]
    pub pending_slot_provenance: Option<HashMap<String, SlotSource>>,
    /// Transient: last proposal set from the proposal engine.
    /// Set when proposals are generated, consumed when user selects one.
    #[serde(skip)]
    pub last_proposal_set: Option<ProposalSet>,
    /// Phase G: Accumulated decision logs for replay and tuning.
    #[serde(skip)]
    pub decision_log: SessionDecisionLog,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    /// Monotonic counter for runbook version allocation.
    /// Each call to `allocate_runbook_version()` returns a unique, ascending value.
    /// Initialized at 0; first allocation returns 1.
    #[serde(default)]
    pub(super) next_runbook_version: u64,
}

impl ReplSessionV2 {
    /// Create a new session starting in `ScopeGate`.
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
            created_at: now,
            last_active_at: now,
            next_runbook_version: 0,
        }
    }

    /// Allocate the next monotonic runbook version.
    ///
    /// Returns a unique, ascending version number starting from 1.
    /// Guarantees uniqueness within a session — no two compilations share
    /// the same version, even if entries are deleted or re-ordered.
    ///
    /// The counter lives on `runbook.next_version_counter` so it is persisted
    /// when the runbook JSONB is saved. The legacy `self.next_runbook_version`
    /// field is kept in sync for backward compatibility.
    pub fn allocate_runbook_version(&mut self) -> u64 {
        self.runbook.next_version_counter += 1;
        // Keep legacy field in sync (transient — not persisted).
        self.next_runbook_version = self.runbook.next_version_counter;
        self.runbook.next_version_counter
    }

    /// Add a message to the conversation history.
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
    pub fn set_state(&mut self, new_state: ReplStateV2) {
        self.state = new_state;
        self.last_active_at = Utc::now();
    }

    /// Set the client scope (client group id).
    ///
    /// Writes to `runbook.client_group_id` — the canonical source of truth.
    pub fn set_client_scope(&mut self, client_group_id: Uuid) {
        self.runbook.client_group_id = Some(client_group_id);
        self.last_active_at = Utc::now();
    }

    /// Activate a journey pack.
    ///
    /// Sets `staged_pack` (for ContextStack reads) and runbook pack fields.
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
    /// The canonical source is `ContextStack::accumulated_answers` derived
    /// from the runbook fold via `derive_answers()`. Answers are stored as
    /// runbook entries, not on the session struct.
    pub fn record_answer(&mut self, _field: String, _value: serde_json::Value) {
        // Answers are recorded via runbook entries — this is now a no-op.
        // Callers should add answer entries to the runbook directly.
        self.last_active_at = Utc::now();
    }

    /// Clear the staged pack (e.g., when switching journeys).
    pub fn clear_staged_pack(&mut self) {
        self.staged_pack = None;
        self.staged_pack_hash = None;
        self.last_active_at = Utc::now();
    }

    /// Build a ContextStack from the current session state.
    ///
    /// This is the primary way to access derived session state.
    /// Call this once per turn and read from the result.
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
    #[allow(deprecated)] // Fallback reads deprecated journey_context for migration compat
    pub fn has_active_pack(&self) -> bool {
        self.staged_pack.is_some() || self.journey_context.is_some()
    }

    /// Get the active pack ID (from staged_pack).
    pub fn active_pack_id(&self) -> Option<String> {
        self.staged_pack.as_ref().map(|p| p.id.clone())
    }

    /// Rehydrate transient fields after loading from database.
    ///
    /// - Restores the Arc<PackManifest> from the pack router using the stored hash.
    /// - Rebuilds the invocation index on the runbook.
    pub fn rehydrate(&mut self, pack_router: &crate::journey::router::PackRouter) {
        // Restore the pack Arc from the router using stored manifest hash.
        // Prefer staged_pack_hash (transient), fall back to runbook.pack_manifest_hash (persisted).
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
        // Rebuild invocation index (lost during serialization due to #[serde(skip)]).
        self.runbook.rebuild_invocation_index();
    }
}

impl Default for ReplSessionV2 {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ChatMessage
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(deprecated)] // Tests exercise deprecated fields for migration coverage
mod tests {
    use super::*;

    fn sample_pack() -> Arc<PackManifest> {
        let yaml = r#"
id: onboarding-request
name: Onboarding Request
version: "1.0"
description: Onboard a new client structure
invocation_phrases:
  - "onboard a client"
  - "set up onboarding"
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
        assert!(session.client_context.is_none());
        assert!(session.journey_context.is_none());
        assert!(session.messages.is_empty());
        assert!(session.runbook.entries.is_empty());
    }

    #[test]
    fn test_push_message() {
        let mut session = ReplSessionV2::new();
        session.push_message(MessageRole::User, "Hello".to_string());
        session.push_message(MessageRole::Assistant, "Hi there!".to_string());

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_set_client_scope() {
        let mut session = ReplSessionV2::new();
        let group_id = Uuid::new_v4();

        session.set_client_scope(group_id);

        assert_eq!(session.runbook.client_group_id, Some(group_id));
    }

    #[test]
    fn test_activate_pack() {
        let mut session = ReplSessionV2::new();
        let pack = sample_pack();
        let hash = "abc123def456".to_string();

        session.activate_pack(pack.clone(), hash.clone(), None);

        // staged_pack and runbook fields are the canonical locations
        assert!(session.staged_pack.is_some());
        assert_eq!(
            session.staged_pack.as_ref().unwrap().id,
            "onboarding-request"
        );
        assert_eq!(session.staged_pack_hash.as_deref(), Some("abc123def456"));

        assert_eq!(
            session.runbook.pack_id,
            Some("onboarding-request".to_string())
        );
        assert_eq!(session.runbook.pack_manifest_hash, Some(hash));
    }

    #[test]
    fn test_record_answer() {
        let mut session = ReplSessionV2::new();
        let pack = sample_pack();
        session.activate_pack(pack, "hash".to_string(), None);

        // record_answer is now a no-op (answers stored via runbook entries).
        // Verify it doesn't panic and updates last_active_at.
        let before = session.last_active_at;
        session.record_answer("products".to_string(), serde_json::json!(["IRS", "EQUITY"]));
        session.record_answer("jurisdiction".to_string(), serde_json::json!("LU"));
        assert!(session.last_active_at >= before);
    }

    #[test]
    fn test_state_transitions() {
        let mut session = ReplSessionV2::new();

        // ScopeGate → JourneySelection
        session.set_state(ReplStateV2::JourneySelection { candidates: None });
        assert!(matches!(
            session.state,
            ReplStateV2::JourneySelection { .. }
        ));

        // JourneySelection → InPack
        session.set_state(ReplStateV2::InPack {
            pack_id: "test".to_string(),
            required_slots_remaining: vec!["products".to_string()],
            last_proposal_id: None,
        });
        assert!(matches!(session.state, ReplStateV2::InPack { .. }));

        // InPack → SentencePlayback
        session.set_state(ReplStateV2::SentencePlayback {
            sentence: "Create CBU".to_string(),
            verb: "cbu.create".to_string(),
            dsl: "(cbu.create)".to_string(),
            args: HashMap::new(),
        });
        assert!(matches!(
            session.state,
            ReplStateV2::SentencePlayback { .. }
        ));
    }

    #[test]
    fn test_session_with_handoff() {
        let mut session = ReplSessionV2::new();
        let pack = sample_pack();
        let handoff = PackHandoff {
            source_runbook_id: Uuid::new_v4(),
            target_pack_id: "onboarding-request".to_string(),
            forwarded_context: HashMap::from([(
                "client_group_id".to_string(),
                Uuid::new_v4().to_string(),
            )]),
            forwarded_outcomes: vec![Uuid::new_v4()],
        };

        // handoff is now ignored by activate_pack (context is derived from runbook)
        // but we verify it doesn't panic and pack is properly activated
        session.activate_pack(pack, "hash".to_string(), Some(handoff));

        assert!(session.staged_pack.is_some());
        assert_eq!(
            session.staged_pack.as_ref().unwrap().id,
            "onboarding-request"
        );
        assert_eq!(
            session.runbook.pack_id,
            Some("onboarding-request".to_string())
        );
    }

    #[test]
    fn test_monotonic_version_allocator() {
        let mut session = ReplSessionV2::new();

        // First allocation starts at 1.
        let v1 = session.allocate_runbook_version();
        assert_eq!(v1, 1);

        // Subsequent allocations are strictly ascending.
        let v2 = session.allocate_runbook_version();
        let v3 = session.allocate_runbook_version();
        assert_eq!(v2, 2);
        assert_eq!(v3, 3);
        assert!(v3 > v2);
        assert!(v2 > v1);
    }

    #[test]
    fn test_version_allocator_survives_serde_roundtrip() {
        let mut session = ReplSessionV2::new();

        // Allocate some versions.
        let _ = session.allocate_runbook_version(); // 1
        let _ = session.allocate_runbook_version(); // 2

        // Serialize and deserialize.
        let json = serde_json::to_string(&session).unwrap();
        let mut restored: ReplSessionV2 = serde_json::from_str(&json).unwrap();

        // Next allocation must be > 2 (preserved via serde).
        let v3 = restored.allocate_runbook_version();
        assert_eq!(v3, 3, "Version should continue from serialized counter");
    }

    #[test]
    fn test_rehydrate_restores_pack() {
        use crate::journey::pack::load_pack_from_bytes;
        use crate::journey::router::PackRouter;

        let mut session = ReplSessionV2::new();
        let (manifest, hash) = load_pack_from_bytes(
            r#"
id: onboarding-request
name: Onboarding Request
version: "1.0"
description: Onboard a new client structure
invocation_phrases:
  - "onboard a client"
required_context:
  - client_group_id
"#
            .as_bytes(),
        )
        .unwrap();

        let pack = Arc::new(manifest);
        let router = PackRouter::new(vec![(pack.clone(), hash.clone())]);

        session.activate_pack(pack, hash, None);

        // Simulate serialization roundtrip (pack Arc is lost).
        let json = serde_json::to_string(&session).unwrap();
        let mut loaded: ReplSessionV2 = serde_json::from_str(&json).unwrap();

        // Before rehydration, staged_pack is lost (not serialized).
        assert!(loaded.staged_pack.is_none());

        // After rehydration, staged_pack is restored from router.
        loaded.rehydrate(&router);
        assert_eq!(
            loaded.staged_pack.as_ref().unwrap().id,
            "onboarding-request"
        );
    }
}
