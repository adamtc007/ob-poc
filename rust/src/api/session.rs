//! Session state management for Agent API
//!
//! Provides stateful session handling for multi-turn DSL generation conversations.
//! Sessions accumulate AST statements, validate them, and track execution.
//! The AST is the source of truth - DSL source is generated from it for display.

use super::intent::VerbIntent;
use crate::dsl_v2::ast::{Program, Statement};
use crate::dsl_v2::batch_executor::BatchResultAccumulator;
use crate::mcp::scope_resolution::ScopeContext;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ============================================================================
// Session Mode - What kind of interaction is happening
// ============================================================================

/// Session mode - determines how the session processes user input
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// Normal chat mode - agent generates DSL from natural language
    #[default]
    Chat,
    /// Template expansion mode - agent is collecting params for a template
    TemplateExpansion,
    /// Batch execution mode - iterating over a key set, expanding template per item
    BatchExecution,
}

// ============================================================================
// Sub-Session Types - Scoped agent conversations
// ============================================================================

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Info about an unresolved entity reference
///
/// This is the s-expression for the entity reference, carrying:
/// - Entity type (from verb arg definition)
/// - Search key fields with current values
/// - Discriminator fields with current values
/// - The resolved PK (UUID) once found
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnresolvedRefInfo {
    /// Unique ID for this ref (stmt_idx:arg_name)
    pub ref_id: String,
    /// Entity type (e.g., "entity", "cbu", "person") - from verb arg lookup config
    pub entity_type: String,
    /// The search value from DSL (e.g., "John Smith")
    pub search_value: String,
    /// DSL context line for display
    pub context_line: String,
    /// Initial search matches (pre-fetched)
    pub initial_matches: Vec<EntityMatchInfo>,

    // === Entity Resolution Config (from EntityGateway) ===
    /// Search key fields for this entity type (e.g., name, lei, jurisdiction)
    /// Each field has: key name, display label, current value (if populated)
    #[serde(default)]
    pub search_keys: Vec<SearchKeyField>,

    /// Discriminator fields for narrowing results (e.g., manco_name, fund_type)
    #[serde(default)]
    pub discriminators: Vec<DiscriminatorField>,

    /// Resolution mode hint from entity config
    #[serde(default)]
    pub resolution_mode: ResolutionModeHint,

    /// The verb and arg that created this ref (for applying resolution back)
    #[serde(default)]
    pub verb_context: Option<VerbArgContext>,

    // === Resolution State ===
    /// Resolved primary key (UUID or code) - populated after user selection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_key: Option<String>,

    /// Display name of resolved entity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_display: Option<String>,
}

/// Search key field definition with current value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SearchKeyField {
    /// Field key (e.g., "name", "lei", "jurisdiction")
    pub key: String,
    /// Display label (e.g., "Entity Name", "LEI Code")
    pub label: String,
    /// Current value (populated from DSL or user input)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Whether this is a primary search key
    #[serde(default)]
    pub is_primary: bool,
}

/// Discriminator field for narrowing search results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DiscriminatorField {
    /// Field key (e.g., "manco_name", "fund_type")
    pub key: String,
    /// Display label
    pub label: String,
    /// Current value (if set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Available options (for dropdown)
    #[serde(default)]
    pub options: Vec<String>,
}

/// Resolution mode hint from entity config
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionModeHint {
    /// Show search modal with multiple fields
    #[default]
    SearchModal,
    /// Show inline autocomplete (single field)
    InlineAutocomplete,
    /// Show dropdown with all options
    Dropdown,
}

/// Context about which verb/arg created this ref
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VerbArgContext {
    /// Full verb name (e.g., "session.set-cbu")
    pub verb: String,
    /// Argument name (e.g., "cbu-id")
    pub arg_name: String,
    /// Statement index in AST
    pub stmt_index: usize,
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

// ============================================================================
// ResolutionSubSession Implementation
// ============================================================================

impl ResolutionSubSession {
    /// Create a new empty resolution sub-session
    pub fn new() -> Self {
        Self {
            unresolved_refs: Vec::new(),
            parent_dsl_index: 0,
            current_ref_index: 0,
            resolutions: HashMap::new(),
        }
    }

    /// Extract unresolved refs from AST statements
    ///
    /// Walks the AST and collects all EntityRef nodes that have `resolved_key: None`.
    /// Each ref gets a unique ref_id based on its span location.
    ///
    /// Also captures verb/arg context so we can look up the LookupConfig from verb registry.
    pub fn from_statements(statements: &[Statement]) -> Self {
        use crate::dsl_v2::verb_registry::registry;

        let mut unresolved_refs = Vec::new();

        for (stmt_idx, stmt) in statements.iter().enumerate() {
            if let Statement::VerbCall(vc) = stmt {
                // Build context line for display
                let context_line = vc.to_dsl_string();
                let full_verb = format!("{}.{}", vc.domain, vc.verb);

                // Look up verb definition to get arg lookup configs
                let verb_def = registry().get(&vc.domain, &vc.verb);

                for arg in &vc.arguments {
                    // Get lookup config for this arg from verb definition
                    let lookup_config = verb_def
                        .as_ref()
                        .and_then(|v| v.args.iter().find(|a| a.name == arg.key))
                        .and_then(|a| a.lookup.as_ref());

                    Self::collect_unresolved_refs(
                        &arg.value,
                        stmt_idx,
                        &context_line,
                        &full_verb,
                        &arg.key,
                        lookup_config,
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

    /// Recursively collect unresolved EntityRefs from an AstNode
    ///
    /// Now also captures verb context and extracts search key config from LookupConfig
    fn collect_unresolved_refs(
        node: &crate::dsl_v2::ast::AstNode,
        stmt_idx: usize,
        context_line: &str,
        verb: &str,
        arg_name: &str,
        lookup_config: Option<&dsl_core::config::types::LookupConfig>,
        out: &mut Vec<UnresolvedRefInfo>,
    ) {
        use crate::dsl_v2::ast::AstNode;

        match node {
            AstNode::EntityRef {
                entity_type,
                value,
                resolved_key,
                span,
                ref_id,
                ..
            } => {
                if resolved_key.is_none() {
                    // Use ref_id if available, otherwise generate from span
                    let ref_id_str = ref_id
                        .clone()
                        .unwrap_or_else(|| format!("{}:{}-{}", stmt_idx, span.start, span.end));

                    // Extract search keys and discriminators from LookupConfig
                    let (search_keys, discriminators, resolution_mode) =
                        Self::extract_search_config(lookup_config, value);

                    out.push(UnresolvedRefInfo {
                        ref_id: ref_id_str,
                        entity_type: entity_type.clone(),
                        search_value: value.clone(),
                        context_line: context_line.to_string(),
                        initial_matches: Vec::new(), // Populated by pre_fetch_matches
                        search_keys,
                        discriminators,
                        resolution_mode,
                        verb_context: Some(VerbArgContext {
                            verb: verb.to_string(),
                            arg_name: arg_name.to_string(),
                            stmt_index: stmt_idx,
                        }),
                        resolved_key: None,
                        resolved_display: None,
                    });
                }
            }
            AstNode::List { items, .. } => {
                for item in items {
                    Self::collect_unresolved_refs(
                        item,
                        stmt_idx,
                        context_line,
                        verb,
                        arg_name,
                        lookup_config,
                        out,
                    );
                }
            }
            AstNode::Map { entries, .. } => {
                for (_, v) in entries {
                    Self::collect_unresolved_refs(
                        v,
                        stmt_idx,
                        context_line,
                        verb,
                        arg_name,
                        lookup_config,
                        out,
                    );
                }
            }
            AstNode::Nested(vc) => {
                for arg in &vc.arguments {
                    // For nested verb calls, we don't have lookup config
                    Self::collect_unresolved_refs(
                        &arg.value,
                        stmt_idx,
                        context_line,
                        verb,
                        &arg.key,
                        None,
                        out,
                    );
                }
            }
            // Literals and SymbolRefs don't contain EntityRefs
            _ => {}
        }
    }

    /// Extract search keys and discriminators from LookupConfig
    fn extract_search_config(
        lookup_config: Option<&dsl_core::config::types::LookupConfig>,
        search_value: &str,
    ) -> (
        Vec<SearchKeyField>,
        Vec<DiscriminatorField>,
        ResolutionModeHint,
    ) {
        let Some(config) = lookup_config else {
            // No lookup config - return minimal defaults
            return (
                vec![SearchKeyField {
                    key: "name".to_string(),
                    label: "Name".to_string(),
                    value: Some(search_value.to_string()),
                    is_primary: true,
                }],
                vec![],
                ResolutionModeHint::SearchModal,
            );
        };

        // Extract primary search key
        let primary_col = config.search_key.primary_column();
        let search_keys = vec![SearchKeyField {
            key: primary_col.to_string(),
            label: Self::humanize_label(primary_col),
            value: Some(search_value.to_string()),
            is_primary: true,
        }];

        // Extract discriminators from composite search key
        let discriminators: Vec<DiscriminatorField> = config
            .search_key
            .discriminators()
            .iter()
            .map(|d| DiscriminatorField {
                key: d.field.clone(),
                label: Self::humanize_label(&d.field),
                value: None,     // Will be populated from other verb args or user input
                options: vec![], // Could be populated from reference data
            })
            .collect();

        // Determine resolution mode from config
        let resolution_mode = match config.resolution_mode {
            Some(dsl_core::config::types::ResolutionMode::Entity) => {
                ResolutionModeHint::SearchModal
            }
            Some(dsl_core::config::types::ResolutionMode::Reference) => {
                ResolutionModeHint::Dropdown
            }
            None => ResolutionModeHint::SearchModal, // Default to modal
        };

        (search_keys, discriminators, resolution_mode)
    }

    /// Convert snake_case field names to human-readable labels
    fn humanize_label(field: &str) -> String {
        field
            .split('_')
            .map(|word| {
                let mut chars: Vec<char> = word.chars().collect();
                if let Some(first) = chars.first_mut() {
                    *first = first.to_uppercase().next().unwrap_or(*first);
                }
                chars.into_iter().collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Select a resolution for a ref_id
    ///
    /// Stores the mapping from ref_id to resolved_key.
    /// Validation against gateway should be done by caller before calling this.
    pub fn select(&mut self, ref_id: &str, resolved_key: &str) -> Result<(), String> {
        // Verify ref_id exists
        if !self.unresolved_refs.iter().any(|r| r.ref_id == ref_id) {
            return Err(format!("Unknown ref_id: {}", ref_id));
        }

        self.resolutions
            .insert(ref_id.to_string(), resolved_key.to_string());
        Ok(())
    }

    /// Clear a resolution for a ref_id (undo selection)
    pub fn clear(&mut self, ref_id: &str) {
        self.resolutions.remove(ref_id);
    }

    /// Check if all refs have been resolved
    pub fn is_complete(&self) -> bool {
        self.unresolved_refs
            .iter()
            .all(|r| self.resolutions.contains_key(&r.ref_id))
    }

    /// Get number of resolved vs total refs
    pub fn progress(&self) -> ResolutionProgress {
        let resolved = self
            .unresolved_refs
            .iter()
            .filter(|r| self.resolutions.contains_key(&r.ref_id))
            .count();
        ResolutionProgress {
            resolved,
            total: self.unresolved_refs.len(),
        }
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

    /// Move to previous ref
    pub fn prev_ref(&mut self) -> bool {
        if self.current_ref_index > 0 {
            self.current_ref_index -= 1;
            true
        } else {
            false
        }
    }

    /// Apply all resolutions to AST statements
    ///
    /// Walks the AST and sets `resolved_key` on EntityRef nodes
    /// where we have a resolution stored.
    pub fn apply_to_statements(&self, statements: &mut [Statement]) -> Result<usize, String> {
        let mut applied = 0;

        for stmt in statements.iter_mut() {
            if let Statement::VerbCall(vc) = stmt {
                for arg in &mut vc.arguments {
                    applied += self.apply_to_node(&mut arg.value)?;
                }
            }
        }

        Ok(applied)
    }

    /// Recursively apply resolutions to an AstNode
    fn apply_to_node(&self, node: &mut crate::dsl_v2::ast::AstNode) -> Result<usize, String> {
        use crate::dsl_v2::ast::AstNode;

        match node {
            AstNode::EntityRef {
                ref_id,
                resolved_key,
                span,
                ..
            } => {
                // Get the ref_id for this node
                let node_ref_id = ref_id
                    .clone()
                    .unwrap_or_else(|| format!("unknown:{}-{}", span.start, span.end));

                // Check if we have a resolution
                if let Some(key) = self.resolutions.get(&node_ref_id) {
                    // Validate it's a UUID before setting
                    uuid::Uuid::parse_str(key)
                        .map_err(|_| format!("Resolution '{}' is not a valid UUID", key))?;
                    *resolved_key = Some(key.clone());
                    Ok(1)
                } else {
                    Ok(0)
                }
            }
            AstNode::List { items, .. } => {
                let mut count = 0;
                for item in items {
                    count += self.apply_to_node(item)?;
                }
                Ok(count)
            }
            AstNode::Map { entries, .. } => {
                let mut count = 0;
                for (_, v) in entries {
                    count += self.apply_to_node(v)?;
                }
                Ok(count)
            }
            AstNode::Nested(vc) => {
                let mut count = 0;
                for arg in &mut vc.arguments {
                    count += self.apply_to_node(&mut arg.value)?;
                }
                Ok(count)
            }
            _ => Ok(0),
        }
    }
}

impl Default for ResolutionSubSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Research sub-session state - GLEIF/UBO discovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchSubSession {
    /// Target entity being researched (if known)
    pub target_entity_id: Option<Uuid>,
    /// Research type (gleif, ubo, companies_house, etc.)
    pub research_type: String,
    /// Search query used
    pub search_query: Option<String>,
}

/// Review sub-session state - DSL review before execute
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorrectionSubSession {
    /// Entity with screening hit
    pub entity_id: Uuid,
    /// Screening hit ID
    pub hit_id: Uuid,
    /// Correction type being applied
    pub correction_type: Option<String>,
}

// ============================================================================
// Session State Machine
// ============================================================================

/// Session lifecycle states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SessionState {
    /// Just created, awaiting scope selection (client/CBU set)
    /// This is the initial state - nothing else can happen until scope is set
    #[default]
    New,
    /// Scope is set (client/CBU set selected), ready for operations
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
///
/// Use with `AgentSession::transition()` - the single entry point for all state changes.
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

/// Status of a DSL/AST pair in the pipeline
/// This is the state machine for individual DSL fragments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DslStatus {
    /// DSL parsed and AST valid, awaiting user confirmation
    #[default]
    Draft,
    /// User confirmed, ready to execute
    Ready,
    /// Successfully executed against database
    Executed,
    /// User declined to run (logical delete)
    Cancelled,
    /// Execution attempted but failed
    Failed,
}

impl DslStatus {
    /// Can this DSL be executed?
    pub fn is_runnable(&self) -> bool {
        matches!(self, DslStatus::Draft | DslStatus::Ready)
    }

    /// Is this DSL in a terminal state?
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DslStatus::Executed | DslStatus::Cancelled | DslStatus::Failed
        )
    }

    /// Should this DSL be persisted to database?
    pub fn should_persist(&self) -> bool {
        // Only persist executed DSL - drafts and cancelled stay in memory
        matches!(self, DslStatus::Executed)
    }
}

// =============================================================================
// RUN SHEET - Server-side DSL statement ledger
// =============================================================================

/// Server-side run sheet - DSL statement ledger with per-statement status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerRunSheet {
    /// Entries in the run sheet (ordered by creation)
    pub entries: Vec<ServerRunSheetEntry>,
    /// Current cursor position (index of active/draft entry)
    pub cursor: usize,
}

impl ServerRunSheet {
    /// Add a new draft entry
    pub fn add_draft(&mut self, dsl_source: String, ast: Vec<Statement>) -> Uuid {
        let id = Uuid::new_v4();
        self.entries.push(ServerRunSheetEntry {
            id,
            dsl_source,
            ast,
            plan: None,
            status: DslStatus::Draft,
            created_at: Utc::now(),
            executed_at: None,
            affected_entities: Vec::new(),
            bindings: HashMap::new(),
            error: None,
        });
        self.cursor = self.entries.len() - 1;
        id
    }

    /// Get current entry at cursor
    pub fn current(&self) -> Option<&ServerRunSheetEntry> {
        self.entries.get(self.cursor)
    }

    /// Get mutable current entry at cursor
    pub fn current_mut(&mut self) -> Option<&mut ServerRunSheetEntry> {
        self.entries.get_mut(self.cursor)
    }

    /// Get entry by ID
    pub fn get(&self, id: Uuid) -> Option<&ServerRunSheetEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get mutable entry by ID
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut ServerRunSheetEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Mark entry as executed
    pub fn mark_executed(
        &mut self,
        id: Uuid,
        affected: Vec<Uuid>,
        bindings: HashMap<String, BoundEntity>,
    ) {
        if let Some(entry) = self.get_mut(id) {
            entry.status = DslStatus::Executed;
            entry.executed_at = Some(Utc::now());
            entry.affected_entities = affected;
            entry.bindings = bindings;
        }
    }

    /// Mark entry as failed
    pub fn mark_failed(&mut self, id: Uuid, error: String) {
        if let Some(entry) = self.get_mut(id) {
            entry.status = DslStatus::Failed;
            entry.error = Some(error);
        }
    }

    /// Mark all runnable entries as executed
    pub fn mark_all_executed(&mut self) {
        let now = Utc::now();
        for entry in &mut self.entries {
            if entry.status.is_runnable() {
                entry.status = DslStatus::Executed;
                entry.executed_at = Some(now);
            }
        }
    }

    /// Get all executed entries
    pub fn executed(&self) -> impl Iterator<Item = &ServerRunSheetEntry> {
        self.entries
            .iter()
            .filter(|e| e.status == DslStatus::Executed)
    }

    /// Get combined DSL source (for backwards compat)
    pub fn combined_dsl(&self) -> Option<String> {
        let sources: Vec<_> = self.entries.iter().map(|e| e.dsl_source.as_str()).collect();
        if sources.is_empty() {
            None
        } else {
            Some(sources.join("\n"))
        }
    }

    /// Check if there's any runnable DSL
    pub fn has_runnable(&self) -> bool {
        self.entries.iter().any(|e| e.status.is_runnable())
    }

    /// Count runnable entries
    pub fn runnable_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status.is_runnable())
            .count()
    }

    /// Undo (cancel) the last draft/ready entry, returns the removed entry
    pub fn undo_last(&mut self) -> Option<ServerRunSheetEntry> {
        // Find last undoable entry (draft or ready, not executed)
        let idx = self.entries.iter().rposition(|e| e.status.is_runnable())?;
        let mut entry = self.entries.remove(idx);
        entry.status = DslStatus::Cancelled;
        // Adjust cursor if needed
        if self.cursor >= self.entries.len() && !self.entries.is_empty() {
            self.cursor = self.entries.len() - 1;
        } else if self.entries.is_empty() {
            self.cursor = 0;
        }
        Some(entry)
    }

    /// Clear all draft/ready entries (cancel them)
    pub fn clear_drafts(&mut self) {
        for entry in &mut self.entries {
            if entry.status.is_runnable() {
                entry.status = DslStatus::Cancelled;
            }
        }
        // Remove cancelled entries
        self.entries.retain(|e| e.status != DslStatus::Cancelled);
        self.cursor = 0;
    }

    /// Remove a runnable entry matching the search term (case-insensitive)
    pub fn remove_matching(&mut self, search_term: &str) -> Option<ServerRunSheetEntry> {
        let search_lower = search_term.to_lowercase();
        let idx = self.entries.iter().position(|e| {
            e.status.is_runnable() && e.dsl_source.to_lowercase().contains(&search_lower)
        })?;
        let mut entry = self.entries.remove(idx);
        entry.status = DslStatus::Cancelled;
        // Adjust cursor
        if self.cursor >= self.entries.len() && !self.entries.is_empty() {
            self.cursor = self.entries.len() - 1;
        } else if self.entries.is_empty() {
            self.cursor = 0;
        }
        Some(entry)
    }

    /// Convert to API type for responses
    pub fn to_api(&self) -> ob_poc_types::RunSheet {
        ob_poc_types::RunSheet {
            entries: self.entries.iter().map(|e| e.to_api()).collect(),
            cursor: self.cursor,
        }
    }
}

/// Single entry in the server-side run sheet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRunSheetEntry {
    /// Unique entry ID
    pub id: Uuid,
    /// DSL source text
    pub dsl_source: String,
    /// Parsed AST statements
    pub ast: Vec<Statement>,
    /// Compiled execution plan (toposorted, ready to run)
    #[serde(skip)]
    pub plan: Option<crate::dsl_v2::execution_plan::ExecutionPlan>,
    /// Current status
    pub status: DslStatus,
    /// When this was created
    pub created_at: DateTime<Utc>,
    /// When this was executed (if executed)
    pub executed_at: Option<DateTime<Utc>>,
    /// Entity IDs affected by execution
    pub affected_entities: Vec<Uuid>,
    /// Symbol bindings created by this entry
    pub bindings: HashMap<String, BoundEntity>,
    /// Error message if failed
    pub error: Option<String>,
}

impl ServerRunSheetEntry {
    /// Convert to API type
    pub fn to_api(&self) -> ob_poc_types::RunSheetEntry {
        ob_poc_types::RunSheetEntry {
            id: self.id.to_string(),
            dsl_source: self.dsl_source.clone(),
            display_dsl: None,
            status: match self.status {
                DslStatus::Draft => ob_poc_types::RunSheetEntryStatus::Draft,
                DslStatus::Ready => ob_poc_types::RunSheetEntryStatus::Ready,
                DslStatus::Executed => ob_poc_types::RunSheetEntryStatus::Executed,
                DslStatus::Cancelled => ob_poc_types::RunSheetEntryStatus::Cancelled,
                DslStatus::Failed => ob_poc_types::RunSheetEntryStatus::Failed,
            },
            created_at: Some(self.created_at.to_rfc3339()),
            executed_at: self.executed_at.map(|t| t.to_rfc3339()),
            affected_entities: self
                .affected_entities
                .iter()
                .map(|id| id.to_string())
                .collect(),
            bindings: self
                .bindings
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        ob_poc_types::BoundEntityInfo {
                            id: v.id.to_string(),
                            name: v.display_name.clone(),
                            entity_type: v.entity_type.clone(),
                        },
                    )
                })
                .collect(),
            error: self.error.clone(),
        }
    }
}

/// Pending DSL/AST pair in the session (not yet persisted to DB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDsl {
    /// Unique ID for this pending DSL fragment
    pub id: Uuid,
    /// The DSL source code
    pub source: String,
    /// Parsed AST statements
    pub ast: Vec<Statement>,
    /// Compiled execution plan (toposorted, ready to run)
    /// This is the VerbCall-based plan that the executor expects
    #[serde(skip)]
    pub plan: Option<crate::dsl_v2::execution_plan::ExecutionPlan>,
    /// Current status
    pub status: DslStatus,
    /// When this was created
    pub created_at: DateTime<Utc>,
    /// Error message if status is Failed
    pub error: Option<String>,
    /// Bindings that would be created if executed
    pub pending_bindings: HashMap<String, String>,
    /// Whether statements were reordered for dependency resolution
    pub was_reordered: bool,
}

impl PendingDsl {
    /// DSL for user display (in chat) - human readable, no UUIDs
    /// Shows entity names like "BlackRock ManCo" not UUIDs
    pub fn to_user_dsl(&self) -> String {
        self.ast
            .iter()
            .map(|s| s.to_user_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// DSL for execution/internal use (with resolved UUIDs)
    /// Note: Executor should use AST directly, not this string
    pub fn to_exec_dsl(&self) -> String {
        self.ast
            .iter()
            .map(|s| s.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ============================================================================
// Session Types
// ============================================================================

/// The main agent session - lives server-side
/// Session key = (user_id, entity_type, entity_id)
#[derive(Debug, Clone, Serialize)]
pub struct AgentSession {
    /// Unique session identifier (for backwards compat - may equal entity_id)
    pub id: Uuid,
    /// User ID for audit trail (nil UUID = anonymous/dev mode)
    pub user_id: Uuid,
    /// Entity type this session operates on ("cbu", "kyc_case", "onboarding", etc.)
    pub entity_type: String,
    /// Entity ID this session operates on (CBU ID, case ID, etc.)
    pub entity_id: Option<Uuid>,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last updated
    pub updated_at: DateTime<Utc>,
    /// Current state in the session lifecycle
    pub state: SessionState,

    // =========================================================================
    // Sub-Session Support
    // =========================================================================
    /// Parent session ID (None for root sessions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<Uuid>,

    /// Sub-session type (determines behavior and scoped capabilities)
    #[serde(default)]
    pub sub_session_type: SubSessionType,

    /// Symbols inherited from parent session (pre-populated on creation)
    /// These are available in validation context for reference resolution
    #[serde(default)]
    pub inherited_symbols: HashMap<String, BoundEntity>,

    /// Conversation history
    pub messages: Vec<ChatMessage>,

    /// Run sheet - DSL statement ledger with per-statement status
    /// Replaces: assembled_dsl, pending, executed_results
    #[serde(default)]
    pub run_sheet: ServerRunSheet,

    /// Pending verb intents during disambiguation flow
    /// Stored temporarily while user resolves entity ambiguities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_intents: Vec<super::intent::VerbIntent>,

    /// Context accumulated during session
    pub context: SessionContext,
}

impl AgentSession {
    /// Create a new session for an entity
    /// - user_id: None = anonymous/dev mode (nil UUID)
    /// - entity_type: "cbu", "kyc_case", "onboarding", etc.
    /// - entity_id: Some(uuid) if known, None if creating new
    pub fn new_for_entity(
        user_id: Option<Uuid>,
        entity_type: &str,
        entity_id: Option<Uuid>,
        domain_hint: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: entity_id.unwrap_or_else(Uuid::new_v4),
            user_id: user_id.unwrap_or(Uuid::nil()),
            entity_type: entity_type.to_string(),
            entity_id,
            created_at: now,
            updated_at: now,
            state: SessionState::New,
            // Sub-session fields (root session defaults)
            parent_session_id: None,
            sub_session_type: SubSessionType::Root,
            inherited_symbols: HashMap::new(),
            messages: Vec::new(),
            run_sheet: ServerRunSheet::default(),
            pending_intents: Vec::new(),
            context: SessionContext {
                domain_hint,
                ..Default::default()
            },
        }
    }

    /// Create a new CBU session (anonymous/dev mode)
    pub fn new(domain_hint: Option<String>) -> Self {
        Self::new_for_entity(None, "cbu", None, domain_hint)
    }

    /// Create a sub-session inheriting context from parent
    ///
    /// Sub-sessions are scoped agent conversations for specific workflows
    /// (resolution, research, review, correction). They inherit:
    /// - User ID from parent
    /// - Entity context from parent
    /// - Symbol bindings for reference resolution
    pub fn new_subsession(parent: &AgentSession, sub_session_type: SubSessionType) -> Self {
        let now = Utc::now();

        // Inherit symbols from parent's bindings
        let inherited_symbols = parent.context.bindings.clone();

        Self {
            id: Uuid::new_v4(),
            user_id: parent.user_id,
            entity_type: parent.entity_type.clone(),
            entity_id: parent.entity_id,
            created_at: now,
            updated_at: now,
            state: SessionState::New,
            // Sub-session specific
            parent_session_id: Some(parent.id),
            sub_session_type,
            inherited_symbols,
            messages: Vec::new(),
            run_sheet: ServerRunSheet::default(),
            pending_intents: Vec::new(),
            context: SessionContext {
                // Inherit key context from parent
                domain_hint: parent.context.domain_hint.clone(),
                last_cbu_id: parent.context.last_cbu_id,
                last_entity_id: parent.context.last_entity_id,
                active_cbu: parent.context.active_cbu.clone(),
                ..Default::default()
            },
        }
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

    /// Get all known symbols for validation (own bindings + inherited from parent)
    /// Returns HashMap<String, Uuid> suitable for ValidationContext::with_known_symbols
    pub fn all_known_symbols(&self) -> HashMap<String, Uuid> {
        let mut symbols = HashMap::new();

        // Add inherited symbols first (can be overridden by own bindings)
        for (name, bound) in &self.inherited_symbols {
            symbols.insert(name.clone(), bound.id);
        }

        // Add own bindings (from context)
        for (name, bound) in &self.context.bindings {
            symbols.insert(name.clone(), bound.id);
        }

        // Add named_refs (legacy UUID-only references)
        for (name, uuid) in &self.context.named_refs {
            symbols.insert(name.clone(), *uuid);
        }

        symbols
    }

    /// Set entity ID after creation (e.g., after cbu.ensure executes)
    pub fn set_entity_id(&mut self, entity_id: Uuid) {
        self.entity_id = Some(entity_id);
        self.id = entity_id; // Session ID follows entity ID
        self.updated_at = Utc::now();
    }

    // ========================================================================
    // State Machine - Single Entry Point for All State Transitions
    // ========================================================================

    /// Transition session state based on an event.
    ///
    /// This is the SINGLE entry point for all session state changes.
    /// All state transition logic lives here to ensure consistency.
    ///
    /// # State Machine
    /// ```text
    /// New ──[ScopeSet]──► Scoped ──[DslPendingValidation]──► PendingValidation
    ///                        │                                      │
    ///                        │◄─────────[Cancelled]─────────────────┘
    ///                        │
    ///                        └──[DslReady]──► ReadyToExecute ──[ExecutionStarted]──► Executing
    ///                                                │                                   │
    ///                                                │◄──[ExecutionCompleted]────────────┘
    ///                                                │
    ///                                                └──► (back to Scoped if has_scope, else Executed)
    /// ```
    pub fn transition(&mut self, event: SessionEvent) {
        use SessionEvent::*;
        use SessionState::*;

        let new_state = match (&self.state, event) {
            // Scope setting - from New or re-scoping from Scoped/Executed
            (New, ScopeSet) => Scoped,
            (Scoped, ScopeSet) => Scoped, // Re-scoping
            (Executed, ScopeSet) => Scoped,

            // DSL pending validation (unresolved refs)
            (New, DslPendingValidation) => PendingValidation, // Before scope set
            (Scoped, DslPendingValidation) => PendingValidation,
            (Executed, DslPendingValidation) => PendingValidation,

            // DSL ready to execute
            (New, DslReady) => ReadyToExecute, // Before scope set (e.g., session.load-*)
            (Scoped, DslReady) => ReadyToExecute,
            (PendingValidation, DslReady) => ReadyToExecute,
            (Executed, DslReady) => ReadyToExecute,

            // Execution lifecycle
            (ReadyToExecute, ExecutionStarted) => Executing,
            (Executing, ExecutionCompleted) => self.compute_post_execution_state(),

            // Cancellation returns to appropriate state
            (New, Cancelled) => New,       // No-op if nothing to cancel
            (Scoped, Cancelled) => Scoped, // Stay scoped
            (PendingValidation, Cancelled) => self.compute_idle_state(),
            (ReadyToExecute, Cancelled) => self.compute_idle_state(),
            (Executed, Cancelled) => self.compute_idle_state(),

            // Close from any state
            (_, Close) => Closed,

            // Invalid transitions - log and keep current state
            (current, event) => {
                tracing::warn!(
                    "Invalid session state transition: {:?} + {:?} (session {})",
                    current,
                    event,
                    self.id
                );
                return; // Don't update timestamp for invalid transitions
            }
        };

        tracing::debug!(
            "Session {} state transition: {:?} -> {:?}",
            self.id,
            self.state,
            new_state
        );
        self.state = new_state;
        self.updated_at = Utc::now();
    }

    /// Compute the appropriate idle state based on whether scope is set
    fn compute_idle_state(&self) -> SessionState {
        if self.has_scope_set() {
            SessionState::Scoped
        } else {
            SessionState::New
        }
    }

    /// Compute state after execution completes
    fn compute_post_execution_state(&self) -> SessionState {
        if self.has_scope_set() {
            SessionState::Scoped
        } else {
            SessionState::Executed
        }
    }

    /// Check if session has scope set (CBUs loaded)
    pub fn has_scope_set(&self) -> bool {
        !self.context.cbu_ids.is_empty() || self.context.has_scope()
    }

    /// Convenience: update state after operations complete
    /// Determines appropriate state based on current session context
    pub fn update_state(&mut self) {
        // Determine what just happened and transition appropriately
        if self.has_scope_set() && self.state == SessionState::New {
            self.transition(SessionEvent::ScopeSet);
        } else if self.run_sheet.has_runnable() {
            // Has pending DSL ready to run
            self.transition(SessionEvent::DslReady);
        } else if self.state == SessionState::Executing {
            self.transition(SessionEvent::ExecutionCompleted);
        }
        // Otherwise keep current state
        self.updated_at = Utc::now();
    }

    // ========================================================================
    // DSL Management
    // ========================================================================

    /// Set pending DSL (parsed, validated, and planned - ready for user confirmation)
    pub fn set_pending_dsl(
        &mut self,
        source: String,
        ast: Vec<Statement>,
        plan: Option<crate::dsl_v2::execution_plan::ExecutionPlan>,
        _was_reordered: bool,
    ) {
        // Add to run sheet as draft entry
        let id = self.run_sheet.add_draft(source, ast);
        // Set plan on the entry
        if let Some(entry) = self.run_sheet.get_mut(id) {
            entry.plan = plan;
        }
        self.transition(SessionEvent::DslReady);
    }

    /// Cancel pending DSL (user declined)
    pub fn cancel_pending(&mut self) {
        // Cancel last draft entry
        self.run_sheet.undo_last();
        self.transition(SessionEvent::Cancelled);
    }

    /// Mark pending DSL as ready to execute (user confirmed)
    pub fn confirm_pending(&mut self) {
        // Mark current entry as ready
        if let Some(entry) = self.run_sheet.current_mut() {
            entry.status = DslStatus::Ready;
        }
        self.updated_at = Utc::now();
    }

    /// Mark current run sheet entry as executed (after successful execution)
    pub fn mark_executed(&mut self) {
        if let Some(entry) = self.run_sheet.current_mut() {
            entry.status = DslStatus::Executed;
            entry.executed_at = Some(Utc::now());
        }
        self.update_state();
    }

    /// Mark current run sheet entry as failed (execution error)
    pub fn mark_failed(&mut self, error: String) {
        if let Some(entry) = self.run_sheet.current_mut() {
            entry.status = DslStatus::Failed;
            entry.error = Some(error);
        }
        // Execution completed (with failure) - return to idle state
        self.transition(SessionEvent::ExecutionCompleted);
    }

    /// Get current runnable entry from run sheet
    pub fn get_runnable_dsl(&self) -> Option<&ServerRunSheetEntry> {
        self.run_sheet.current().filter(|e| e.status.is_runnable())
    }

    /// Check if there's runnable DSL in the run sheet
    pub fn has_pending(&self) -> bool {
        self.run_sheet.has_runnable()
    }

    /// Add a user message to the session
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

    /// Add an agent message to the session
    pub fn add_agent_message(
        &mut self,
        content: String,
        intents: Option<Vec<VerbIntent>>,
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

    /// Add intents and transition state
    pub fn add_intents(&mut self, intents: Vec<VerbIntent>) {
        self.pending_intents.extend(intents);
        self.transition(SessionEvent::DslPendingValidation);
    }

    /// Set assembled DSL after validation (keep intents for execution-time resolution)
    /// Deprecated: Use set_pending_dsl which adds to run_sheet
    pub fn set_assembled_dsl(&mut self, dsl: Vec<String>) {
        // Add each DSL string to run sheet as draft
        for source in dsl {
            self.run_sheet.add_draft(source, Vec::new());
        }
        // NOTE: Don't clear pending_intents - we need them for execution-time ref resolution
        self.transition(SessionEvent::DslReady);
    }

    /// Clear draft DSL entries
    pub fn clear_assembled_dsl(&mut self) {
        self.run_sheet.clear_drafts();
        self.transition(SessionEvent::Cancelled);
    }

    /// Record execution results and update context
    pub fn record_execution(&mut self, results: Vec<ExecutionResult>) {
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

        // Mark executed entries in run sheet (execution results are now per-entry)
        // For now, just update session state
        let _ = results; // Results stored per-entry via run_sheet.mark_executed()

        // Use centralized state transition
        self.transition(SessionEvent::ExecutionCompleted);
        self.updated_at = Utc::now();
    }

    /// Get all accumulated DSL as a single combined string
    pub fn combined_dsl(&self) -> String {
        self.run_sheet.combined_dsl().unwrap_or_default()
    }

    /// Check if the session can execute
    pub fn can_execute(&self) -> bool {
        self.state == SessionState::ReadyToExecute && self.run_sheet.has_runnable()
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID
    pub id: Uuid,
    /// Who sent this message
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// When the message was sent
    pub timestamp: DateTime<Utc>,
    /// Intents extracted from this message (if user message processed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intents: Option<Vec<VerbIntent>>,
    /// DSL generated from this message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl: Option<String>,
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Agent,
    System,
}

// ============================================================================
// Session Context
// ============================================================================

/// Information about a bound entity in the session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundEntity {
    /// The UUID of the entity
    pub id: Uuid,
    /// The entity type (e.g., "cbu", "entity", "case")
    pub entity_type: String,
    /// Human-readable display name (e.g., "Aviva Lux 9")
    pub display_name: String,
}

// ============================================================================
// Batch Context - For bulk REPL operations
// ============================================================================

// ============================================================================
// Progress Structs (replacing anonymous tuples for function returns)
// ============================================================================

/// Status counts for batch items: pending/completed/skipped/failed
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BatchStatusCounts {
    pub pending: usize,
    pub completed: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Progress tracking for batch execution: processed/total/success/failed
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BatchProgress {
    pub processed: usize,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
}

/// Resolution progress: resolved/total
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResolutionProgress {
    pub resolved: usize,
    pub total: usize,
}

/// Status of a batch item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BatchItemStatus {
    /// Not yet processed
    #[default]
    Pending,
    /// Currently being processed (DSL in editor)
    Active,
    /// Successfully executed
    Completed,
    /// Skipped by user
    Skipped,
    /// Execution failed
    Failed,
}

/// A single item in the batch working set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItem {
    /// Source entity ID (e.g., fund entity that will become a CBU)
    pub source_id: Uuid,
    /// Display name for the item
    pub name: String,
    /// Source entity type (e.g., "fund", "entity")
    pub source_type: String,
    /// Additional metadata for display/context
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Processing status
    #[serde(default)]
    pub status: BatchItemStatus,
    /// Created entity ID after execution (e.g., the CBU ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_id: Option<Uuid>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Template Key Set - Collected entity references for template expansion
// ============================================================================

/// A resolved entity reference for template expansion
/// This is the LookupRef triplet: (entity_type, search_key, uuid)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEntityRef {
    /// Entity type (e.g., "fund", "limited_company")
    pub entity_type: String,
    /// Human-readable search key / display name
    pub display_name: String,
    /// Resolved UUID from EntityGateway
    pub entity_id: Uuid,
    /// Optional metadata (jurisdiction, date, etc.)
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

/// Key set for a template parameter
/// Captures what the agent has collected for a specific param
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParamKeySet {
    /// Parameter name from template (e.g., "fund_entity", "manco_entity")
    pub param_name: String,
    /// Entity type expected (e.g., "fund", "limited_company")
    pub entity_type: String,
    /// Cardinality: "batch" (one per iteration) or "shared" (same for all)
    pub cardinality: String,
    /// The collected entities
    pub entities: Vec<ResolvedEntityRef>,
    /// Whether collection is complete (user confirmed)
    #[serde(default)]
    pub is_complete: bool,
    /// Search/filter description used to find these (for audit)
    #[serde(default)]
    pub filter_description: String,
}

/// Agent's working memory for template-driven batch execution
/// This is what the agent reads/writes across conversation turns
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateExecutionContext {
    /// Template being used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,

    /// Current phase of template execution
    #[serde(default)]
    pub phase: TemplatePhase,

    /// Key sets collected for each template parameter
    /// Key = param_name, Value = collected entities
    #[serde(default)]
    pub key_sets: HashMap<String, TemplateParamKeySet>,

    /// Scalar params (non-entity values like jurisdiction, dates)
    #[serde(default)]
    pub scalar_params: HashMap<String, String>,

    /// Which batch item is currently being processed (0-indexed)
    #[serde(default)]
    pub current_batch_index: usize,

    /// Results from each batch item execution
    #[serde(default)]
    pub batch_results: Vec<BatchItemResult>,

    /// Whether to auto-continue without prompting
    #[serde(default)]
    pub auto_execute: bool,
}

/// Phase of template execution workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TemplatePhase {
    /// Agent is identifying which template to use
    #[default]
    SelectingTemplate,
    /// Agent is collecting shared params (same for all batch items)
    CollectingSharedParams,
    /// Agent is collecting batch params (the iteration set)
    CollectingBatchParams,
    /// User is reviewing collected key sets before execution
    ReviewingKeySets,
    /// Executing batch items one by one
    Executing,
    /// All items processed
    Complete,
}

/// Result from executing one batch item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemResult {
    /// Index in the batch
    pub index: usize,
    /// Source entity that was processed
    pub source_entity: ResolvedEntityRef,
    /// Whether execution succeeded
    pub success: bool,
    /// Created entity ID (e.g., the new CBU)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_id: Option<Uuid>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The DSL that was executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_dsl: Option<String>,
}

impl TemplateExecutionContext {
    /// Check if all required key sets are complete
    pub fn all_key_sets_complete(&self) -> bool {
        self.key_sets.values().all(|ks| ks.is_complete)
    }

    /// Get the batch key set (the one we iterate over)
    pub fn batch_key_set(&self) -> Option<&TemplateParamKeySet> {
        self.key_sets.values().find(|ks| ks.cardinality == "batch")
    }

    /// Get count of batch items to process
    pub fn batch_size(&self) -> usize {
        self.batch_key_set()
            .map(|ks| ks.entities.len())
            .unwrap_or(0)
    }

    /// Get current batch item being processed
    pub fn current_batch_entity(&self) -> Option<&ResolvedEntityRef> {
        self.batch_key_set()
            .and_then(|ks| ks.entities.get(self.current_batch_index))
    }

    /// Get shared entities (same for all batch items)
    pub fn shared_entities(&self) -> Vec<(&str, &ResolvedEntityRef)> {
        self.key_sets
            .iter()
            .filter(|(_, ks)| ks.cardinality == "shared")
            .flat_map(|(name, ks)| ks.entities.first().map(|e| (name.as_str(), e)))
            .collect()
    }

    /// Advance to next batch item, returns true if more to process
    pub fn advance(&mut self) -> bool {
        self.current_batch_index += 1;
        self.current_batch_index < self.batch_size()
    }

    /// Get progress string like "3/10 complete"
    pub fn progress_string(&self) -> String {
        let total = self.batch_size();
        let completed = self.batch_results.iter().filter(|r| r.success).count();
        let failed = self.batch_results.iter().filter(|r| !r.success).count();
        if failed > 0 {
            format!("{}/{} complete, {} failed", completed, total, failed)
        } else {
            format!("{}/{} complete", completed, total)
        }
    }

    /// Check if template execution is active
    pub fn is_active(&self) -> bool {
        self.template_id.is_some() && self.phase != TemplatePhase::Complete
    }

    /// Reset for a new template execution
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Batch context for bulk REPL operations
/// Holds the "working set" of entities the agent and user are processing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchContext {
    /// Whether batch mode is active
    #[serde(default)]
    pub is_active: bool,
    /// Source entity type we're iterating over (e.g., "fund")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_entity_type: Option<String>,
    /// Filter criteria used to build the set (for display/audit)
    #[serde(default)]
    pub filter_description: String,
    /// The batch working set - entities to process
    #[serde(default)]
    pub items: Vec<BatchItem>,
    /// Current index in the batch (which item is active)
    #[serde(default)]
    pub current_index: usize,
    /// Template being used for expansion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    /// Auto-continue mode (run all without prompting)
    #[serde(default)]
    pub auto_continue: bool,
    /// Common bindings shared across all batch items (e.g., @manco, @im)
    #[serde(default)]
    pub shared_bindings: HashMap<String, BoundEntity>,
}

impl BatchContext {
    /// Get the current batch item being processed
    pub fn current_item(&self) -> Option<&BatchItem> {
        self.items.get(self.current_index)
    }

    /// Get mutable reference to current batch item
    pub fn current_item_mut(&mut self) -> Option<&mut BatchItem> {
        self.items.get_mut(self.current_index)
    }

    /// Advance to next pending item, returns true if there's more to process
    pub fn advance_to_next(&mut self) -> bool {
        for i in (self.current_index + 1)..self.items.len() {
            if self.items[i].status == BatchItemStatus::Pending {
                self.current_index = i;
                return true;
            }
        }
        false
    }

    /// Count items by status
    pub fn count_by_status(&self) -> BatchStatusCounts {
        let mut counts = BatchStatusCounts::default();
        for item in &self.items {
            match item.status {
                BatchItemStatus::Pending | BatchItemStatus::Active => counts.pending += 1,
                BatchItemStatus::Completed => counts.completed += 1,
                BatchItemStatus::Skipped => counts.skipped += 1,
                BatchItemStatus::Failed => counts.failed += 1,
            }
        }
        counts
    }

    /// Get progress string like "5/15 complete"
    pub fn progress_string(&self) -> String {
        let counts = self.count_by_status();
        let total = self.items.len();
        if counts.failed > 0 {
            format!(
                "{}/{} complete, {} failed",
                counts.completed, total, counts.failed
            )
        } else if counts.skipped > 0 {
            format!(
                "{}/{} complete, {} skipped",
                counts.completed, total, counts.skipped
            )
        } else {
            format!("{}/{} complete", counts.completed, total)
        }
    }
}

// ============================================================================
// Active Batch State - For DSL-native template.batch pause/resume
// ============================================================================

/// Status of a DSL-native batch execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    /// Batch is actively running
    #[default]
    Running,
    /// Batch is paused (can resume)
    Paused,
    /// Batch completed successfully
    Completed,
    /// Batch failed (cannot resume)
    Failed,
    /// Batch was aborted by user
    Aborted,
}

/// Active batch execution state for pause/resume support
///
/// This is the session-persisted state for DSL-native `template.batch` execution.
/// Unlike `TemplateExecutionContext` which is for agent-driven conversational batch,
/// this is for programmatic batch execution that can be paused/resumed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveBatchState {
    /// Template being executed
    pub template_id: String,

    /// Source query that generated the item list (for audit/retry)
    pub source_query: String,

    /// Bind parameter name (e.g., "fund_entity")
    pub bind_param: String,

    /// Shared parameters (same for all iterations)
    pub shared_params: HashMap<String, String>,

    /// Resolved shared bindings (e.g., @manco -> UUID)
    pub shared_bindings: HashMap<String, Uuid>,

    /// Total items to process
    pub total_items: usize,

    /// Current position (0-indexed, next item to process)
    pub current_index: usize,

    /// Remaining items to process: (entity_id, display_name)
    pub remaining_items: Vec<(Uuid, String)>,

    /// Accumulated results so far
    #[serde(skip)]
    pub results: BatchResultAccumulator,

    /// Error handling mode: "continue", "stop", "rollback"
    pub on_error: String,

    /// Current batch status
    pub status: BatchStatus,

    /// When batch started
    pub started_at: DateTime<Utc>,

    /// When batch was paused (if paused)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused_at: Option<DateTime<Utc>>,

    /// When batch completed/failed/aborted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,

    /// Error message if status is Failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for ActiveBatchState {
    fn default() -> Self {
        Self {
            template_id: String::new(),
            source_query: String::new(),
            bind_param: String::new(),
            shared_params: HashMap::new(),
            shared_bindings: HashMap::new(),
            total_items: 0,
            current_index: 0,
            remaining_items: Vec::new(),
            results: BatchResultAccumulator::default(),
            on_error: "continue".to_string(),
            status: BatchStatus::Running,
            started_at: Utc::now(),
            paused_at: None,
            ended_at: None,
            error: None,
        }
    }
}

impl ActiveBatchState {
    /// Create a new active batch state
    pub fn new(
        template_id: impl Into<String>,
        source_query: impl Into<String>,
        bind_param: impl Into<String>,
        items: Vec<(Uuid, String)>,
        shared_params: HashMap<String, String>,
        on_error: impl Into<String>,
    ) -> Self {
        Self {
            template_id: template_id.into(),
            source_query: source_query.into(),
            bind_param: bind_param.into(),
            total_items: items.len(),
            remaining_items: items,
            shared_params,
            on_error: on_error.into(),
            started_at: Utc::now(),
            ..Default::default()
        }
    }

    /// Pause the batch execution
    pub fn pause(&mut self) {
        if self.status == BatchStatus::Running {
            self.status = BatchStatus::Paused;
            self.paused_at = Some(Utc::now());
        }
    }

    /// Resume the batch execution
    pub fn resume(&mut self) {
        if self.status == BatchStatus::Paused {
            self.status = BatchStatus::Running;
            self.paused_at = None;
        }
    }

    /// Mark batch as completed
    pub fn complete(&mut self) {
        self.status = BatchStatus::Completed;
        self.ended_at = Some(Utc::now());
    }

    /// Mark batch as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = BatchStatus::Failed;
        self.ended_at = Some(Utc::now());
        self.error = Some(error.into());
    }

    /// Abort the batch
    pub fn abort(&mut self) {
        self.status = BatchStatus::Aborted;
        self.ended_at = Some(Utc::now());
    }

    /// Get the next item to process
    pub fn next_item(&self) -> Option<&(Uuid, String)> {
        self.remaining_items.first()
    }

    /// Advance to next item, returning the item that was processed
    pub fn advance(&mut self) -> Option<(Uuid, String)> {
        if !self.remaining_items.is_empty() {
            self.current_index += 1;
            Some(self.remaining_items.remove(0))
        } else {
            None
        }
    }

    /// Skip the current item
    pub fn skip_current(&mut self) -> Option<(Uuid, String)> {
        self.advance()
    }

    /// Check if batch can continue
    pub fn can_continue(&self) -> bool {
        self.status == BatchStatus::Running && !self.remaining_items.is_empty()
    }

    /// Check if batch is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            BatchStatus::Completed | BatchStatus::Failed | BatchStatus::Aborted
        )
    }

    /// Get progress as BatchProgress struct
    pub fn progress(&self) -> BatchProgress {
        BatchProgress {
            processed: self.current_index,
            total: self.total_items,
            success: self.results.success_count,
            failed: self.results.failure_count,
        }
    }

    /// Get progress string like "47/205 (45 success, 2 failed)"
    pub fn progress_string(&self) -> String {
        let p = self.progress();
        if p.failed > 0 {
            format!(
                "{}/{} ({} success, {} failed)",
                p.processed, p.total, p.success, p.failed
            )
        } else {
            format!("{}/{} complete", p.processed, p.total)
        }
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> chrono::Duration {
        let end = self.ended_at.unwrap_or_else(Utc::now);
        end - self.started_at
    }

    /// Get elapsed time as human-readable string
    pub fn elapsed_string(&self) -> String {
        let elapsed = self.elapsed();
        let secs = elapsed.num_seconds();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{:02}:{:02}", secs / 60, secs % 60)
        } else {
            format!(
                "{:02}:{:02}:{:02}",
                secs / 3600,
                (secs % 3600) / 60,
                secs % 60
            )
        }
    }

    /// Get status summary for batch.status verb
    pub fn status_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "template": self.template_id,
            "status": self.status,
            "progress": {
                "current": self.current_index,
                "total": self.total_items,
                "success": self.results.success_count,
                "failed": self.results.failure_count,
                "remaining": self.remaining_items.len(),
            },
            "current_item": self.next_item().map(|(_, name)| name.clone()),
            "elapsed": self.elapsed_string(),
            "on_error": self.on_error,
            "error": self.error,
        })
    }
}

/// Context maintained across the session for reference resolution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionContext {
    /// Version of business_reference when loaded (for optimistic locking)
    /// When saving, this version must match the DB version or we get a conflict
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_dsl_version: Option<i32>,

    /// Business reference for this session's DSL instance (e.g., CBU name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_reference: Option<String>,

    /// Taxonomy navigation stack for fractal drill-down
    #[serde(default)]
    pub taxonomy_stack: crate::taxonomy::TaxonomyStack,

    // =========================================================================
    // Taxonomy-Driven Layout Fields
    // =========================================================================
    /// Current navigation scope path (e.g., /Universe/Book/CBU/Entity)
    /// Tracks where the user is in the hierarchical navigation
    #[serde(default)]
    pub scope_path: crate::session::ScopePath,

    /// Structural mass of the current scope - weighted complexity measure
    /// Used for automatic view mode selection (Detail < SolarSystem < Universe)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub struct_mass: Option<crate::session::StructMass>,

    /// Cached mass breakdown for quick access without recomputing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass_breakdown: Option<crate::session::MassBreakdown>,

    /// Auto-selected view mode based on structural mass
    /// Overridden if user manually selects a view mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_view_mode: Option<crate::session::MassViewMode>,

    /// Whether the current view_mode was manually selected (overrides auto)
    #[serde(default)]
    pub view_mode_manual: bool,

    /// Most recently created CBU
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_cbu_id: Option<Uuid>,
    /// Most recently created entity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_entity_id: Option<Uuid>,
    /// All CBUs created in this session
    #[serde(default)]
    pub cbu_ids: Vec<Uuid>,
    /// All entities created in this session
    #[serde(default)]
    pub entity_ids: Vec<Uuid>,
    /// Domain hint for RAG context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_hint: Option<String>,
    /// Named references for complex workflows (legacy - UUID only)
    #[serde(default)]
    pub named_refs: HashMap<String, Uuid>,
    /// Typed bindings with display names for LLM context (populated after execution)
    #[serde(default)]
    pub bindings: HashMap<String, BoundEntity>,
    /// Pending bindings from assembled DSL that hasn't been executed yet
    /// Format: binding_name -> (inferred_type, display_name)
    /// These are extracted from :as @name patterns in DSL
    #[serde(default)]
    pub pending_bindings: HashMap<String, (String, String)>,
    /// The accumulated AST - source of truth for the session's DSL
    /// Each chat message can add/modify statements in this AST
    #[serde(default)]
    pub ast: Vec<Statement>,
    /// Index from binding name to AST statement index
    /// Allows lookup like: get_ast_by_key("cbu_id") → returns the Statement that created it
    #[serde(default)]
    pub ast_index: HashMap<String, usize>,
    /// The ACTIVE CBU for this session - used as implicit context for incremental operations
    /// When set, operations like cbu.add-product will auto-use this CBU ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_cbu: Option<BoundEntity>,
    /// Currently focused stage in the onboarding journey
    /// When set, agent verb suggestions are filtered to this stage's relevant verbs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_focus: Option<String>,
    /// Primary domain keys - the main identifiers for this onboarding session
    #[serde(default)]
    pub primary_keys: PrimaryDomainKeys,
    /// Batch context for bulk REPL operations (legacy - use template_execution instead)
    /// When active, session iterates over a set of entities (e.g., funds → CBUs)
    #[serde(default)]
    pub batch: BatchContext,
    /// Current session mode - determines how input is processed
    #[serde(default)]
    pub mode: SessionMode,
    /// Template execution context - agent's working memory for batch operations
    /// This holds the key sets, template state, and execution progress
    #[serde(default)]
    pub template_execution: TemplateExecutionContext,

    /// Active DSL-native batch execution state (for template.batch pause/resume)
    /// This is separate from template_execution which is for conversational batch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_batch: Option<ActiveBatchState>,

    /// Research macro state - tracks pending results and approvals
    #[serde(default)]
    pub research: crate::session::ResearchContext,

    /// View state from view.* operations (universe, book, cbu, entity-forest)
    /// This captures what the user is currently viewing - the unified "it" that
    /// operations target. Populated after DSL execution when view.* verbs run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_state: Option<crate::session::ViewState>,

    /// Viewport state from viewport.* DSL verbs (focus, enhance, filter, camera)
    /// This tracks the CBU-focused viewport with focus state machine, enhance levels,
    /// camera state, and filters. Populated after DSL execution when viewport.* verbs run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_state: Option<ob_poc_types::ViewportState>,

    /// Session scope from session.* DSL verbs (set-galaxy, set-cbu, set-jurisdiction, etc.)
    /// This defines what data the user is operating on - the "where" for all operations.
    /// Populated after DSL execution when session.* scope verbs run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<crate::session::SessionScope>,

    // =========================================================================
    // Client Scope Context - For entity resolution within client group
    // =========================================================================
    /// Client group scope from Stage 0 scope resolution.
    /// When set, entity searches are filtered to this client group.
    /// Set by IntentPipeline when user says "work on allianz" etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_scope: Option<ScopeContext>,

    // =========================================================================
    // View State Fields - For REPL/View synchronization
    // =========================================================================
    /// Current view mode (e.g., "KYC_UBO", "UBO_ONLY", "SERVICE_DELIVERY", "PRODUCTS_ONLY")
    /// Determines which layers and edges are visible in the graph visualization.
    /// Syncs between UI graph panel and REPL session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_mode: Option<String>,

    /// Current zoom level for the graph visualization (0.0 - 1.0+ range)
    /// Persisted so zoom state is maintained across page refreshes and
    /// synchronized between REPL and graph view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom_level: Option<f32>,

    /// Set of expanded node IDs in the current view.
    /// Tracks which nodes have been expanded to show children.
    /// Used for fractal navigation persistence.
    #[serde(default, skip_serializing_if = "std::collections::HashSet::is_empty")]
    pub expanded_nodes: std::collections::HashSet<Uuid>,

    // =========================================================================
    // Learning Loop Fields
    // =========================================================================
    /// Pending feedback row ID (bigserial) from chat handler
    /// Links dsl_generation_log to intent_feedback when DSL is executed
    /// Set by chat_session, consumed by execute_session_dsl
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_feedback_id: Option<i64>,

    /// Pending feedback interaction_id (UUID) for recording outcomes
    /// Used to call FeedbackService.record_outcome() after execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_interaction_id: Option<Uuid>,

    // =========================================================================
    // DSL Diff Tracking (for learning from user edits)
    // =========================================================================
    /// DSL as proposed by agent (before user edits in REPL)
    /// Set when agent generates DSL, compared against final_dsl on execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_dsl: Option<String>,

    /// Current DSL in REPL (may differ from proposed if user edited)
    /// Updated via REPL edit events, used to compute diff on execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_dsl: Option<String>,
}

/// Primary domain keys tracked across the session
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrimaryDomainKeys {
    /// Onboarding request ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarding_request_id: Option<Uuid>,
    /// Primary CBU being onboarded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cbu_id: Option<Uuid>,
    /// Primary KYC case for this onboarding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyc_case_id: Option<Uuid>,
    /// Primary document collection (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_batch_id: Option<Uuid>,
    /// Primary service resource instance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_instance_id: Option<Uuid>,
}

impl SessionContext {
    /// Resolve a reference like "@last_cbu" or "@last_entity"
    pub fn resolve_ref(&self, ref_name: &str) -> Option<String> {
        match ref_name {
            "@last_cbu" => self.last_cbu_id.map(|u| format!("\"{}\"", u)),
            "@last_entity" => self.last_entity_id.map(|u| format!("\"{}\"", u)),
            _ if ref_name.starts_with('@') => {
                let name = &ref_name[1..];
                self.named_refs.get(name).map(|u| format!("\"{}\"", u))
            }
            _ => None,
        }
    }

    /// Set a named reference
    pub fn set_named_ref(&mut self, name: &str, id: Uuid) {
        self.named_refs.insert(name.to_string(), id);
    }

    /// Set a typed binding with display name
    /// Returns the actual binding name used (may have suffix if collision)
    ///
    /// Special handling for "cbu" binding: always replaces (no suffix) since
    /// the UI's active CBU should always be @cbu, not @cbu_2, @cbu_3, etc.
    pub fn set_binding(
        &mut self,
        name: &str,
        id: Uuid,
        entity_type: &str,
        display_name: &str,
    ) -> String {
        // Special case: "cbu" binding always replaces - the UI's active CBU
        // should always be accessible as @cbu, not @cbu_2, @cbu_3
        let actual_name = if name == "cbu" {
            name.to_string()
        } else if self.bindings.contains_key(name) {
            // Handle collision - append suffix if name already exists
            let mut suffix = 2;
            loop {
                let candidate = format!("{}_{}", name, suffix);
                if !self.bindings.contains_key(&candidate) {
                    break candidate;
                }
                suffix += 1;
            }
        } else {
            name.to_string()
        };

        // Also set in named_refs for backward compatibility
        self.named_refs.insert(actual_name.clone(), id);
        self.bindings.insert(
            actual_name.clone(),
            BoundEntity {
                id,
                entity_type: entity_type.to_string(),
                display_name: display_name.to_string(),
            },
        );

        actual_name
    }

    /// Get bindings formatted for LLM context
    /// Returns strings like "@aviva_lux_9 (CBU: Aviva Lux 9)"
    pub fn bindings_for_llm(&self) -> Vec<String> {
        self.bindings
            .iter()
            .map(|(name, binding)| {
                format!(
                    "@{} ({}: {})",
                    name,
                    binding.entity_type.to_uppercase(),
                    binding.display_name
                )
            })
            .collect()
    }

    /// Get the active CBU context formatted for LLM
    /// Returns something like "ACTIVE_CBU: Aviva Lux 9 (uuid: 327804f8-...)"
    pub fn active_cbu_for_llm(&self) -> Option<String> {
        self.active_cbu
            .as_ref()
            .map(|cbu| format!("ACTIVE_CBU: \"{}\" (id: {})", cbu.display_name, cbu.id))
    }

    /// Get the session scope context formatted for LLM
    /// Returns multi-CBU scope info for bulk operations
    pub fn scope_context_for_llm(&self) -> Option<String> {
        if self.cbu_ids.is_empty() {
            return None;
        }

        let count = self.cbu_ids.len();
        if count == 1 {
            // Single CBU - just reference active_cbu
            return None;
        }

        // Multi-CBU scope - provide count and hint for bulk operations
        Some(format!(
            "SESSION_SCOPE: {} CBUs in scope. Use @session_cbus for bulk operations that should apply to all CBUs in scope.",
            count
        ))
    }

    /// Set the active CBU for this session
    pub fn set_active_cbu(&mut self, id: Uuid, display_name: &str) {
        self.active_cbu = Some(BoundEntity {
            id,
            entity_type: "cbu".to_string(),
            display_name: display_name.to_string(),
        });
    }

    /// Clear the active CBU
    pub fn clear_active_cbu(&mut self) {
        self.active_cbu = None;
    }

    // =========================================================================
    // AST MANIPULATION
    // =========================================================================

    /// Add statements to the AST
    pub fn add_statements(&mut self, statements: Vec<Statement>) {
        for stmt in statements {
            self.add_statement(stmt);
        }
    }

    /// Add a single statement to the AST, indexing by binding name if present
    pub fn add_statement(&mut self, statement: Statement) {
        let idx = self.ast.len();

        // If statement has a binding (:as @name), index it
        if let Statement::VerbCall(ref verb_call) = statement {
            if let Some(ref binding_name) = verb_call.binding {
                self.ast_index.insert(binding_name.to_string(), idx);

                // Also update primary keys based on domain
                let domain = &verb_call.domain;
                if domain == "cbu" && self.primary_keys.cbu_id.is_none() {
                    // Will be set when we get the UUID from execution
                }
                if domain == "kyc-case" && self.primary_keys.kyc_case_id.is_none() {
                    // Will be set when we get the UUID from execution
                }
            }
        }

        self.ast.push(statement);
    }

    /// Get AST statement by binding key (e.g., "cbu_id" → the cbu.ensure statement)
    pub fn get_ast_by_key(&self, key: &str) -> Option<&Statement> {
        self.ast_index.get(key).and_then(|&idx| self.ast.get(idx))
    }

    /// Get AST statement index by binding key
    pub fn get_ast_index_by_key(&self, key: &str) -> Option<usize> {
        self.ast_index.get(key).copied()
    }

    /// Update primary keys from execution result
    pub fn update_primary_key(&mut self, domain: &str, binding: &str, id: Uuid) {
        match domain {
            "cbu" => {
                if self.primary_keys.cbu_id.is_none() {
                    self.primary_keys.cbu_id = Some(id);
                }
            }
            "kyc-case" => {
                if self.primary_keys.kyc_case_id.is_none() {
                    self.primary_keys.kyc_case_id = Some(id);
                }
            }
            "service-resource" => {
                if self.primary_keys.resource_instance_id.is_none() {
                    self.primary_keys.resource_instance_id = Some(id);
                }
            }
            _ => {}
        }
        // Also index by the specific binding name
        if let Some(idx) = self.ast_index.get(binding) {
            // Already indexed when statement was added
            let _ = idx;
        }
    }

    /// Get the AST as a Program for compilation/execution
    pub fn as_program(&self) -> Program {
        Program {
            statements: self.ast.clone(),
        }
    }

    /// Render the AST back to DSL source for display
    pub fn to_dsl_source(&self) -> String {
        self.ast
            .iter()
            .map(|s| s.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Find a statement by binding name
    pub fn find_by_binding(&self, binding_name: &str) -> Option<&Statement> {
        self.ast.iter().find(|s| {
            if let Statement::VerbCall(vc) = s {
                vc.binding.as_deref() == Some(binding_name)
            } else {
                false
            }
        })
    }

    /// Update a statement's argument value by binding name
    pub fn update_arg(
        &mut self,
        binding_name: &str,
        arg_key: &str,
        new_value: crate::dsl_v2::ast::AstNode,
    ) -> bool {
        for stmt in &mut self.ast {
            if let Statement::VerbCall(vc) = stmt {
                if vc.binding.as_deref() == Some(binding_name) {
                    for arg in &mut vc.arguments {
                        if arg.key == arg_key {
                            arg.value = new_value;
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Remove a statement by binding name
    pub fn remove_by_binding(&mut self, binding_name: &str) -> bool {
        let original_len = self.ast.len();
        self.ast.retain(|s| {
            if let Statement::VerbCall(vc) = s {
                vc.binding.as_deref() != Some(binding_name)
            } else {
                true
            }
        });
        self.ast.len() != original_len
    }

    /// Clear all AST statements
    pub fn clear_ast(&mut self) {
        self.ast.clear();
    }

    /// Get count of statements
    pub fn statement_count(&self) -> usize {
        self.ast.len()
    }

    // =========================================================================
    // VIEW STATE METHODS - For view.* verb output propagation
    // =========================================================================

    /// Set the view state from view.* operations
    /// This is called after DSL execution when view.* verbs produce a ViewState
    pub fn set_view_state(&mut self, view: crate::session::ViewState) {
        self.view_state = Some(view);
    }

    /// Get the current view state
    pub fn view_state(&self) -> Option<&crate::session::ViewState> {
        self.view_state.as_ref()
    }

    /// Take the view state (consumes it)
    pub fn take_view_state(&mut self) -> Option<crate::session::ViewState> {
        self.view_state.take()
    }

    /// Check if there's a view state set
    pub fn has_view_state(&self) -> bool {
        self.view_state.is_some()
    }

    // =========================================================================
    // VIEWPORT STATE METHODS - For viewport.* verb output propagation
    // =========================================================================

    /// Set the viewport state from viewport.* operations
    /// This is called after DSL execution when viewport.* verbs produce a ViewportState
    pub fn set_viewport_state(&mut self, state: ob_poc_types::ViewportState) {
        self.viewport_state = Some(state);
    }

    /// Get the current viewport state
    pub fn viewport_state(&self) -> Option<&ob_poc_types::ViewportState> {
        self.viewport_state.as_ref()
    }

    /// Get mutable reference to the viewport state
    pub fn viewport_state_mut(&mut self) -> Option<&mut ob_poc_types::ViewportState> {
        self.viewport_state.as_mut()
    }

    /// Take the viewport state (consumes it)
    pub fn take_viewport_state(&mut self) -> Option<ob_poc_types::ViewportState> {
        self.viewport_state.take()
    }

    /// Check if there's a viewport state set
    pub fn has_viewport_state(&self) -> bool {
        self.viewport_state.is_some()
    }

    /// Get or initialize the viewport state with default
    /// Useful for operations that need to modify viewport state
    pub fn viewport_state_or_default(&mut self) -> &mut ob_poc_types::ViewportState {
        if self.viewport_state.is_none() {
            self.viewport_state = Some(ob_poc_types::ViewportState::default());
        }
        self.viewport_state.as_mut().unwrap()
    }

    // =========================================================================
    // SCOPE METHODS - For session.* verb output propagation
    // =========================================================================

    /// Set the session scope from session.* operations
    /// This is called after DSL execution when session.set-* verbs produce a scope change
    pub fn set_scope(&mut self, scope: crate::session::SessionScope) {
        self.scope = Some(scope);
    }

    /// Get the current session scope
    pub fn scope(&self) -> Option<&crate::session::SessionScope> {
        self.scope.as_ref()
    }

    /// Take the session scope (consumes it)
    pub fn take_scope(&mut self) -> Option<crate::session::SessionScope> {
        self.scope.take()
    }

    /// Check if there's a session scope set
    pub fn has_scope(&self) -> bool {
        self.scope.is_some()
    }

    /// Get or initialize the scope with empty
    /// Useful for operations that need to modify scope
    pub fn scope_or_default(&mut self) -> &mut crate::session::SessionScope {
        if self.scope.is_none() {
            self.scope = Some(crate::session::SessionScope::empty());
        }
        self.scope.as_mut().unwrap()
    }

    // =========================================================================
    // CLIENT SCOPE METHODS - For client group-scoped entity resolution
    // =========================================================================

    /// Set the client scope from Stage 0 scope resolution
    pub fn set_client_scope(&mut self, scope: ScopeContext) {
        self.client_scope = Some(scope);
    }

    /// Get the current client scope
    pub fn client_scope(&self) -> Option<&ScopeContext> {
        self.client_scope.as_ref()
    }

    /// Take the client scope (consumes it)
    pub fn take_client_scope(&mut self) -> Option<ScopeContext> {
        self.client_scope.take()
    }

    /// Check if client scope is set
    pub fn has_client_scope(&self) -> bool {
        self.client_scope
            .as_ref()
            .map(|s| s.has_scope())
            .unwrap_or(false)
    }

    /// Get the client group ID if scope is set
    pub fn client_group_id(&self) -> Option<Uuid> {
        self.client_scope.as_ref().and_then(|s| s.client_group_id)
    }

    /// Get the client group name if scope is set
    pub fn client_group_name(&self) -> Option<&str> {
        self.client_scope
            .as_ref()
            .and_then(|s| s.client_group_name.as_deref())
    }

    /// Clear the client scope
    pub fn clear_client_scope(&mut self) {
        self.client_scope = None;
    }

    // =========================================================================
    // VIEW MODE / ZOOM / EXPANDED NODES METHODS
    // =========================================================================

    /// Set the view mode
    pub fn set_view_mode(&mut self, mode: &str) {
        self.view_mode = Some(mode.to_string());
    }

    /// Get the view mode, defaulting to "KYC_UBO" if not set
    pub fn get_view_mode(&self) -> &str {
        self.view_mode.as_deref().unwrap_or("KYC_UBO")
    }

    /// Clear the view mode (reset to default)
    pub fn clear_view_mode(&mut self) {
        self.view_mode = None;
    }

    /// Set the zoom level
    pub fn set_zoom_level(&mut self, level: f32) {
        self.zoom_level = Some(level);
    }

    /// Get the zoom level, defaulting to 1.0 if not set
    pub fn get_zoom_level(&self) -> f32 {
        self.zoom_level.unwrap_or(1.0)
    }

    /// Clear the zoom level (reset to default)
    pub fn clear_zoom_level(&mut self) {
        self.zoom_level = None;
    }

    /// Expand a node (add to expanded set)
    pub fn expand_node(&mut self, node_id: Uuid) {
        self.expanded_nodes.insert(node_id);
    }

    /// Collapse a node (remove from expanded set)
    pub fn collapse_node(&mut self, node_id: Uuid) {
        self.expanded_nodes.remove(&node_id);
    }

    /// Toggle node expansion
    pub fn toggle_node_expansion(&mut self, node_id: Uuid) {
        if self.expanded_nodes.contains(&node_id) {
            self.expanded_nodes.remove(&node_id);
        } else {
            self.expanded_nodes.insert(node_id);
        }
    }

    /// Check if a node is expanded
    pub fn is_node_expanded(&self, node_id: Uuid) -> bool {
        self.expanded_nodes.contains(&node_id)
    }

    /// Clear all expanded nodes
    pub fn clear_expanded_nodes(&mut self) {
        self.expanded_nodes.clear();
    }

    /// Get the number of expanded nodes
    pub fn expanded_node_count(&self) -> usize {
        self.expanded_nodes.len()
    }

    /// Set multiple expanded nodes at once (replaces existing)
    pub fn set_expanded_nodes(&mut self, nodes: impl IntoIterator<Item = Uuid>) {
        self.expanded_nodes = nodes.into_iter().collect();
    }

    // =========================================================================
    // SCOPE PATH METHODS - Hierarchical navigation
    // =========================================================================

    /// Get the current scope path
    pub fn scope_path(&self) -> &crate::session::ScopePath {
        &self.scope_path
    }

    /// Get mutable reference to scope path
    pub fn scope_path_mut(&mut self) -> &mut crate::session::ScopePath {
        &mut self.scope_path
    }

    /// Navigate into universe view (cluster by dimension)
    pub fn navigate_to_universe(&mut self, cluster_by: &str) {
        self.scope_path = crate::session::ScopePath::universe(cluster_by);
        // Clear mass cache - will be recomputed
        self.struct_mass = None;
        self.mass_breakdown = None;
        self.auto_view_mode = None;
    }

    /// Navigate into a specific book within a clustering dimension
    pub fn navigate_to_book(&mut self, cluster_by: &str, book_id: &str, label: &str) {
        self.scope_path = crate::session::ScopePath::book(cluster_by, book_id, label);
        self.struct_mass = None;
        self.mass_breakdown = None;
        self.auto_view_mode = None;
    }

    /// Navigate into a specific CBU
    pub fn navigate_to_cbu(&mut self, cbu_id: Uuid, name: &str) {
        self.scope_path = crate::session::ScopePath::cbu(cbu_id, name);
        self.struct_mass = None;
        self.mass_breakdown = None;
        self.auto_view_mode = None;
    }

    /// Navigate into a specific entity within current scope
    pub fn navigate_to_entity(&mut self, entity_id: Uuid, name: &str, entity_type: &str) {
        self.scope_path.push(crate::session::ScopeSegment::Entity {
            entity_id,
            name: name.to_string(),
            entity_type: entity_type.to_string(),
        });
        self.struct_mass = None;
        self.mass_breakdown = None;
        self.auto_view_mode = None;
    }

    /// Navigate up one level in the scope hierarchy
    pub fn navigate_up(&mut self) -> bool {
        let popped = self.scope_path.pop();
        if popped.is_some() {
            self.struct_mass = None;
            self.mass_breakdown = None;
            self.auto_view_mode = None;
        }
        popped.is_some()
    }

    /// Get breadcrumbs for current scope
    pub fn scope_breadcrumbs(&self) -> Vec<String> {
        self.scope_path
            .breadcrumbs()
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Get current scope depth
    pub fn scope_depth(&self) -> usize {
        self.scope_path.depth()
    }

    // =========================================================================
    // STRUCTURAL MASS METHODS - Complexity-based view selection
    // =========================================================================

    /// Set structural mass (computed externally from graph data)
    pub fn set_struct_mass(&mut self, mass: crate::session::StructMass) {
        self.mass_breakdown = Some(mass.breakdown.clone());
        self.auto_view_mode = Some(mass.suggested_view_mode());
        self.struct_mass = Some(mass);
    }

    /// Get the current structural mass
    pub fn struct_mass(&self) -> Option<&crate::session::StructMass> {
        self.struct_mass.as_ref()
    }

    /// Get the mass breakdown
    pub fn mass_breakdown(&self) -> Option<&crate::session::MassBreakdown> {
        self.mass_breakdown.as_ref()
    }

    /// Get the effective view mode (manual override or auto-selected)
    pub fn effective_view_mode(&self) -> Option<&crate::session::MassViewMode> {
        if self.view_mode_manual {
            // Manual mode takes precedence - but we return auto for type consistency
            // The actual string view_mode field is used directly
            self.auto_view_mode.as_ref()
        } else {
            self.auto_view_mode.as_ref()
        }
    }

    /// Set view mode manually (overrides auto-selection)
    pub fn set_view_mode_manual(&mut self, mode: &str) {
        self.view_mode = Some(mode.to_string());
        self.view_mode_manual = true;
    }

    /// Clear manual view mode override (revert to auto-selection)
    pub fn clear_view_mode_manual(&mut self) {
        self.view_mode_manual = false;
        // Restore auto mode if available
        if let Some(ref auto_mode) = self.auto_view_mode {
            self.view_mode = Some(auto_mode.as_str().to_string());
        }
    }

    /// Check if view mode is manually overridden
    pub fn is_view_mode_manual(&self) -> bool {
        self.view_mode_manual
    }

    /// Get total mass value (convenience method)
    pub fn total_mass(&self) -> Option<f32> {
        self.struct_mass.as_ref().map(|m| m.total)
    }

    /// Clear all mass-related cached data
    pub fn clear_mass_cache(&mut self) {
        self.struct_mass = None;
        self.mass_breakdown = None;
        self.auto_view_mode = None;
    }
}

// ============================================================================
// Execution Result
// ============================================================================

/// Result of executing a single DSL statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Index of the statement in the assembled DSL
    pub statement_index: usize,
    /// The DSL statement that was executed
    pub dsl: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Human-readable message about the result
    pub message: String,
    /// Entity ID if one was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    /// Type of entity created (CBU, ENTITY, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    /// Result data for Record/RecordSet operations (e.g., cbu.show)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

// ============================================================================
// Session Store
// ============================================================================

/// Thread-safe in-memory session store
///
/// NOTE: This now uses `UnifiedSession` as the single session type.
/// `AgentSession` is deprecated and will be removed in a future version.
pub type SessionStore = Arc<RwLock<HashMap<Uuid, crate::session::UnifiedSession>>>;

/// Create a new session store
pub fn create_session_store() -> SessionStore {
    Arc::new(RwLock::new(HashMap::new()))
}

// ============================================================================
// API Request/Response Types
// ============================================================================

/// Reference to a client for session scope initialization
#[derive(Debug, Clone, Deserialize)]
pub struct InitialClientRef {
    /// Client group ID (if known)
    pub client_id: Option<Uuid>,
    /// Client name/alias to search for (if client_id not known)
    pub client_name: Option<String>,
}

/// Request to create a new session
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// Optional domain hint to focus generation
    pub domain_hint: Option<String>,
    /// Optional initial client to set session scope
    /// If provided, the session starts in Scoped state with the client context set
    /// If not provided, session starts in New state and prompts for client selection
    #[serde(default)]
    pub initial_client: Option<InitialClientRef>,
    /// Optional structure type to filter by (pe, sicav, hedge, etc.)
    #[serde(default)]
    pub structure_type: Option<String>,
}

/// Welcome message constant - agent's opening question
pub const WELCOME_MESSAGE: &str = "Which client or CBU set would you like to work on?";

/// Response after creating a session
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    /// The new session ID
    pub session_id: Uuid,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// Initial state (always AwaitingScope until client/CBU set selected)
    pub state: SessionState,
    /// Welcome message from agent - asks for scope selection
    pub welcome_message: String,
}

// ChatRequest is now in ob-poc-types - SINGLE source of truth

/// Response from a chat message (intent-based)
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    /// Agent's response message
    pub message: String,
    /// Extracted intents
    pub intents: Vec<VerbIntent>,
    /// Validation results for each intent
    pub validation_results: Vec<super::intent::IntentValidation>,
    /// Assembled DSL (if all intents valid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assembled_dsl: Option<super::intent::AssembledDsl>,
    /// Current session state
    pub session_state: SessionState,
    /// Whether the session can execute
    pub can_execute: bool,
    /// DSL source rendered from AST (for display in UI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl_source: Option<String>,
    /// The full AST for debugging (JSON serialized)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<Vec<Statement>>,
    /// Session bindings with type info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bindings: Option<HashMap<String, BoundEntity>>,
    /// Commands for the UI to execute (uses shared type from ob_poc_types)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<ob_poc_types::AgentCommand>>,
}

/// Response with session state
#[derive(Debug, Serialize)]
pub struct SessionStateResponse {
    /// Session ID
    pub session_id: Uuid,
    /// Entity type this session operates on ("cbu", "kyc_case", "onboarding", "bulk", etc.)
    pub entity_type: String,
    /// Entity ID this session operates on (None if creating new or bulk mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    /// Current state
    pub state: SessionState,
    /// Number of messages in the session
    pub message_count: usize,
    /// Pending intents awaiting validation (empty vec if none)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_intents: Vec<VerbIntent>,
    /// Assembled DSL statements (empty vec if none)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembled_dsl: Vec<String>,
    /// Combined DSL (None if no DSL assembled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combined_dsl: Option<String>,
    /// Session context
    pub context: SessionContext,
    /// Conversation history (empty vec if none)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<ChatMessage>,
    /// Whether the session can execute
    pub can_execute: bool,
    /// Session version (ISO timestamp from updated_at)
    /// UI uses this to detect external changes (MCP/REPL modifying session)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Run sheet - DSL statement ledger with per-statement status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_sheet: Option<ob_poc_types::RunSheet>,
    /// Symbol bindings in this session
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub bindings: std::collections::HashMap<String, ob_poc_types::BoundEntityInfo>,
}

/// Request to execute accumulated DSL
#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    /// Whether to execute in dry-run mode
    #[serde(default)]
    pub dry_run: bool,
}

// ============================================================================
// Run Sheet Response Types
// ============================================================================

/// Response for run sheet state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSheetResponse {
    pub entries: Vec<RunSheetEntryResponse>,
    pub cursor: usize,
}

/// Response for a single run sheet entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSheetEntryResponse {
    pub id: Uuid,
    pub dsl_source: String,
    pub display_dsl: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub executed_at: Option<DateTime<Utc>>,
    pub affected_entities: Vec<Uuid>,
    pub error: Option<String>,
}

/// Response from executing DSL
#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    /// Overall success status
    pub success: bool,
    /// Results for each DSL statement
    pub results: Vec<ExecutionResult>,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// New session state after execution
    pub new_state: SessionState,
    /// All bindings created during execution (name -> UUID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bindings: Option<std::collections::HashMap<String, uuid::Uuid>>,
}

// ============================================================================
// Disambiguation Types
// ============================================================================

/// Disambiguation request - sent when entity references are ambiguous
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationRequest {
    /// Unique ID for this disambiguation request
    pub request_id: Uuid,
    /// The ambiguous items that need resolution
    pub items: Vec<DisambiguationItem>,
    /// Human-readable prompt for the user
    pub prompt: String,
    /// Original intents that need disambiguation (preserved for re-processing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_intents: Option<Vec<VerbIntent>>,
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
        matches: Vec<EntityMatchOption>,
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
    pub group_id: Uuid,
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
pub struct EntityMatchOption {
    /// Entity UUID
    pub entity_id: Uuid,
    /// Display name
    pub name: String,
    /// Entity type (e.g., "proper_person", "limited_company")
    pub entity_type: String,
    /// Jurisdiction code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Additional context (roles, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Match score (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
}

/// User's disambiguation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationResponse {
    /// The request ID being responded to
    pub request_id: Uuid,
    /// Selected resolutions
    pub selections: Vec<DisambiguationSelection>,
}

/// A single disambiguation selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationSelection {
    /// Selected entity for an EntityMatch
    Entity {
        param: String,
        entity_id: Uuid,
        /// Display name of the selected entity (for user-friendly DSL)
        #[serde(default)]
        display_name: Option<String>,
    },
    /// Selected interpretation for an InterpretationChoice
    Interpretation {
        text: String,
        interpretation_id: String,
    },
}

/// Chat response status - indicates whether response is ready or needs disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChatResponseStatus {
    /// DSL is ready (no ambiguity or already resolved)
    Ready,
    /// Needs user disambiguation before generating DSL
    NeedsDisambiguation {
        disambiguation: DisambiguationRequest,
    },
    /// Error occurred
    Error { message: String },
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let session = AgentSession::new(Some("cbu".to_string()));
        assert!(!session.id.is_nil());
        assert_eq!(session.state, SessionState::New);
        assert!(session.messages.is_empty());
        assert!(session.run_sheet.entries.is_empty());
        assert_eq!(session.context.domain_hint, Some("cbu".to_string()));
    }

    #[test]
    fn test_session_state_transitions() {
        let mut session = AgentSession::new(None);
        assert_eq!(session.state, SessionState::New);

        // Add intents -> PendingValidation
        session.add_intents(vec![VerbIntent {
            verb: "cbu.ensure".to_string(),
            params: Default::default(),
            refs: Default::default(),
            lookups: None,
            sequence: None,
        }]);
        assert_eq!(session.state, SessionState::PendingValidation);

        // Set assembled DSL -> ReadyToExecute
        // NOTE: pending_intents are kept for execution-time ref resolution
        session.set_assembled_dsl(vec!["(cbu.ensure :cbu-name \"Test\")".to_string()]);
        assert_eq!(session.state, SessionState::ReadyToExecute);
        assert_eq!(session.pending_intents.len(), 1); // Intents preserved
        assert!(session.run_sheet.has_runnable()); // Entry is draft/runnable

        // Get the entry ID to mark as executed
        let entry_id = session.run_sheet.entries[0].id;

        // Transition to Executing state before execution
        session.transition(SessionEvent::ExecutionStarted);
        assert_eq!(session.state, SessionState::Executing);

        // Mark entry as executed via run_sheet (new approach)
        let cbu_id = Uuid::new_v4();
        session
            .run_sheet
            .mark_executed(entry_id, vec![cbu_id], HashMap::new());

        // Record execution results (updates context)
        session.record_execution(vec![ExecutionResult {
            statement_index: 0,
            dsl: "(cbu.ensure :cbu-name \"Test\")".to_string(),
            success: true,
            message: "OK".to_string(),
            entity_id: Some(cbu_id),
            entity_type: Some("CBU".to_string()),
            result: None,
        }]);
        // After execution with CBU created, state transitions to Scoped (has scope)
        assert_eq!(session.state, SessionState::Scoped);
        // After execution, there should be no more runnable entries
        // (entries remain in run_sheet with Executed status but none are Draft/Ready)
        assert!(!session.run_sheet.has_runnable());
    }

    #[test]
    fn test_context_resolve_ref() {
        let mut ctx = SessionContext::default();
        let cbu_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        ctx.last_cbu_id = Some(cbu_id);
        ctx.last_entity_id = Some(entity_id);

        assert_eq!(
            ctx.resolve_ref("@last_cbu"),
            Some(format!("\"{}\"", cbu_id))
        );
        assert_eq!(
            ctx.resolve_ref("@last_entity"),
            Some(format!("\"{}\"", entity_id))
        );
        assert_eq!(ctx.resolve_ref("@unknown"), None);
    }

    #[test]
    fn test_context_named_refs() {
        let mut ctx = SessionContext::default();
        let id = Uuid::new_v4();

        ctx.set_named_ref("my_entity", id);
        assert_eq!(ctx.resolve_ref("@my_entity"), Some(format!("\"{}\"", id)));
    }

    #[test]
    fn test_add_messages() {
        let mut session = AgentSession::new(None);

        let user_id = session.add_user_message("Create a CBU".to_string());
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].id, user_id);
        assert_eq!(session.messages[0].role, MessageRole::User);

        let agent_id = session.add_agent_message(
            "Here's the DSL".to_string(),
            None,
            Some("(cbu.ensure :cbu-name \"Test\")".to_string()),
        );
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].id, agent_id);
        assert_eq!(session.messages[1].role, MessageRole::Agent);
        assert!(session.messages[1].dsl.is_some());
    }

    #[tokio::test]
    async fn test_session_store() {
        use crate::session::UnifiedSession;

        let store = create_session_store();
        let session = UnifiedSession::new();
        let id = session.id;

        // Insert
        {
            let mut write = store.write().await;
            write.insert(id, session);
        }

        // Read
        {
            let read = store.read().await;
            assert!(read.contains_key(&id));
            // UnifiedSession uses crate::session::unified::SessionState
            assert_eq!(
                read.get(&id).unwrap().state,
                crate::session::unified::SessionState::New
            );
        }
    }
}
