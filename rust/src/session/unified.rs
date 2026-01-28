//! Unified Session State
//!
//! Single source of truth for session state.
//! Replaces: SessionContext, SessionState, SubSessionType, BatchContext, CbuSession, DslSheet
//!
//! The session is the REPL DSL "sheet" with:
//! - Universe (client) - the constraint cascade anchor
//! - CBU sets - entity_scope.cbu_ids
//! - CBU state keys - dag_state (structure.exists, case.approved, etc.)
//!
//! See: ai-thoughts/036-session-rip-and-replace.md
//! See: rust/src/mcp/TODO_UNIFIED_ARCHITECTURE.md

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// The unified session - single source of truth
///
/// Session = REPL DSL sheet with universe (client), CBU sets, and CBU state keys.
/// The constraint cascade flows: client → structure_type → current_structure → verb schema filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSession {
    // === Identity ===
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // === Target Universe (anchor) ===
    /// Declared at session start, refined over time
    pub target_universe: Option<TargetUniverse>,

    // === Entity Scope (current working set) ===
    pub entity_scope: EntityScope,

    // === Run Sheet (DSL ledger) ===
    pub run_sheet: RunSheet,

    // === State Stack (navigation history) ===
    pub state_stack: StateStack,

    // === View State (for viewport sync) ===
    pub view_state: ViewState,

    // === Resolution (inline, optional) ===
    /// When Some, UI should show resolution modal
    pub resolution: Option<ResolutionState>,

    // === Conversation ===
    pub messages: Vec<ChatMessage>,

    // === Symbol Table ===
    pub bindings: HashMap<String, BoundEntity>,

    // === Constraint Cascade Context ===
    /// Current client context (narrows entity search from 10,000 to ~500)
    pub client: Option<ClientRef>,
    /// Structure type filter (PE, SICAV, etc.) (narrows to ~50)
    pub structure_type: Option<StructureType>,
    /// Current structure being worked on
    pub current_structure: Option<StructureRef>,
    /// Current KYC case being worked on
    pub current_case: Option<CaseRef>,
    /// Current mandate (trading profile) being worked on
    pub current_mandate: Option<MandateRef>,

    // === Persona (filters available verbs) ===
    pub persona: Persona,

    // === DAG Navigation State ===
    /// Tracks verb completions and state flags for prereq checking
    pub dag_state: DagState,

    // === CBU Undo/Redo History (migrated from CbuSession) ===
    /// Undo stack for CBU set changes
    #[serde(default)]
    pub cbu_history: Vec<CbuSnapshot>,
    /// Redo stack for CBU set changes
    #[serde(default)]
    pub cbu_future: Vec<CbuSnapshot>,

    // === REPL State Machine (migrated from CbuSession) ===
    /// Current state in the REPL pipeline
    #[serde(default)]
    pub repl_state: ReplState,
    /// DSL that defined the current scope (for audit/replay)
    #[serde(default)]
    pub scope_dsl: Vec<String>,
    /// Template DSL before expansion
    #[serde(default)]
    pub template_dsl: Option<String>,
    /// Target entity type for template expansion
    #[serde(default)]
    pub target_entity_type: Option<String>,
    /// Whether the user has confirmed the intent
    #[serde(default)]
    pub intent_confirmed: bool,

    // === Persistence Tracking ===
    /// Whether state has changed since last save
    #[serde(skip)]
    pub dirty: bool,
    /// Session name (optional)
    #[serde(default)]
    pub name: Option<String>,

    // === Agent Session Fields (migrated from AgentSession) ===
    /// Entity type this session operates on ("cbu", "kyc_case", "onboarding", etc.)
    #[serde(default)]
    pub entity_type: String,
    /// Entity ID this session operates on (CBU ID, case ID, etc.)
    #[serde(default)]
    pub entity_id: Option<Uuid>,
    /// Current state in the session lifecycle (New, Scoped, PendingValidation, etc.)
    #[serde(default)]
    pub state: SessionState,
    /// Parent session ID (None for root sessions)
    #[serde(default)]
    pub parent_session_id: Option<Uuid>,
    /// Sub-session type (determines behavior and scoped capabilities)
    #[serde(default)]
    pub sub_session_type: SubSessionType,
    /// Symbols inherited from parent session (pre-populated on creation)
    #[serde(default)]
    pub inherited_symbols: HashMap<String, BoundEntity>,
    /// Pending verb intents during disambiguation flow
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_intents: Vec<crate::api::intent::VerbIntent>,
    /// Domain hint for RAG context (duplicated in context for backward compat)
    #[serde(default)]
    pub domain_hint: Option<String>,

    // === Session Context (backward compatibility with AgentSession) ===
    /// Full session context for DSL execution, AST, bindings, view state, etc.
    /// This provides backward compatibility with code that accesses session.context
    #[serde(default)]
    pub context: crate::api::session::SessionContext,
}

/// Target universe - the anchor for this session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetUniverse {
    pub description: String,
    pub definition: UniverseDefinition,
    pub declared_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UniverseDefinition {
    /// All CBUs under an apex entity
    Galaxy {
        apex_entity_id: Uuid,
        apex_name: String,
    },
    /// CBUs matching filters
    Filtered {
        filter_description: String,
        cbu_ids: Vec<Uuid>,
    },
    /// Explicit list
    Explicit {
        cbu_ids: Vec<Uuid>,
        names: Vec<String>,
    },
    /// Single CBU focus
    SingleCbu { cbu_id: Uuid, cbu_name: String },
    /// Full book (all accessible CBUs)
    FullBook,
}

/// Current entity scope - what the viewport shows
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityScope {
    /// CBU IDs in scope
    pub cbu_ids: HashSet<Uuid>,
    /// Entity IDs in scope (within CBUs)
    pub entity_ids: HashSet<Uuid>,
    /// Focal entity (if zoomed)
    pub focal_entity_id: Option<Uuid>,
    /// Current zoom level
    pub zoom_level: ZoomLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoomLevel {
    #[default]
    Universe,
    Galaxy,
    System,
    Planet,
    Surface,
    Core,
}

impl ZoomLevel {
    /// Returns true if this level is more zoomed in than other
    pub fn is_deeper_than(&self, other: &ZoomLevel) -> bool {
        self.depth() > other.depth()
    }

    fn depth(&self) -> u8 {
        match self {
            ZoomLevel::Universe => 0,
            ZoomLevel::Galaxy => 1,
            ZoomLevel::System => 2,
            ZoomLevel::Planet => 3,
            ZoomLevel::Surface => 4,
            ZoomLevel::Core => 5,
        }
    }
}

/// Run sheet - DSL statement ledger
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunSheet {
    pub entries: Vec<RunSheetEntry>,
    pub cursor: usize,
}

impl RunSheet {
    /// Get entry at cursor
    pub fn current(&self) -> Option<&RunSheetEntry> {
        self.entries.get(self.cursor)
    }

    /// Get mutable entry at cursor
    pub fn current_mut(&mut self) -> Option<&mut RunSheetEntry> {
        self.entries.get_mut(self.cursor)
    }

    /// Count entries by status
    pub fn count_by_status(&self, status: EntryStatus) -> usize {
        self.entries.iter().filter(|e| e.status == status).count()
    }

    /// Get all draft entries
    pub fn drafts(&self) -> impl Iterator<Item = &RunSheetEntry> {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Draft)
    }

    /// Get all executed entries
    pub fn executed(&self) -> impl Iterator<Item = &RunSheetEntry> {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Executed)
    }

    /// Get entries by DAG depth (phase)
    pub fn by_phase(&self, depth: u32) -> impl Iterator<Item = &RunSheetEntry> {
        self.entries.iter().filter(move |e| e.dag_depth == depth)
    }

    /// Get the maximum DAG depth in the run sheet
    pub fn max_depth(&self) -> u32 {
        self.entries.iter().map(|e| e.dag_depth).max().unwrap_or(0)
    }

    /// Get entries ready for execution (dependencies satisfied)
    pub fn ready_for_execution(&self) -> Vec<&RunSheetEntry> {
        let executed_ids: HashSet<_> = self
            .entries
            .iter()
            .filter(|e| e.status == EntryStatus::Executed)
            .map(|e| e.id)
            .collect();

        self.entries
            .iter()
            .filter(|e| {
                e.status == EntryStatus::Ready
                    && e.validation_errors.is_empty()
                    && e.dependencies.iter().all(|dep| executed_ids.contains(dep))
            })
            .collect()
    }

    /// Check if all entries in a phase are complete
    pub fn phase_complete(&self, depth: u32) -> bool {
        self.entries
            .iter()
            .filter(|e| e.dag_depth == depth)
            .all(|e| e.status.is_terminal())
    }

    /// Mark all entries that depend on a failed entry as skipped
    pub fn cascade_skip(&mut self, failed_id: Uuid) {
        let mut to_skip: Vec<Uuid> = vec![failed_id];
        let mut idx = 0;

        while idx < to_skip.len() {
            let skip_id = to_skip[idx];
            for entry in &self.entries {
                if entry.dependencies.contains(&skip_id) && !to_skip.contains(&entry.id) {
                    to_skip.push(entry.id);
                }
            }
            idx += 1;
        }

        // Skip all dependents (except the original failed entry)
        for entry in &mut self.entries {
            if to_skip.contains(&entry.id) && entry.id != failed_id {
                entry.status = EntryStatus::Skipped;
                entry.error = Some(format!("Skipped: dependency {} failed", failed_id));
            }
        }
    }

    /// Get combined DSL source from all entries
    pub fn combined_dsl(&self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        Some(
            self.entries
                .iter()
                .map(|e| e.dsl_source.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Check if there are runnable entries (draft or ready)
    pub fn has_runnable(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e.status, EntryStatus::Draft | EntryStatus::Ready))
    }

    /// Mark all draft/ready entries as executed
    pub fn mark_all_executed(&mut self) {
        for entry in &mut self.entries {
            if matches!(entry.status, EntryStatus::Draft | EntryStatus::Ready) {
                entry.status = EntryStatus::Executed;
                entry.executed_at = Some(Utc::now());
            }
        }
    }

    /// Convert to API response format (ob_poc_types::RunSheet)
    pub fn to_api(&self) -> ob_poc_types::RunSheet {
        ob_poc_types::RunSheet {
            entries: self
                .entries
                .iter()
                .map(|e| ob_poc_types::RunSheetEntry {
                    id: e.id.to_string(),
                    dsl_source: e.dsl_source.clone(),
                    display_dsl: Some(e.display_dsl.clone()),
                    status: match e.status {
                        EntryStatus::Draft => ob_poc_types::RunSheetEntryStatus::Draft,
                        EntryStatus::Ready => ob_poc_types::RunSheetEntryStatus::Ready,
                        EntryStatus::Executing => ob_poc_types::RunSheetEntryStatus::Executing,
                        EntryStatus::Executed => ob_poc_types::RunSheetEntryStatus::Executed,
                        EntryStatus::Cancelled => ob_poc_types::RunSheetEntryStatus::Cancelled,
                        EntryStatus::Failed => ob_poc_types::RunSheetEntryStatus::Failed,
                        // Map Skipped to Failed since ob_poc_types doesn't have Skipped
                        EntryStatus::Skipped => ob_poc_types::RunSheetEntryStatus::Failed,
                    },
                    error: e.error.clone(),
                    affected_entities: e
                        .affected_entities
                        .iter()
                        .map(|id| id.to_string())
                        .collect(),
                    created_at: None,
                    executed_at: e.executed_at.map(|dt| dt.to_rfc3339()),
                    bindings: std::collections::HashMap::new(),
                })
                .collect(),
            cursor: self.cursor,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSheetEntry {
    pub id: Uuid,
    pub dsl_source: String,
    pub display_dsl: String,
    pub status: EntryStatus,
    pub created_at: DateTime<Utc>,
    pub executed_at: Option<DateTime<Utc>>,
    pub affected_entities: Vec<Uuid>,
    pub error: Option<String>,

    // === DAG Fields (for phased execution) ===
    /// Depth in the DAG (0 = no dependencies, 1 = depends on phase 0, etc.)
    #[serde(default)]
    pub dag_depth: u32,
    /// Entry IDs this entry depends on (must execute first)
    #[serde(default)]
    pub dependencies: Vec<Uuid>,
    /// Validation errors (blocking execution)
    #[serde(default)]
    pub validation_errors: Vec<ValidationError>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    #[default]
    Draft,
    Ready,
    Executing,
    Executed,
    Cancelled,
    Failed,
    /// Blocked by failed dependency
    Skipped,
}

/// Validation error for a run sheet entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    pub span: Option<(usize, usize)>,
}

impl EntryStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EntryStatus::Executed
                | EntryStatus::Cancelled
                | EntryStatus::Failed
                | EntryStatus::Skipped
        )
    }

    pub fn is_pending(&self) -> bool {
        matches!(
            self,
            EntryStatus::Draft | EntryStatus::Ready | EntryStatus::Executing
        )
    }

    pub fn is_success(&self) -> bool {
        matches!(self, EntryStatus::Executed)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, EntryStatus::Failed | EntryStatus::Skipped)
    }
}

/// State stack for navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateStack {
    snapshots: Vec<StateSnapshot>,
    position: usize,
    max_size: usize,
}

impl Default for StateStack {
    fn default() -> Self {
        Self::new(100)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub entity_scope: EntityScope,
    pub run_sheet_cursor: usize,
    pub action_label: String,
}

impl StateStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            position: 0,
            max_size: if max_size == 0 { 100 } else { max_size },
        }
    }

    pub fn push(&mut self, scope: EntityScope, cursor: usize, label: String) {
        // Truncate forward history
        if self.position + 1 < self.snapshots.len() {
            self.snapshots.truncate(self.position + 1);
        }

        self.snapshots.push(StateSnapshot {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            entity_scope: scope,
            run_sheet_cursor: cursor,
            action_label: label,
        });

        // Enforce max size
        while self.snapshots.len() > self.max_size {
            self.snapshots.remove(0);
        }

        self.position = self.snapshots.len().saturating_sub(1);
    }

    pub fn back(&mut self, n: usize) -> Option<&StateSnapshot> {
        if n == 0 || self.position == 0 {
            return None;
        }
        self.position = self.position.saturating_sub(n);
        self.snapshots.get(self.position)
    }

    pub fn forward(&mut self, n: usize) -> Option<&StateSnapshot> {
        let target = self.position + n;
        if target >= self.snapshots.len() {
            return None;
        }
        self.position = target;
        self.snapshots.get(self.position)
    }

    pub fn go_to_start(&mut self) -> Option<&StateSnapshot> {
        if self.snapshots.is_empty() {
            return None;
        }
        self.position = 0;
        self.snapshots.first()
    }

    pub fn go_to_last(&mut self) -> Option<&StateSnapshot> {
        if self.snapshots.is_empty() {
            return None;
        }
        self.position = self.snapshots.len() - 1;
        self.snapshots.last()
    }

    pub fn current(&self) -> Option<&StateSnapshot> {
        self.snapshots.get(self.position)
    }

    pub fn can_go_back(&self) -> bool {
        self.position > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.position + 1 < self.snapshots.len()
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn position(&self) -> usize {
        self.position
    }
}

/// View state for viewport sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewState {
    pub camera_x: f32,
    pub camera_y: f32,
    pub zoom: f32,
    pub expanded_nodes: HashSet<Uuid>,
    pub selected_nodes: HashSet<Uuid>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            camera_x: 0.0,
            camera_y: 0.0,
            zoom: 1.0,
            expanded_nodes: HashSet::new(),
            selected_nodes: HashSet::new(),
        }
    }
}

/// Inline resolution state (not a sub-session)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionState {
    /// Refs needing resolution
    pub refs: Vec<UnresolvedRef>,
    /// Current index being resolved
    pub current_index: usize,
    /// Resolutions made so far
    pub resolutions: HashMap<String, ResolvedRef>,
}

impl ResolutionState {
    /// Get current ref being resolved
    pub fn current(&self) -> Option<&UnresolvedRef> {
        self.refs.get(self.current_index)
    }

    /// Move to next ref (returns true if moved, false if at end)
    pub fn advance(&mut self) -> bool {
        if self.current_index + 1 < self.refs.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }

    /// Move to previous ref
    pub fn prev(&mut self) -> bool {
        if self.current_index > 0 {
            self.current_index -= 1;
            true
        } else {
            false
        }
    }

    /// Check if all refs are resolved
    pub fn is_complete(&self) -> bool {
        self.refs
            .iter()
            .all(|r| self.resolutions.contains_key(&r.ref_id))
    }

    /// Get unresolved count
    pub fn unresolved_count(&self) -> usize {
        self.refs.len() - self.resolutions.len()
    }

    /// Resolve current ref
    pub fn resolve_current(&mut self, resolved_key: String, display: String) {
        if let Some(current) = self.current() {
            let ref_id = current.ref_id.clone();
            self.resolutions.insert(
                ref_id.clone(),
                ResolvedRef {
                    ref_id,
                    resolved_key,
                    display,
                },
            );
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub ref_id: String,
    pub entity_type: String,
    pub search_value: String,
    pub context_line: String,
    pub search_keys: Vec<SearchKeyField>,
    pub discriminators: Vec<DiscriminatorField>,
    pub initial_matches: Vec<EntityMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchKeyField {
    pub name: String,
    pub label: String,
    pub value: Option<String>,
    pub is_primary: bool,
    pub field_type: FieldType,
    pub enum_values: Option<Vec<EnumValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscriminatorField {
    pub name: String,
    pub label: String,
    pub value: Option<String>,
    pub field_type: FieldType,
    pub enum_values: Option<Vec<EnumValue>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    Text,
    Enum,
    Date,
    Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub code: String,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub id: String,
    pub display: String,
    pub score: f32,
    pub details: Option<String>,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRef {
    pub ref_id: String,
    pub resolved_key: String,
    pub display: String,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    /// Intents extracted from this message (if user message processed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intents: Option<Vec<crate::api::intent::VerbIntent>>,
    /// DSL generated from this message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Agent,
    System,
}

/// Bound entity in symbol table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundEntity {
    pub id: Uuid,
    pub entity_type: String,
    pub display_name: String,
}

// =============================================================================
// Constraint Cascade Types
// =============================================================================

/// Reference to a client (constraint cascade level 1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRef {
    pub client_id: Uuid,
    pub display_name: String,
}

/// Structure type (constraint cascade level 2)
/// Operator sees: "PE", "SICAV", "Hedge Fund"
/// Internal: "private-equity", "sicav", "hedge"
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StructureType {
    #[default]
    Pe,
    Sicav,
    Hedge,
    Etf,
    Pension,
    Trust,
    Fof,
}

impl StructureType {
    /// Display label (what operator sees)
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Pe => "Private Equity",
            Self::Sicav => "SICAV",
            Self::Hedge => "Hedge Fund",
            Self::Etf => "ETF",
            Self::Pension => "Pension",
            Self::Trust => "Trust",
            Self::Fof => "Fund of Funds",
        }
    }

    /// Internal token (what DSL uses)
    pub fn internal_token(&self) -> &'static str {
        match self {
            Self::Pe => "private-equity",
            Self::Sicav => "sicav",
            Self::Hedge => "hedge",
            Self::Etf => "etf",
            Self::Pension => "pension",
            Self::Trust => "trust",
            Self::Fof => "fund-of-funds",
        }
    }

    /// Parse from internal token
    pub fn from_internal(token: &str) -> Option<Self> {
        match token {
            "private-equity" | "pe" => Some(Self::Pe),
            "sicav" => Some(Self::Sicav),
            "hedge" => Some(Self::Hedge),
            "etf" => Some(Self::Etf),
            "pension" => Some(Self::Pension),
            "trust" => Some(Self::Trust),
            "fund-of-funds" | "fof" => Some(Self::Fof),
            _ => None,
        }
    }
}

impl std::fmt::Display for StructureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

/// Reference to a structure (constraint cascade level 3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureRef {
    pub structure_id: Uuid,
    pub display_name: String,
    pub structure_type: StructureType,
}

/// Reference to a KYC case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseRef {
    pub case_id: Uuid,
    pub display_name: String,
}

/// Reference to a mandate (trading profile)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandateRef {
    pub mandate_id: Uuid,
    pub display_name: String,
}

/// Persona - filters available verbs by user role
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Persona {
    #[default]
    Ops,
    Kyc,
    Trading,
    Admin,
}

impl Persona {
    /// Get mode tags this persona can access
    pub fn mode_tags(&self) -> &'static [&'static str] {
        match self {
            Self::Ops => &["onboarding", "kyc", "trading"],
            Self::Kyc => &["kyc", "onboarding"],
            Self::Trading => &["trading"],
            Self::Admin => &["onboarding", "kyc", "trading", "admin"],
        }
    }
}

// =============================================================================
// DAG Navigation State
// =============================================================================

/// DAG state for prereq checking
/// Tracks which verbs have been completed and which state flags are set
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DagState {
    /// Completed verb FQNs (e.g., "structure.setup", "case.open")
    pub completed: HashSet<String>,
    /// State flags (e.g., "structure.exists" → true, "case.approved" → false)
    pub state_flags: HashMap<String, bool>,
    /// Fact predicates for more complex conditions
    pub facts: HashMap<String, serde_json::Value>,
}

impl DagState {
    /// Mark a verb as completed
    pub fn mark_completed(&mut self, verb_fqn: &str) {
        self.completed.insert(verb_fqn.to_string());
    }

    /// Check if a verb was completed
    pub fn is_completed(&self, verb_fqn: &str) -> bool {
        self.completed.contains(verb_fqn)
    }

    /// Set a state flag
    pub fn set_flag(&mut self, key: &str, value: bool) {
        self.state_flags.insert(key.to_string(), value);
    }

    /// Get a state flag (defaults to false)
    pub fn get_flag(&self, key: &str) -> bool {
        self.state_flags.get(key).copied().unwrap_or(false)
    }

    /// Set a fact predicate
    pub fn set_fact(&mut self, key: &str, value: serde_json::Value) {
        self.facts.insert(key.to_string(), value);
    }

    /// Get a fact predicate
    pub fn get_fact(&self, key: &str) -> Option<&serde_json::Value> {
        self.facts.get(key)
    }

    /// Clear all state (for session reset)
    pub fn clear(&mut self) {
        self.completed.clear();
        self.state_flags.clear();
        self.facts.clear();
    }
}

/// Search scope derived from constraint cascade
/// Used to narrow entity searches based on session context
#[derive(Debug, Clone, Default)]
pub struct SearchScope {
    pub client_id: Option<Uuid>,
    pub structure_type: Option<StructureType>,
    pub structure_id: Option<Uuid>,
}

impl SearchScope {
    /// Check if scope has any constraints set
    pub fn is_constrained(&self) -> bool {
        self.client_id.is_some() || self.structure_type.is_some() || self.structure_id.is_some()
    }
}

// =============================================================================
// CBU UNDO/REDO TYPES (migrated from CbuSession)
// =============================================================================

/// Snapshot of CBU set for undo/redo
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CbuSnapshot {
    /// CBU IDs at this point
    pub cbu_ids: HashSet<Uuid>,
    /// Action that led to this state
    pub action: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl CbuSnapshot {
    /// Create a snapshot of current CBU set
    pub fn capture(cbu_ids: &HashSet<Uuid>, action: &str) -> Self {
        Self {
            cbu_ids: cbu_ids.clone(),
            action: action.to_string(),
            timestamp: Utc::now(),
        }
    }
}

// =============================================================================
// REPL STATE MACHINE (migrated from CbuSession)
// =============================================================================

/// State machine for REPL session DSL execution pipeline.
///
/// Transitions:
/// ```text
/// Empty → Scoped → Templated → Generated → Parsed → Resolving → Ready → Executing → Executed
///   │        │          │           │          │          │                            │
///   └────────┴──────────┴───────────┴──────────┴──────────┴────────────────────────────┘
///                                   (reset on failure or restart)
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ReplState {
    /// No scope defined yet
    #[default]
    Empty,
    /// Scope is set (CBU set loaded)
    Scoped,
    /// Template DSL is set, awaiting confirmation
    Templated { confirmed: bool },
    /// DSL sheet generated from template × entity set
    Generated,
    /// Sheet parsed, symbols extracted, DAG computed
    Parsed,
    /// Resolving entity references
    Resolving { remaining: usize },
    /// All references resolved, ready for execution
    Ready,
    /// Execution in progress
    Executing { completed: usize, total: usize },
    /// Execution complete
    Executed { success: bool },
}

impl ReplState {
    /// Check if state allows setting scope
    pub fn can_set_scope(&self) -> bool {
        matches!(self, Self::Empty | Self::Scoped | Self::Executed { .. })
    }

    /// Check if state allows setting template
    pub fn can_set_template(&self) -> bool {
        matches!(self, Self::Scoped)
    }

    /// Check if state allows confirming intent
    pub fn can_confirm_intent(&self) -> bool {
        matches!(self, Self::Templated { confirmed: false })
    }

    /// Check if state allows generating sheet
    pub fn can_generate(&self) -> bool {
        matches!(self, Self::Templated { confirmed: true })
    }

    /// Check if state allows execution
    pub fn can_execute(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Check if state is terminal
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Executed { .. })
    }

    /// Get human-readable state name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Scoped => "scoped",
            Self::Templated { .. } => "templated",
            Self::Generated => "generated",
            Self::Parsed => "parsed",
            Self::Resolving { .. } => "resolving",
            Self::Ready => "ready",
            Self::Executing { .. } => "executing",
            Self::Executed { .. } => "executed",
        }
    }
}

impl std::fmt::Display for ReplState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty"),
            Self::Scoped => write!(f, "Scoped"),
            Self::Templated { confirmed } => write!(f, "Templated(confirmed={})", confirmed),
            Self::Generated => write!(f, "Generated"),
            Self::Parsed => write!(f, "Parsed"),
            Self::Resolving { remaining } => write!(f, "Resolving({} remaining)", remaining),
            Self::Ready => write!(f, "Ready"),
            Self::Executing { completed, total } => write!(f, "Executing({}/{})", completed, total),
            Self::Executed { success } => write!(f, "Executed(success={})", success),
        }
    }
}

// =============================================================================
// SESSION STATE MACHINE (migrated from AgentSession)
// =============================================================================

/// Session lifecycle states (migrated from AgentSession)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Just created, awaiting scope selection
    #[default]
    New,
    /// Scope is set, ready for operations
    Scoped,
    /// Has pending intents awaiting validation
    PendingValidation,
    /// Intents validated, DSL assembled, ready to execute
    ReadyToExecute,
    /// Execution in progress
    Executing,
    /// Execution complete (success or partial)
    Executed,
    /// Session ended
    Closed,
}

/// Event that triggers a session state transition
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Scope has been set (CBUs loaded via session.load-*)
    ScopeSet,
    /// DSL is pending validation (unresolved refs, needs user input)
    DslPendingValidation,
    /// DSL is validated and ready to execute
    DslReady,
    /// Execution started
    ExecutionStarted,
    /// Execution completed (success or failure)
    ExecutionCompleted,
    /// User cancelled pending operation
    Cancelled,
    /// Session closed
    Close,
}

// =============================================================================
// SUB-SESSION TYPES (migrated from AgentSession)
// =============================================================================

/// Sub-session type - determines the purpose and behavior of a child session
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubSessionType {
    /// Root session - full agent capabilities (not a sub-session)
    #[default]
    Root,
    /// Resolution sub-session - entity disambiguation workflow
    Resolution(ResolutionSubSession),
    /// Research sub-session - GLEIF/UBO discovery
    Research(ResearchSubSession),
    /// Review sub-session - DSL review before execute
    Review(ReviewSubSession),
    /// Correction sub-session - fix screening hits
    Correction(CorrectionSubSession),
}

/// Resolution sub-session state - entity disambiguation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResolutionSubSession {
    /// Unresolved refs to work through
    pub unresolved_refs: Vec<UnresolvedRefInfo>,
    /// Which DSL statement index triggered this (in parent)
    pub parent_dsl_index: usize,
    /// Current ref being resolved (index into unresolved_refs)
    pub current_ref_index: usize,
    /// Resolutions made so far: ref_id -> resolved_key
    pub resolutions: HashMap<String, String>,
}

/// Info about an unresolved entity reference (full metadata for resolution UI)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnresolvedRefInfo {
    /// Unique ID for this ref (stmt_idx:arg_name)
    pub ref_id: String,
    /// Entity type (e.g., "entity", "cbu", "person")
    pub entity_type: String,
    /// The search value from DSL (e.g., "John Smith")
    pub search_value: String,
    /// DSL context line for display
    pub context_line: String,
    /// Initial search matches (pre-fetched)
    pub initial_matches: Vec<EntityMatchInfo>,
    /// Resolved primary key (UUID or code)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_key: Option<String>,
    /// Display name of resolved entity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_display: Option<String>,
}

/// Entity match info for resolution UI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityMatchInfo {
    /// Primary key (UUID or code)
    pub value: String,
    /// Display name
    pub display: String,
    /// Additional detail (jurisdiction, DOB, etc.)
    pub detail: Option<String>,
    /// Match score as integer percentage (0-100)
    pub score_pct: u8,
}

impl ResolutionSubSession {
    /// Create a new empty resolution sub-session
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from AST statements - extracts unresolved entity refs
    pub fn from_statements(statements: &[crate::dsl_v2::Statement]) -> Self {
        use crate::dsl_v2::Statement;

        let mut unresolved_refs = Vec::new();

        for (stmt_idx, stmt) in statements.iter().enumerate() {
            if let Statement::VerbCall(vc) = stmt {
                for arg in &vc.arguments {
                    Self::collect_entity_refs_from_node(
                        &arg.value,
                        stmt_idx,
                        &arg.key,
                        &mut unresolved_refs,
                    );
                }
            }
        }

        Self {
            unresolved_refs,
            parent_dsl_index: 0,
            current_ref_index: 0,
            resolutions: HashMap::new(),
        }
    }

    /// Recursively collect entity refs from AST node
    fn collect_entity_refs_from_node(
        node: &crate::dsl_v2::ast::AstNode,
        stmt_idx: usize,
        arg_name: &str,
        refs: &mut Vec<UnresolvedRefInfo>,
    ) {
        use crate::dsl_v2::ast::AstNode;

        match node {
            AstNode::EntityRef {
                entity_type,
                value,
                resolved_key,
                ..
            } => {
                // Only add if not yet resolved
                if resolved_key.is_none() {
                    let ref_id = format!("{}:{}", stmt_idx, arg_name);
                    refs.push(UnresolvedRefInfo {
                        ref_id,
                        entity_type: entity_type.clone(),
                        search_value: value.clone(),
                        context_line: format!(":{} <{}>", arg_name, value),
                        initial_matches: vec![],
                        resolved_key: None,
                        resolved_display: None,
                    });
                }
            }
            AstNode::List { items, .. } => {
                for (i, item) in items.iter().enumerate() {
                    let list_arg_name = format!("{}[{}]", arg_name, i);
                    Self::collect_entity_refs_from_node(item, stmt_idx, &list_arg_name, refs);
                }
            }
            AstNode::Map { entries, .. } => {
                for (key, val) in entries {
                    let map_arg_name = format!("{}.{}", arg_name, key);
                    Self::collect_entity_refs_from_node(val, stmt_idx, &map_arg_name, refs);
                }
            }
            AstNode::Nested(vc) => {
                for nested_arg in &vc.arguments {
                    let nested_arg_name = format!("{}.{}", arg_name, nested_arg.key);
                    Self::collect_entity_refs_from_node(
                        &nested_arg.value,
                        stmt_idx,
                        &nested_arg_name,
                        refs,
                    );
                }
            }
            AstNode::Literal(_) | AstNode::SymbolRef { .. } => {}
        }
    }

    /// Get resolution progress as (resolved, total)
    pub fn progress(&self) -> (usize, usize) {
        let resolved = self.resolutions.len();
        let total = self.unresolved_refs.len();
        (resolved, total)
    }

    /// Select a resolution for a ref_id
    pub fn select(&mut self, ref_id: &str, resolved_key: &str) -> Result<(), String> {
        if !self.unresolved_refs.iter().any(|r| r.ref_id == ref_id) {
            return Err(format!("Unknown ref_id: {}", ref_id));
        }
        self.resolutions
            .insert(ref_id.to_string(), resolved_key.to_string());
        Ok(())
    }

    /// Check if all refs have been resolved
    pub fn is_complete(&self) -> bool {
        self.unresolved_refs
            .iter()
            .all(|r| self.resolutions.contains_key(&r.ref_id))
    }

    /// Get the current unresolved ref being worked on
    pub fn current_ref(&self) -> Option<&UnresolvedRefInfo> {
        self.unresolved_refs.get(self.current_ref_index)
    }

    /// Move to next unresolved ref
    pub fn next_ref(&mut self) -> bool {
        if self.current_ref_index + 1 < self.unresolved_refs.len() {
            self.current_ref_index += 1;
            true
        } else {
            false
        }
    }

    /// Apply resolutions to AST statements
    ///
    /// For each resolution in self.resolutions, find the corresponding EntityRef
    /// in the statements and set its resolved_key.
    pub fn apply_to_statements(
        &self,
        statements: &mut [crate::dsl_v2::Statement],
    ) -> Result<(), String> {
        use crate::dsl_v2::Statement;

        for (stmt_idx, stmt) in statements.iter_mut().enumerate() {
            if let Statement::VerbCall(vc) = stmt {
                for arg in &mut vc.arguments {
                    self.apply_to_node(&mut arg.value, stmt_idx, &arg.key)?;
                }
            }
        }
        Ok(())
    }

    /// Recursively apply resolutions to an AST node
    fn apply_to_node(
        &self,
        node: &mut crate::dsl_v2::ast::AstNode,
        stmt_idx: usize,
        arg_name: &str,
    ) -> Result<(), String> {
        use crate::dsl_v2::ast::AstNode;

        match node {
            AstNode::EntityRef { resolved_key, .. } => {
                let ref_id = format!("{}:{}", stmt_idx, arg_name);
                if let Some(resolved) = self.resolutions.get(&ref_id) {
                    *resolved_key = Some(resolved.clone());
                }
            }
            AstNode::List { items, .. } => {
                for (i, item) in items.iter_mut().enumerate() {
                    let list_arg_name = format!("{}[{}]", arg_name, i);
                    self.apply_to_node(item, stmt_idx, &list_arg_name)?;
                }
            }
            AstNode::Map { entries, .. } => {
                for (key, val) in entries {
                    let map_arg_name = format!("{}.{}", arg_name, key);
                    self.apply_to_node(val, stmt_idx, &map_arg_name)?;
                }
            }
            AstNode::Nested(vc) => {
                for nested_arg in &mut vc.arguments {
                    let nested_arg_name = format!("{}.{}", arg_name, nested_arg.key);
                    self.apply_to_node(&mut nested_arg.value, stmt_idx, &nested_arg_name)?;
                }
            }
            AstNode::Literal(_) | AstNode::SymbolRef { .. } => {}
        }
        Ok(())
    }
}

/// Research sub-session state - GLEIF/UBO discovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResearchSubSession {
    /// Target entity being researched (if known)
    pub target_entity_id: Option<Uuid>,
    /// Research type (gleif, ubo, companies_house, etc.)
    pub research_type: String,
    /// Search query used
    pub search_query: Option<String>,
}

/// Review sub-session state - DSL review before execute
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReviewSubSession {
    /// The DSL pending review
    pub pending_dsl: String,
    /// Review status
    pub review_status: ReviewStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
    Modified,
}

/// Correction sub-session state - fix screening hits
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CorrectionSubSession {
    /// Entity with screening hit
    pub entity_id: Option<Uuid>,
    /// Screening hit ID
    pub hit_id: Option<Uuid>,
    /// Correction type being applied
    pub correction_type: Option<String>,
}

/// Prereq condition for DAG navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PrereqCondition {
    /// A specific verb must have been completed
    VerbCompleted { verb: String },
    /// Any of the listed verbs must have been completed
    AnyOf { verbs: Vec<String> },
    /// A state flag must exist (and be true)
    StateExists { key: String },
    /// A fact predicate must exist
    FactExists { predicate: String },
}

impl PrereqCondition {
    /// Check if this prereq is satisfied
    pub fn is_satisfied(&self, dag_state: &DagState) -> bool {
        match self {
            Self::VerbCompleted { verb } => dag_state.is_completed(verb),
            Self::AnyOf { verbs } => verbs.iter().any(|v| dag_state.is_completed(v)),
            Self::StateExists { key } => dag_state.get_flag(key),
            Self::FactExists { predicate } => dag_state.get_fact(predicate).is_some(),
        }
    }
}

/// Canonical prereq/state keys (enforced by lint)
pub mod canonical_keys {
    /// State keys that indicate something exists/is selected
    pub const STATE_KEYS: &[&str] = &[
        "structure.selected",
        "structure.exists",
        "case.selected",
        "case.exists",
        "mandate.selected",
    ];

    /// Completion keys for verb execution tracking
    pub const COMPLETION_KEYS: &[&str] = &[
        "structure.created",
        "case.opened",
        "case.submitted",
        "case.approved",
        "mandate.created",
    ];
}

impl Default for UnifiedSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Max history depth for CBU undo/redo
const MAX_CBU_HISTORY: usize = 50;

impl UnifiedSession {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id: Uuid::nil(),
            created_at: now,
            updated_at: now,
            target_universe: None,
            entity_scope: EntityScope::default(),
            run_sheet: RunSheet::default(),
            state_stack: StateStack::new(100),
            view_state: ViewState::default(),
            resolution: None,
            messages: Vec::new(),
            bindings: HashMap::new(),
            // Constraint cascade (starts empty, narrowed as user works)
            client: None,
            structure_type: None,
            current_structure: None,
            current_case: None,
            current_mandate: None,
            // Persona (defaults to Ops)
            persona: Persona::default(),
            // DAG state (tracks verb completions)
            dag_state: DagState::default(),
            // CBU undo/redo history
            cbu_history: Vec::new(),
            cbu_future: Vec::new(),
            // REPL state machine
            repl_state: ReplState::Empty,
            scope_dsl: Vec::new(),
            template_dsl: None,
            target_entity_type: None,
            intent_confirmed: false,
            // Persistence
            dirty: false,
            name: None,
            // Agent session fields (defaults)
            entity_type: "cbu".to_string(),
            entity_id: None,
            state: SessionState::New,
            parent_session_id: None,
            sub_session_type: SubSessionType::Root,
            inherited_symbols: HashMap::new(),
            pending_intents: Vec::new(),
            domain_hint: None,
            // Session context (backward compatibility)
            context: crate::api::session::SessionContext::default(),
        }
    }

    /// Create with specific user
    pub fn for_user(user_id: Uuid) -> Self {
        let mut session = Self::new();
        session.user_id = user_id;
        session
    }

    /// Create a new session for an entity (AgentSession compatibility)
    pub fn new_for_entity(
        user_id: Option<Uuid>,
        entity_type: &str,
        entity_id: Option<Uuid>,
        domain_hint: Option<String>,
    ) -> Self {
        let mut session = Self::new();
        session.user_id = user_id.unwrap_or(Uuid::nil());
        session.entity_type = entity_type.to_string();
        session.entity_id = entity_id;
        session.domain_hint = domain_hint;
        if let Some(id) = entity_id {
            session.id = id; // Session ID follows entity ID for backward compat
        }
        session
    }

    /// Create a sub-session inheriting context from parent
    pub fn new_subsession(parent: &Self, sub_session_type: SubSessionType) -> Self {
        let mut session = Self::new();
        session.user_id = parent.user_id;
        session.entity_type = parent.entity_type.clone();
        session.entity_id = parent.entity_id;
        session.parent_session_id = Some(parent.id);
        session.sub_session_type = sub_session_type;
        session.inherited_symbols = parent.bindings.clone();
        session.domain_hint = parent.domain_hint.clone();
        // Inherit cascade context
        session.client = parent.client.clone();
        session.structure_type = parent.structure_type;
        session.current_structure = parent.current_structure.clone();
        session.current_case = parent.current_case.clone();
        session.current_mandate = parent.current_mandate.clone();
        session
    }

    /// Check if this is a sub-session
    pub fn is_subsession(&self) -> bool {
        self.parent_session_id.is_some()
    }

    /// Get the sub-session type if this is a resolution session
    pub fn as_resolution(&self) -> Option<&ResolutionSubSession> {
        match &self.sub_session_type {
            SubSessionType::Resolution(r) => Some(r),
            _ => None,
        }
    }

    /// Get mutable resolution sub-session state
    pub fn as_resolution_mut(&mut self) -> Option<&mut ResolutionSubSession> {
        match &mut self.sub_session_type {
            SubSessionType::Resolution(r) => Some(r),
            _ => None,
        }
    }

    /// Set entity ID after creation (e.g., after cbu.ensure executes)
    pub fn set_entity_id(&mut self, entity_id: Uuid) {
        self.entity_id = Some(entity_id);
        self.id = entity_id; // Session ID follows entity ID
        self.updated_at = Utc::now();
    }

    /// Transition session state based on an event (state machine)
    pub fn transition(&mut self, event: SessionEvent) {
        use SessionEvent::*;
        use SessionState::*;

        let new_state = match (&self.state, event) {
            // Scope setting
            (New, ScopeSet) => Scoped,
            (Scoped, ScopeSet) => Scoped,
            (Executed, ScopeSet) => Scoped,

            // DSL pending validation
            (New, DslPendingValidation) => PendingValidation,
            (Scoped, DslPendingValidation) => PendingValidation,
            (Executed, DslPendingValidation) => PendingValidation,

            // DSL ready
            (New, DslReady) => ReadyToExecute,
            (Scoped, DslReady) => ReadyToExecute,
            (PendingValidation, DslReady) => ReadyToExecute,
            (Executed, DslReady) => ReadyToExecute,

            // Execution
            (ReadyToExecute, ExecutionStarted) => Executing,
            (Executing, ExecutionCompleted) => self.compute_post_execution_state(),

            // Cancellation
            (New, Cancelled) => New,
            (Scoped, Cancelled) => Scoped,
            (PendingValidation, Cancelled) => self.compute_idle_state(),
            (ReadyToExecute, Cancelled) => self.compute_idle_state(),
            (Executed, Cancelled) => self.compute_idle_state(),

            // Close
            (_, Close) => Closed,

            // Invalid - keep current state
            (current, _event) => {
                tracing::warn!("Invalid session state transition: {:?}", current);
                return;
            }
        };

        self.state = new_state;
        self.updated_at = Utc::now();
    }

    fn compute_idle_state(&self) -> SessionState {
        if self.has_scope_set() {
            SessionState::Scoped
        } else {
            SessionState::New
        }
    }

    fn compute_post_execution_state(&self) -> SessionState {
        if self.has_scope_set() {
            SessionState::Scoped
        } else {
            SessionState::Executed
        }
    }

    /// Check if session has scope set (CBUs loaded)
    pub fn has_scope_set(&self) -> bool {
        !self.entity_scope.cbu_ids.is_empty()
    }

    /// Get all known symbols for validation (own bindings + inherited from parent)
    pub fn all_known_symbols(&self) -> HashMap<String, Uuid> {
        let mut symbols = HashMap::new();
        for (name, bound) in &self.inherited_symbols {
            symbols.insert(name.clone(), bound.id);
        }
        for (name, bound) in &self.bindings {
            symbols.insert(name.clone(), bound.id);
        }
        symbols
    }

    /// Add intents and transition state
    pub fn add_intents(&mut self, intents: Vec<crate::api::intent::VerbIntent>) {
        self.pending_intents.extend(intents);
        self.transition(SessionEvent::DslPendingValidation);
    }

    /// Clear pending intents
    pub fn clear_pending_intents(&mut self) {
        self.pending_intents.clear();
    }

    // =========================================================================
    // AgentSession Compatibility Methods
    // =========================================================================

    /// Set pending DSL (parsed, validated, and planned - ready for user confirmation)
    /// This mirrors AgentSession.set_pending_dsl for backward compatibility
    pub fn set_pending_dsl(
        &mut self,
        source: String,
        ast: Vec<crate::dsl_v2::ast::Statement>,
        _plan: Option<crate::dsl_v2::execution_plan::ExecutionPlan>,
        _was_reordered: bool,
    ) {
        // Add to run sheet as draft entry
        self.add_dsl(source, "".to_string());
        // Store AST in context for execution
        self.context.ast = ast;
        self.transition(SessionEvent::DslReady);
    }

    /// Cancel pending DSL (user declined)
    pub fn cancel_pending(&mut self) {
        // Remove last draft entry from run sheet
        if let Some(idx) = self
            .run_sheet
            .entries
            .iter()
            .rposition(|e| e.status == EntryStatus::Draft)
        {
            self.run_sheet.entries.remove(idx);
        }
        self.transition(SessionEvent::Cancelled);
    }

    /// Check if there's runnable DSL
    pub fn has_pending(&self) -> bool {
        self.run_sheet
            .entries
            .iter()
            .any(|e| matches!(e.status, EntryStatus::Draft | EntryStatus::Ready))
    }

    /// Check if session can execute
    pub fn can_execute(&self) -> bool {
        self.state == SessionState::ReadyToExecute && self.has_pending()
    }

    /// Get combined DSL source (for backward compat)
    pub fn combined_dsl(&self) -> String {
        self.run_sheet
            .entries
            .iter()
            .map(|e| e.dsl_source.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Record execution results and update context
    pub fn record_execution(&mut self, results: Vec<crate::api::session::ExecutionResult>) {
        // Update context with created entities
        for result in &results {
            if result.success {
                if let Some(id) = result.entity_id {
                    match result.entity_type.as_deref() {
                        Some("CBU") | Some("cbu") => {
                            self.context.last_cbu_id = Some(id);
                            self.context.cbu_ids.push(id);
                        }
                        Some(_) => {
                            self.context.last_entity_id = Some(id);
                            self.context.entity_ids.push(id);
                        }
                        None => {}
                    }
                }
            }
        }
        self.transition(SessionEvent::ExecutionCompleted);
        self.updated_at = Utc::now();
    }

    /// Set target universe (first user declaration)
    pub fn set_target_universe(&mut self, definition: UniverseDefinition, description: String) {
        self.target_universe = Some(TargetUniverse {
            description,
            definition,
            declared_at: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Add CBU to scope
    pub fn add_cbu(&mut self, cbu_id: Uuid) {
        self.entity_scope.cbu_ids.insert(cbu_id);
        self.updated_at = Utc::now();
    }

    /// Remove CBU from scope
    pub fn remove_cbu(&mut self, cbu_id: &Uuid) -> bool {
        let removed = self.entity_scope.cbu_ids.remove(cbu_id);
        if removed {
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Clear all CBUs from scope
    pub fn clear_cbus(&mut self) {
        self.entity_scope.cbu_ids.clear();
        self.updated_at = Utc::now();
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, level: ZoomLevel, focal_entity: Option<Uuid>) {
        self.entity_scope.zoom_level = level;
        self.entity_scope.focal_entity_id = focal_entity;
        self.updated_at = Utc::now();
    }

    /// Add DSL entry to run sheet
    pub fn add_dsl(&mut self, dsl_source: String, display_dsl: String) -> Uuid {
        let id = Uuid::new_v4();
        self.run_sheet.entries.push(RunSheetEntry {
            id,
            dsl_source,
            display_dsl,
            status: EntryStatus::Draft,
            created_at: Utc::now(),
            executed_at: None,
            affected_entities: Vec::new(),
            error: None,
            dag_depth: 0,
            dependencies: Vec::new(),
            validation_errors: Vec::new(),
        });
        self.run_sheet.cursor = self.run_sheet.entries.len() - 1;
        self.updated_at = Utc::now();
        id
    }

    /// Add DSL entry with DAG metadata
    pub fn add_dsl_with_dag(
        &mut self,
        dsl_source: String,
        display_dsl: String,
        dag_depth: u32,
        dependencies: Vec<Uuid>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.run_sheet.entries.push(RunSheetEntry {
            id,
            dsl_source,
            display_dsl,
            status: EntryStatus::Draft,
            created_at: Utc::now(),
            executed_at: None,
            affected_entities: Vec::new(),
            error: None,
            dag_depth,
            dependencies,
            validation_errors: Vec::new(),
        });
        self.run_sheet.cursor = self.run_sheet.entries.len() - 1;
        self.updated_at = Utc::now();
        id
    }

    /// Mark run sheet entry as executed
    pub fn mark_executed(&mut self, entry_id: Uuid, affected: Vec<Uuid>) -> bool {
        if let Some(entry) = self.run_sheet.entries.iter_mut().find(|e| e.id == entry_id) {
            entry.status = EntryStatus::Executed;
            entry.executed_at = Some(Utc::now());
            entry.affected_entities = affected;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Mark run sheet entry as failed
    pub fn mark_failed(&mut self, entry_id: Uuid, error: String) -> bool {
        if let Some(entry) = self.run_sheet.entries.iter_mut().find(|e| e.id == entry_id) {
            entry.status = EntryStatus::Failed;
            entry.error = Some(error);
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Push current state to history
    pub fn push_state(&mut self, action_label: &str) {
        self.state_stack.push(
            self.entity_scope.clone(),
            self.run_sheet.cursor,
            action_label.to_string(),
        );
    }

    /// Navigate back
    pub fn back(&mut self, n: usize) -> bool {
        if let Some(snapshot) = self.state_stack.back(n) {
            self.entity_scope = snapshot.entity_scope.clone();
            self.run_sheet.cursor = snapshot.run_sheet_cursor;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Navigate forward
    pub fn forward(&mut self, n: usize) -> bool {
        if let Some(snapshot) = self.state_stack.forward(n) {
            self.entity_scope = snapshot.entity_scope.clone();
            self.run_sheet.cursor = snapshot.run_sheet_cursor;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Go to start of history
    pub fn go_to_start(&mut self) -> bool {
        if let Some(snapshot) = self.state_stack.go_to_start() {
            self.entity_scope = snapshot.entity_scope.clone();
            self.run_sheet.cursor = snapshot.run_sheet_cursor;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Go to end of history
    pub fn go_to_last(&mut self) -> bool {
        if let Some(snapshot) = self.state_stack.go_to_last() {
            self.entity_scope = snapshot.entity_scope.clone();
            self.run_sheet.cursor = snapshot.run_sheet_cursor;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Start resolution workflow
    pub fn start_resolution(&mut self, refs: Vec<UnresolvedRef>) {
        self.resolution = Some(ResolutionState {
            refs,
            current_index: 0,
            resolutions: HashMap::new(),
        });
        self.updated_at = Utc::now();
    }

    /// Complete resolution and return the resolutions map
    pub fn complete_resolution(&mut self) -> Option<HashMap<String, ResolvedRef>> {
        self.resolution.take().map(|r| {
            self.updated_at = Utc::now();
            r.resolutions
        })
    }

    /// Cancel resolution
    pub fn cancel_resolution(&mut self) {
        if self.resolution.take().is_some() {
            self.updated_at = Utc::now();
        }
    }

    /// Check if resolution is active
    pub fn has_active_resolution(&self) -> bool {
        self.resolution.is_some()
    }

    /// Add user message
    pub fn add_user_message(&mut self, content: String) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::User,
            content,
            timestamp: Utc::now(),
            intents: None,
            dsl: None,
        });
        self.updated_at = Utc::now();
        id
    }

    /// Add agent message with optional intents and DSL
    /// This matches AgentSession.add_agent_message signature for backward compatibility
    pub fn add_agent_message(
        &mut self,
        content: String,
        intents: Option<Vec<crate::api::intent::VerbIntent>>,
        dsl: Option<String>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::Agent,
            content,
            timestamp: Utc::now(),
            intents,
            dsl,
        });
        self.updated_at = Utc::now();
        id
    }

    /// Add system message
    pub fn add_system_message(&mut self, content: String) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::System,
            content,
            timestamp: Utc::now(),
            intents: None,
            dsl: None,
        });
        self.updated_at = Utc::now();
        id
    }

    /// Set binding
    pub fn set_binding(&mut self, name: &str, entity: BoundEntity) {
        self.bindings.insert(name.to_string(), entity);
        self.updated_at = Utc::now();
    }

    /// Get binding
    pub fn get_binding(&self, name: &str) -> Option<&BoundEntity> {
        self.bindings.get(name)
    }

    /// Remove binding
    pub fn remove_binding(&mut self, name: &str) -> Option<BoundEntity> {
        let removed = self.bindings.remove(name);
        if removed.is_some() {
            self.updated_at = Utc::now();
        }
        removed
    }

    // =========================================================================
    // Constraint Cascade Methods
    // =========================================================================

    /// Set client context (constraint level 1)
    pub fn set_client(&mut self, client_id: Uuid, display_name: String) {
        self.client = Some(ClientRef {
            client_id,
            display_name,
        });
        self.updated_at = Utc::now();
    }

    /// Clear client context (resets entire cascade)
    pub fn clear_client(&mut self) {
        self.client = None;
        self.structure_type = None;
        self.current_structure = None;
        self.current_case = None;
        self.updated_at = Utc::now();
    }

    /// Set structure type (constraint level 2)
    pub fn set_structure_type(&mut self, structure_type: StructureType) {
        self.structure_type = Some(structure_type);
        // Clearing structure type should clear deeper levels
        self.current_structure = None;
        self.current_case = None;
        self.updated_at = Utc::now();
    }

    /// Set current structure (constraint level 3)
    pub fn set_current_structure(
        &mut self,
        structure_id: Uuid,
        display_name: String,
        structure_type: StructureType,
    ) {
        self.current_structure = Some(StructureRef {
            structure_id,
            display_name,
            structure_type,
        });
        // Also set the structure type if not already set
        if self.structure_type.is_none() {
            self.structure_type = Some(structure_type);
        }
        self.updated_at = Utc::now();
    }

    /// Set current case
    pub fn set_current_case(&mut self, case_id: Uuid, display_name: String) {
        self.current_case = Some(CaseRef {
            case_id,
            display_name,
        });
        self.updated_at = Utc::now();
    }

    /// Clear current case
    pub fn clear_current_case(&mut self) {
        if self.current_case.is_some() {
            self.current_case = None;
            self.updated_at = Utc::now();
        }
    }

    /// Set current mandate (trading profile)
    pub fn set_current_mandate(&mut self, mandate_id: Uuid, display_name: String) {
        self.current_mandate = Some(MandateRef {
            mandate_id,
            display_name,
        });
        self.updated_at = Utc::now();
    }

    /// Clear current mandate
    pub fn clear_current_mandate(&mut self) {
        if self.current_mandate.is_some() {
            self.current_mandate = None;
            self.updated_at = Utc::now();
        }
    }

    /// Set persona
    pub fn set_persona(&mut self, persona: Persona) {
        self.persona = persona;
        self.updated_at = Utc::now();
    }

    /// Get search scope derived from cascade context
    pub fn derive_search_scope(&self) -> SearchScope {
        SearchScope {
            client_id: self.client.as_ref().map(|c| c.client_id),
            structure_type: self.structure_type,
            structure_id: self.current_structure.as_ref().map(|s| s.structure_id),
        }
    }

    // =========================================================================
    // DAG State Methods
    // =========================================================================

    /// Mark a verb as completed and update DAG state
    pub fn mark_verb_completed(&mut self, verb_fqn: &str) {
        self.dag_state.mark_completed(verb_fqn);
        self.updated_at = Utc::now();
    }

    /// Set a DAG state flag
    pub fn set_dag_flag(&mut self, key: &str, value: bool) {
        self.dag_state.set_flag(key, value);
        self.updated_at = Utc::now();
    }

    /// Check if prereqs are satisfied for a verb
    pub fn check_prereqs(&self, prereqs: &[PrereqCondition]) -> bool {
        prereqs
            .iter()
            .all(|prereq| prereq.is_satisfied(&self.dag_state))
    }

    /// Reset DAG state (for session reset)
    pub fn reset_dag_state(&mut self) {
        self.dag_state.clear();
        self.updated_at = Utc::now();
    }

    /// Update view state
    pub fn update_view(&mut self, camera_x: f32, camera_y: f32, zoom: f32) {
        self.view_state.camera_x = camera_x;
        self.view_state.camera_y = camera_y;
        self.view_state.zoom = zoom;
        self.updated_at = Utc::now();
    }

    /// Select node
    pub fn select_node(&mut self, node_id: Uuid) {
        self.view_state.selected_nodes.insert(node_id);
        self.updated_at = Utc::now();
    }

    /// Deselect node
    pub fn deselect_node(&mut self, node_id: &Uuid) {
        if self.view_state.selected_nodes.remove(node_id) {
            self.updated_at = Utc::now();
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        if !self.view_state.selected_nodes.is_empty() {
            self.view_state.selected_nodes.clear();
            self.updated_at = Utc::now();
        }
    }

    /// Expand node
    pub fn expand_node(&mut self, node_id: Uuid) {
        self.view_state.expanded_nodes.insert(node_id);
        self.updated_at = Utc::now();
    }

    /// Collapse node
    pub fn collapse_node(&mut self, node_id: &Uuid) {
        if self.view_state.expanded_nodes.remove(node_id) {
            self.updated_at = Utc::now();
        }
    }

    // =========================================================================
    // CBU UNDO/REDO (migrated from CbuSession)
    // =========================================================================

    /// Push current CBU state to history before mutation
    fn push_cbu_history(&mut self, action: &str) {
        self.cbu_history
            .push(CbuSnapshot::capture(&self.entity_scope.cbu_ids, action));

        // Trim history if too long
        if self.cbu_history.len() > MAX_CBU_HISTORY {
            self.cbu_history.remove(0);
        }

        // Clear redo stack on new action
        self.cbu_future.clear();
        self.dirty = true;
    }

    /// Load a single CBU with undo support
    /// Returns true if CBU was newly added
    pub fn load_cbu(&mut self, cbu_id: Uuid) -> bool {
        if self.entity_scope.cbu_ids.contains(&cbu_id) {
            return false;
        }
        self.push_cbu_history("load_cbu");
        self.entity_scope.cbu_ids.insert(cbu_id);
        self.updated_at = Utc::now();
        true
    }

    /// Load multiple CBUs with undo support
    /// Returns count of newly added CBUs
    pub fn load_cbus(&mut self, ids: impl IntoIterator<Item = Uuid>) -> usize {
        let new_ids: Vec<Uuid> = ids
            .into_iter()
            .filter(|id| !self.entity_scope.cbu_ids.contains(id))
            .collect();

        if new_ids.is_empty() {
            return 0;
        }

        self.push_cbu_history("load_cbus");
        let count = new_ids.len();
        self.entity_scope.cbu_ids.extend(new_ids);
        self.updated_at = Utc::now();
        count
    }

    /// Unload a CBU with undo support
    /// Returns true if CBU was present and removed
    pub fn unload_cbu(&mut self, cbu_id: Uuid) -> bool {
        if !self.entity_scope.cbu_ids.contains(&cbu_id) {
            return false;
        }
        self.push_cbu_history("unload_cbu");
        self.entity_scope.cbu_ids.remove(&cbu_id);
        self.updated_at = Utc::now();
        true
    }

    /// Clear all CBUs with undo support
    /// Returns count of removed CBUs
    pub fn clear_cbus_with_history(&mut self) -> usize {
        if self.entity_scope.cbu_ids.is_empty() {
            return 0;
        }
        self.push_cbu_history("clear");
        let count = self.entity_scope.cbu_ids.len();
        self.entity_scope.cbu_ids.clear();
        self.updated_at = Utc::now();
        count
    }

    /// Undo last CBU action
    pub fn undo_cbu(&mut self) -> bool {
        if let Some(prev) = self.cbu_history.pop() {
            self.cbu_future
                .push(CbuSnapshot::capture(&self.entity_scope.cbu_ids, "undo"));
            self.entity_scope.cbu_ids = prev.cbu_ids;
            self.dirty = true;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Redo previously undone CBU action
    pub fn redo_cbu(&mut self) -> bool {
        if let Some(next) = self.cbu_future.pop() {
            self.cbu_history
                .push(CbuSnapshot::capture(&self.entity_scope.cbu_ids, "redo"));
            self.entity_scope.cbu_ids = next.cbu_ids;
            self.dirty = true;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Check if undo is available
    pub fn can_undo_cbu(&self) -> bool {
        !self.cbu_history.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo_cbu(&self) -> bool {
        !self.cbu_future.is_empty()
    }

    /// Get CBU history depth
    pub fn cbu_history_depth(&self) -> usize {
        self.cbu_history.len()
    }

    /// Get CBU future depth
    pub fn cbu_future_depth(&self) -> usize {
        self.cbu_future.len()
    }

    /// Get count of loaded CBUs
    pub fn cbu_count(&self) -> usize {
        self.entity_scope.cbu_ids.len()
    }

    /// Check if a CBU is loaded
    pub fn contains_cbu(&self, cbu_id: &Uuid) -> bool {
        self.entity_scope.cbu_ids.contains(cbu_id)
    }

    /// Get all CBU IDs as Vec (for SQL queries)
    pub fn cbu_ids_vec(&self) -> Vec<Uuid> {
        self.entity_scope.cbu_ids.iter().copied().collect()
    }

    // =========================================================================
    // REPL STATE MACHINE (migrated from CbuSession)
    // =========================================================================

    /// Set scope from DSL commands (transition: Empty/Scoped/Executed → Scoped)
    pub fn set_repl_scope(&mut self, scope_dsl: Vec<String>) -> Result<(), String> {
        if !self.repl_state.can_set_scope() {
            return Err(format!(
                "Cannot set scope in state '{}'. Must be Empty, Scoped, or Executed.",
                self.repl_state
            ));
        }
        self.scope_dsl = scope_dsl;
        self.repl_state = ReplState::Scoped;
        // Clear downstream state
        self.template_dsl = None;
        self.target_entity_type = None;
        self.intent_confirmed = false;
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Set template DSL (transition: Scoped → Templated)
    pub fn set_repl_template(
        &mut self,
        template_dsl: String,
        target_type: String,
    ) -> Result<(), String> {
        if !self.repl_state.can_set_template() {
            return Err(format!(
                "Cannot set template in state '{}'. Must be Scoped.",
                self.repl_state
            ));
        }
        self.template_dsl = Some(template_dsl);
        self.target_entity_type = Some(target_type);
        self.intent_confirmed = false;
        self.repl_state = ReplState::Templated { confirmed: false };
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Confirm the intent (transition: Templated(unconfirmed) → Templated(confirmed))
    pub fn confirm_repl_intent(&mut self) -> Result<(), String> {
        if !self.repl_state.can_confirm_intent() {
            return Err(format!(
                "Cannot confirm intent in state '{}'. Must be Templated(unconfirmed).",
                self.repl_state
            ));
        }
        self.intent_confirmed = true;
        self.repl_state = ReplState::Templated { confirmed: true };
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Set generated sheet (transition: Templated(confirmed) → Generated)
    pub fn set_repl_generated(&mut self, sheet: RunSheet) -> Result<(), String> {
        if !self.repl_state.can_generate() {
            return Err(format!(
                "Cannot set generated sheet in state '{}'. Must be Templated(confirmed).",
                self.repl_state
            ));
        }
        self.run_sheet = sheet;
        self.repl_state = ReplState::Generated;
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark sheet as parsed (transition: Generated → Parsed/Resolving/Ready)
    pub fn set_repl_parsed(&mut self, unresolved_count: usize) -> Result<(), String> {
        if !matches!(self.repl_state, ReplState::Generated) {
            return Err(format!(
                "Cannot set parsed in state '{}'. Must be Generated.",
                self.repl_state
            ));
        }
        self.repl_state = if unresolved_count > 0 {
            ReplState::Resolving {
                remaining: unresolved_count,
            }
        } else {
            ReplState::Ready
        };
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Resolve a reference (decrements remaining count)
    pub fn resolve_repl_ref(&mut self, remaining: usize) -> Result<(), String> {
        if !matches!(self.repl_state, ReplState::Resolving { .. }) {
            return Err(format!(
                "Cannot resolve ref in state '{}'. Must be Resolving.",
                self.repl_state
            ));
        }
        self.repl_state = if remaining > 0 {
            ReplState::Resolving { remaining }
        } else {
            ReplState::Ready
        };
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark ready for execution
    pub fn set_repl_ready(&mut self) -> Result<(), String> {
        match &self.repl_state {
            ReplState::Resolving { remaining: 0 } | ReplState::Parsed => {
                self.repl_state = ReplState::Ready;
                self.dirty = true;
                self.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(format!(
                "Cannot set ready in state '{}'. Must be Resolving(0) or Parsed.",
                self.repl_state
            )),
        }
    }

    /// Start execution (transition: Ready → Executing)
    pub fn set_repl_executing(&mut self, total: usize) -> Result<(), String> {
        if !self.repl_state.can_execute() {
            return Err(format!(
                "Cannot execute in state '{}'. Must be Ready.",
                self.repl_state
            ));
        }
        self.repl_state = ReplState::Executing {
            completed: 0,
            total,
        };
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Update execution progress
    pub fn update_repl_progress(&mut self, completed: usize, total: usize) {
        if matches!(self.repl_state, ReplState::Executing { .. }) {
            self.repl_state = ReplState::Executing { completed, total };
            // Don't mark dirty for every progress update
        }
    }

    /// Mark execution complete (transition: Executing → Executed)
    pub fn mark_repl_executed(&mut self, success: bool) -> Result<(), String> {
        if !matches!(self.repl_state, ReplState::Executing { .. }) {
            return Err(format!(
                "Cannot mark executed in state '{}'. Must be Executing.",
                self.repl_state
            ));
        }
        self.repl_state = ReplState::Executed { success };
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Reset to scoped state (for retry or new intent)
    pub fn reset_repl_to_scoped(&mut self) -> Result<(), String> {
        if self.scope_dsl.is_empty() {
            return Err("Cannot reset to scoped - no scope defined".to_string());
        }
        self.template_dsl = None;
        self.target_entity_type = None;
        self.intent_confirmed = false;
        self.run_sheet = RunSheet::default();
        self.repl_state = ReplState::Scoped;
        self.dirty = true;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Reset to empty state (full reset)
    pub fn reset_repl_to_empty(&mut self) {
        self.scope_dsl.clear();
        self.template_dsl = None;
        self.target_entity_type = None;
        self.intent_confirmed = false;
        self.run_sheet = RunSheet::default();
        self.repl_state = ReplState::Empty;
        self.dirty = true;
        self.updated_at = Utc::now();
    }

    /// Check if session is ready for execution
    pub fn is_repl_ready(&self) -> bool {
        self.repl_state.can_execute()
    }

    /// Check if session has unsaved changes
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark session as clean (after save)
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    // =========================================================================
    // PERSISTENCE (migrated from CbuSession)
    // =========================================================================

    /// Save session to database
    #[cfg(feature = "database")]
    pub async fn save(&mut self, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        let cbu_ids: Vec<Uuid> = self.entity_scope.cbu_ids.iter().copied().collect();
        let history_json = serde_json::to_value(&self.cbu_history).unwrap_or_default();
        let future_json = serde_json::to_value(&self.cbu_future).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".sessions (id, user_id, name, cbu_ids, history, future)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                name = EXCLUDED.name,
                cbu_ids = EXCLUDED.cbu_ids,
                history = EXCLUDED.history,
                future = EXCLUDED.future,
                updated_at = NOW()
            "#,
        )
        .bind(self.id)
        .bind(if self.user_id.is_nil() {
            None
        } else {
            Some(self.user_id)
        })
        .bind(&self.name)
        .bind(&cbu_ids)
        .bind(&history_json)
        .bind(&future_json)
        .execute(pool)
        .await?;

        self.dirty = false;
        Ok(())
    }

    /// Load session from database
    #[cfg(feature = "database")]
    #[allow(clippy::type_complexity)]
    pub async fn load(id: Uuid, pool: &sqlx::PgPool) -> Result<Option<Self>, sqlx::Error> {
        let row: Option<(
            Uuid,
            Option<Uuid>,
            Option<String>,
            Vec<Uuid>,
            serde_json::Value,
            serde_json::Value,
        )> = sqlx::query_as(
            r#"
            SELECT id, user_id, name, cbu_ids, history, future
            FROM "ob-poc".sessions
            WHERE id = $1 AND expires_at > NOW()
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|(id, user_id, name, cbu_ids, history, future)| {
            let cbu_history: Vec<CbuSnapshot> = serde_json::from_value(history).unwrap_or_default();
            let cbu_future: Vec<CbuSnapshot> = serde_json::from_value(future).unwrap_or_default();

            let mut session = Self::new();
            session.id = id;
            session.user_id = user_id.unwrap_or(Uuid::nil());
            session.name = name;
            session.entity_scope.cbu_ids = cbu_ids.into_iter().collect();
            session.cbu_history = cbu_history;
            session.cbu_future = cbu_future;
            session.dirty = false;
            session
        }))
    }

    /// Load session or create new if not found
    #[cfg(feature = "database")]
    pub async fn load_or_new(id: Option<Uuid>, pool: &sqlx::PgPool) -> Self {
        if let Some(id) = id {
            match tokio::time::timeout(std::time::Duration::from_secs(2), Self::load(id, pool))
                .await
            {
                Ok(Ok(Some(session))) => {
                    tracing::debug!("Session {} loaded from DB", id);
                    return session;
                }
                Ok(Ok(None)) => tracing::debug!("Session {} not found, creating new", id),
                Ok(Err(e)) => tracing::warn!("Session load failed (non-fatal): {}", e),
                Err(_) => tracing::warn!("Session load timed out (non-fatal)"),
            }
        }
        Self::new()
    }

    /// Delete session from database
    #[cfg(feature = "database")]
    pub async fn delete(id: Uuid, pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM "ob-poc".sessions WHERE id = $1"#)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List recent sessions
    #[cfg(feature = "database")]
    #[allow(clippy::type_complexity)]
    pub async fn list_recent(
        user_id: Option<Uuid>,
        limit: i64,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<SessionListItem>, sqlx::Error> {
        let rows: Vec<(Uuid, Option<String>, Option<i32>, chrono::DateTime<Utc>)> = sqlx::query_as(
            r#"
            SELECT
                id,
                name,
                array_length(cbu_ids, 1) as cbu_count,
                updated_at
            FROM "ob-poc".sessions
            WHERE ($1::uuid IS NULL AND user_id IS NULL) OR user_id = $1
            AND expires_at > NOW()
            ORDER BY updated_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, cbu_count, updated_at)| SessionListItem {
                id,
                name,
                cbu_count: cbu_count.unwrap_or(0),
                updated_at,
            })
            .collect())
    }
}

/// Session list item for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListItem {
    pub id: Uuid,
    pub name: Option<String>,
    pub cbu_count: i32,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = UnifiedSession::new();
        assert!(!session.id.is_nil());
        assert!(session.target_universe.is_none());
        assert!(session.entity_scope.cbu_ids.is_empty());
        assert!(session.messages.is_empty());
    }

    #[test]
    fn test_add_cbu() {
        let mut session = UnifiedSession::new();
        let cbu_id = Uuid::new_v4();
        session.add_cbu(cbu_id);
        assert!(session.entity_scope.cbu_ids.contains(&cbu_id));
    }

    #[test]
    fn test_state_stack_navigation() {
        let mut stack = StateStack::new(10);

        // Push some states
        stack.push(EntityScope::default(), 0, "State 1".to_string());
        stack.push(EntityScope::default(), 1, "State 2".to_string());
        stack.push(EntityScope::default(), 2, "State 3".to_string());

        assert_eq!(stack.len(), 3);
        assert_eq!(stack.position(), 2);
        assert!(stack.can_go_back());
        assert!(!stack.can_go_forward());

        // Go back
        let snapshot = stack.back(1).unwrap();
        assert_eq!(snapshot.action_label, "State 2");
        assert!(stack.can_go_forward());

        // Go forward
        let snapshot = stack.forward(1).unwrap();
        assert_eq!(snapshot.action_label, "State 3");
    }

    #[test]
    fn test_resolution_state() {
        let mut resolution = ResolutionState {
            refs: vec![
                UnresolvedRef {
                    ref_id: "ref1".to_string(),
                    entity_type: "company".to_string(),
                    search_value: "Acme".to_string(),
                    context_line: "create company Acme".to_string(),
                    search_keys: vec![],
                    discriminators: vec![],
                    initial_matches: vec![],
                },
                UnresolvedRef {
                    ref_id: "ref2".to_string(),
                    entity_type: "person".to_string(),
                    search_value: "Smith".to_string(),
                    context_line: "assign director Smith".to_string(),
                    search_keys: vec![],
                    discriminators: vec![],
                    initial_matches: vec![],
                },
            ],
            current_index: 0,
            resolutions: HashMap::new(),
        };

        assert_eq!(resolution.current().unwrap().ref_id, "ref1");
        assert!(!resolution.is_complete());
        assert_eq!(resolution.unresolved_count(), 2);

        // Resolve first
        resolution.resolve_current("acme-id".to_string(), "Acme Corp".to_string());
        assert!(resolution.advance());
        assert_eq!(resolution.current().unwrap().ref_id, "ref2");
        assert_eq!(resolution.unresolved_count(), 1);

        // Resolve second
        resolution.resolve_current("smith-id".to_string(), "John Smith".to_string());
        assert!(!resolution.advance()); // No more refs
        assert!(resolution.is_complete());
    }

    #[test]
    fn test_run_sheet() {
        let mut session = UnifiedSession::new();

        let id1 = session.add_dsl(
            "(entity.create :name \"Acme\")".to_string(),
            "Create Acme".to_string(),
        );
        let id2 = session.add_dsl(
            "(entity.create :name \"Beta\")".to_string(),
            "Create Beta".to_string(),
        );

        assert_eq!(session.run_sheet.entries.len(), 2);
        assert_eq!(session.run_sheet.count_by_status(EntryStatus::Draft), 2);

        session.mark_executed(id1, vec![]);
        assert_eq!(session.run_sheet.count_by_status(EntryStatus::Draft), 1);
        assert_eq!(session.run_sheet.count_by_status(EntryStatus::Executed), 1);

        session.mark_failed(id2, "Test error".to_string());
        assert_eq!(session.run_sheet.count_by_status(EntryStatus::Failed), 1);
    }

    #[test]
    fn test_zoom_level_depth() {
        assert!(ZoomLevel::Core.is_deeper_than(&ZoomLevel::Universe));
        assert!(ZoomLevel::Planet.is_deeper_than(&ZoomLevel::Galaxy));
        assert!(!ZoomLevel::Universe.is_deeper_than(&ZoomLevel::Core));
    }

    #[test]
    fn test_constraint_cascade() {
        let mut session = UnifiedSession::new();

        // Initially empty
        assert!(session.client.is_none());
        assert!(session.structure_type.is_none());

        // Set client (level 1)
        let client_id = Uuid::new_v4();
        session.set_client(client_id, "Allianz".to_string());
        assert!(session.client.is_some());
        assert_eq!(session.client.as_ref().unwrap().display_name, "Allianz");

        // Set structure type (level 2)
        session.set_structure_type(StructureType::Sicav);
        assert_eq!(session.structure_type, Some(StructureType::Sicav));

        // Set current structure (level 3)
        let structure_id = Uuid::new_v4();
        session.set_current_structure(
            structure_id,
            "Allianz SICAV 1".to_string(),
            StructureType::Sicav,
        );
        assert!(session.current_structure.is_some());

        // Clear client should clear entire cascade
        session.clear_client();
        assert!(session.client.is_none());
        assert!(session.structure_type.is_none());
        assert!(session.current_structure.is_none());
    }

    #[test]
    fn test_structure_type_mapping() {
        assert_eq!(StructureType::Pe.display_label(), "Private Equity");
        assert_eq!(StructureType::Pe.internal_token(), "private-equity");
        assert_eq!(
            StructureType::from_internal("private-equity"),
            Some(StructureType::Pe)
        );
        assert_eq!(StructureType::from_internal("pe"), Some(StructureType::Pe));
        assert_eq!(StructureType::from_internal("unknown"), None);
    }

    #[test]
    fn test_dag_state() {
        let mut dag = DagState::default();

        // Initially empty
        assert!(!dag.is_completed("structure.setup"));
        assert!(!dag.get_flag("structure.exists"));

        // Mark verb completed
        dag.mark_completed("structure.setup");
        assert!(dag.is_completed("structure.setup"));

        // Set flag
        dag.set_flag("structure.exists", true);
        assert!(dag.get_flag("structure.exists"));

        // Clear
        dag.clear();
        assert!(!dag.is_completed("structure.setup"));
        assert!(!dag.get_flag("structure.exists"));
    }

    #[test]
    fn test_prereq_conditions() {
        let mut dag = DagState::default();
        dag.mark_completed("structure.setup");
        dag.set_flag("structure.exists", true);

        // VerbCompleted
        let prereq = PrereqCondition::VerbCompleted {
            verb: "structure.setup".to_string(),
        };
        assert!(prereq.is_satisfied(&dag));

        let prereq = PrereqCondition::VerbCompleted {
            verb: "case.open".to_string(),
        };
        assert!(!prereq.is_satisfied(&dag));

        // AnyOf
        let prereq = PrereqCondition::AnyOf {
            verbs: vec!["structure.setup".to_string(), "case.open".to_string()],
        };
        assert!(prereq.is_satisfied(&dag));

        // StateExists
        let prereq = PrereqCondition::StateExists {
            key: "structure.exists".to_string(),
        };
        assert!(prereq.is_satisfied(&dag));

        let prereq = PrereqCondition::StateExists {
            key: "case.exists".to_string(),
        };
        assert!(!prereq.is_satisfied(&dag));
    }

    #[test]
    fn test_run_sheet_dag_operations() {
        let mut session = UnifiedSession::new();

        // Add entries with DAG metadata
        let id1 = session.add_dsl_with_dag(
            "(structure.setup :name \"Fund A\")".to_string(),
            "Set up Fund A".to_string(),
            0,
            vec![],
        );
        let id2 = session.add_dsl_with_dag(
            "(case.open :structure @fund)".to_string(),
            "Open case".to_string(),
            1,
            vec![id1],
        );

        assert_eq!(session.run_sheet.max_depth(), 1);
        assert_eq!(session.run_sheet.by_phase(0).count(), 1);
        assert_eq!(session.run_sheet.by_phase(1).count(), 1);

        // Mark first as ready, then executed
        if let Some(entry) = session.run_sheet.entries.iter_mut().find(|e| e.id == id1) {
            entry.status = EntryStatus::Ready;
        }
        session.mark_executed(id1, vec![]);

        // Now second entry should be ready for execution (deps satisfied)
        if let Some(entry) = session.run_sheet.entries.iter_mut().find(|e| e.id == id2) {
            entry.status = EntryStatus::Ready;
        }
        let ready = session.run_sheet.ready_for_execution();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, id2);
    }

    #[test]
    fn test_cascade_skip() {
        let mut sheet = RunSheet::default();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        sheet.entries.push(RunSheetEntry {
            id: id1,
            dsl_source: "stmt1".to_string(),
            display_dsl: "stmt1".to_string(),
            status: EntryStatus::Failed,
            created_at: Utc::now(),
            executed_at: None,
            affected_entities: vec![],
            error: Some("test error".to_string()),
            dag_depth: 0,
            dependencies: vec![],
            validation_errors: vec![],
        });

        sheet.entries.push(RunSheetEntry {
            id: id2,
            dsl_source: "stmt2".to_string(),
            display_dsl: "stmt2".to_string(),
            status: EntryStatus::Ready,
            created_at: Utc::now(),
            executed_at: None,
            affected_entities: vec![],
            error: None,
            dag_depth: 1,
            dependencies: vec![id1],
            validation_errors: vec![],
        });

        sheet.entries.push(RunSheetEntry {
            id: id3,
            dsl_source: "stmt3".to_string(),
            display_dsl: "stmt3".to_string(),
            status: EntryStatus::Ready,
            created_at: Utc::now(),
            executed_at: None,
            affected_entities: vec![],
            error: None,
            dag_depth: 2,
            dependencies: vec![id2],
            validation_errors: vec![],
        });

        // Cascade skip from id1
        sheet.cascade_skip(id1);

        // id2 and id3 should be skipped
        assert_eq!(sheet.entries[1].status, EntryStatus::Skipped);
        assert_eq!(sheet.entries[2].status, EntryStatus::Skipped);
        // id1 should still be Failed (not changed to Skipped)
        assert_eq!(sheet.entries[0].status, EntryStatus::Failed);
    }

    #[test]
    fn test_persona_mode_tags() {
        assert!(Persona::Ops.mode_tags().contains(&"onboarding"));
        assert!(Persona::Ops.mode_tags().contains(&"kyc"));
        assert!(!Persona::Kyc.mode_tags().contains(&"trading"));
        assert!(Persona::Admin.mode_tags().contains(&"admin"));
    }
}

// =============================================================================
// SHEET EXECUTION TYPES
// =============================================================================
// These types support phased execution of RunSheet entries.
// Migrated from dsl_sheet.rs to consolidate all sheet-related types here.

/// Execution phase - a group of entries at the same DAG depth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPhase {
    /// Phase depth (0 = first phase, no dependencies)
    pub depth: u32,
    /// Entry IDs in this phase
    pub entry_ids: Vec<Uuid>,
    /// Symbols produced by this phase
    pub produces: Vec<String>,
    /// Symbols consumed by this phase
    pub consumes: Vec<String>,
}

impl ExecutionPhase {
    /// Create a new empty phase
    pub fn new(depth: u32) -> Self {
        Self {
            depth,
            entry_ids: Vec::new(),
            produces: Vec::new(),
            consumes: Vec::new(),
        }
    }

    /// Get count of entries in this phase
    pub fn entry_count(&self) -> usize {
        self.entry_ids.len()
    }
}

/// Error codes for DSL execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    // Syntax errors
    SyntaxError,
    InvalidVerb,
    InvalidArgument,
    MissingRequired,

    // Resolution errors
    UnresolvedSymbol,
    AmbiguousEntity,
    EntityNotFound,
    TypeMismatch,

    // Execution errors
    DbConstraint,
    DbConnection,
    Timeout,
    PermissionDenied,

    // Dependency errors
    Blocked,
    CyclicDependency,

    // Internal errors
    InternalError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxError => write!(f, "SYNTAX_ERROR"),
            Self::InvalidVerb => write!(f, "INVALID_VERB"),
            Self::InvalidArgument => write!(f, "INVALID_ARGUMENT"),
            Self::MissingRequired => write!(f, "MISSING_REQUIRED"),
            Self::UnresolvedSymbol => write!(f, "UNRESOLVED_SYMBOL"),
            Self::AmbiguousEntity => write!(f, "AMBIGUOUS_ENTITY"),
            Self::EntityNotFound => write!(f, "ENTITY_NOT_FOUND"),
            Self::TypeMismatch => write!(f, "TYPE_MISMATCH"),
            Self::DbConstraint => write!(f, "DB_CONSTRAINT"),
            Self::DbConnection => write!(f, "DB_CONNECTION"),
            Self::Timeout => write!(f, "TIMEOUT"),
            Self::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            Self::Blocked => write!(f, "BLOCKED"),
            Self::CyclicDependency => write!(f, "CYCLIC_DEPENDENCY"),
            Self::InternalError => write!(f, "INTERNAL_ERROR"),
        }
    }
}

/// Overall sheet execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SheetStatus {
    /// All entries executed successfully
    Success,
    /// At least one entry failed, transaction rolled back
    Failed,
    /// Execution was rolled back
    RolledBack,
}

impl SheetStatus {
    /// Get status as string for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::RolledBack => "rolled_back",
        }
    }
}

/// Detailed error for a single entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryError {
    /// Error code
    pub code: ErrorCode,
    /// Error message
    pub message: String,
    /// Additional detail
    pub detail: Option<String>,
    /// Source span for highlighting
    pub span: Option<(usize, usize)>,
    /// ID of entry that blocked this one (if Blocked)
    pub blocked_by: Option<Uuid>,
}

/// Result of executing a single entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryResult {
    /// Entry ID
    pub entry_id: Uuid,
    /// DAG depth
    pub dag_depth: u32,
    /// Original source
    pub source: String,
    /// Resolved source (with UUIDs)
    pub resolved_source: Option<String>,
    /// Final status
    pub status: EntryStatus,
    /// Error details (if failed)
    pub error: Option<EntryError>,
    /// Returned primary key (if any)
    pub returned_pk: Option<Uuid>,
    /// Execution time in milliseconds
    pub execution_time_ms: Option<u64>,
}

/// Result of executing a run sheet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetExecutionResult {
    /// Session ID
    pub session_id: Uuid,
    /// Overall status
    pub overall_status: SheetStatus,
    /// Phases completed before stopping
    pub phases_completed: usize,
    /// Total phases
    pub phases_total: usize,
    /// Per-entry results
    pub entries: Vec<EntryResult>,
    /// Execution start time
    pub started_at: DateTime<Utc>,
    /// Execution end time
    pub completed_at: DateTime<Utc>,
    /// Total execution time in milliseconds
    pub duration_ms: u64,
}

impl SheetExecutionResult {
    /// Get count of successful entries
    pub fn success_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Executed)
            .count()
    }

    /// Get count of failed entries
    pub fn failed_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Failed)
            .count()
    }

    /// Get count of skipped entries
    pub fn skipped_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Skipped)
            .count()
    }

    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.overall_status == SheetStatus::Success
    }
}

// =============================================================================
// TYPE CONVERSIONS (for backward compatibility with api::session types)
// =============================================================================

impl From<SessionState> for crate::api::session::SessionState {
    fn from(state: SessionState) -> Self {
        match state {
            SessionState::New => crate::api::session::SessionState::New,
            SessionState::Scoped => crate::api::session::SessionState::Scoped,
            SessionState::PendingValidation => crate::api::session::SessionState::PendingValidation,
            SessionState::ReadyToExecute => crate::api::session::SessionState::ReadyToExecute,
            SessionState::Executing => crate::api::session::SessionState::Executing,
            SessionState::Executed => crate::api::session::SessionState::Executed,
            SessionState::Closed => crate::api::session::SessionState::Closed,
        }
    }
}

impl From<crate::api::session::SessionState> for SessionState {
    fn from(state: crate::api::session::SessionState) -> Self {
        match state {
            crate::api::session::SessionState::New => SessionState::New,
            crate::api::session::SessionState::Scoped => SessionState::Scoped,
            crate::api::session::SessionState::PendingValidation => SessionState::PendingValidation,
            crate::api::session::SessionState::ReadyToExecute => SessionState::ReadyToExecute,
            crate::api::session::SessionState::Executing => SessionState::Executing,
            crate::api::session::SessionState::Executed => SessionState::Executed,
            crate::api::session::SessionState::Closed => SessionState::Closed,
        }
    }
}

impl From<MessageRole> for crate::api::session::MessageRole {
    fn from(role: MessageRole) -> Self {
        match role {
            MessageRole::User => crate::api::session::MessageRole::User,
            MessageRole::Agent => crate::api::session::MessageRole::Agent,
            MessageRole::System => crate::api::session::MessageRole::System,
        }
    }
}

impl From<ChatMessage> for crate::api::session::ChatMessage {
    fn from(msg: ChatMessage) -> Self {
        crate::api::session::ChatMessage {
            id: msg.id,
            role: msg.role.into(),
            content: msg.content,
            timestamp: msg.timestamp,
            intents: msg.intents,
            dsl: msg.dsl,
        }
    }
}
