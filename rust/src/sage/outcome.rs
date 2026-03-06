//! OutcomeIntent — structured intent understood by the Sage.
//!
//! This is the Sage's output type. It carries enough information for the Coder
//! to resolve to a specific verb and assemble arguments — without ever exposing
//! verb FQNs to the Sage itself.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sage::plane::ObservationPlane;
use crate::sage::polarity::IntentPolarity;

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

/// The semantic action the user intends (verb-agnostic).
///
/// These are action verbs in the domain sense, not DSL verb FQNs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeAction {
    /// Read / view / describe / list
    Read,
    /// Create / add / new / register
    Create,
    /// Update / edit / modify / change
    Update,
    /// Delete / remove / retire / archive
    Delete,
    /// Assign / link / attach / associate
    Assign,
    /// Import / sync / load from external
    Import,
    /// Search / find / discover
    Search,
    /// Compute / derive / calculate
    Compute,
    /// Publish / promote / approve
    Publish,
    /// Free-form action not matching known categories
    Other(String),
}

impl OutcomeAction {
    pub fn as_str(&self) -> &str {
        match self {
            OutcomeAction::Read => "read",
            OutcomeAction::Create => "create",
            OutcomeAction::Update => "update",
            OutcomeAction::Delete => "delete",
            OutcomeAction::Assign => "assign",
            OutcomeAction::Import => "import",
            OutcomeAction::Search => "search",
            OutcomeAction::Compute => "compute",
            OutcomeAction::Publish => "publish",
            OutcomeAction::Other(s) => s.as_str(),
        }
    }

    /// Derive a best-guess action from the first verb-like word in the utterance.
    pub fn from_first_word(utterance: &str) -> Self {
        let first = utterance
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();

        match first.as_str() {
            "show" | "list" | "get" | "fetch" | "describe" | "view" | "what" | "who"
            | "find" | "display" | "inspect" | "tell" | "read" | "look" | "lookup"
            | "query" | "count" | "check" | "trace" | "summarize" | "summary" | "explain"
            | "explore" | "report" | "which" | "where" | "how" => OutcomeAction::Read,

            "create" | "add" | "make" | "new" | "register" | "build" | "generate" => {
                OutcomeAction::Create
            }

            "update" | "edit" | "change" | "modify" | "rename" | "set" => OutcomeAction::Update,

            "delete" | "remove" | "drop" | "archive" | "retire" | "deprecate" => {
                OutcomeAction::Delete
            }

            "assign" | "attach" | "link" | "enroll" | "onboard" => OutcomeAction::Assign,

            "import" | "sync" | "push" | "pull" | "load" => OutcomeAction::Import,

            "search" => OutcomeAction::Search,

            "compute" | "calculate" | "derive" | "analyze" | "analyse" => OutcomeAction::Compute,

            "publish" | "approve" | "promote" | "deploy" | "propose" | "submit" => {
                OutcomeAction::Publish
            }

            other => OutcomeAction::Other(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Confidence
// ---------------------------------------------------------------------------

/// How confident the Sage is in its classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SageConfidence {
    /// All three signals (plane, polarity, domain) align deterministically.
    High,
    /// Two of three signals are deterministic; one is inferred.
    Medium,
    /// Mostly guessing — LLM or more context needed.
    Low,
}

impl SageConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            SageConfidence::High => "high",
            SageConfidence::Medium => "medium",
            SageConfidence::Low => "low",
        }
    }
}

// ---------------------------------------------------------------------------
// EntityRef
// ---------------------------------------------------------------------------

/// A reference to an entity mentioned in the utterance (pre-resolution).
///
/// At Sage time the entity has NOT been resolved to a UUID — this is intentional.
/// Entity linking runs AFTER the Sage (E-SAGE-1 invariant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    /// The raw text mention from the utterance (e.g., "the allianz fund")
    pub mention: String,
    /// Domain kind hint if determinable (e.g., "cbu", "entity", "deal")
    pub kind_hint: Option<String>,
    /// UUID if already known from session context (NOT from entity linking)
    pub uuid: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Clarification
// ---------------------------------------------------------------------------

/// A pending question the Sage needs answered before it can be confident.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clarification {
    /// Human-readable question (e.g., "Are you exploring the fund schema or a specific fund instance?")
    pub question: String,
    /// Suggested answer options (if available)
    pub options: Vec<String>,
    /// Which field of OutcomeIntent this clarifies
    pub clarifies: String,
}

// ---------------------------------------------------------------------------
// OutcomeStep
// ---------------------------------------------------------------------------

/// A single step in a multi-step intent (for compound operations).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeStep {
    /// Semantic action for this step
    pub action: OutcomeAction,
    /// Domain target (e.g., "fund", "entity", "schema")
    pub target: String,
    /// Key-value params derived from the utterance (NOT yet DSL args)
    pub params: std::collections::HashMap<String, String>,
    /// Human-readable notes about this step
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// OutcomeIntent — the Sage's output
// ---------------------------------------------------------------------------

/// The structured intent understood by the Sage.
///
/// The Sage produces this from the raw utterance + session context.
/// The Coder consumes this to resolve a specific verb and assemble args.
///
/// ## Invariants
/// - `plane` is always deterministic from session context (never guessed)
/// - `polarity` is always deterministic from clue words (never guessed)
/// - `domain_concept` may be inferred (confidence drops to Medium/Low)
/// - No verb FQNs appear anywhere in this struct (E-SAGE-2 invariant)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeIntent {
    /// One-sentence summary of what the user wants
    pub summary: String,

    /// Which slice of the system is being operated on
    pub plane: ObservationPlane,

    /// Read or Write (or Ambiguous if unclear)
    pub polarity: IntentPolarity,

    /// The domain concept being operated on (e.g., "fund", "deal", "kyc-case", "schema")
    pub domain_concept: String,

    /// The semantic action (create, read, update, delete, assign, etc.)
    pub action: OutcomeAction,

    /// Entity reference from the utterance (pre-resolution, may be None)
    pub subject: Option<EntityRef>,

    /// Steps for multi-step intents; single-step intents have one entry
    pub steps: Vec<OutcomeStep>,

    /// Sage's confidence in this classification
    pub confidence: SageConfidence,

    /// Questions the Sage needs answered (empty = confident)
    pub pending_clarifications: Vec<Clarification>,
}

impl OutcomeIntent {
    /// Construct a minimal, low-confidence OutcomeIntent suitable as a stub.
    pub fn stub(utterance: &str, plane: ObservationPlane, polarity: IntentPolarity) -> Self {
        let action = OutcomeAction::from_first_word(utterance);
        OutcomeIntent {
            summary: format!("Intent from: {}", &utterance[..utterance.len().min(60)]),
            plane,
            polarity,
            domain_concept: String::new(),
            action,
            subject: None,
            steps: Vec::new(),
            confidence: SageConfidence::Low,
            pending_clarifications: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_action_from_first_word() {
        assert_eq!(OutcomeAction::from_first_word("show me the deals"), OutcomeAction::Read);
        assert_eq!(OutcomeAction::from_first_word("create a new fund"), OutcomeAction::Create);
        assert_eq!(OutcomeAction::from_first_word("import the gleif tree"), OutcomeAction::Import);
        assert_eq!(OutcomeAction::from_first_word("assign a role to the entity"), OutcomeAction::Assign);
        assert_eq!(OutcomeAction::from_first_word("xyzzy unknown verb"), OutcomeAction::Other("xyzzy".to_string()));
    }

    #[test]
    fn test_stub_construction() {
        let intent = OutcomeIntent::stub(
            "describe the deal schema",
            ObservationPlane::Structure,
            IntentPolarity::Read,
        );
        assert_eq!(intent.plane, ObservationPlane::Structure);
        assert_eq!(intent.polarity, IntentPolarity::Read);
        assert_eq!(intent.confidence, SageConfidence::Low);
        assert!(intent.pending_clarifications.is_empty());
    }

    #[test]
    fn test_sage_confidence_str() {
        assert_eq!(SageConfidence::High.as_str(), "high");
        assert_eq!(SageConfidence::Medium.as_str(), "medium");
        assert_eq!(SageConfidence::Low.as_str(), "low");
    }
}
