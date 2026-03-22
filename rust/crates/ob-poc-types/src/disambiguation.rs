//! Disambiguation API types
//!
//! Types for entity disambiguation, verb disambiguation, and intent tier
//! clarification. These handle ambiguous user input at multiple stages
//! of the pipeline.

use serde::{Deserialize, Serialize};

use crate::ExecuteResult;

// ============================================================================
// ENTITY DISAMBIGUATION API
// ============================================================================

/// Disambiguation request - sent when user input is ambiguous
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationRequest {
    /// Unique ID for this disambiguation request
    pub request_id: String,
    /// The ambiguous items that need resolution
    pub items: Vec<DisambiguationItem>,
    /// Human-readable prompt for the user
    pub prompt: String,
}

/// A single ambiguous item needing resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationItem {
    /// Multiple entities match a search term
    EntityMatch {
        /// Parameter name (e.g., "entity-id")
        param: String,
        /// Original search text (e.g., "John Smith")
        search_text: String,
        /// Matching entities to choose from
        matches: Vec<EntityMatch>,
        /// Entity type for search (e.g., "entity", "cbu") - Fix K
        #[serde(skip_serializing_if = "Option::is_none")]
        entity_type: Option<String>,
        /// Search column from lookup config (e.g., "name") - Fix K
        #[serde(skip_serializing_if = "Option::is_none")]
        search_column: Option<String>,
        /// Unique ref_id for commit targeting (e.g., "0:15-30") - Fix K
        #[serde(skip_serializing_if = "Option::is_none")]
        ref_id: Option<String>,
    },
    /// Ambiguous interpretation (e.g., "UK" = name part or jurisdiction?)
    InterpretationChoice {
        /// The ambiguous text
        text: String,
        /// Possible interpretations
        options: Vec<Interpretation>,
    },
    /// Multiple client groups match - used for Stage 0 scope resolution
    ClientGroupMatch {
        /// The original search text (e.g., "allianz")
        search_text: String,
        /// Matching client groups to choose from
        candidates: Vec<ClientGroupCandidate>,
    },
}

/// A matching client group for scope disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGroupCandidate {
    /// Client group UUID
    pub group_id: String,
    /// Canonical group name (e.g., "Allianz Global Investors")
    pub group_name: String,
    /// The alias that matched (e.g., "allianz", "AGI")
    pub matched_alias: String,
    /// Match confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Number of entities in this group (optional, for display)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_count: Option<i64>,
}

/// A matching entity for disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    /// Entity UUID
    pub entity_id: String,
    /// Display name
    pub name: String,
    /// Entity type (e.g., "proper_person", "limited_company")
    pub entity_type: String,
    /// Jurisdiction code
    #[serde(default)]
    pub jurisdiction: Option<String>,
    /// Additional context (roles, etc.)
    #[serde(default)]
    pub context: Option<String>,
    /// Match score (0.0 - 1.0)
    #[serde(default)]
    pub score: Option<f64>,
}

/// A possible interpretation of ambiguous text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    /// Interpretation ID
    pub id: String,
    /// Human-readable label
    pub label: String,
    /// What this interpretation means
    pub description: String,
    /// How this affects the generated DSL
    #[serde(default)]
    pub effect: Option<String>,
}

/// User's disambiguation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationResponse {
    /// The request ID being responded to
    pub request_id: String,
    /// Selected resolutions
    pub selections: Vec<DisambiguationSelection>,
}

/// A single disambiguation selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationSelection {
    /// Selected entity for an EntityMatch
    Entity { param: String, entity_id: String },
    /// Selected interpretation for an InterpretationChoice
    Interpretation {
        text: String,
        interpretation_id: String,
    },
}

// ============================================================================
// VERB DISAMBIGUATION API (for ambiguous verb matches)
// ============================================================================

/// Verb disambiguation request - sent when multiple verbs match user input
///
/// This is separate from entity disambiguation because:
/// 1. It happens earlier in the pipeline (verb search before DSL generation)
/// 2. User selects from verbs, not entities
/// 3. Selection is gold-standard training data (high confidence label)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbDisambiguationRequest {
    /// Unique ID for this disambiguation request
    pub request_id: String,
    /// Original user input that triggered disambiguation
    pub original_input: String,
    /// Matching verbs to choose from (ordered by score, descending)
    pub options: Vec<VerbOption>,
    /// Human-readable prompt for the user
    pub prompt: String,
}

/// A single verb option in disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbOption {
    /// Fully qualified verb name (e.g., "cbu.create")
    pub verb_fqn: String,
    /// Human-readable description
    pub description: String,
    /// Example DSL showing this verb
    pub example: String,
    /// Match score (0.0 - 1.0)
    pub score: f32,
    /// The phrase that matched this verb (from verb search)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_phrase: Option<String>,
    /// Domain label for grouping in UI (e.g., "Session & Navigation")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_label: Option<String>,
    /// Category within domain (e.g., "Load CBUs")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_label: Option<String>,
    /// Clear utterance the user can say to unambiguously select this verb.
    /// E.g., "Open a new KYC case" or "Check the KYC case status".
    /// Sourced from the verb's best invocation phrase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_utterance: Option<String>,

    // ── Differentiation context ───────────────────────────────────
    // Explains WHY this option differs from the alternatives.
    // Without this, the user sees two identical-looking options.

    /// What kind of verb this is: "primitive" (single operation),
    /// "macro" (multi-step workflow), "query" (read-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_kind: Option<String>,

    /// Human-readable reason this verb is an option.
    /// E.g., "Single PEP check for one entity" vs "Full screening workflow (3 steps)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub differentiation: Option<String>,

    /// What state the entity must be in for this verb to fire.
    /// E.g., "Requires KYC case in REVIEW state".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_state: Option<String>,

    /// What state the entity moves to after this verb executes.
    /// E.g., "Moves case to APPROVED".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produces_state: Option<String>,

    /// Scope of this verb: "single_entity", "batch", "group".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Number of steps if this is a macro/workflow verb.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_count: Option<u32>,

    // ── Entity & constellation context ─────────────────────────────
    // Shows WHERE in the constellation this verb operates.
    // This is almost always the cause of confusion — the user is
    // looking at entity A but the verb applies to entity B.

    /// The entity type this verb operates on (e.g., "cbu", "entity", "case").
    /// Derived from the verb's `subject_kinds` or `produces` declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_entity_kind: Option<String>,

    /// The constellation slot this verb belongs to (e.g., "kyc_case", "screening").
    /// Shows the user WHERE in the onboarding DAG this action lives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constellation_slot: Option<String>,

    /// Human-readable constellation position context.
    /// E.g., "Operates on the KYC case for this CBU" or
    /// "Applies to the depositary entity in this structure".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_context: Option<String>,

    /// The specific entity name if one is in scope and relevant.
    /// E.g., "Allianz Dynamic Commodities" or "HSBC Holdings plc".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_entity_name: Option<String>,
}

// ============================================================================
// INTENT TIER DISAMBIGUATION
// ============================================================================
// When verb search returns candidates spanning multiple intents (e.g., navigate
// vs create), we first ask the user to clarify their intent before showing
// specific verbs. This reduces cognitive load and improves learning signals.

/// Request for intent tier clarification (shown before verb disambiguation)
///
/// Example flow:
/// User: "load something"
/// → Tier 1: "What are you trying to do?" [Navigate, Create, Modify]
/// → User picks "Navigate"
/// → Tier 2: "What scope?" [Single structure, Client book, Jurisdiction]
/// → User picks "Client book"
/// → Verb options: [session.load-galaxy, session.load-cluster]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentTierRequest {
    /// Unique ID for this tier request (used for selection tracking)
    pub request_id: String,
    /// Which tier level (1 = action intent, 2 = scope/domain)
    pub tier_number: u32,
    /// Original user input that triggered disambiguation
    pub original_input: String,
    /// Options to choose from at this tier
    pub options: Vec<IntentTierOption>,
    /// Human-readable prompt (e.g., "What are you trying to do?")
    pub prompt: String,
    /// Previously selected tiers (for context display)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_path: Vec<IntentTierSelection>,
}

/// A single option in an intent tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentTierOption {
    /// Option identifier (e.g., "navigate", "create", "single_structure")
    pub id: String,
    /// Human-readable label (e.g., "Set session scope / Navigate")
    pub label: String,
    /// Longer description explaining this option
    pub description: String,
    /// Optional hint text (e.g., "You want to work with existing data")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Number of verbs this option would narrow down to
    #[serde(default)]
    pub verb_count: usize,
}

/// A recorded tier selection (for tracking the path through tiers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentTierSelection {
    /// Tier number that was selected
    pub tier: u32,
    /// Selected option ID
    pub option_id: String,
    /// Selected option label (for display)
    pub option_label: String,
}

/// User's selection at an intent tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentTierSelectionRequest {
    /// The tier request ID being responded to
    pub request_id: String,
    /// Selected option ID
    pub selected_option: String,
    /// Original user input (for learning)
    pub original_input: String,
}

/// Response after intent tier selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentTierSelectionResponse {
    /// Whether to show another tier or proceed to verb disambiguation
    #[serde(flatten)]
    pub next_step: IntentTierNextStep,
}

/// What happens after an intent tier selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IntentTierNextStep {
    /// Show another tier (narrowing down further)
    ShowTier { tier_request: IntentTierRequest },
    /// Show verb disambiguation (final step before DSL)
    ShowVerbs {
        verb_disambiguation: VerbDisambiguationRequest,
    },
    /// Clear match found - proceed directly to DSL generation
    Proceed {
        selected_verb: String,
        message: String,
    },
}

// ============================================================================
// VERB SELECTION (user responds to verb disambiguation)
// ============================================================================

/// User's verb selection response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSelectionRequest {
    /// The disambiguation request ID being responded to
    pub request_id: String,
    /// Original user input (for learning)
    pub original_input: String,
    /// Selected verb fully-qualified name
    pub selected_verb: String,
    /// All verbs that were shown as options (for negative learning)
    pub all_candidates: Vec<String>,
}

/// Response after verb selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSelectionResponse {
    /// Whether the selection was recorded successfully
    pub recorded: bool,
    /// Execution result (if verb was executed)
    #[serde(default)]
    pub execution_result: Option<ExecuteResult>,
    /// Message to display
    pub message: String,
}
