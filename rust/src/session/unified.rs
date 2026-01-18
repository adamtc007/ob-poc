//! Unified Session State
//!
//! Single source of truth for session state.
//! Replaces: SessionContext, SessionState, SubSessionType, BatchContext
//!
//! See: ai-thoughts/036-session-rip-and-replace.md

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// The unified session - single source of truth
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
}

impl EntryStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EntryStatus::Executed | EntryStatus::Cancelled | EntryStatus::Failed
        )
    }

    pub fn is_pending(&self) -> bool {
        matches!(
            self,
            EntryStatus::Draft | EntryStatus::Ready | EntryStatus::Executing
        )
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

impl Default for UnifiedSession {
    fn default() -> Self {
        Self::new()
    }
}

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
        }
    }

    /// Create with specific user
    pub fn for_user(user_id: Uuid) -> Self {
        let mut session = Self::new();
        session.user_id = user_id;
        session
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
            dsl: None,
        });
        self.updated_at = Utc::now();
        id
    }

    /// Add agent message
    pub fn add_agent_message(&mut self, content: String, dsl: Option<String>) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::Agent,
            content,
            timestamp: Utc::now(),
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
}
