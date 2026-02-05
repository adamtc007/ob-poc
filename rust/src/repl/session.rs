//! REPL Session Model
//!
//! Clean session model with command ledger as the single source of truth.
//! All derived state is computed from ledger entries, enabling full replay.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::types::{
    EntryStatus, LedgerEntry, LedgerExecutionResult, ReplState, ScopeContext, UserInput,
};

// ============================================================================
// ReplSession
// ============================================================================

/// Clean session model - single source of truth
///
/// The session follows these principles:
/// 1. **Ledger is authority**: All state derives from ledger entries
/// 2. **State machine is explicit**: Current state is always clear
/// 3. **Derived state is recomputable**: Call `recompute_derived()` to rebuild
/// 4. **Context is user-provided**: client_group, scope, etc. set by user actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplSession {
    // ========================================================================
    // Identity
    // ========================================================================
    /// Unique session ID
    pub id: Uuid,

    /// User ID (if authenticated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,

    /// When this session was created
    pub created_at: DateTime<Utc>,

    /// When this session was last active
    pub last_active_at: DateTime<Utc>,

    // ========================================================================
    // State Machine
    // ========================================================================
    /// Current REPL state (Idle, Clarifying, DslReady, Executing)
    pub state: ReplState,

    // ========================================================================
    // Command Ledger (THE authority)
    // ========================================================================
    /// Immutable log of all user interactions and results
    pub ledger: Vec<LedgerEntry>,

    // ========================================================================
    // Derived State (computed from ledger)
    // ========================================================================
    /// State derived from replaying ledger
    #[serde(default)]
    pub derived: DerivedState,

    // ========================================================================
    // User-Provided Context (not derived from ledger)
    // ========================================================================
    /// Current client group ID (set by user selection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_group_id: Option<Uuid>,

    /// Client group name for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_group_name: Option<String>,

    /// Current scope (CBU set being worked on)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeContext>,

    /// Dominant entity from previous interactions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dominant_entity_id: Option<Uuid>,

    /// Domain hint for verb search (e.g., "kyc", "trading")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_hint: Option<String>,
}

impl Default for ReplSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplSession {
    /// Create a new empty session
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id: None,
            created_at: now,
            last_active_at: now,
            state: ReplState::Idle,
            ledger: Vec::new(),
            derived: DerivedState::default(),
            client_group_id: None,
            client_group_name: None,
            scope: None,
            dominant_entity_id: None,
            domain_hint: None,
        }
    }

    /// Create a session with a specific ID
    pub fn with_id(id: Uuid) -> Self {
        let mut session = Self::new();
        session.id = id;
        session
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Set client group
    pub fn with_client_group(mut self, group_id: Uuid, group_name: String) -> Self {
        self.client_group_id = Some(group_id);
        self.client_group_name = Some(group_name);
        self
    }

    /// Get the current entry count
    pub fn entry_count(&self) -> usize {
        self.ledger.len()
    }

    /// Get the last entry (if any)
    pub fn last_entry(&self) -> Option<&LedgerEntry> {
        self.ledger.last()
    }

    /// Get the last entry mutably
    pub fn last_entry_mut(&mut self) -> Option<&mut LedgerEntry> {
        self.ledger.last_mut()
    }

    /// Add a new ledger entry
    pub fn add_entry(&mut self, entry: LedgerEntry) {
        self.ledger.push(entry);
        self.last_active_at = Utc::now();
    }

    /// Update the last entry's status
    pub fn update_last_status(&mut self, status: EntryStatus) {
        if let Some(entry) = self.ledger.last_mut() {
            entry.status = status;
        }
    }

    /// Update the last entry with execution result
    pub fn update_last_execution(&mut self, result: LedgerExecutionResult) {
        if let Some(entry) = self.ledger.last_mut() {
            entry.execution_result = Some(result);
            entry.status = EntryStatus::Executed;
        }
        // Recompute derived state after execution
        self.recompute_derived();
    }

    /// Mark the last entry as failed
    pub fn mark_last_failed(&mut self, error: impl Into<String>) {
        if let Some(entry) = self.ledger.last_mut() {
            entry.status = EntryStatus::Failed {
                error: error.into(),
            };
        }
    }

    /// Supersede all non-terminal entries (when user sends new input)
    pub fn supersede_pending(&mut self) {
        for entry in &mut self.ledger {
            if !entry.status.is_terminal() {
                entry.status = EntryStatus::Superseded;
            }
        }
    }

    /// Recompute derived state from ledger
    ///
    /// This is the key operation that makes sessions replayable.
    /// All state in `derived` can be reconstructed by replaying the ledger.
    pub fn recompute_derived(&mut self) {
        let mut cbu_ids = Vec::new();
        let mut bindings = HashMap::new();
        let mut messages = Vec::new();

        for entry in &self.ledger {
            // Extract user message
            if let UserInput::Message { content } = &entry.input {
                messages.push(ChatMessage {
                    role: MessageRole::User,
                    content: content.clone(),
                    timestamp: entry.timestamp,
                });
            }

            // Extract execution results
            if let Some(result) = &entry.execution_result {
                // Merge CBU IDs (dedupe)
                for cbu_id in &result.affected_cbu_ids {
                    if !cbu_ids.contains(cbu_id) {
                        cbu_ids.push(*cbu_id);
                    }
                }

                // Merge bindings (newer overwrites older)
                for (name, entity_id) in &result.bindings {
                    bindings.insert(name.clone(), *entity_id);
                }

                // Add agent message
                messages.push(ChatMessage {
                    role: MessageRole::Agent,
                    content: result.message.clone(),
                    timestamp: entry.timestamp,
                });
            }
        }

        self.derived = DerivedState {
            cbu_ids,
            bindings,
            messages,
            executed_count: self
                .ledger
                .iter()
                .filter(|e| matches!(e.status, EntryStatus::Executed))
                .count(),
            failed_count: self
                .ledger
                .iter()
                .filter(|e| matches!(e.status, EntryStatus::Failed { .. }))
                .count(),
        };
    }

    /// Get messages for display (alternating user/agent)
    pub fn messages(&self) -> &[ChatMessage] {
        &self.derived.messages
    }

    /// Get current CBU IDs in scope
    pub fn cbu_ids(&self) -> &[Uuid] {
        &self.derived.cbu_ids
    }

    /// Get current bindings
    pub fn bindings(&self) -> &HashMap<String, Uuid> {
        &self.derived.bindings
    }

    /// Check if session needs client group selection
    pub fn needs_client_group(&self) -> bool {
        self.client_group_id.is_none()
    }

    /// Transition to idle state
    pub fn transition_to_idle(&mut self) {
        self.state = ReplState::Idle;
    }

    /// Transition to clarifying state
    pub fn transition_to_clarifying(&mut self, state: super::types::ClarifyingState) {
        self.state = ReplState::Clarifying(state);
    }

    /// Transition to DSL ready state
    pub fn transition_to_dsl_ready(&mut self, dsl: String, verb: String, can_auto_execute: bool) {
        self.state = ReplState::DslReady {
            dsl,
            verb,
            can_auto_execute,
        };
    }

    /// Transition to executing state
    pub fn transition_to_executing(&mut self, dsl: String) {
        self.state = ReplState::Executing {
            dsl,
            started_at: Utc::now(),
        };
    }
}

// ============================================================================
// Derived State
// ============================================================================

/// State derived from ledger (recomputable at any time)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DerivedState {
    /// CBU IDs affected by executed commands
    #[serde(default)]
    pub cbu_ids: Vec<Uuid>,

    /// Named bindings from executed commands (e.g., @cbu -> UUID)
    #[serde(default)]
    pub bindings: HashMap<String, Uuid>,

    /// Chat messages (for display)
    #[serde(default)]
    pub messages: Vec<ChatMessage>,

    /// Count of executed entries
    #[serde(default)]
    pub executed_count: usize,

    /// Count of failed entries
    #[serde(default)]
    pub failed_count: usize,
}

// ============================================================================
// Chat Message
// ============================================================================

/// A chat message for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Message role
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Agent,
    System,
}

impl ChatMessage {
    /// Create a user message
    pub fn user(content: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            timestamp,
        }
    }

    /// Create an agent message
    pub fn agent(content: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self {
            role: MessageRole::Agent,
            content: content.into(),
            timestamp,
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            timestamp,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session() {
        let session = ReplSession::new();
        assert!(session.ledger.is_empty());
        assert!(matches!(session.state, ReplState::Idle));
        assert!(session.needs_client_group());
    }

    #[test]
    fn test_add_entry() {
        let mut session = ReplSession::new();
        let entry = LedgerEntry::new_message("test message".to_string());
        session.add_entry(entry);

        assert_eq!(session.entry_count(), 1);
        assert!(session.last_entry().is_some());
    }

    #[test]
    fn test_recompute_derived() {
        let mut session = ReplSession::new();

        // Add a message entry
        let mut entry1 = LedgerEntry::new_message("load allianz".to_string());
        entry1.execution_result = Some(LedgerExecutionResult {
            message: "Loaded 10 CBUs".to_string(),
            affected_cbu_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
            bindings: vec![("@cbu".to_string(), Uuid::new_v4())],
            view_state_update: None,
            duration_ms: Some(150),
        });
        entry1.status = EntryStatus::Executed;
        session.add_entry(entry1);

        // Add another message
        let mut entry2 = LedgerEntry::new_message("show details".to_string());
        entry2.execution_result = Some(LedgerExecutionResult {
            message: "Showing CBU details".to_string(),
            affected_cbu_ids: vec![],
            bindings: vec![],
            view_state_update: None,
            duration_ms: Some(50),
        });
        entry2.status = EntryStatus::Executed;
        session.add_entry(entry2);

        // Recompute
        session.recompute_derived();

        assert_eq!(session.derived.cbu_ids.len(), 2);
        assert_eq!(session.derived.bindings.len(), 1);
        assert_eq!(session.derived.messages.len(), 4); // 2 user + 2 agent
        assert_eq!(session.derived.executed_count, 2);
    }

    #[test]
    fn test_state_transitions() {
        let mut session = ReplSession::new();

        // Start idle
        assert!(matches!(session.state, ReplState::Idle));

        // Transition to DSL ready
        session.transition_to_dsl_ready(
            "(cbu.create :name \"test\")".to_string(),
            "cbu.create".to_string(),
            false,
        );
        assert!(matches!(session.state, ReplState::DslReady { .. }));

        // Transition to executing
        session.transition_to_executing("(cbu.create :name \"test\")".to_string());
        assert!(matches!(session.state, ReplState::Executing { .. }));

        // Transition back to idle
        session.transition_to_idle();
        assert!(matches!(session.state, ReplState::Idle));
    }

    #[test]
    fn test_supersede_pending() {
        let mut session = ReplSession::new();

        // Add some entries with different statuses
        let mut entry1 = LedgerEntry::new_message("first".to_string());
        entry1.status = EntryStatus::Draft;
        session.add_entry(entry1);

        let mut entry2 = LedgerEntry::new_message("second".to_string());
        entry2.status = EntryStatus::Executed;
        session.add_entry(entry2);

        let mut entry3 = LedgerEntry::new_message("third".to_string());
        entry3.status = EntryStatus::Ready;
        session.add_entry(entry3);

        // Supersede pending
        session.supersede_pending();

        // Check statuses
        assert!(matches!(session.ledger[0].status, EntryStatus::Superseded));
        assert!(matches!(session.ledger[1].status, EntryStatus::Executed)); // Terminal, not changed
        assert!(matches!(session.ledger[2].status, EntryStatus::Superseded));
    }

    #[test]
    fn test_serialization() {
        let session = ReplSession::new()
            .with_user_id(Uuid::new_v4())
            .with_client_group(Uuid::new_v4(), "Test Group".to_string());

        let json = serde_json::to_string(&session).unwrap();
        let parsed: ReplSession = serde_json::from_str(&json).unwrap();

        assert_eq!(session.id, parsed.id);
        assert_eq!(session.client_group_name, parsed.client_group_name);
    }
}
