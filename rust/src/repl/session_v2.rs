//! Session V2 — Owns a Runbook instead of a ledger
//!
//! `ReplSessionV2` is the single container for a user's in-progress work.
//! It holds the state machine, client context, journey context, runbook,
//! and conversation history.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::proposal_engine::ProposalSet;
use super::runbook::{ArgExtractionAudit, Runbook};
use super::types_v2::ReplStateV2;
use crate::journey::handoff::PackHandoff;
use crate::journey::pack::PackManifest;

// ---------------------------------------------------------------------------
// ReplSessionV2
// ---------------------------------------------------------------------------

/// A v2 REPL session — the single source of truth for a user's work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplSessionV2 {
    pub id: Uuid,
    pub state: ReplStateV2,
    pub client_context: Option<ClientContext>,
    pub journey_context: Option<JourneyContext>,
    pub runbook: Runbook,
    pub messages: Vec<ChatMessage>,
    /// Transient: audit from the most recent IntentMatcher result.
    /// Set when a verb is matched, consumed when the entry is confirmed.
    #[serde(skip)]
    pub pending_arg_audit: Option<ArgExtractionAudit>,
    /// Transient: last proposal set from the proposal engine.
    /// Set when proposals are generated, consumed when user selects one.
    #[serde(skip)]
    pub last_proposal_set: Option<ProposalSet>,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
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
            },
            client_context: None,
            journey_context: None,
            runbook: Runbook::new(id),
            messages: Vec::new(),
            pending_arg_audit: None,
            last_proposal_set: None,
            created_at: now,
            last_active_at: now,
        }
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

    /// Set the client context (scope).
    pub fn set_client_context(&mut self, ctx: ClientContext) {
        self.runbook.client_group_id = Some(ctx.client_group_id);
        self.client_context = Some(ctx);
        self.last_active_at = Utc::now();
    }

    /// Activate a journey pack.
    pub fn activate_pack(
        &mut self,
        pack: Arc<PackManifest>,
        manifest_hash: String,
        handoff: Option<PackHandoff>,
    ) {
        self.runbook.pack_id = Some(pack.id.clone());
        self.runbook.pack_version = Some(pack.version.clone());
        self.runbook.pack_manifest_hash = Some(manifest_hash.clone());

        self.journey_context = Some(JourneyContext {
            pack,
            pack_manifest_hash: manifest_hash,
            answers: HashMap::new(),
            template_id: None,
            progress: PackProgress::default(),
            handoff_source: handoff,
        });

        self.last_active_at = Utc::now();
    }

    /// Record an answer to a pack question.
    pub fn record_answer(&mut self, field: String, value: serde_json::Value) {
        if let Some(ref mut ctx) = self.journey_context {
            ctx.answers.insert(field, value);
        }
        self.last_active_at = Utc::now();
    }

    /// Rehydrate transient fields after loading from database.
    ///
    /// - Restores the Arc<PackManifest> from the pack router using the stored hash.
    /// - Rebuilds the invocation index on the runbook.
    pub fn rehydrate(&mut self, pack_router: &crate::journey::router::PackRouter) {
        // Restore the pack Arc from the router using stored manifest hash.
        if let Some(ref mut jctx) = self.journey_context {
            if let Some((manifest, _)) = pack_router.get_pack_by_hash(&jctx.pack_manifest_hash) {
                jctx.pack = manifest.clone();
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
// ClientContext
// ---------------------------------------------------------------------------

/// The scope the user is operating in — which client group and defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientContext {
    pub client_group_id: Uuid,
    pub client_group_name: String,
    pub default_cbu: Option<Uuid>,
    pub default_book: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// JourneyContext
// ---------------------------------------------------------------------------

/// Context for an active journey pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyContext {
    /// The active pack manifest (not serialized — reloaded from pack files).
    #[serde(skip, default = "default_pack")]
    pub pack: Arc<PackManifest>,

    /// Canonical hash of the pack manifest file.
    pub pack_manifest_hash: String,

    /// Answers to pack questions collected so far.
    pub answers: HashMap<String, serde_json::Value>,

    /// Selected template (if any).
    pub template_id: Option<String>,

    /// Progress tracking.
    pub progress: PackProgress,

    /// If this session was started via handoff from another pack.
    pub handoff_source: Option<PackHandoff>,
}

/// Default pack for deserialization — placeholder until reloaded from files.
fn default_pack() -> Arc<PackManifest> {
    Arc::new(PackManifest {
        id: String::new(),
        name: String::new(),
        version: String::new(),
        description: String::new(),
        invocation_phrases: Vec::new(),
        required_context: Vec::new(),
        optional_context: Vec::new(),
        allowed_verbs: Vec::new(),
        forbidden_verbs: Vec::new(),
        risk_policy: Default::default(),
        required_questions: Vec::new(),
        optional_questions: Vec::new(),
        stop_rules: Vec::new(),
        templates: Vec::new(),
        pack_summary_template: None,
        section_layout: Vec::new(),
        definition_of_done: Vec::new(),
        progress_signals: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// PackProgress
// ---------------------------------------------------------------------------

/// Progress within an active pack.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackProgress {
    pub questions_answered: usize,
    pub questions_remaining: usize,
    pub steps_proposed: usize,
    pub steps_confirmed: usize,
    pub steps_executed: usize,
    pub signals_emitted: Vec<String>,
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
    fn test_set_client_context() {
        let mut session = ReplSessionV2::new();
        let group_id = Uuid::new_v4();

        session.set_client_context(ClientContext {
            client_group_id: group_id,
            client_group_name: "Allianz".to_string(),
            default_cbu: None,
            default_book: None,
        });

        assert!(session.client_context.is_some());
        assert_eq!(session.runbook.client_group_id, Some(group_id));
    }

    #[test]
    fn test_activate_pack() {
        let mut session = ReplSessionV2::new();
        let pack = sample_pack();
        let hash = "abc123def456".to_string();

        session.activate_pack(pack.clone(), hash.clone(), None);

        assert!(session.journey_context.is_some());
        let ctx = session.journey_context.as_ref().unwrap();
        assert_eq!(ctx.pack_manifest_hash, hash);
        assert_eq!(ctx.pack.id, "onboarding-request");
        assert!(ctx.answers.is_empty());

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

        session.record_answer("products".to_string(), serde_json::json!(["IRS", "EQUITY"]));
        session.record_answer("jurisdiction".to_string(), serde_json::json!("LU"));

        let ctx = session.journey_context.as_ref().unwrap();
        assert_eq!(ctx.answers.len(), 2);
        assert_eq!(ctx.answers["jurisdiction"], serde_json::json!("LU"));
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

        session.activate_pack(pack, "hash".to_string(), Some(handoff));

        let ctx = session.journey_context.as_ref().unwrap();
        assert!(ctx.handoff_source.is_some());
        assert_eq!(
            ctx.handoff_source.as_ref().unwrap().target_pack_id,
            "onboarding-request"
        );
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

        // Before rehydration, pack is the default placeholder.
        assert!(loaded.journey_context.as_ref().unwrap().pack.id.is_empty());

        // After rehydration, pack is restored.
        loaded.rehydrate(&router);
        assert_eq!(
            loaded.journey_context.as_ref().unwrap().pack.id,
            "onboarding-request"
        );
    }
}
