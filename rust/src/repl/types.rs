//! Core types for the REPL state machine
//!
//! All types in this module are serializable and designed for:
//! - State machine transitions
//! - Ledger persistence
//! - API response/request contracts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// REPL State Machine
// ============================================================================

/// REPL state machine states
///
/// The state machine follows this flow:
/// ```text
/// IDLE ─── user input ──► INTENT_MATCHING
///   ▲                          │
///   │                          ├─► Matched ─────────► DSL_READY ─► EXECUTING ─┐
///   │                          │                          │                   │
///   │                          ├─► Ambiguous ───► CLARIFYING ─────────────────┤
///   │                          │                                              │
///   │                          └─► NoMatch/Error ─────────────────────────────┤
///   │                                                                         │
///   └─────────────────────────────────────────────────────────────────────────┘
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReplState {
    /// Waiting for user input
    #[default]
    Idle,

    /// Processing natural language input
    IntentMatching { started_at: DateTime<Utc> },

    /// Waiting for user clarification (verb selection, entity resolution, etc.)
    Clarifying(ClarifyingState),

    /// DSL generated, waiting for execution or new input
    DslReady {
        dsl: String,
        verb: String,
        can_auto_execute: bool,
    },

    /// Currently executing DSL
    Executing {
        dsl: String,
        started_at: DateTime<Utc>,
    },
}

impl ReplState {
    /// Check if we're in an idle state (can accept new messages)
    pub fn is_idle(&self) -> bool {
        matches!(self, ReplState::Idle)
    }

    /// Check if we're waiting for user clarification
    pub fn is_clarifying(&self) -> bool {
        matches!(self, ReplState::Clarifying(_))
    }

    /// Check if DSL is ready for execution
    pub fn is_dsl_ready(&self) -> bool {
        matches!(self, ReplState::DslReady { .. })
    }

    /// Get the clarifying state if we're in clarifying mode
    pub fn clarifying_state(&self) -> Option<&ClarifyingState> {
        match self {
            ReplState::Clarifying(state) => Some(state),
            _ => None,
        }
    }
}

/// What we're waiting for the user to clarify
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClarifyingState {
    /// Multiple verbs matched - user must choose
    VerbSelection {
        options: Vec<VerbCandidate>,
        original_input: String,
        margin: f32,
    },

    /// Multiple client groups/scopes matched - user must choose
    ScopeSelection {
        options: Vec<ScopeCandidate>,
        original_input: String,
    },

    /// Entity references couldn't be resolved unambiguously
    EntityResolution {
        unresolved_refs: Vec<UnresolvedRef>,
        partial_dsl: String,
    },

    /// Action requires explicit confirmation (destructive operations)
    Confirmation {
        dsl: String,
        verb: String,
        summary: String,
    },

    /// Intent tier selection (high-level "what are you trying to do?")
    IntentTier {
        tier_number: u32,
        options: Vec<IntentTierOption>,
        original_input: String,
    },

    /// Client group selection (session context)
    ClientGroupSelection {
        options: Vec<ClientGroupOption>,
        prompt: String,
    },
}

impl ClarifyingState {
    /// Get the kind of clarification needed
    pub fn kind(&self) -> ClarifyingKind {
        match self {
            ClarifyingState::VerbSelection { .. } => ClarifyingKind::VerbSelection,
            ClarifyingState::ScopeSelection { .. } => ClarifyingKind::ScopeSelection,
            ClarifyingState::EntityResolution { .. } => ClarifyingKind::EntityResolution,
            ClarifyingState::Confirmation { .. } => ClarifyingKind::Confirmation,
            ClarifyingState::IntentTier { .. } => ClarifyingKind::IntentTier,
            ClarifyingState::ClientGroupSelection { .. } => ClarifyingKind::ClientGroupSelection,
        }
    }
}

/// Simple enum for clarifying state kind (for status tracking)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClarifyingKind {
    VerbSelection,
    ScopeSelection,
    EntityResolution,
    Confirmation,
    IntentTier,
    ClientGroupSelection,
}

// ============================================================================
// User Input Types
// ============================================================================

/// All types of user input to the REPL
///
/// Every interaction is logged to the ledger, enabling:
/// - Full replay of session state
/// - Audit trail of user decisions
/// - Learning signal collection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserInput {
    /// Natural language message
    Message { content: String },

    /// User selected a verb from disambiguation options
    VerbSelection {
        option_index: usize,
        selected_verb: String,
        original_input: String,
    },

    /// User selected a scope/client group
    ScopeSelection {
        option_id: String,
        option_name: String,
    },

    /// User selected an entity to resolve a reference
    EntitySelection {
        ref_id: String,
        entity_id: Uuid,
        entity_name: String,
    },

    /// User confirmed or rejected an action
    Confirmation { confirmed: bool },

    /// User selected an intent tier option
    IntentTierSelection { tier: u32, selected_id: String },

    /// User selected a client group
    ClientGroupSelection { group_id: Uuid, group_name: String },

    /// REPL command (run, undo, clear, etc.)
    Command { command: ReplCommand },
}

impl UserInput {
    /// Create a message input
    pub fn message(content: impl Into<String>) -> Self {
        Self::Message {
            content: content.into(),
        }
    }

    /// Create a command input
    pub fn command(cmd: ReplCommand) -> Self {
        Self::Command { command: cmd }
    }

    /// Get the natural language content if this is a message
    pub fn as_message(&self) -> Option<&str> {
        match self {
            UserInput::Message { content } => Some(content),
            _ => None,
        }
    }
}

/// REPL commands (not natural language)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplCommand {
    /// Execute the pending DSL
    Run,
    /// Undo the last executed command
    Undo,
    /// Redo a previously undone command
    Redo,
    /// Clear the session (start fresh)
    Clear,
    /// Cancel the current clarifying/pending state
    Cancel,
    /// Show session info
    Info,
    /// Show help
    Help,
}

// ============================================================================
// Ledger Entry
// ============================================================================

/// Single entry in the command ledger
///
/// The ledger is the **single source of truth** for session state.
/// All session state (CBU IDs, bindings, messages) can be derived
/// by replaying ledger entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Unique entry ID
    pub id: Uuid,

    /// When this entry was created
    pub timestamp: DateTime<Utc>,

    /// User input that triggered this entry
    pub input: UserInput,

    /// Intent matching result (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_result: Option<IntentMatchResult>,

    /// Generated DSL (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl: Option<String>,

    /// Execution result (if executed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_result: Option<LedgerExecutionResult>,

    /// Entry status
    pub status: EntryStatus,
}

impl LedgerEntry {
    /// Create a new ledger entry for a user message
    pub fn new_message(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            input: UserInput::message(content),
            intent_result: None,
            dsl: None,
            execution_result: None,
            status: EntryStatus::Draft,
        }
    }

    /// Create a new ledger entry for user input
    pub fn new(input: UserInput) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            input,
            intent_result: None,
            dsl: None,
            execution_result: None,
            status: EntryStatus::Draft,
        }
    }

    /// Update the entry with an intent result
    pub fn with_intent_result(mut self, result: IntentMatchResult) -> Self {
        self.dsl = result.generated_dsl.clone();
        self.intent_result = Some(result);
        self
    }

    /// Update the entry status
    pub fn with_status(mut self, status: EntryStatus) -> Self {
        self.status = status;
        self
    }
}

/// Entry status (tracks full lifecycle)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EntryStatus {
    /// Just added, being processed
    Draft,
    /// Waiting for user clarification
    Clarifying { kind: ClarifyingKind },
    /// DSL generated, ready to execute
    Ready,
    /// Currently executing
    Executing,
    /// Successfully executed
    Executed,
    /// Execution failed
    Failed { error: String },
    /// User cancelled
    Cancelled,
    /// Superseded by a newer entry (e.g., user typed new message)
    Superseded,
}

impl EntryStatus {
    /// Check if this is a terminal status
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EntryStatus::Executed
                | EntryStatus::Failed { .. }
                | EntryStatus::Cancelled
                | EntryStatus::Superseded
        )
    }
}

/// Execution result stored in ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerExecutionResult {
    /// Human-readable result message
    pub message: String,

    /// CBU IDs affected by this execution
    #[serde(default)]
    pub affected_cbu_ids: Vec<Uuid>,

    /// Bindings created (e.g., @cbu -> UUID)
    #[serde(default)]
    pub bindings: Vec<(String, Uuid)>,

    /// View state changes (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_state_update: Option<serde_json::Value>,

    /// Execution duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

// ============================================================================
// Intent Matching Types
// ============================================================================

/// Context for intent matching (immutable, passed in)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchContext {
    /// Current client group (if set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_group_id: Option<Uuid>,

    /// Current client group name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_group_name: Option<String>,

    /// Current scope context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeContext>,

    /// Dominant entity from previous interactions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dominant_entity_id: Option<Uuid>,

    /// User ID (for personalized matching)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,

    /// Domain hint (e.g., "kyc", "trading")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_hint: Option<String>,

    /// Current bindings (symbol -> entity)
    #[serde(default)]
    pub bindings: Vec<(String, Uuid)>,
}

/// Scope context for matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeContext {
    /// CBU IDs in scope
    pub cbu_ids: Vec<Uuid>,
    /// Scope description
    pub description: String,
}

/// Intent match result (returned from IntentMatcher)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentMatchResult {
    /// Match outcome
    pub outcome: MatchOutcome,

    /// Verb candidates (sorted by score, highest first)
    #[serde(default)]
    pub verb_candidates: Vec<VerbCandidate>,

    /// Entity mentions found in input
    #[serde(default)]
    pub entity_mentions: Vec<EntityMention>,

    /// Scope candidates (if scope selection needed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_candidates: Option<Vec<ScopeCandidate>>,

    /// Generated DSL (if verb matched and args extracted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_dsl: Option<String>,

    /// Unresolved entity references
    #[serde(default)]
    pub unresolved_refs: Vec<UnresolvedRef>,

    /// Debug information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<MatchDebugInfo>,
}

impl IntentMatchResult {
    /// Create a "no match" result
    pub fn no_match(reason: impl Into<String>) -> Self {
        Self {
            outcome: MatchOutcome::NoMatch {
                reason: reason.into(),
            },
            verb_candidates: vec![],
            entity_mentions: vec![],
            scope_candidates: None,
            generated_dsl: None,
            unresolved_refs: vec![],
            debug: None,
        }
    }

    /// Create a "needs client group" result
    pub fn needs_client_group(options: Vec<ClientGroupOption>) -> Self {
        Self {
            outcome: MatchOutcome::NeedsClientGroup { options },
            verb_candidates: vec![],
            entity_mentions: vec![],
            scope_candidates: None,
            generated_dsl: None,
            unresolved_refs: vec![],
            debug: None,
        }
    }
}

/// Match outcome
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MatchOutcome {
    /// Clear winner found
    Matched { verb: String, confidence: f32 },

    /// Multiple verbs matched with similar scores
    Ambiguous { margin: f32 },

    /// Need user to select scope/client group
    NeedsScopeSelection,

    /// Entity references need resolution
    NeedsEntityResolution,

    /// Need user to select client group (session context)
    NeedsClientGroup { options: Vec<ClientGroupOption> },

    /// Need user to select intent tier
    NeedsIntentTier { options: Vec<IntentTierOption> },

    /// No matching verb found
    NoMatch { reason: String },

    /// Direct DSL input (bypass intent matching)
    DirectDsl { source: String },
}

/// Verb candidate from search
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerbCandidate {
    /// Fully qualified verb name (e.g., "session.load-galaxy")
    pub verb_fqn: String,
    /// Verb description
    pub description: String,
    /// Match score (0.0-1.0)
    pub score: f32,
    /// Example phrase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// Domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// Entity mention found in input
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityMention {
    /// Text span in original input
    pub text: String,
    /// Character offset start
    pub start: usize,
    /// Character offset end
    pub end: usize,
    /// Resolved entity ID (if unambiguous)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    /// Resolved entity name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_name: Option<String>,
    /// Entity kind (company, person, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_kind: Option<String>,
    /// Confidence score
    pub confidence: f32,
}

/// Scope candidate for selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScopeCandidate {
    /// Unique ID for this option
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Number of CBUs in this scope
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cbu_count: Option<usize>,
}

/// Unresolved entity reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnresolvedRef {
    /// Reference ID (for tracking)
    pub ref_id: String,
    /// Original text
    pub text: String,
    /// Expected entity type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_kind: Option<String>,
    /// Candidate entities
    pub candidates: Vec<EntityCandidate>,
}

/// Entity candidate for resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityCandidate {
    /// Entity ID
    pub entity_id: Uuid,
    /// Entity name
    pub name: String,
    /// Entity kind
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Match score
    pub score: f32,
}

/// Client group option for selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientGroupOption {
    /// Group ID
    pub group_id: Uuid,
    /// Group name
    pub name: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Number of CBUs in this group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cbu_count: Option<usize>,
}

/// Intent tier option (high-level categorization)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentTierOption {
    /// Option ID
    pub id: String,
    /// Label (e.g., "Navigate & View")
    pub label: String,
    /// Description
    pub description: String,
    /// Hint about related verbs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Number of verbs in this category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb_count: Option<usize>,
}

/// Debug information for intent matching
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchDebugInfo {
    /// Time spent on each stage (ms)
    #[serde(default)]
    pub timing: Vec<(String, u64)>,
    /// Search tier that produced the match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_tier: Option<String>,
    /// Entity linking details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_linking: Option<serde_json::Value>,
    /// Additional notes
    #[serde(default)]
    pub notes: Vec<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_state_serialization() {
        let state = ReplState::Clarifying(ClarifyingState::VerbSelection {
            options: vec![VerbCandidate {
                verb_fqn: "session.load-galaxy".to_string(),
                description: "Load all CBUs under an apex entity".to_string(),
                score: 0.85,
                example: Some("load the allianz book".to_string()),
                domain: Some("session".to_string()),
            }],
            original_input: "load the book".to_string(),
            margin: 0.05,
        });

        let json = serde_json::to_string(&state).unwrap();
        let parsed: ReplState = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            ReplState::Clarifying(ClarifyingState::VerbSelection { .. })
        ));
    }

    #[test]
    fn test_user_input_variants() {
        let inputs = vec![
            UserInput::message("load the allianz book"),
            UserInput::command(ReplCommand::Run),
            UserInput::VerbSelection {
                option_index: 0,
                selected_verb: "session.load-galaxy".to_string(),
                original_input: "load".to_string(),
            },
        ];

        for input in inputs {
            let json = serde_json::to_string(&input).unwrap();
            let _parsed: UserInput = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_ledger_entry_lifecycle() {
        let mut entry = LedgerEntry::new_message("test".to_string());
        assert!(matches!(entry.status, EntryStatus::Draft));

        entry.status = EntryStatus::Executed;
        assert!(entry.status.is_terminal());
    }

    #[test]
    fn test_match_outcome_variants() {
        let outcomes = vec![
            MatchOutcome::Matched {
                verb: "cbu.create".to_string(),
                confidence: 0.95,
            },
            MatchOutcome::Ambiguous { margin: 0.02 },
            MatchOutcome::NeedsScopeSelection,
            MatchOutcome::NoMatch {
                reason: "No verbs matched".to_string(),
            },
        ];

        for outcome in outcomes {
            let json = serde_json::to_string(&outcome).unwrap();
            let _parsed: MatchOutcome = serde_json::from_str(&json).unwrap();
        }
    }
}
